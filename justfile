# Justfile for obsctl utility

# Default task: build and run tests
default:
  just check

# Format and lint
check:
  cargo fmt --all -- --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo check

# Run tests
unit:
  cargo test

# Build release binary
build:
  cargo build --release

# Install to /usr/local/bin
install:
  cp target/release/obsctl /usr/local/bin/obsctl

# Rebuild and install
reinstall:
  just build
  just install

# Clean build artifacts
clean:
  cargo clean

# Run with local arguments
run *ARGS:
  cargo run --release -- {{ARGS}}

# Export OTEL env and run a dry test
otel-dryrun:
  export AWS_ACCESS_KEY_ID="fake:key"
  export AWS_SECRET_ACCESS_KEY="fake_secret"
  export OTEL_EXPORTER_OTLP_ENDPOINT="https://otel.dev/trace"
  just run --source ./tests/data --bucket test-bucket --endpoint https://obs.ru-moscow-1.hc.sbercloud.ru --prefix test/ --dry-run

# Build a .deb package
deb:
  VERSION=$$(grep '^version =' Cargo.toml | head -1 | cut -d '"' -f2)
  mkdir -p deb/usr/local/bin
  cp target/release/obsctl deb/usr/local/bin/
  mkdir -p deb/DEBIAN
  cp packaging/debian/control deb/DEBIAN/control
  chmod 755 deb/DEBIAN
  if [ -f packaging/debian/postinst ]; then cp packaging/debian/postinst deb/DEBIAN/postinst && chmod 755 deb/DEBIAN/postinst; fi
  if [ -f packaging/debian/prerm ]; then cp packaging/debian/prerm deb/DEBIAN/prerm && chmod 755 deb/DEBIAN/prerm; fi
  mkdir -p deb/etc/obsctl
  if [ -f packaging/debian/config ]; then cp packaging/debian/config deb/etc/obsctl/obsctl.conf; fi
  dpkg-deb --build deb upload-obs_$$VERSION_amd64.deb
