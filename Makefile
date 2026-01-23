.PHONY: help setup build run dev test clean fmt lint install check-deps
.PHONY: setup-rust setup-telegram build-rust build-telegram build-ai
.PHONY: run-ai run-telegram dev-ai dev-telegram
.PHONY: test-rust test-telegram clean-rust clean-telegram
.PHONY: dist dist-linux dist-macos dist-windows dist-clean

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
	@echo "  make install            Install fiochat binary and fio-notify to /usr/local/bin"
	@echo ""
	@echo "$(GREEN)Distribution:$(NC)"
	@echo "  make dist               Build distribution for current platform (no cross-compile)"
	@echo "  make dist-linux         Build Linux distribution (x86_64, static musl)"
	@echo "  make dist-linux-arm64   Build Linux distribution (arm64, static musl)"
	@echo "  make dist-macos-x86_64  Build Intel macOS (if on ARM64 Apple Silicon)"
	@echo "  make dist-macos-arm64   Build macOS ARM (Apple Silicon)"
	@echo "  make dist-windows       Build Windows distribution (x86_64)"
	@echo "  make dist-linux-all     Build Linux x86_64 + arm64"
	@echo "  make dist-all           Build all distributions (Linux, macOS, Windows)"
	@echo "  make dist-clean         Clean all dist artifacts"
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
	@echo "$(GREEN)✓ AI service built: target/release/fiochat$(NC)"

## build-ai: Build AI service (alias for build-rust)
build-ai: build-rust

## build-telegram: Build Telegram bot
build-telegram:
	@echo "$(BLUE)Building Telegram bot...$(NC)"
	cd telegram && npm run build
	@echo "$(GREEN)✓ Telegram bot built: telegram/dist/$(NC)"

## install: Install AI service binary and fio-notify script
install: build-rust
	@echo "$(BLUE)Installing fiochat binary...$(NC)"
	sudo install -m 755 target/release/fiochat /usr/local/bin/fiochat
	@echo "$(GREEN)✓ Installed to /usr/local/bin/fiochat$(NC)"
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
	@if [ ! -f ~/.config/fiochat/config.yaml ]; then \
		echo "$(RED)Warning: Config not found at ~/.config/fiochat/config.yaml$(NC)"; \
		echo "$(YELLOW)Run 'make config' to set up configuration$(NC)"; \
	fi
	cargo run --bin fiochat -- --serve 127.0.0.1:8000

## dev-telegram: Run Telegram bot in watch mode
dev-telegram:
	@echo "$(BLUE)Starting Telegram bot (watch mode)...$(NC)"
	@if [ ! -f ~/.config/fiochat/config.yaml ]; then \
		echo "$(RED)Warning: Config not found at ~/.config/fiochat/config.yaml$(NC)"; \
		echo "$(YELLOW)Run 'make config' to set up configuration$(NC)"; \
		echo "$(YELLOW)Or set TELEGRAM_BOT_TOKEN and ALLOWED_USER_IDS as environment variables$(NC)"; \
	fi
	cd telegram && npm run dev

## run-ai: Run AI service (production build)
run-ai: build-rust
	@echo "$(BLUE)Starting AI service (production)...$(NC)"
	./target/release/fiochat --serve 127.0.0.1:8000

## run-telegram: Run Telegram bot (production build)
run-telegram: build-telegram
	@echo "$(BLUE)Starting Telegram bot (production)...$(NC)"
	@if [ ! -f ~/.config/fiochat/config.yaml ]; then \
		echo "$(RED)Warning: Config not found at ~/.config/fiochat/config.yaml$(NC)"; \
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
	@if [ ! -f ~/.config/fiochat/config.yaml ]; then \
		if [ -f ~/.config/aichat/config.yaml ]; then \
			echo "$(BLUE)Found legacy aichat config, copying to fiochat...$(NC)"; \
			mkdir -p ~/.config/fiochat; \
			cp ~/.config/aichat/config.yaml ~/.config/fiochat/config.yaml; \
			echo "$(GREEN)✓ Migrated config from aichat to fiochat$(NC)"; \
		else \
			echo "$(YELLOW)Creating AI service config...$(NC)"; \
			mkdir -p ~/.config/fiochat; \
			cp config.example.yaml ~/.config/fiochat/config.yaml; \
			echo "$(GREEN)✓ Created ~/.config/fiochat/config.yaml$(NC)"; \
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
	@echo "  AI Service: ~/.config/fiochat/config.yaml"
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
# =============================================================================
# Distribution targets - create release tarballs
# =============================================================================

