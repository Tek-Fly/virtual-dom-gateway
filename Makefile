# Virtual DOM Gateway Makefile
SHELL := /bin/bash

# Version information
VERSION ?= $(shell git describe --tags --always --dirty 2>/dev/null || echo "dev")
COMMIT ?= $(shell git rev-parse --short HEAD 2>/dev/null || echo "unknown")
BUILD_DATE := $(shell date -u +%Y-%m-%dT%H:%M:%SZ)

# Docker settings
DOCKER_REGISTRY ?= flybywire.io
DOCKER_ORG ?= tekfly
GATEWAY_IMAGE := $(DOCKER_REGISTRY)/$(DOCKER_ORG)/virtualdom-gateway
GITHUB_BRIDGE_IMAGE := $(DOCKER_REGISTRY)/$(DOCKER_ORG)/virtualdom-github-bridge

# Build settings
RUST_LOG ?= info
GO_FLAGS := -ldflags "-X main.version=$(VERSION) -X main.commit=$(COMMIT) -X main.date=$(BUILD_DATE)"

.PHONY: all build test clean docker-build docker-push deploy help

all: build test ## Build and test everything

# Development targets
.PHONY: dev-gateway dev-github-bridge dev-up dev-down

dev-gateway: ## Run gateway in development mode
	cd src/gateway && RUST_LOG=$(RUST_LOG) cargo run

dev-github-bridge: ## Run GitHub bridge in development mode
	cd src/github-bridge && go run $(GO_FLAGS) .

dev-up: ## Start development environment
	docker-compose up -d mongodb
	@echo "Waiting for MongoDB to be ready..."
	@sleep 5
	@echo "Development environment is ready!"

dev-down: ## Stop development environment
	docker-compose down -v

# Build targets
.PHONY: build build-gateway build-github-bridge

build: build-gateway build-github-bridge ## Build all services

build-gateway: ## Build gateway service
	cd src/gateway && cargo build --release

build-github-bridge: ## Build GitHub bridge service
	cd src/github-bridge && go build $(GO_FLAGS) -o ../../target/github-bridge .

# Test targets
.PHONY: test test-gateway test-github-bridge test-integration

test: test-gateway test-github-bridge ## Run all tests

test-gateway: ## Run gateway tests
	cd src/gateway && cargo test

test-github-bridge: ## Run GitHub bridge tests
	cd src/github-bridge && go test ./...

test-integration: dev-up ## Run integration tests
	@echo "Running integration tests..."
	cd tests && go test -tags integration ./...
	$(MAKE) dev-down

# Docker targets
.PHONY: docker-build docker-build-gateway docker-build-github-bridge

docker-build: docker-build-gateway docker-build-github-bridge ## Build all Docker images

docker-build-gateway: ## Build gateway Docker image
	docker build \
		--build-arg VERSION=$(VERSION) \
		-f docker/Dockerfile.gateway \
		-t $(GATEWAY_IMAGE):$(VERSION) \
		-t $(GATEWAY_IMAGE):latest \
		.

docker-build-github-bridge: ## Build GitHub bridge Docker image
	docker build \
		--build-arg VERSION=$(VERSION) \
		--build-arg COMMIT=$(COMMIT) \
		-f docker/Dockerfile.github-bridge \
		-t $(GITHUB_BRIDGE_IMAGE):$(VERSION) \
		-t $(GITHUB_BRIDGE_IMAGE):latest \
		.

docker-push: ## Push Docker images to registry
	docker push $(GATEWAY_IMAGE):$(VERSION)
	docker push $(GATEWAY_IMAGE):latest
	docker push $(GITHUB_BRIDGE_IMAGE):$(VERSION)
	docker push $(GITHUB_BRIDGE_IMAGE):latest

# Security scanning
.PHONY: security-scan scan-gateway scan-github-bridge

security-scan: scan-gateway scan-github-bridge ## Run security scans on all images

scan-gateway: docker-build-gateway ## Scan gateway image
	@echo "Scanning gateway image with Docker Scout..."
	docker scout cves $(GATEWAY_IMAGE):$(VERSION)
	docker scout recommendations $(GATEWAY_IMAGE):$(VERSION)

scan-github-bridge: docker-build-github-bridge ## Scan GitHub bridge image
	@echo "Scanning GitHub bridge image with Docker Scout..."
	docker scout cves $(GITHUB_BRIDGE_IMAGE):$(VERSION)
	docker scout recommendations $(GITHUB_BRIDGE_IMAGE):$(VERSION)

# Deployment targets
.PHONY: deploy deploy-local deploy-production

deploy-local: docker-build ## Deploy to local environment
	docker-compose up -d
	@echo "Virtual DOM Gateway is running!"
	@echo "Gateway: localhost:50051"
	@echo "Metrics: localhost:9090 (gateway), localhost:9091 (github-bridge)"
	@echo "Grafana: localhost:3000 (admin/admin)"

deploy-production: docker-push ## Deploy to production
	@echo "Deploying to production..."
	# Add your production deployment commands here
	# kubectl apply -f k8s/ or terraform apply

# Utility targets
.PHONY: clean logs proto lint format

clean: ## Clean build artifacts
	rm -rf target/
	cd src/gateway && cargo clean
	cd src/github-bridge && go clean -cache

logs: ## View logs
	docker-compose logs -f

proto: ## Generate protobuf code
	cd src/proto && \
	protoc --go_out=../github-bridge/proto --go_opt=paths=source_relative \
		--go-grpc_out=../github-bridge/proto --go-grpc_opt=paths=source_relative \
		*.proto

lint: ## Run linters
	cd src/gateway && cargo clippy -- -D warnings
	cd src/github-bridge && golangci-lint run

format: ## Format code
	cd src/gateway && cargo fmt
	cd src/github-bridge && go fmt ./...

# MongoDB utilities
.PHONY: mongo-shell mongo-init

mongo-shell: ## Open MongoDB shell
	docker-compose exec mongodb mongosh -u admin -p changeme

mongo-init: ## Initialize MongoDB
	docker-compose exec -T mongodb mongosh -u admin -p changeme < scripts/mongo-init.js

# Help target
help: ## Show this help
	@echo "Virtual DOM Gateway - Available targets:"
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

.DEFAULT_GOAL := help