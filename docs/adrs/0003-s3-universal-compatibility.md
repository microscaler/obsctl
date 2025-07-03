# ADR-0003: Universal S3 Compatibility Strategy

## Status
**Accepted** - Implemented (July 2025)

## Context
obsctl was originally designed for Cloud.ru OBS but users needed support for multiple S3-compatible providers (AWS S3, MinIO, Wasabi, DigitalOcean Spaces, etc.) without maintaining separate tools.

## Decision
Implement **universal S3 compatibility** while preserving Cloud.ru OBS as the "original use case" in documentation and examples.

### Supported Providers
| Provider | Status | Endpoint Pattern |
|----------|--------|------------------|
| AWS S3 | ✅ Default | `s3.amazonaws.com` |
| Cloud.ru OBS | ✅ Original | `obs.ru-moscow-1.hc.sbercloud.ru` |
| MinIO | ✅ Dev/Test | `localhost:9000` |
| Wasabi | ✅ Hot Storage | `s3.wasabisys.com` |
| DigitalOcean Spaces | ✅ Simple Cloud | `nyc3.digitaloceanspaces.com` |
| Backblaze B2 | ✅ Backup | `s3.us-west-000.backblazeb2.com` |

### Configuration Strategy
- **Environment Variables**: `AWS_ENDPOINT_URL` for any S3-compatible provider
- **CLI Flags**: `--endpoint` for runtime provider switching
- **AWS Config Compatibility**: Standard `~/.aws/config` and `~/.aws/credentials`

## Consequences

### Positive
- ✅ **Broader market appeal** - Works with any S3-compatible storage
- ✅ **Migration flexibility** - Users can switch providers easily
- ✅ **Standard compliance** - Uses AWS SDK patterns
- ✅ **Documentation clarity** - Clear provider-specific examples

### Negative
- ⚠️ **Testing complexity** - Must validate against multiple providers
- ⚠️ **Documentation maintenance** - Provider-specific examples to maintain

## Implementation
- **Configuration**: `src/config.rs` - endpoint URL handling
- **Documentation**: Provider-specific examples in README.md and man page
- **Testing**: Validated against MinIO in CI/CD

## Related ADRs
- ADR-0004: OTEL Integration (provider-agnostic telemetry)

## References
- Configuration: `src/config.rs` - AWS-compatible configuration
- Documentation: README.md - provider compatibility table 