# Host OS detection
OS_UNAME := $(shell uname -s)

# Helper: detect host platform
_detect_host_platform = $(shell \
	if uname -s | grep -q Darwin; then \
		uname -m | grep -q arm64 && echo "aarch64-apple-darwin" || echo "x86_64-apple-darwin"; \
	else \
		uname -m | grep -Eq '^(aarch64|arm64)$$' && echo "aarch64-unknown-linux-musl" || echo "x86_64-unknown-linux-musl"; \
	fi)

# Helper: detect if we have cross tool
_check-cross = @command -v cross >/dev/null 2>&1 || { echo "$(RED)cross not found. Install: cargo install cross$(NC)"; exit 1; }

## dist: Build distribution for current platform (no cross-compile)
dist: build _ensure-dist-dir
	@echo "$(BLUE)Building distribution for current platform...$(NC)"
	@host=$(_detect_host_platform); \
	case "$$host" in \
		aarch64-apple-darwin) os_label=macos-arm64 ;; \
		x86_64-apple-darwin) os_label=macos-x86_64 ;; \
		aarch64-unknown-linux-musl) os_label=linux-arm64 ;; \
		x86_64-unknown-linux-musl) os_label=linux-x86_64 ;; \
		x86_64-pc-windows-msvc) os_label=windows-x86_64 ;; \
		*) os_label=$$host ;; \
	esac; \
	echo "  Detected: $$host -> $$os_label"; \
	rustup target list --installed | grep -qx "$$host" || rustup target add "$$host"; \
	cargo build --release --target $$host; \
	$(MAKE) _create-dist-tarball target=$$host os=$$os_label

## dist-linux: Build Linux x86_64 static binary (works on any Linux)
dist-linux: build-telegram _ensure-dist-dir
	@if [ "$(OS_UNAME)" != "Linux" ]; then \
		echo "$(RED)Linux dist is disabled on $(OS_UNAME).$(NC)"; \
		echo "$(YELLOW)Build on Linux, or install musl cross toolchain (e.g., cargo cross + musl) and re-enable.$(NC)"; \
		exit 1; \
	fi
	@echo "$(BLUE)Building Linux x86_64 (static musl)...$(NC)"
	@rustup target list --installed | grep -qx x86_64-unknown-linux-musl || rustup target add x86_64-unknown-linux-musl
	@if uname -m | grep -Eq '^x86_64$$'; then \
		cargo build --release --target x86_64-unknown-linux-musl; \
	else \
		$(_check-cross); \
		cross build --release --target x86_64-unknown-linux-musl; \
	fi
	@$(MAKE) _create-dist-tarball target=x86_64-unknown-linux-musl os=linux-x86_64

## dist-linux-arm64: Build Linux arm64 static binary
dist-linux-arm64: build-telegram _ensure-dist-dir
	@if [ "$(OS_UNAME)" != "Linux" ]; then \
		echo "$(RED)Linux dist is disabled on $(OS_UNAME).$(NC)"; \
		echo "$(YELLOW)Build on Linux, or install musl cross toolchain (e.g., cargo cross + musl) and re-enable.$(NC)"; \
		exit 1; \
	fi
	@echo "$(BLUE)Building Linux arm64 (static musl)...$(NC)"
	@rustup target list --installed | grep -qx aarch64-unknown-linux-musl || rustup target add aarch64-unknown-linux-musl
	@if uname -m | grep -Eq '^(aarch64|arm64)$$'; then \
		cargo build --release --target aarch64-unknown-linux-musl; \
	else \
		$(_check-cross); \
		cross build --release --target aarch64-unknown-linux-musl; \
	fi
	@$(MAKE) _create-dist-tarball target=aarch64-unknown-linux-musl os=linux-arm64

