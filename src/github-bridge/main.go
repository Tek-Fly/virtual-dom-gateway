package main

import (
	"context"
	"fmt"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/joho/godotenv"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promhttp"
	"github.com/sirupsen/logrus"
	"github.com/tekfly/virtual-dom-gateway/github-bridge/internal/bridge"
	"github.com/tekfly/virtual-dom-gateway/github-bridge/internal/config"
	"github.com/tekfly/virtual-dom-gateway/github-bridge/internal/metrics"
	"net/http"
)

var (
	version = "dev"
	commit  = "none"
	date    = "unknown"
)

func main() {
	// Load environment variables
	if err := godotenv.Load(); err != nil {
		logrus.Debug("No .env file found")
	}

	// Initialize logger
	logger := logrus.New()
	logger.SetFormatter(&logrus.JSONFormatter{})
	
	logLevel, err := logrus.ParseLevel(os.Getenv("LOG_LEVEL"))
	if err != nil {
		logLevel = logrus.InfoLevel
	}
	logger.SetLevel(logLevel)

	logger.WithFields(logrus.Fields{
		"version": version,
		"commit":  commit,
		"date":    date,
	}).Info("Starting GitHub Bridge")

	// Load configuration
	cfg, err := config.Load()
	if err != nil {
		logger.Fatalf("Failed to load configuration: %v", err)
	}

	// Validate configuration
	if err := cfg.Validate(); err != nil {
		logger.Fatalf("Invalid configuration: %v", err)
	}

	// Initialize metrics
	metrics.Init()

	// Create bridge instance
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	bridgeService, err := bridge.New(ctx, cfg, logger)
	if err != nil {
		logger.Fatalf("Failed to create bridge: %v", err)
	}

	// Start metrics server
	go startMetricsServer(cfg.MetricsPort, logger)

	// Handle shutdown gracefully
	sigChan := make(chan os.Signal, 1)
	signal.Notify(sigChan, syscall.SIGINT, syscall.SIGTERM)

	// Start the bridge
	errChan := make(chan error, 1)
	go func() {
		if err := bridgeService.Start(); err != nil {
			errChan <- err
		}
	}()

	// Wait for shutdown signal or error
	select {
	case sig := <-sigChan:
		logger.Infof("Received signal %v, shutting down gracefully", sig)
		cancel()
		
		// Give the bridge time to cleanup
		shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 30*time.Second)
		defer shutdownCancel()
		
		if err := bridgeService.Shutdown(shutdownCtx); err != nil {
			logger.Errorf("Error during shutdown: %v", err)
		}
	case err := <-errChan:
		logger.Fatalf("Bridge error: %v", err)
	}

	logger.Info("GitHub Bridge stopped")
}

func startMetricsServer(port int, logger *logrus.Logger) {
	mux := http.NewServeMux()
	mux.Handle("/metrics", promhttp.Handler())
	mux.HandleFunc("/health", func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("OK"))
	})

	server := &http.Server{
		Addr:         fmt.Sprintf(":%d", port),
		Handler:      mux,
		ReadTimeout:  5 * time.Second,
		WriteTimeout: 10 * time.Second,
		IdleTimeout:  15 * time.Second,
	}

	logger.Infof("Metrics server listening on :%d", port)
	if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		logger.Errorf("Metrics server error: %v", err)
	}
}