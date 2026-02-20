
Introduction to Development Guide
==============================================

  🏗️ Project Structure

  - Clear layout of Rust AI service vs. TypeScript Telegram bot
  - Where to find key files

  🛠️ Development Setup

  Rust AI Service:
  cargo build              # Debug build
  cargo build --release    # Optimized build
  cargo run -- --serve 127.0.0.1:8000  # Run HTTP server

  Telegram Bot:
  cd telegram
  npm install
  npm run build    # Production build
  npm run dev      # Auto-reload development

  🏃 Running Locally

  - AI Service: CLI mode vs HTTP server mode
  - Telegram Bot: Development vs production modes
  - Configuration setup for both

  📝 Code Style

  - Rust: cargo fmt, cargo clippy, cargo test
  - TypeScript: 2-space indent, existing style
  - Conventional commits format

  🔄 Working with Upstream

  Since this is an aichat fork:
  git fetch upstream
  git merge upstream/main
  - What to merge (bug fixes, new providers)
  - What to avoid (breaking API changes)

  ➕ Adding Features

  - New Telegram bot commands
  - New MCP tools
  - New LLM providers

  🐛 Troubleshooting

  Common issues for:
  - Rust build failures
  - TypeScript build issues
  - Service won't start
  - Bot won't connect

  🚀 Release Process

  - Version updates
  - Tagging
  - Building release binaries

 ### 🎯 Makefile Features

#### Core Workflows

#### One-Command Setup:
```
  make setup          # Install all dependencies (Rust + Node.js)
  make config         # Create config files interactively
  make build          # Build both components
```  
  
  #### Development Mode:
  ```
  make dev-ai         # Run AI service with auto-reload
  make dev-telegram   # Run Telegram bot with auto-reload
  make dev            # Instructions for running both
```
  Testing & Quality:
  make test           # Run all tests
  make fmt            # Format Rust code
  make lint           # Run all linters

  Production:
  make build          # Production builds
  make run-ai         # Run AI service (production)
  make run-telegram   # Run Telegram bot (production)
  make install        # Install fio to /usr/local/bin

  Cleanup:
  make clean          # Remove all build artifacts

  Smart Features

  ✅ Dependency Checking - Verifies Rust and Node.js are installed
  ✅ Color Output - Easy-to-read terminal output
  ✅ Config Helper - Interactive config file creation
  ✅ Component-Specific - Can work with Rust or Node.js independently
  ✅ Helpful Messages - Guides you through each step

  Usage Examples

  For New Contributors:
  git clone https://github.com/joon-aca/fiochat.git
  cd fiochat
  make setup && make config && make build
  
  Edit config files, then:
  make dev-ai        # Terminal 1
  make dev-telegram  # Terminal 2

  Daily Development:
  make fmt           # Format before committing
  make lint          # Check for issues
  make test          # Run tests

  Production Build:
  make build
  make install       # Install system-wide