## dist-linux-all: Build Linux distributions (x86_64 + arm64)
dist-linux-all: dist-linux dist-linux-arm64
	@echo "$(GREEN)✓ Linux distributions ready in dist/$(NC)"

## dist-macos-x86_64: Build Intel macOS (for compatibility)
dist-macos-x86_64: build-telegram _ensure-dist-dir
	@echo "$(BLUE)Building macOS x86_64 (Intel)...$(NC)"
	@rustup target list --installed | grep -qx x86_64-apple-darwin || rustup target add x86_64-apple-darwin
	cargo build --release --target x86_64-apple-darwin
	@$(MAKE) _create-dist-tarball target=x86_64-apple-darwin os=macos-x86_64

## dist-macos-arm64: Build ARM macOS (Apple Silicon)
dist-macos-arm64: build-telegram _ensure-dist-dir
	@echo "$(BLUE)Building macOS aarch64 (ARM/Apple Silicon)...$(NC)"
	@rustup target list --installed | grep -qx aarch64-apple-darwin || rustup target add aarch64-apple-darwin
	cargo build --release --target aarch64-apple-darwin
	@$(MAKE) _create-dist-tarball target=aarch64-apple-darwin os=macos-arm64

## dist-windows: Build Windows x86_64
dist-windows: build-telegram _ensure-dist-dir
	@echo "$(BLUE)Building Windows x86_64...$(NC)"
	@rustup target list --installed | grep -qx x86_64-pc-windows-msvc || rustup target add x86_64-pc-windows-msvc
	$(_check-cross)
	cross build --release --target x86_64-pc-windows-msvc
	@$(MAKE) _create-dist-zip target=x86_64-pc-windows-msvc os=windows-x86_64

## dist-all: Build all platforms (Linux, macOS both archs, Windows)
dist-all: build-telegram _ensure-dist-dir
	@echo "$(BLUE)Building all platforms...$(NC)"
	@if [ "$(OS_UNAME)" = "Linux" ]; then \
		$(MAKE) _dist-linux-x86_64; \
		$(MAKE) _dist-linux-arm64; \
	elif [ "$(OS_UNAME)" = "Darwin" ]; then \
		$(MAKE) _dist-macos-x86_64; \
		$(MAKE) _dist-macos-arm64; \
	elif echo "$(OS_UNAME)" | grep -qi windows; then \
		$(MAKE) _dist-windows-x86_64; \
	else \
		echo "$(YELLOW)Unknown host $(OS_UNAME); build native with 'make dist'.$(NC)"; \
	fi
	@echo "$(GREEN)✓ All distributions ready in dist/$(NC)"

# Internal targets for dist-all
_dist-linux-x86_64: _ensure-dist-dir
	@if [ "$(OS_UNAME)" != "Linux" ]; then \
		echo "$(RED)Linux dist is disabled on $(OS_UNAME).$(NC)"; \
		exit 1; \
	fi
	@echo "  Linux x86_64..."
	@rustup target list --installed | grep -qx x86_64-unknown-linux-musl || rustup target add x86_64-unknown-linux-musl
	cargo build --release --target x86_64-unknown-linux-musl
	@$(MAKE) _create-dist-tarball target=x86_64-unknown-linux-musl os=linux-x86_64

_dist-macos-x86_64: _ensure-dist-dir
	@echo "  macOS x86_64..."
	@rustup target list --installed | grep -qx x86_64-apple-darwin || rustup target add x86_64-apple-darwin
	cargo build --release --target x86_64-apple-darwin
	@$(MAKE) _create-dist-tarball target=x86_64-apple-darwin os=macos-x86_64

_dist-macos-arm64: _ensure-dist-dir
	@echo "  macOS aarch64..."
	@rustup target list --installed | grep -qx aarch64-apple-darwin || rustup target add aarch64-apple-darwin
	cargo build --release --target aarch64-apple-darwin
	@$(MAKE) _create-dist-tarball target=aarch64-apple-darwin os=macos-arm64

