package bridge

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"time"

	"github.com/sirupsen/logrus"
	"github.com/tekfly/virtual-dom-gateway/github-bridge/internal/config"
	"github.com/tekfly/virtual-dom-gateway/github-bridge/internal/git"
	"github.com/tekfly/virtual-dom-gateway/github-bridge/internal/metrics"
	"github.com/tekfly/virtual-dom-gateway/github-bridge/internal/mongodb"
	"go.mongodb.org/mongo-driver/bson"
)

// Bridge handles syncing between MongoDB and GitHub
type Bridge struct {
	config    *config.Config
	mongo     *mongodb.Client
	logger    *logrus.Logger
	ctx       context.Context
	cancel    context.CancelFunc
	wg        sync.WaitGroup
	workQueue chan *mongodb.PushIntent
}

// New creates a new Bridge instance
func New(ctx context.Context, cfg *config.Config, logger *logrus.Logger) (*Bridge, error) {
	// Connect to MongoDB
	mongoClient, err := mongodb.NewClient(ctx, cfg.MongoDBURI, cfg.MongoDBDatabase)
	if err != nil {
		return nil, fmt.Errorf("failed to create MongoDB client: %w", err)
	}

	// Create indexes
	if err := mongoClient.CreateIndexes(ctx); err != nil {
		logger.WithError(err).Warn("Failed to create indexes")
	}

	bridgeCtx, cancel := context.WithCancel(ctx)

	return &Bridge{
		config:    cfg,
		mongo:     mongoClient,
		logger:    logger,
		ctx:       bridgeCtx,
		cancel:    cancel,
		workQueue: make(chan *mongodb.PushIntent, cfg.BatchSize),
	}, nil
}

// Start begins the bridge operations
func (b *Bridge) Start() error {
	b.logger.Info("Starting GitHub Bridge")

	// Start workers
	for i := 0; i < b.config.WorkerCount; i++ {
		b.wg.Add(1)
		go b.worker(i)
	}

	// Start watching for changes if webhooks are disabled
	if !b.config.EnableWebhooks {
		b.wg.Add(1)
		go b.pollForChanges()
	} else {
		b.wg.Add(1)
		go b.watchChanges()
	}

	// Wait for all workers to complete
	b.wg.Wait()
	return nil
}

// Shutdown gracefully shuts down the bridge
func (b *Bridge) Shutdown(ctx context.Context) error {
	b.logger.Info("Shutting down GitHub Bridge")
	
	// Cancel context to stop all operations
	b.cancel()
	
	// Close work queue
	close(b.workQueue)
	
	// Wait for shutdown or timeout
	done := make(chan struct{})
	go func() {
		b.wg.Wait()
		close(done)
	}()

	select {
	case <-done:
		b.logger.Info("All workers stopped")
	case <-ctx.Done():
		b.logger.Warn("Shutdown timeout exceeded")
	}

	// Close MongoDB connection
	if err := b.mongo.Close(context.Background()); err != nil {
		b.logger.WithError(err).Error("Failed to close MongoDB connection")
	}

	return nil
}

// worker processes push intents from the queue
func (b *Bridge) worker(id int) {
	defer b.wg.Done()
	
	b.logger.WithField("worker_id", id).Info("Worker started")
	metrics.ActiveWorkers.Inc()
	defer metrics.ActiveWorkers.Dec()

	for intent := range b.workQueue {
		select {
		case <-b.ctx.Done():
			return
		default:
			if err := b.processPushIntent(intent); err != nil {
				b.logger.WithError(err).WithField("intent_id", intent.ID).Error("Failed to process push intent")
				metrics.ErrorsByType.WithLabelValues("processing").Inc()
			}
		}
	}

	b.logger.WithField("worker_id", id).Info("Worker stopped")
}

// pollForChanges polls MongoDB for new push intents
func (b *Bridge) pollForChanges() {
	defer b.wg.Done()
	
	ticker := time.NewTicker(time.Duration(b.config.PollInterval) * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-b.ctx.Done():
			return
		case <-ticker.C:
			if err := b.checkForPushIntents(); err != nil {
				b.logger.WithError(err).Error("Failed to check for push intents")
				metrics.ErrorsByType.WithLabelValues("polling").Inc()
			}
		}
	}
}

// watchChanges uses MongoDB change streams to watch for new push intents
func (b *Bridge) watchChanges() {
	defer b.wg.Done()

	for {
		select {
		case <-b.ctx.Done():
			return
		default:
			if err := b.watchChangeStream(); err != nil {
				b.logger.WithError(err).Error("Change stream error, retrying in 5 seconds")
				metrics.ErrorsByType.WithLabelValues("changestream").Inc()
				time.Sleep(5 * time.Second)
			}
		}
	}
}

// watchChangeStream watches MongoDB for new push intents
func (b *Bridge) watchChangeStream() error {
	stream, err := b.mongo.WatchPushIntents(b.ctx)
	if err != nil {
		return err
	}
	defer stream.Close(b.ctx)

	b.logger.Info("Watching for push intents via change stream")

	for stream.Next(b.ctx) {
		var event struct {
			FullDocument *mongodb.PushIntent `bson:"fullDocument"`
		}

		if err := stream.Decode(&event); err != nil {
			b.logger.WithError(err).Error("Failed to decode change event")
			continue
		}

		if event.FullDocument != nil && !event.FullDocument.Processed {
			select {
			case b.workQueue <- event.FullDocument:
				metrics.QueueSize.Inc()
			case <-b.ctx.Done():
				return nil
			}
		}
	}

	return stream.Err()
}

