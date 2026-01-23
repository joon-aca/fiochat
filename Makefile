.PHONY: help setup build run dev test clean fmt lint install check-deps
.PHONY: setup-rust setup-telegram build-rust build-telegram build-ai
.PHONY: run-ai run-telegram dev-ai dev-telegram
.PHONY: test-rust test-telegram clean-rust clean-telegram

# Default target
.DEFAULT_GOAL := help

# Colors for output
BLUE := \033[0;34m
GREEN := \033[0;32m
YELLOW := \033[0;33m
RED := \033[0;31m
NC := \033[0m # No Color

## help: Show this help message
help:
	@echo "$(BLUE)Fiochat Development Makefile$(NC)"
	@echo ""
	@echo "$(GREEN)Setup & Build:$(NC)"
	@echo "  make setup              Install all dependencies (Rust + Node.js)"
	@echo "  make config             Interactive config wizard (recommended)"
	@echo "  make config-simple      Create config templates (non-interactive)"
	@echo "  make build              Build both AI service and Telegram bot"
	@echo "  make install            Install fio binary and fio-notify to /usr/local/bin"
	@echo ""
	@echo "$(GREEN)Development:$(NC)"
	@echo "  make dev                Run both services in development mode"
	@echo "  make dev-ai             Run AI service in development mode"
	@echo "  make dev-telegram       Run Telegram bot in watch mode"
	@echo ""
	@echo "$(GREEN)Running:$(NC)"
	@echo "  make run-ai             Run AI service (HTTP server)"
	@echo "  make run-telegram       Run Telegram bot"
	@echo ""
	@echo "$(GREEN)Testing & Quality:$(NC)"
	@echo "  make test               Run all tests"
	@echo "  make fmt                Format all code"
	@echo "  make lint               Run linters"
	@echo ""
	@echo "$(GREEN)Cleanup:$(NC)"
	@echo "  make clean              Clean all build artifacts"
	@echo ""
	@echo "$(GREEN)Component-specific:$(NC)"
	@echo "  make setup-rust         Install Rust dependencies only"
	@echo "  make setup-telegram     Install Node.js dependencies only"
	@echo "  make build-ai           Build AI service only"
	@echo "  make build-rust         Build AI service only (alias)"
	@echo "  make build-telegram     Build Telegram bot only"
	@echo ""
	@echo "$(YELLOW)Quick start:$(NC)"
	@echo "  make setup && make config && make build && make dev"

## check-deps: Check if required tools are installed
check-deps:
	@echo "$(BLUE)Checking dependencies...$(NC)"
	@command -v cargo >/dev/null 2>&1 || { echo "$(RED)Error: cargo not found. Install Rust from https://rustup.rs/$(NC)"; exit 1; }
	@command -v node >/dev/null 2>&1 || { echo "$(RED)Error: node not found. Install Node.js 20+$(NC)"; exit 1; }
	@command -v npm >/dev/null 2>&1 || { echo "$(RED)Error: npm not found. Install Node.js 20+$(NC)"; exit 1; }
	@echo "$(GREEN)✓ All required tools found$(NC)"

## setup: Install all dependencies
setup: check-deps setup-rust setup-telegram
	@echo "$(GREEN)✓ Setup complete!$(NC)"
	@echo ""
	@echo "$(YELLOW)Next steps:$(NC)"
	@echo "  1. Configure: make config (interactive wizard)"
	@echo "  2. Build: make build"
	@echo "  3. Run: make dev-ai (Terminal 1) && make dev-telegram (Terminal 2)"

## setup-rust: Install Rust dependencies
setup-rust:
	@echo "$(BLUE)Installing Rust dependencies...$(NC)"
	cargo fetch
	@echo "$(GREEN)✓ Rust dependencies installed$(NC)"

## setup-telegram: Install Node.js dependencies
setup-telegram:
	@echo "$(BLUE)Installing Telegram bot dependencies...$(NC)"
	cd telegram && npm install
	@echo "$(GREEN)✓ Node.js dependencies installed$(NC)"

## build: Build both components
build: build-rust build-telegram
	@echo "$(GREEN)✓ Build complete!$(NC)"

## build-rust: Build AI service
build-rust:
	@echo "$(BLUE)Building AI service (release)...$(NC)"
	cargo build --release
	@echo "$(GREEN)✓ AI service built: target/release/fio$(NC)"

## build-ai: Build AI service (alias for build-rust)
build-ai: build-rust

## build-telegram: Build Telegram bot
build-telegram:
	@echo "$(BLUE)Building Telegram bot...$(NC)"
	cd telegram && npm run build
	@echo "$(GREEN)✓ Telegram bot built: telegram/dist/$(NC)"

## install: Install AI service binary and fio-notify script
install: build-rust
	@echo "$(BLUE)Installing fio binary...$(NC)"
	sudo install -m 755 target/release/fio /usr/local/bin/fio
	@echo "$(GREEN)✓ Installed to /usr/local/bin/fio$(NC)"
	@echo "$(BLUE)Installing fio-notify script...$(NC)"
	sudo install -m 755 scripts/fio-notify /usr/local/bin/fio-notify
	@echo "$(GREEN)✓ Installed to /usr/local/bin/fio-notify$(NC)"

## dev: Run both services in development mode
dev:
	@echo "$(BLUE)Starting development mode...$(NC)"
	@echo ""
	@echo "$(YELLOW)This will run both services. You need two terminals:$(NC)"
	@echo "  Terminal 1: make dev-ai"
	@echo "  Terminal 2: make dev-telegram"
	@echo ""
	@echo "Or use tmux/screen to run in background"
	@echo ""
	@echo "$(YELLOW)Starting AI service in this terminal...$(NC)"
	@$(MAKE) dev-ai

## dev-ai: Run AI service in development mode
dev-ai:
	@echo "$(BLUE)Starting AI service (HTTP server)...$(NC)"
	@echo "$(YELLOW)Endpoint: http://127.0.0.1:8000$(NC)"
	@if [ ! -f ~/.config/fio/config.yaml ]; then \
		echo "$(RED)Warning: Config not found at ~/.config/fio/config.yaml$(NC)"; \
		echo "$(YELLOW)Run 'make config' to set up configuration$(NC)"; \
	fi
	cargo run --bin fio -- --serve 127.0.0.1:8000

## dev-telegram: Run Telegram bot in watch mode
dev-telegram:
	@echo "$(BLUE)Starting Telegram bot (watch mode)...$(NC)"
	@if [ ! -f ~/.config/fio/config.yaml ]; then \
		echo "$(RED)Warning: Config not found at ~/.config/fio/config.yaml$(NC)"; \
		echo "$(YELLOW)Run 'make config' to set up configuration$(NC)"; \
		echo "$(YELLOW)Or set TELEGRAM_BOT_TOKEN and ALLOWED_USER_IDS as environment variables$(NC)"; \
	fi
	cd telegram && npm run dev

## run-ai: Run AI service (production build)
run-ai: build-rust
	@echo "$(BLUE)Starting AI service (production)...$(NC)"
	./target/release/fio --serve 127.0.0.1:8000

## run-telegram: Run Telegram bot (production build)
run-telegram: build-telegram
	@echo "$(BLUE)Starting Telegram bot (production)...$(NC)"
	@if [ ! -f ~/.config/fio/config.yaml ]; then \
		echo "$(RED)Warning: Config not found at ~/.config/fio/config.yaml$(NC)"; \
		echo "$(YELLOW)Run 'make config' or set environment variables$(NC)"; \
	fi
	cd telegram && npm start

## test: Run all tests
test: test-rust test-telegram
	@echo "$(GREEN)✓ All tests passed!$(NC)"

## test-rust: Run Rust tests
test-rust:
	@echo "$(BLUE)Running Rust tests...$(NC)"
	cargo test

## test-telegram: Run Telegram bot tests (type checking)
test-telegram:
	@echo "$(BLUE)Running Telegram bot tests (type check)...$(NC)"
	cd telegram && npm run build

## fmt: Format all code
fmt:
	@echo "$(BLUE)Formatting Rust code...$(NC)"
	cargo fmt
	@echo "$(GREEN)✓ Rust code formatted$(NC)"

## lint: Run all linters
lint:
	@echo "$(BLUE)Linting Rust code...$(NC)"
	cargo clippy -- -D warnings
	@echo "$(BLUE)Type checking TypeScript...$(NC)"
	cd telegram && npm run build
	@echo "$(GREEN)✓ All linters passed$(NC)"

## clean: Clean all build artifacts
clean: clean-rust clean-telegram
	@echo "$(GREEN)✓ Cleanup complete!$(NC)"

## clean-rust: Clean Rust build artifacts
clean-rust:
	@echo "$(BLUE)Cleaning Rust build artifacts...$(NC)"
	cargo clean

## clean-telegram: Clean Telegram bot build artifacts
clean-telegram:
	@echo "$(BLUE)Cleaning Telegram bot build artifacts...$(NC)"
	rm -rf telegram/dist telegram/node_modules

## config: Setup configuration files interactively
config:
	@./scripts/setup-config.sh

## config-simple: Setup config files with templates (non-interactive)
config-simple:
	@echo "$(BLUE)Setting up configuration files...$(NC)"
	@echo ""
	@if [ ! -f ~/.config/fio/config.yaml ]; then \
		if [ -f ~/.config/aichat/config.yaml ]; then \
			echo "$(BLUE)Found legacy aichat config, copying to fio...$(NC)"; \
			mkdir -p ~/.config/fio; \
			cp ~/.config/aichat/config.yaml ~/.config/fio/config.yaml; \
			echo "$(GREEN)✓ Migrated config from aichat to fio$(NC)"; \
		else \
			echo "$(YELLOW)Creating AI service config...$(NC)"; \
			mkdir -p ~/.config/fio; \
			cp config.example.yaml ~/.config/fio/config.yaml; \
			echo "$(GREEN)✓ Created ~/.config/fio/config.yaml$(NC)"; \
			echo "$(YELLOW)  Please edit this file and add your LLM API keys$(NC)"; \
		fi \
	else \
		echo "$(GREEN)✓ AI service config already exists$(NC)"; \
	fi
	@echo ""
	@if [ ! -f telegram/.env ]; then \
		echo "$(YELLOW)Creating Telegram bot config...$(NC)"; \
		cp telegram/.env.example telegram/.env; \
		echo "$(GREEN)✓ Created telegram/.env$(NC)"; \
		echo "$(YELLOW)  Please edit this file and add:$(NC)"; \
		echo "$(YELLOW)    - TELEGRAM_BOT_TOKEN (from @BotFather)$(NC)"; \
		echo "$(YELLOW)    - ALLOWED_USER_IDS (your Telegram user ID)$(NC)"; \
	else \
		echo "$(GREEN)✓ Telegram bot config already exists$(NC)"; \
	fi
	@echo ""
	@echo "$(GREEN)Configuration setup complete!$(NC)"

## quick-start: Full setup for new developers
quick-start: check-deps setup config build
	@echo ""
	@echo "$(GREEN)═══════════════════════════════════════$(NC)"
	@echo "$(GREEN)✓ Quick start complete!$(NC)"
	@echo "$(GREEN)═══════════════════════════════════════$(NC)"
	@echo ""
	@echo "$(YELLOW)Configuration files:$(NC)"
	@echo "  AI Service: ~/.config/fio/config.yaml"
	@echo "  Telegram:   telegram/.env"
	@echo ""
	@echo "$(YELLOW)Next steps - Run in two terminals:$(NC)"
	@echo "     Terminal 1: $(BLUE)make dev-ai$(NC)"
	@echo "     Terminal 2: $(BLUE)make dev-telegram$(NC)"
	@echo ""
	@echo "$(YELLOW)Or use tmux:$(NC)"
	@echo "  tmux new-session -d -s fiochat 'make dev-ai'"
	@echo "  tmux split-window -t fiochat -h 'make dev-telegram'"
	@echo "  tmux attach -t fiochat"
