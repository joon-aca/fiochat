.PHONY: help setup build run dev test clean fmt lint install check-deps
.PHONY: setup-rust setup-telegram build-rust build-telegram
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
	@echo "  make build              Build both AI service and Telegram bot"
	@echo "  make install            Install AI service binary to /usr/local/bin"
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
	@echo "  make build-rust         Build AI service only"
	@echo "  make build-telegram     Build Telegram bot only"
	@echo ""
	@echo "$(YELLOW)Quick start:$(NC)"
	@echo "  make setup && make build && make dev"

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
	@echo "  1. Configure AI service: cp config.example.yaml ~/.config/aichat/config.yaml"
	@echo "  2. Configure Telegram bot: cp telegram/.env.example telegram/.env"
	@echo "  3. Build: make build"
	@echo "  4. Run: make dev"

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

## build-telegram: Build Telegram bot
build-telegram:
	@echo "$(BLUE)Building Telegram bot...$(NC)"
	cd telegram && npm run build
	@echo "$(GREEN)✓ Telegram bot built: telegram/dist/$(NC)"

## install: Install AI service binary
install: build-rust
	@echo "$(BLUE)Installing fio binary...$(NC)"
	sudo install -m 755 target/release/fio /usr/local/bin/fio
	@echo "$(GREEN)✓ Installed to /usr/local/bin/fio$(NC)"

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
	@if [ ! -f ~/.config/aichat/config.yaml ]; then \
		echo "$(RED)Warning: Config not found at ~/.config/aichat/config.yaml$(NC)"; \
		echo "$(YELLOW)Copy config.example.yaml and configure your API keys$(NC)"; \
	fi
	cargo run -- --serve 127.0.0.1:8000

## dev-telegram: Run Telegram bot in watch mode
dev-telegram:
	@echo "$(BLUE)Starting Telegram bot (watch mode)...$(NC)"
	@if [ ! -f telegram/.env ]; then \
		echo "$(RED)Error: telegram/.env not found$(NC)"; \
		echo "$(YELLOW)Copy telegram/.env.example and configure$(NC)"; \
		exit 1; \
	fi
	cd telegram && npm run dev

## run-ai: Run AI service (production build)
run-ai: build-rust
	@echo "$(BLUE)Starting AI service (production)...$(NC)"
	./target/release/fio --serve 127.0.0.1:8000

## run-telegram: Run Telegram bot (production build)
run-telegram: build-telegram
	@echo "$(BLUE)Starting Telegram bot (production)...$(NC)"
	@if [ ! -f telegram/.env ]; then \
		echo "$(RED)Error: telegram/.env not found$(NC)"; \
		exit 1; \
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
	@echo "$(BLUE)Setting up configuration files...$(NC)"
	@echo ""
	@if [ ! -f ~/.config/aichat/config.yaml ]; then \
		echo "$(YELLOW)Creating AI service config...$(NC)"; \
		mkdir -p ~/.config/aichat; \
		cp config.example.yaml ~/.config/aichat/config.yaml; \
		echo "$(GREEN)✓ Created ~/.config/aichat/config.yaml$(NC)"; \
		echo "$(YELLOW)  Please edit this file and add your LLM API keys$(NC)"; \
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
	@echo "$(YELLOW)Next steps:$(NC)"
	@echo "  1. Edit ~/.config/aichat/config.yaml (add your LLM API keys)"
	@echo "  2. Edit telegram/.env (add bot token and user IDs)"
	@echo "  3. Run in two terminals:"
	@echo "     Terminal 1: $(BLUE)make dev-ai$(NC)"
	@echo "     Terminal 2: $(BLUE)make dev-telegram$(NC)"
	@echo ""
	@echo "$(YELLOW)Or use tmux:$(NC)"
	@echo "  tmux new-session -d -s fiochat 'make dev-ai'"
	@echo "  tmux split-window -t fiochat -h 'make dev-telegram'"
	@echo "  tmux attach -t fiochat"
