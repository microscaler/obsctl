# ADR-0008: Release Management Strategy

## Status
**Accepted** - Implemented (July 2025)

## Context

obsctl requires automated, reliable release management to support multiple platforms, package formats, and deployment targets. Manual releases were error-prone and inconsistent across the 20+ files requiring version updates.

## Decision

Implement Google's Release Please for automated version management with comprehensive GitHub Actions workflows supporting all platforms and packaging formats.

### Core Strategy
- **Release Please** - Automated version management and changelog generation
- **Conventional Commits** - Standardized commit format for automated releases
- **GitHub Actions** - Multi-platform build and packaging automation
- **Centralized Versioning** - Single source of truth in Cargo.toml

## Implementation Details

### Release Please Configuration
```yaml
# .github/workflows/release-please.yml
release-type: rust
package-name: obsctl
extra-files:
  - packaging/homebrew/obsctl.rb
  - packaging/debian/control
  - packaging/rpm/obsctl.spec
  - packaging/obsctl.1
  - docs/index.md
  - README.md
```

### Version Management
- **Primary Source** - Cargo.toml version field
- **Automatic Updates** - Release Please updates all 20+ version references
- **Dev/Release Handling** - Development versions (1.2.3-dev) strip suffixes for telemetry
- **Service Version** - Centralized get_service_version() function in src/lib.rs

### Conventional Commits Format
```
feat: add new filtering capabilities
fix: resolve S3 connection timeout
docs: update installation instructions
chore: bump dependencies
```

### Release Workflow
1. **Development** - Conventional commits merged to main branch
2. **Release PR** - Release Please creates PR with version bumps and changelog
3. **Approval** - Team reviews and approves release PR
4. **Merge** - PR merge triggers automated release build
5. **Distribution** - Packages published to all supported channels

## Platform Support Matrix

### Target Platforms
- **Linux x64** - x86_64-unknown-linux-gnu
- **Linux ARM64** - aarch64-unknown-linux-gnu  
- **Linux ARMv7** - armv7-unknown-linux-gnueabihf (Raspberry Pi)
- **macOS Universal** - Combined Intel + ARM64 binary
- **Windows x64** - x86_64-pc-windows-msvc

### Package Formats
- **Homebrew** - macOS Universal Binary formula
- **Debian** - .deb packages for all Linux architectures
- **RPM** - .rpm packages for all Linux architectures
- **Chocolatey** - Windows .nupkg package
- **Archives** - .tar.gz/.zip for manual installation

## GitHub Actions Architecture

### Release Workflow (release.yml)
```yaml
strategy:
  matrix:
    include:
      - target: x86_64-unknown-linux-gnu
        os: ubuntu-latest
      - target: aarch64-unknown-linux-gnu
        os: ubuntu-latest
      - target: armv7-unknown-linux-gnueabihf
        os: ubuntu-latest
      - target: x86_64-apple-darwin
        os: macos-latest
      - target: aarch64-apple-darwin
        os: macos-latest
      - target: x86_64-pc-windows-msvc
        os: windows-latest
```

### Build Optimizations
- **Cross Compilation** - Linux ARM targets built on x64 runners
- **Universal Binaries** - macOS Intel + ARM64 combined with lipo
- **Parallel Builds** - All platforms built simultaneously
- **Artifact Management** - Comprehensive artifact collection and release

### CI Workflow (ci.yml)
- **Cross-Platform Testing** - All platforms and architectures
- **Security Audits** - Cargo audit and dependency scanning
- **Packaging Validation** - All package formats tested
- **Integration Testing** - UUID-based comprehensive testing

## Version Handling Strategy

### Development Versions
```rust
// Cargo.toml: version = "1.2.3-dev"
// Service version: "1.2.3" (suffix stripped)
// User display: "obsctl 1.2.3-dev"
```

### Release Versions
```rust
// Cargo.toml: version = "1.2.3"
// Service version: "1.2.3"
// User display: "obsctl 1.2.3"
```

### Centralized Version Function
```rust
pub fn get_service_version() -> String {
    env!("CARGO_PKG_VERSION")
        .split('-')
        .next()
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .to_string()
}
```

## Alternatives Considered

1. **Manual Releases** - Rejected due to error-prone process
2. **Semantic Release** - Rejected in favor of Release Please ecosystem
3. **Custom Versioning Scripts** - Rejected due to maintenance overhead
4. **Single Platform Releases** - Rejected due to user demand
5. **Manual Package Management** - Rejected due to scalability issues

## Consequences

### Positive
- **Automated Consistency** - All 20+ files updated automatically
- **Reduced Errors** - Eliminates manual version management mistakes
- **Multi-Platform Support** - Comprehensive platform coverage
- **Professional Releases** - Consistent changelog and release notes
- **Developer Productivity** - Zero manual release overhead
- **User Experience** - Reliable, predictable releases

### Negative
- **Conventional Commits** - Team must follow commit format
- **GitHub Actions Dependency** - Relies on GitHub infrastructure
- **Build Complexity** - Complex multi-platform build matrix
- **Release Please Learning** - Team needs Release Please knowledge

## Release Validation

### Success Criteria Met
- ✅ Automated version updates across 20+ files
- ✅ Multi-platform builds for 6 target architectures
- ✅ All package formats (Homebrew, Debian, RPM, Chocolatey) working
- ✅ macOS Universal Binaries created successfully
- ✅ GitHub Actions workflows reliable and fast
- ✅ Conventional commits workflow adopted by team
- ✅ Release notes automatically generated

### Performance Metrics
- **Build Time** - <15 minutes for full multi-platform release
- **Package Size** - Optimized binaries <10MB per platform
- **Release Frequency** - Weekly releases supported
- **Error Rate** - <1% build failures

## Package Distribution

### Homebrew
```ruby
# Formula automatically updated by Release Please
class Obsctl < Formula
  desc "High-performance S3-compatible CLI tool"
  homepage "https://github.com/user/obsctl"
  version "#{version}" # x-release-please-version
  # Universal Binary for seamless Intel/ARM64 support
end
```

### Debian/RPM
```bash
# Automatic package building for all architectures
- obsctl_1.2.3_amd64.deb
- obsctl_1.2.3_arm64.deb  
- obsctl_1.2.3_armhf.deb
- obsctl-1.2.3-1.x86_64.rpm
- obsctl-1.2.3-1.aarch64.rpm
- obsctl-1.2.3-1.armv7hl.rpm
```

### Chocolatey
```powershell
# Windows package with PowerShell template processing
$version = "1.2.3" # x-release-please-version
$url64 = "https://github.com/user/obsctl/releases/download/v$version/obsctl-$version-x86_64-pc-windows-msvc.zip"
```

## Migration Notes

Successfully migrated from manual releases to fully automated system:
- Eliminated hardcoded version strings in 12+ source files
- Automated packaging for all supported platforms
- Integrated with existing CI/CD workflows
- Maintained backward compatibility for all package formats

## References
- [Release Please Documentation](https://github.com/googleapis/release-please)
- [Conventional Commits Specification](https://www.conventionalcommits.org/)
- [GitHub Actions Workflows](../.github/workflows/)
- [Packaging Configuration](../packaging/)
- [Version Management Code](../src/lib.rs) 