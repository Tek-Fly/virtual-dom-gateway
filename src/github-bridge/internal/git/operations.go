package git

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/config"
	"github.com/go-git/go-git/v5/plumbing"
	"github.com/go-git/go-git/v5/plumbing/object"
	"github.com/go-git/go-git/v5/plumbing/transport"
	"github.com/go-git/go-git/v5/plumbing/transport/http"
	"github.com/sirupsen/logrus"
)

// Repository manages Git operations
type Repository struct {
	repo      *git.Repository
	worktree  *git.Worktree
	auth      transport.AuthMethod
	remoteName string
	logger    *logrus.Logger
	tempDir   string
}

// CloneOptions contains options for cloning a repository
type CloneOptions struct {
	URL        string
	Branch     string
	Token      string
	TempDir    string
	RemoteName string
}

// Clone creates a new Repository by cloning from remote
func Clone(ctx context.Context, opts CloneOptions, logger *logrus.Logger) (*Repository, error) {
	// Create temporary directory
	tempDir := filepath.Join(opts.TempDir, fmt.Sprintf("repo-%d", time.Now().UnixNano()))
	if err := os.MkdirAll(tempDir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create temp dir: %w", err)
	}

	// Setup authentication
	auth := &http.BasicAuth{
		Username: "x-access-token",
		Password: opts.Token,
	}

	// Clone repository
	cloneOpts := &git.CloneOptions{
		URL:           opts.URL,
		Auth:          auth,
		Progress:      nil,
		ReferenceName: plumbing.NewBranchReferenceName(opts.Branch),
		SingleBranch:  true,
		Depth:         1, // Shallow clone for performance
	}

	logger.WithFields(logrus.Fields{
		"url":    opts.URL,
		"branch": opts.Branch,
	}).Info("Cloning repository")

	repo, err := git.PlainCloneContext(ctx, tempDir, false, cloneOpts)
	if err != nil {
		os.RemoveAll(tempDir)
		return nil, fmt.Errorf("failed to clone repository: %w", err)
	}

	worktree, err := repo.Worktree()
	if err != nil {
		os.RemoveAll(tempDir)
		return nil, fmt.Errorf("failed to get worktree: %w", err)
	}

	return &Repository{
		repo:       repo,
		worktree:   worktree,
		auth:       auth,
		remoteName: opts.RemoteName,
		logger:     logger,
		tempDir:    tempDir,
	}, nil
}

// WriteFile writes content to a file in the repository
func (r *Repository) WriteFile(path string, content []byte) error {
	fullPath := filepath.Join(r.tempDir, path)
	
	// Create directory if needed
	dir := filepath.Dir(fullPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create directory: %w", err)
	}

	// Write file
	if err := os.WriteFile(fullPath, content, 0644); err != nil {
		return fmt.Errorf("failed to write file: %w", err)
	}

	// Add to git
	if _, err := r.worktree.Add(path); err != nil {
		return fmt.Errorf("failed to add file to git: %w", err)
	}

	return nil
}

// RemoveFile removes a file from the repository
func (r *Repository) RemoveFile(path string) error {
	fullPath := filepath.Join(r.tempDir, path)
	
	// Remove file
	if err := os.Remove(fullPath); err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("failed to remove file: %w", err)
	}

	// Remove from git
	if _, err := r.worktree.Remove(path); err != nil {
		return fmt.Errorf("failed to remove file from git: %w", err)
	}

	return nil
}

// Commit creates a commit with the given message
func (r *Repository) Commit(message string, author CommitAuthor) (string, error) {
	// Check if there are changes to commit
	status, err := r.worktree.Status()
	if err != nil {
		return "", fmt.Errorf("failed to get status: %w", err)
	}

	if status.IsClean() {
		return "", fmt.Errorf("no changes to commit")
	}

	// Create commit
	commitOpts := &git.CommitOptions{
		Author: &object.Signature{
			Name:  author.Name,
			Email: author.Email,
			When:  time.Now(),
		},
	}

	hash, err := r.worktree.Commit(message, commitOpts)
	if err != nil {
		return "", fmt.Errorf("failed to commit: %w", err)
	}

	r.logger.WithField("hash", hash.String()).Info("Created commit")
	return hash.String(), nil
}

// Push pushes commits to remote
func (r *Repository) Push(ctx context.Context) error {
	pushOpts := &git.PushOptions{
		RemoteName: r.remoteName,
		Auth:       r.auth,
		Progress:   nil,
	}

	r.logger.Info("Pushing to remote")
	
	err := r.repo.PushContext(ctx, pushOpts)
	if err != nil && err != git.NoErrAlreadyUpToDate {
		return fmt.Errorf("failed to push: %w", err)
	}

	return nil
}

// Pull pulls latest changes from remote
func (r *Repository) Pull(ctx context.Context) error {
	pullOpts := &git.PullOptions{
		RemoteName: r.remoteName,
		Auth:       r.auth,
		Progress:   nil,
	}

	err := r.worktree.PullContext(ctx, pullOpts)
	if err != nil && err != git.NoErrAlreadyUpToDate {
		return fmt.Errorf("failed to pull: %w", err)
	}

	return nil
}

// GetStatus returns the current repository status
func (r *Repository) GetStatus() (git.Status, error) {
	return r.worktree.Status()
}

// Cleanup removes the temporary directory
func (r *Repository) Cleanup() error {
	if r.tempDir != "" {
		r.logger.WithField("path", r.tempDir).Debug("Cleaning up repository")
		return os.RemoveAll(r.tempDir)
	}
	return nil
}

// CommitAuthor represents commit author information
type CommitAuthor struct {
	Name  string
	Email string
}

// ApplyDocuments applies a set of document changes to the repository
func (r *Repository) ApplyDocuments(documents []Document) error {
	for _, doc := range documents {
		switch doc.Operation {
		case "create", "update":
			if err := r.WriteFile(doc.Path, doc.Content); err != nil {
				return fmt.Errorf("failed to write %s: %w", doc.Path, err)
			}
		case "delete":
			if err := r.RemoveFile(doc.Path); err != nil {
				return fmt.Errorf("failed to remove %s: %w", doc.Path, err)
			}
		default:
			r.logger.WithField("operation", doc.Operation).Warn("Unknown operation")
		}
	}
	return nil
}

// Document represents a document to be applied to the repository
type Document struct {
	Path      string
	Content   []byte
	Operation string // create, update, delete
}