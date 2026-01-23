# Contributing to Fiochat

Thanks for your interest in contributing! This guide covers development setup, building, and contribution guidelines.

## Project Structure

Fiochat is a unified product with two components:

```
fiochat/
├── src/                  # Rust AI service (aichat fork)
│   ├── main.rs          # CLI entry point
│   ├── serve.rs         # HTTP server
│   ├── client/          # LLM provider clients
│   ├── mcp/             # Model Context Protocol
│   └── ...
├── telegram/            # TypeScript Telegram bot
│   ├── src/index.ts     # Bot entry point
│   └── ...
└── deploy/              # Deployment configs
```

## Prerequisites

### For Rust AI Service
- **Rust toolchain** 1.75+ (`rustup install stable`)
- **Cargo** (comes with Rust)

### For Telegram Bot
- **Node.js** 20+
- **npm** 10+

## Development Setup

### Quick Start with Makefile (Recommended)

The easiest way to get started:

```bash
# Clone the repo
git clone https://github.com/joon-aca/fiochat.git
cd fiochat

# One-command setup (installs all dependencies)
make setup

# Interactive config wizard (recommended)
make config
# Follow the prompts to configure:
# - LLM provider and API keys
# - Telegram bot token
# - Allowed user IDs

# Build everything
make build

# Run in development mode (use two terminals)
# Terminal 1:
make dev-ai

# Terminal 2:
make dev-telegram
```

The interactive config wizard (`make config`) will guide you through:
1. Selecting your LLM provider (OpenAI, Claude, Azure, Ollama, etc.)
2. Entering API keys (with hidden input for security)
3. Setting up your Telegram bot credentials
4. Configuring allowed users and server name

If you prefer manual configuration, use `make config-simple` instead to create template files.

**Available Makefile targets:**
- `make help` - Show all available commands
- `make setup` - Install all dependencies
- `make build` - Build both components
- `make dev` - Development mode instructions
- `make test` - Run all tests
- `make fmt` - Format code
- `make lint` - Run linters
- `make clean` - Clean build artifacts

### Manual Setup (Alternative)

If you prefer manual setup or need more control:

#### 1. Clone and Setup

```bash
git clone https://github.com/joon-aca/fiochat.git
cd fiochat

# Add upstream remote for syncing with aichat
git remote add upstream https://github.com/sigoden/aichat.git
```

#### 2. Build the AI Service

```bash
# Development build (faster, with debug symbols)
cargo build

# Release build (optimized)
cargo build --release

# Binary location:
# - Debug: target/debug/fiochat
# - Release: target/release/fiochat
```

#### 3. Setup the Telegram Bot

```bash
cd telegram

# Install dependencies
npm install

# Build TypeScript
npm run build

# Development mode (auto-rebuild on changes)
npm run dev
```

## Running Locally

### AI Service

**CLI Mode:**
```bash
cargo run -- "hello, how are you?"
```

**HTTP Server Mode** (required for Telegram bot):
```bash
cargo run -- --serve 127.0.0.1:8000
```

**With Config:**
```bash
# Create config first
mkdir -p ~/.config/fiochat
cp config.example.yaml ~/.config/fiochat/config.yaml
# Edit with your API keys

cargo run -- --serve 127.0.0.1:8000
```

### Telegram Bot

**Setup:**

The bot reads configuration from `~/.config/fiochat/config.yaml` (telegram section). You can also use environment variables:

```bash
cd telegram
cp .env.example .env
# Edit .env with:
# - TELEGRAM_BOT_TOKEN (from @BotFather)
# - ALLOWED_USER_IDS (your Telegram user ID)
# - SERVER_NAME (e.g., "dev-server")
```

Or run `make config` from the project root for interactive setup.

**Run:**
```bash
# Production mode
npm start

# Development mode (auto-restart on changes)
npm run dev
```

## Code Style

### Rust
- Follow standard Rust formatting: `cargo fmt`
- Run linter: `cargo clippy`
- Run tests: `cargo test`

### TypeScript
- Use 2-space indentation
- No semicolons (matches existing style)
- Format with your editor's TypeScript formatter

## Testing

### AI Service
```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture
```

