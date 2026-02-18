# Contributing to OVOS Rust Messagebus

Thanks for your interest in contributing!

## Development Setup

### Prerequisites

- Rust 1.70+ (stable)
- Cargo (comes with Rust)

### Getting Started

```bash
git clone https://github.com/OscillateLabsLLC/ovos-rust-messagebus
cd ovos-rust-messagebus
cargo build
```

### Running Locally

```bash
# Development build
cargo run

# With environment variables
OVOS_BUS_HOST=0.0.0.0 OVOS_BUS_PORT=8181 cargo run

# Production build
cargo build --release
./target/release/ovos_messagebus
```

## Common Commands

```bash
# Run tests
cargo test --all-features -- --test-threads=1

# Format code
cargo fmt --all

# Check formatting
cargo fmt --all -- --check

# Run linter
cargo clippy --all --all-features --tests -- -D warnings

# Build for release
cargo build --release
```

## Code Style

This project uses standard Rust formatting and linting:

- **Formatting**: `rustfmt` with default settings
- **Linting**: `clippy` with warnings treated as errors
- All code must pass both checks before merging

## Testing

We use Rust's built-in testing framework with `serial_test` for tests that require sequential execution.

```bash
# Run all tests
cargo test --all-features -- --test-threads=1

# Run specific test
cargo test test_name --all-features -- --test-threads=1
```

Note: Tests currently use `--test-threads=1` to ensure proper sequential execution for WebSocket tests.

## Security

### Dependency Auditing

Check for known security vulnerabilities:

```bash
# Install cargo-audit
cargo install cargo-audit

# Run audit
cargo audit
```

### Deny List

Check for license compliance and banned dependencies:

```bash
# Install cargo-deny
cargo install cargo-deny

# Run checks
cargo deny check
```

## Pull Requests

1. Create a feature branch: `git checkout -b feat/my-feature`
2. Make your changes and add tests if applicable
3. Run `cargo fmt --all` and `cargo clippy --all --all-features --tests -- -D warnings`
4. Run `cargo test --all-features -- --test-threads=1` to ensure all tests pass
5. Commit using [Conventional Commits](https://www.conventionalcommits.org/) (e.g., `feat:`, `fix:`, `docs:`)
6. Push to your fork
7. Open a pull request

**PR Guidelines:**

- Keep PRs focused on a single concern
- Include tests for new functionality
- Update documentation as needed
- Ensure all CI checks pass
- Link related issues

## Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/) format:

```
feat: add support for SSL/TLS connections
fix: resolve message routing issue for broadcast events
docs: update configuration examples
test: add integration tests for WebSocket connections
chore: update dependencies
```

**Common prefixes:**

- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation changes
- `test:` - Test additions or modifications
- `refactor:` - Code refactoring
- `perf:` - Performance improvements
- `chore:` - Maintenance tasks
- `ci:` - CI/CD changes

## Docker

Build and test the Docker image:

```bash
# Build image
docker build -t ovos-rust-messagebus .

# Run container
docker run -p 8181:8181 -e OVOS_BUS_HOST=0.0.0.0 ovos-rust-messagebus
```

## Release Process

This project uses [release-please](https://github.com/googleapis/release-please) for automated versioning:

1. Create commits using conventional commit messages
2. Release-please creates a PR with version bump and changelog
3. When the PR is merged, a release is created automatically
4. Binaries and Docker images are built and published

## Cross-Compilation

For ARM and other architectures:

```bash
# Install cross
cargo install cross

# Build for ARM64
cross build --release --target aarch64-unknown-linux-gnu

# Build for ARMv7
cross build --release --target armv7-unknown-linux-gnueabihf
```

## Questions?

- Open an issue for bugs or feature requests
- Join the [OpenVoiceOS Matrix chat](https://matrix.to/#/!XFpdtmgyCoPDxOMPpH:matrix.org?via=matrix.org)
- Check existing issues before creating new ones

## Code of Conduct

Be respectful and constructive. We're building tools for the OpenVoiceOS community - professionalism and clear communication are essential.

## License

By contributing, you agree that your contributions will be licensed under the Apache 2.0 License.
