package metrics

import (
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promauto"
)

var (
	// Push metrics
	PushAttempts = promauto.NewCounter(prometheus.CounterOpts{
		Name: "github_bridge_push_attempts_total",
		Help: "Total number of push attempts",
	})

	PushSuccesses = promauto.NewCounter(prometheus.CounterOpts{
		Name: "github_bridge_push_successes_total",
		Help: "Total number of successful pushes",
	})

	PushFailures = promauto.NewCounter(prometheus.CounterOpts{
		Name: "github_bridge_push_failures_total",
		Help: "Total number of failed pushes",
	})

	// Document metrics
	DocumentsProcessed = promauto.NewCounter(prometheus.CounterOpts{
		Name: "github_bridge_documents_processed_total",
		Help: "Total number of documents processed",
	})

	DocumentsSkipped = promauto.NewCounter(prometheus.CounterOpts{
		Name: "github_bridge_documents_skipped_total",
		Help: "Total number of documents skipped",
	})

	// Batch metrics
	BatchSize = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "github_bridge_batch_size",
		Help:    "Size of document batches processed",
		Buckets: prometheus.ExponentialBuckets(1, 2, 10),
	})

	BatchDuration = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "github_bridge_batch_duration_seconds",
		Help:    "Time taken to process a batch",
		Buckets: prometheus.DefBuckets,
	})

	// Git operations
	GitCloneDuration = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "github_bridge_git_clone_duration_seconds",
		Help:    "Time taken to clone repository",
		Buckets: prometheus.DefBuckets,
	})

	GitPushDuration = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "github_bridge_git_push_duration_seconds",
		Help:    "Time taken to push changes",
		Buckets: prometheus.DefBuckets,
	})

	// MongoDB operations
	MongoQueryDuration = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "github_bridge_mongo_query_duration_seconds",
		Help:    "Time taken for MongoDB queries",
		Buckets: prometheus.DefBuckets,
	})

	MongoUpdateDuration = promauto.NewHistogram(prometheus.HistogramOpts{
		Name:    "github_bridge_mongo_update_duration_seconds",
		Help:    "Time taken for MongoDB updates",
		Buckets: prometheus.DefBuckets,
	})

	// Errors by type
	ErrorsByType = promauto.NewCounterVec(prometheus.CounterOpts{
		Name: "github_bridge_errors_total",
		Help: "Total errors by type",
	}, []string{"type"})

	// Active workers
	ActiveWorkers = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "github_bridge_active_workers",
		Help: "Number of active worker goroutines",
	})

	// Queue size
	QueueSize = promauto.NewGauge(prometheus.GaugeOpts{
		Name: "github_bridge_queue_size",
		Help: "Number of documents in processing queue",
	})
)

// Init initializes the metrics
func Init() {
	// Set initial values
	ActiveWorkers.Set(0)
	QueueSize.Set(0)
}