### Telegram Bot
```bash
cd telegram

# Type checking
npm run build

# Manual testing: send messages via Telegram
```

## Making Changes

### Branching Strategy

```bash
# Create feature branch from develop
git checkout develop
git pull
git checkout -b feature/your-feature-name

# Make changes, commit
git add .
git commit -m "feat: add new feature"

# Push and create PR
git push origin feature/your-feature-name
```

### Commit Messages

Use conventional commits format:

- `feat:` New feature
- `fix:` Bug fix
- `docs:` Documentation changes
- `refactor:` Code refactoring
- `test:` Adding tests
- `chore:` Maintenance tasks

Examples:
```bash
git commit -m "feat: add MCP support for Docker operations"
git commit -m "fix: handle Telegram rate limiting"
git commit -m "docs: update deployment guide for Docker"
```

## Working with Upstream (aichat)

Fiochat is forked from [aichat](https://github.com/sigoden/aichat). We maintain compatibility while adding custom features.

### Syncing with Upstream

```bash
# Fetch upstream changes
git fetch upstream

# Merge upstream main into develop
git checkout develop
git merge upstream/main

# Resolve conflicts if any
# Test thoroughly after merging
cargo test
cargo run -- --serve 127.0.0.1:8000

# Push merged changes
git push origin develop
```

### What to Merge

- ✅ Bug fixes from upstream
- ✅ New LLM provider support
- ✅ Core feature improvements
- ⚠️ Review carefully: Changes to HTTP server API (affects Telegram bot)

### What NOT to Merge

- ❌ Changes that conflict with Telegram integration
- ❌ Breaking API changes without adaptation

## Adding Features

### New Telegram Bot Commands

1. Edit `telegram/src/index.ts`
2. Add command handler:
   ```typescript
   bot.command("mycommand", async (ctx) => {
     await ctx.reply("Response");
   });
   ```
3. Update `telegram/README.md` with new command
4. Test via Telegram

### New MCP Tools

1. See `docs/mcp-integrations.md`
2. Add tool definition in config
3. Implement tool handler in Rust
4. Update documentation

### New LLM Provider

This should come from upstream aichat. If adding custom:

1. Add client in `src/client/`
2. Register in client registry
3. Add to `models.yaml`
4. Update documentation

## Release Process

1. **Update version** in `Cargo.toml` and `telegram/package.json`
2. **Update CHANGELOG** (if you maintain one)
3. **Tag release:**
   ```bash
   git tag -a v0.2.0 -m "Release v0.2.0"
   git push origin v0.2.0
   ```
4. **Build release binaries:**
   ```bash
   cargo build --release
   ```

## Project-Specific Guidelines

### Telegram Bot
- Keep `src/index.ts` focused on bot logic
- Extract complex operations into separate modules
- Handle errors gracefully (Telegram users should get helpful messages)
- Respect Telegram rate limits

### Rust AI Service
- Maintain compatibility with aichat CLI interface
- Document new environment variables
- Add tests for new features
- Keep HTTP API OpenAI-compatible

## Troubleshooting Development Issues

### Rust Build Fails
```bash
# Clean and rebuild
cargo clean
cargo build

# Update dependencies
cargo update
```

### TypeScript Build Fails
```bash
cd telegram
rm -rf node_modules package-lock.json
npm install
npm run build
```

### AI Service Won't Start
- Check config: `cat ~/.config/fiochat/config.yaml`
- Verify API keys are set
- Check port not in use: `lsof -i :8000`

### Telegram Bot Won't Connect
- Verify bot token: `curl https://api.telegram.org/bot<TOKEN>/getMe`
- Check AI service is running: `curl http://127.0.0.1:8000/v1/models`
- Review logs: `npm run dev` shows detailed output

## Getting Help

- **Issues:** Open an issue on GitHub
- **Discussions:** Use GitHub Discussions for questions
- **Upstream (aichat):** For aichat-specific features, see [aichat wiki](https://github.com/sigoden/aichat/wiki)

## Code of Conduct

- Be respectful and inclusive
- Provide constructive feedback
- Focus on what's best for the project
- Help others learn and grow

## License

By contributing, you agree that your contributions will be licensed under the same terms as the project (MIT OR Apache-2.0).