// checkForPushIntents checks for pending push intents
func (b *Bridge) checkForPushIntents() error {
	intents, err := b.mongo.GetPendingPushIntents(b.ctx, b.config.BatchSize)
	if err != nil {
		return err
	}

	if len(intents) == 0 {
		return nil
	}

	b.logger.WithField("count", len(intents)).Debug("Found pending push intents")

	for _, intent := range intents {
		select {
		case b.workQueue <- intent:
			metrics.QueueSize.Inc()
		case <-b.ctx.Done():
			return nil
		}
	}

	return nil
}

// processPushIntent processes a single push intent
func (b *Bridge) processPushIntent(intent *mongodb.PushIntent) error {
	defer func() {
		metrics.QueueSize.Dec()
	}()

	timer := time.Now()
	metrics.PushAttempts.Inc()

	b.logger.WithFields(logrus.Fields{
		"id":     intent.ID,
		"repo":   intent.Repo,
		"branch": intent.Branch,
		"author": intent.Author,
	}).Info("Processing push intent")

	// Process the intent
	err := b.pushToGitHub(intent)
	
	// Mark as processed regardless of outcome
	if markErr := b.mongo.MarkPushIntentProcessed(b.ctx, intent.ID, err); markErr != nil {
		b.logger.WithError(markErr).Error("Failed to mark push intent as processed")
		metrics.ErrorsByType.WithLabelValues("mongodb").Inc()
	}

	metrics.BatchDuration.Observe(time.Since(timer).Seconds())

	if err != nil {
		metrics.PushFailures.Inc()
		return err
	}

	metrics.PushSuccesses.Inc()
	return nil
}

// pushToGitHub performs the actual push operation
func (b *Bridge) pushToGitHub(intent *mongodb.PushIntent) error {
	if b.config.DryRun {
		b.logger.Info("DRY RUN: Would push to GitHub")
		return nil
	}

	// Get documents for this push intent
	documents, err := b.mongo.GetDocumentsByIDs(b.ctx, intent.Documents)
	if err != nil {
		return fmt.Errorf("failed to get documents: %w", err)
	}

	if len(documents) == 0 {
		return fmt.Errorf("no documents found for push intent")
	}

	metrics.DocumentsProcessed.Add(float64(len(documents)))
	metrics.BatchSize.Observe(float64(len(documents)))

	// Create temporary directory for git operations
	tempDir := filepath.Join(os.TempDir(), "github-bridge")
	if err := os.MkdirAll(tempDir, 0755); err != nil {
		return fmt.Errorf("failed to create temp dir: %w", err)
	}

	// Clone repository
	cloneTimer := time.Now()
	repo, err := git.Clone(b.ctx, git.CloneOptions{
		URL:        fmt.Sprintf("https://github.com/%s.git", b.config.GetRepoFullName()),
		Branch:     intent.Branch,
		Token:      b.config.GitHubToken,
		TempDir:    tempDir,
		RemoteName: "origin",
	}, b.logger)
	if err != nil {
		return fmt.Errorf("failed to clone repository: %w", err)
	}
	defer repo.Cleanup()
	
	metrics.GitCloneDuration.Observe(time.Since(cloneTimer).Seconds())

	// Pull latest changes
	if err := repo.Pull(b.ctx); err != nil {
		b.logger.WithError(err).Warn("Failed to pull latest changes")
	}

	// Apply documents to repository
	gitDocs := make([]git.Document, 0, len(documents))
	for _, doc := range documents {
		operation := "update"
		if meta, ok := doc.Metadata["operation"].(string); ok {
			operation = meta
		}

		gitDocs = append(gitDocs, git.Document{
			Path:      doc.Path,
			Content:   doc.Blob,
			Operation: operation,
		})
	}

	if err := repo.ApplyDocuments(gitDocs); err != nil {
		return fmt.Errorf("failed to apply documents: %w", err)
	}

	// Check if there are changes
	status, err := repo.GetStatus()
	if err != nil {
		return fmt.Errorf("failed to get status: %w", err)
	}

	if status.IsClean() {
		b.logger.Info("No changes to commit")
		metrics.DocumentsSkipped.Add(float64(len(documents)))
		return nil
	}

	// Commit changes
	commitHash, err := repo.Commit(intent.Message, git.CommitAuthor{
		Name:  b.config.GitUserName,
		Email: b.config.GitUserEmail,
	})
	if err != nil {
		return fmt.Errorf("failed to commit: %w", err)
	}

	b.logger.WithField("commit", commitHash).Info("Created commit")

	// Push to GitHub
	pushTimer := time.Now()
	if err := repo.Push(b.ctx); err != nil {
		return fmt.Errorf("failed to push: %w", err)
	}
	
	metrics.GitPushDuration.Observe(time.Since(pushTimer).Seconds())

	b.logger.WithFields(logrus.Fields{
		"commit":    commitHash,
		"documents": len(documents),
	}).Info("Successfully pushed to GitHub")

	return nil
}