_dist-linux-arm64: _ensure-dist-dir
	@if [ "$(OS_UNAME)" != "Linux" ]; then \
		echo "$(RED)Linux dist is disabled on $(OS_UNAME).$(NC)"; \
		exit 1; \
	fi
	@echo "  Linux arm64..."
	@rustup target list --installed | grep -qx aarch64-unknown-linux-musl || rustup target add aarch64-unknown-linux-musl
	@if uname -m | grep -Eq '^(aarch64|arm64)$$'; then \
		cargo build --release --target aarch64-unknown-linux-musl; \
	else \
		$(_check-cross); \
		cross build --release --target aarch64-unknown-linux-musl; \
	fi
	@$(MAKE) _create-dist-tarball target=aarch64-unknown-linux-musl os=linux-arm64

_dist-windows-x86_64: _ensure-dist-dir
	@echo "  Windows x86_64..."
	@rustup target list --installed | grep -qx x86_64-pc-windows-msvc || rustup target add x86_64-pc-windows-msvc
	$(_check-cross)
	cross build --release --target x86_64-pc-windows-msvc
	@$(MAKE) _create-dist-zip target=x86_64-pc-windows-msvc os=windows-x86_64

# Helper targets
_ensure-dist-dir:
	@mkdir -p dist

_create-dist-tarball:
	@version=$$(grep '^version = ' Cargo.toml | head -1 | sed 's/.*"\([^"]*\)".*/v\1/'); \
	tarname=fiochat-$$version-$(os); \
	tardir=dist/$$tarname; \
	rm -rf $$tardir; \
	mkdir -p $$tardir; \
	echo "  Packaging $$tarname..."; \
	cp target/$(target)/release/fiochat $$tardir/; \
	mkdir -p $$tardir/telegram; \
	cp -r telegram/dist $$tardir/telegram/ 2>/dev/null || true; \
	cp telegram/package.json $$tardir/telegram/ 2>/dev/null || true; \
	cp telegram/package-lock.json $$tardir/telegram/ 2>/dev/null || true; \
	mkdir -p $$tardir/deploy/systemd; \
	cp deploy/systemd/fiochat.service $$tardir/deploy/systemd/; \
	cp deploy/systemd/fiochat-telegram.service $$tardir/deploy/systemd/; \
	cd dist && tar -czf $$tarname.tar.gz $$tarname/; \
	sha256sum $$tarname.tar.gz > $$tarname.tar.gz.sha256 || shasum -a 256 $$tarname.tar.gz > $$tarname.tar.gz.sha256; \
	echo "  ✓ Created $$tarname.tar.gz"; \
	rm -rf $$tarname

_create-dist-zip:
	@version=$$(grep '^version = ' Cargo.toml | head -1 | sed 's/.*"\([^"]*\)".*/v\1/'); \
	zipname=fiochat-$$version-$(os); \
	zipdir=dist/$$zipname; \
	rm -rf $$zipdir; \
	mkdir -p $$zipdir; \
	echo "  Packaging $$zipname..."; \
	cp target/$(target)/release/fiochat.exe $$zipdir/; \
	mkdir -p $$zipdir/telegram; \
	cp -r telegram/dist $$zipdir/telegram/ 2>/dev/null || true; \
	cp telegram/package.json $$zipdir/telegram/ 2>/dev/null || true; \
	cp telegram/package-lock.json $$zipdir/telegram/ 2>/dev/null || true; \
	mkdir -p $$zipdir/deploy/systemd; \
	cp deploy/systemd/fiochat.service $$zipdir/deploy/systemd/; \
	cp deploy/systemd/fiochat-telegram.service $$zipdir/deploy/systemd/; \
	cd dist && 7z a $$zipname.zip $$zipname/ && (sha256sum $$zipname.zip > $$zipname.zip.sha256 || shasum -a 256 $$zipname.zip > $$zipname.zip.sha256); \
	echo "  ✓ Created $$zipname.zip"; \
	rm -rf $$zipname

## dist-clean: Clean all distribution artifacts
dist-clean:
	@echo "$(BLUE)Cleaning distributions...$(NC)"
	rm -rf dist
	@echo "$(GREEN)✓ Distributions cleaned$(NC)"