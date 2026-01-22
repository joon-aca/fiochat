# Quick Development Guide

> **TL;DR**: Run `make help` to see all available commands. Use `make setup && make config && make build` to get started.

## 🏗️ Project Structure

Fiochat is a unified product with two components:

```
fiochat/
├── src/                  # Rust AI service (aichat fork)
│   ├── main.rs          # CLI entry point
│   ├── serve.rs         # HTTP server
│   └── ...
├── telegram/            # TypeScript Telegram bot
│   ├── src/index.ts     # Bot entry point
│   └── ...
└── Makefile             # Development automation
```

**Key files to know:**
- `Cargo.toml` - Rust dependencies and metadata
- `telegram/package.json` - Node.js dependencies
- `config.example.yaml` - Unified config template (AI service + Telegram bot)
- `telegram/.env.example` - Optional: environment variable overrides

## 🛠️ Development Setup

### Quick Start (Makefile)

```bash
# Install all dependencies
make setup

# Interactive config wizard (walks you through setup)
make config

# Build everything
make build
```

**The `make config` wizard will:**
- Prompt for LLM provider (OpenAI, Claude, Azure OpenAI, Ollama, etc.)
- Request API keys (hidden input for security)
- Set up Telegram bot token and allowed users
- Create both config files with your settings

**Alternative**: Use `make config-simple` to create template files that you edit manually.

### Manual Commands

**Rust AI Service:**
```bash
cargo build              # Debug build
cargo build --release    # Optimized build
cargo run -- --serve 127.0.0.1:8000  # Run HTTP server
```

**Telegram Bot:**
```bash
cd telegram
npm install
npm run build    # Production build
npm run dev      # Auto-reload development
```

## 🏃 Running Locally

### Development Mode (Recommended)

**Terminal 1 - AI Service:**
```bash
make dev-ai
# or manually:
cargo run -- --serve 127.0.0.1:8000
```

**Terminal 2 - Telegram Bot:**
```bash
make dev-telegram
# or manually:
cd telegram && npm run dev
```

### Production Mode

```bash
make run-ai        # Run optimized AI service
make run-telegram  # Run built Telegram bot
```

### Configuration

**AI Service** (`~/.config/fiochat/config.yaml`):
```yaml
# Telegram Bot Configuration
telegram:
  telegram_bot_token: YOUR_BOT_TOKEN_HERE
  allowed_user_ids: "123456789"
  server_name: dev-server
  ai_service_api_url: http://127.0.0.1:8000/v1/chat/completions

# AI Service Configuration
model: openai:gpt-4o-mini
clients:
- type: openai
  api_key: sk-...
```

**Alternatively**, use environment variables in `telegram/.env`:
```env
TELEGRAM_BOT_TOKEN=your_token
ALLOWED_USER_IDS=123456789
SERVER_NAME=dev-server
```

## 📝 Code Style

### Rust
```bash
cargo fmt          # Format code
cargo clippy       # Lint
cargo test         # Run tests
```

### TypeScript
- **Indentation**: 2 spaces
- **Semicolons**: None (matches existing style)
- **Type checking**: `npm run build`

### Commit Messages
Use conventional commits:
```bash
feat: add new feature
fix: bug fix
docs: documentation
refactor: code refactoring
test: adding tests
chore: maintenance
```

## 🔄 Working with Upstream

Fiochat is forked from [aichat](https://github.com/sigoden/aichat):

```bash
# Sync with upstream
git fetch upstream
git merge upstream/main

# Test after merging
cargo test
cargo run -- --serve 127.0.0.1:8000
```

**What to merge:**
- ✅ Bug fixes from upstream
- ✅ New LLM provider support
- ✅ Core feature improvements

**What to avoid:**
- ❌ Breaking API changes (affects Telegram bot)
- ❌ Changes conflicting with Telegram integration

## ➕ Adding Features

### New Telegram Bot Command

Edit `telegram/src/index.ts`:
```typescript
bot.command("mycommand", async (ctx) => {
  await ctx.reply("Response");
});
```

### New MCP Tool

1. Add tool definition in config
2. Implement handler in Rust
3. Update documentation

### New LLM Provider

Should come from upstream aichat. See [aichat docs](https://github.com/sigoden/aichat/wiki).

## 🐛 Troubleshooting

### Rust Build Fails
```bash
cargo clean
cargo build
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
- Verify API keys
- Check port: `lsof -i :8000`

### Telegram Bot Won't Connect
- Test bot token: `curl https://api.telegram.org/bot<TOKEN>/getMe`
- Check AI service: `curl http://127.0.0.1:8000/v1/models`
- Review logs: `make dev-telegram`

## 🎯 Makefile Reference

### Core Workflows

**Setup:**
```bash
make setup          # Install all dependencies
make config         # Create config files
make build          # Build both components
```

**Development:**
```bash
make dev-ai         # Run AI service with auto-reload
make dev-telegram   # Run Telegram bot with auto-reload
make dev            # Instructions for running both
```

**Testing & Quality:**
```bash
make test           # Run all tests
make fmt            # Format Rust code
make lint           # Run all linters
```

**Production:**
```bash
make build          # Production builds
make run-ai         # Run AI service (production)
make run-telegram   # Run Telegram bot (production)
make install        # Install fio to /usr/local/bin
```

**Cleanup:**
```bash
make clean          # Remove all build artifacts
```

### Smart Features

✅ **Dependency Checking** - Verifies Rust and Node.js are installed
✅ **Color Output** - Easy-to-read terminal output
✅ **Config Helper** - Interactive config file creation
✅ **Component-Specific** - Can work with Rust or Node.js independently
✅ **Helpful Messages** - Guides you through each step

## 🚀 Common Workflows

### For New Contributors

```bash
git clone https://github.com/joon-aca/fiochat.git
cd fiochat
make setup && make config && make build

# Config wizard will have created ~/.config/fiochat/config.yaml
# with both AI service and Telegram bot configuration.
# Edit if needed to add additional settings.

# Run in two terminals:
make dev-ai        # Terminal 1
make dev-telegram  # Terminal 2
```

### Daily Development

```bash
make fmt           # Format before committing
make lint          # Check for issues
make test          # Run tests

git add .
git commit -m "feat: add new feature"
```

### Production Build

```bash
make build
make install       # Install system-wide
```

### Release

```bash
# Update version in Cargo.toml and telegram/package.json
make build
git tag -a v0.2.0 -m "Release v0.2.0"
git push origin v0.2.0
```

## 📚 Further Reading

- [CONTRIBUTING.md](CONTRIBUTING.md) - Detailed contribution guidelines
- [DEPLOYMENT.md](DEPLOYMENT.md) - Production deployment guide
- [README.md](README.md) - Project overview
- [MCP.md](MCP.md) - MCP configuration
- [telegram/README.md](telegram/README.md) - Telegram bot specifics

## 💡 Tips

- **Use `make help`** to see all available commands
- **Run `make config`** if you need to recreate config files
- **Check logs** with `journalctl -u fiochat.service -f` (production) or watch terminal output (dev)
- **Test locally** before deploying to production
- **Keep upstream synced** regularly to get bug fixes and new features

---

**Happy coding!** 🚀

For questions, see [CONTRIBUTING.md](CONTRIBUTING.md) or open an issue on GitHub.
