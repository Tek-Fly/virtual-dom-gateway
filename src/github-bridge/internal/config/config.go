package config

import (
	"fmt"
	"os"
	"strconv"
	"strings"
)

// Config holds the configuration for the GitHub Bridge
type Config struct {
	// MongoDB configuration
	MongoDBURI      string
	MongoDBDatabase string

	// GitHub configuration
	GitHubToken        string
	GitHubOrganization string
	GitHubRepo         string
	GitHubBranch       string
	
	// Git configuration
	GitUserName  string
	GitUserEmail string
	
	// Bridge configuration
	PollInterval   int // seconds
	BatchSize      int
	WorkerCount    int
	MetricsPort    int
	
	// Security
	EnableSigning  bool
	GPGKeyPath     string
	
	// Feature flags
	DryRun         bool
	EnableWebhooks bool
}

// Load configuration from environment variables
func Load() (*Config, error) {
	cfg := &Config{
		MongoDBURI:         getEnv("MONGODB_URI", "mongodb://localhost:27017"),
		MongoDBDatabase:    getEnv("MONGODB_DATABASE", "virtual_dom"),
		GitHubToken:        getEnv("GITHUB_TOKEN", ""),
		GitHubOrganization: getEnv("GITHUB_ORG", ""),
		GitHubRepo:         getEnv("GITHUB_REPO", ""),
		GitHubBranch:       getEnv("GITHUB_BRANCH", "main"),
		GitUserName:        getEnv("GIT_USER_NAME", "Virtual DOM Bot"),
		GitUserEmail:       getEnv("GIT_USER_EMAIL", "bot@tekfly.io"),
		PollInterval:       getEnvInt("POLL_INTERVAL", 5),
		BatchSize:          getEnvInt("BATCH_SIZE", 100),
		WorkerCount:        getEnvInt("WORKER_COUNT", 3),
		MetricsPort:        getEnvInt("METRICS_PORT", 9091),
		EnableSigning:      getEnvBool("ENABLE_SIGNING", false),
		GPGKeyPath:         getEnv("GPG_KEY_PATH", ""),
		DryRun:             getEnvBool("DRY_RUN", false),
		EnableWebhooks:     getEnvBool("ENABLE_WEBHOOKS", false),
	}

	return cfg, nil
}

// Validate checks if the configuration is valid
func (c *Config) Validate() error {
	if c.GitHubToken == "" {
		return fmt.Errorf("GITHUB_TOKEN is required")
	}

	if c.GitHubOrganization == "" && !strings.Contains(c.GitHubRepo, "/") {
		return fmt.Errorf("GITHUB_ORG is required when GITHUB_REPO doesn't contain org/repo format")
	}

	if c.GitHubRepo == "" {
		return fmt.Errorf("GITHUB_REPO is required")
	}

	if c.EnableSigning && c.GPGKeyPath == "" {
		return fmt.Errorf("GPG_KEY_PATH is required when signing is enabled")
	}

	if c.PollInterval < 1 {
		return fmt.Errorf("POLL_INTERVAL must be at least 1 second")
	}

	if c.BatchSize < 1 {
		return fmt.Errorf("BATCH_SIZE must be at least 1")
	}

	if c.WorkerCount < 1 {
		return fmt.Errorf("WORKER_COUNT must be at least 1")
	}

	return nil
}

// GetRepoFullName returns the full repository name (org/repo)
func (c *Config) GetRepoFullName() string {
	if strings.Contains(c.GitHubRepo, "/") {
		return c.GitHubRepo
	}
	return fmt.Sprintf("%s/%s", c.GitHubOrganization, c.GitHubRepo)
}

func getEnv(key, defaultValue string) string {
	if value := os.Getenv(key); value != "" {
		return value
	}
	return defaultValue
}

func getEnvInt(key string, defaultValue int) int {
	if value := os.Getenv(key); value != "" {
		if intValue, err := strconv.Atoi(value); err == nil {
			return intValue
		}
	}
	return defaultValue
}

func getEnvBool(key string, defaultValue bool) bool {
	if value := os.Getenv(key); value != "" {
		if boolValue, err := strconv.ParseBool(value); err == nil {
			return boolValue
		}
	}
	return defaultValue
}