# Release Process

## Automated Release Workflow

The GitHub Actions release workflow (`.github/workflows/release.yaml`) automatically creates production releases when you push a version tag.

### How It Works

1. **Push a version tag:**
   ```bash
   git tag v0.1.0
   git push origin v0.1.0
   ```

2. **GitHub Actions triggers:**
   - Detects the `v*` tag
   - Builds Rust binary for 10 platforms (x86_64, aarch64, armv7, i686, Windows, macOS, Linux)
   - Builds TypeScript Telegram bot (Node.js `dist/`)
   - Creates release tarballs with complete structure
   - Generates SHA256 checksums
   - Publishes to GitHub Releases

3. **Release contains:**
   ```
   fiochat-v0.1.0-linux-amd64.tar.gz
   ├── fiochat (binary)
   ├── telegram/
   │   ├── dist/ (compiled TypeScript)
   │   ├── package.json
   │   └── package-lock.json
   └── deploy/systemd/
       ├── fiochat.service
       └── fiochat-telegram.service
   
   fiochat-v0.1.0-linux-amd64.tar.gz.sha256 (checksum file)
   ```

### Supported Targets

| Target | Platform | Architecture |
| --- | --- | --- |
| `x86_64-unknown-linux-musl` | Linux | 64-bit Intel/AMD |
| `aarch64-unknown-linux-musl` | Linux | 64-bit ARM (Raspberry Pi 4, GCP e2-micro) |
| `armv7-unknown-linux-musleabihf` | Linux | 32-bit ARM (Raspberry Pi 3) |
| `arm-unknown-linux-musleabihf` | Linux | 32-bit ARM (older RPi) |
| `i686-unknown-linux-musl` | Linux | 32-bit Intel/AMD |
| `x86_64-apple-darwin` | macOS | 64-bit Intel |
| `aarch64-apple-darwin` | macOS | 64-bit ARM (Apple Silicon) |
| `x86_64-pc-windows-msvc` | Windows | 64-bit |
| `i686-pc-windows-msvc` | Windows | 32-bit |

## Creating a Release

### Step 1: Update Version

Update the version in `Cargo.toml`:
```toml
[package]
name = "fiochat"
version = "0.1.0"  # Update this
```

### Step 2: Tag and Push

```bash
# Create annotated tag
git tag -a v0.1.0 -m "Release v0.1.0: Initial public release"

# Push to trigger workflow
git push origin v0.1.0
```

### Step 3: Monitor Workflow

1. Go to GitHub → Actions → Release
2. Watch the build progress (usually 10-15 minutes for all platforms)
3. Once complete, releases appear under Releases tab

### Step 4: Verify Release

```bash
# Download and verify a release
cd /tmp
curl -fsSL https://github.com/joon-aca/fiochat/releases/download/v0.1.0/fiochat-v0.1.0-linux-amd64.tar.gz -O
curl -fsSL https://github.com/joon-aca/fiochat/releases/download/v0.1.0/fiochat-v0.1.0-linux-amd64.tar.gz.sha256 -O

# Verify checksum
sha256sum -c fiochat-v0.1.0-linux-amd64.tar.gz.sha256

# Extract and inspect
tar -tzf fiochat-v0.1.0-linux-amd64.tar.gz | head -20
```

## Installing from a Release

Once a release is published, users can install with:

```bash
curl -fsSL https://raw.githubusercontent.com/joon-aca/fiochat/master/scripts/install.sh | bash --tag v0.1.0
```

Or to pin to a specific version:
```bash
./install.sh --tag v0.1.0
```

## Release Channels

### Stable Release
- Tag format: `v0.1.0` (semantic versioning)
- All checks pass
- Published as "Latest Release"

### Release Candidate
- Tag format: `v0.1.0-rc1`, `v0.1.0-rc2`, etc.
- Published as "Pre-release"
- Marked with pre-release flag on GitHub

## Troubleshooting

### Workflow Failed

1. **Check the Actions tab** for error logs
2. **Common issues:**
   - `npm ci` failed → Check `telegram/package-lock.json` is committed
   - Build failed → Check Rust code compiles locally with `cargo build --release`
   - Telegram build failed → Check TypeScript with `cd telegram && npm run build`

### Rollback a Release

If you accidentally pushed a bad tag:
```bash
# Delete local tag
git tag -d v0.1.0

# Delete remote tag
git push --delete origin v0.1.0

# Delete GitHub Release (in GitHub web UI)
```

Then fix the issue and re-tag.

## Testing Locally

Before pushing a tag, test the full build locally:

```bash
# Build Rust binary
cargo build --release --target x86_64-unknown-linux-musl

# Build Telegram bot
cd telegram
npm ci
npm run build
cd ..

# Create test tarball (like the workflow does)
mkdir -p test-release/fiochat-v0.1.0-linux-amd64
cp target/x86_64-unknown-linux-musl/release/fiochat test-release/fiochat-v0.1.0-linux-amd64/
cp -r telegram/dist test-release/fiochat-v0.1.0-linux-amd64/telegram/
cp telegram/package*.json test-release/fiochat-v0.1.0-linux-amd64/telegram/
mkdir -p test-release/fiochat-v0.1.0-linux-amd64/deploy/systemd
cp deploy/systemd/*.service test-release/fiochat-v0.1.0-linux-amd64/deploy/systemd/

# Create tarball
cd test-release
tar -czf fiochat-v0.1.0-linux-amd64.tar.gz fiochat-v0.1.0-linux-amd64/
sha256sum fiochat-v0.1.0-linux-amd64.tar.gz

# Test installation
tar -tzf fiochat-v0.1.0-linux-amd64.tar.gz | head -20
```

## Next Steps

To make your first release:

1. Update `Cargo.toml` version to `0.1.0`
2. Run local build tests
3. Commit: `git commit -am "Version 0.1.0"`
4. Tag: `git tag -a v0.1.0 -m "Release v0.1.0"`
5. Push: `git push origin develop && git push origin v0.1.0`
6. Monitor the workflow in GitHub Actions
7. Share the release URL: `https://github.com/joon-aca/fiochat/releases/tag/v0.1.0`
