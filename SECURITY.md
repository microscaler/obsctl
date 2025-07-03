# Security Policy

## Supported Versions

We actively support the following versions of obsctl with security updates:

| Version | Supported          |
| ------- | ------------------ |
| 1.x.x   | :white_check_mark: |
| 0.9.x   | :white_check_mark: |
| 0.8.x   | :x:                |
| < 0.8   | :x:                |

## Reporting a Vulnerability

We take security vulnerabilities seriously. If you discover a security vulnerability in obsctl, please report it responsibly.

### How to Report

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report security vulnerabilities by:

1. **Email**: Send details to security@microscaler.com
2. **GitHub Security Advisories**: Use the [GitHub Security Advisory](https://github.com/microscaler/obsctl/security/advisories/new) feature
3. **Private Disclosure**: Contact the maintainers directly through secure channels

### What to Include

When reporting a vulnerability, please include:

- **Description**: A clear description of the vulnerability
- **Impact**: What could an attacker accomplish with this vulnerability?
- **Reproduction**: Step-by-step instructions to reproduce the issue
- **Affected Versions**: Which versions of obsctl are affected
- **Environment**: Operating system, Rust version, and other relevant details
- **Proof of Concept**: If possible, include a minimal proof of concept

### Response Timeline

We will acknowledge receipt of vulnerability reports within **48 hours** and aim to provide a more detailed response within **7 days**.

Our security response process:

1. **Acknowledgment** (within 48 hours)
2. **Initial Assessment** (within 7 days)
3. **Detailed Investigation** (within 14 days)
4. **Fix Development** (timeline varies by severity)
5. **Security Advisory Publication** (coordinated disclosure)
6. **Release with Fix** (as soon as safely possible)

## Security Features

### Current Security Measures

- **Dependency Scanning**: Automated scanning with Dependabot and `cargo audit`
- **Code Analysis**: Static analysis with Clippy and security-focused linting
- **Supply Chain Security**: Verified dependencies and reproducible builds
- **Secrets Management**: No hardcoded credentials or API keys
- **Input Validation**: Comprehensive validation of user inputs and S3 responses
- **TLS/SSL**: All network communications use secure protocols
- **Memory Safety**: Rust's memory safety guarantees prevent common vulnerabilities

### Authentication & Authorization

- **AWS Credentials**: Uses standard AWS credential chain (environment, config files, IAM roles)
- **S3 Compatibility**: Works with any S3-compatible service with proper authentication
- **No Credential Storage**: obsctl never stores credentials permanently
- **Least Privilege**: Encourages minimal required permissions

### Data Protection

- **In-Transit Encryption**: All data transfers use TLS/SSL
- **No Data Persistence**: obsctl doesn't cache or store user data
- **Temporary Files**: Secure handling of temporary files during operations
- **Memory Management**: Secure cleanup of sensitive data in memory

## Security Best Practices

### For Users

1. **Keep Updated**: Always use the latest version of obsctl
2. **Secure Credentials**: Use IAM roles or temporary credentials when possible
3. **Network Security**: Use obsctl in secure network environments
4. **Audit Logs**: Monitor S3 access logs for unusual activity
5. **Principle of Least Privilege**: Grant minimal required S3 permissions

### Required S3 Permissions

Minimal permissions for obsctl operations:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "s3:ListBucket",
        "s3:GetObject",
        "s3:PutObject",
        "s3:DeleteObject",
        "s3:GetBucketLocation"
      ],
      "Resource": [
        "arn:aws:s3:::your-bucket-name",
        "arn:aws:s3:::your-bucket-name/*"
      ]
    }
  ]
}
```

### For Developers

1. **Security Reviews**: All code changes undergo security review
2. **Dependency Updates**: Regular updates of dependencies
3. **Static Analysis**: Use `cargo clippy` and `cargo audit` before commits
4. **Input Validation**: Validate all external inputs
5. **Error Handling**: Don't expose sensitive information in error messages

## Vulnerability Disclosure Policy

We follow responsible disclosure practices:

1. **Coordinated Disclosure**: We work with reporters to coordinate public disclosure
2. **CVE Assignment**: We request CVE numbers for confirmed vulnerabilities
3. **Security Advisories**: We publish GitHub Security Advisories for confirmed issues
4. **Release Notes**: Security fixes are clearly documented in release notes
5. **Credit**: We provide appropriate credit to vulnerability reporters (unless they prefer anonymity)

## Security Contacts

- **Primary Contact**: security@microscaler.com
- **Backup Contact**: Use GitHub Security Advisories
- **PGP Key**: Available upon request for encrypted communications

## Scope

This security policy applies to:

- **obsctl CLI tool**: The main binary and all its features
- **Dependencies**: Direct and transitive dependencies
- **Build Process**: CI/CD pipeline and release artifacts
- **Documentation**: Security-related documentation and examples

This policy does NOT cover:

- **Third-party S3 services**: Security of external S3-compatible services
- **User environments**: Security of user systems or networks
- **AWS Infrastructure**: Security of AWS services themselves

## Security Updates

Security updates are released as:

- **Patch Releases**: For low-severity issues (e.g., 1.2.3 → 1.2.4)
- **Minor Releases**: For medium-severity issues (e.g., 1.2.x → 1.3.0)
- **Major Releases**: For high-severity breaking changes (e.g., 1.x.x → 2.0.0)

All security updates are:
- Clearly marked in release notes
- Announced through GitHub releases
- Documented in security advisories
- Available through all distribution channels (Homebrew, Chocolatey, etc.)

## Compliance

obsctl is designed to support compliance with:

- **SOC 2**: Secure data handling practices
- **GDPR**: Data protection and privacy requirements
- **HIPAA**: Healthcare data security (when used appropriately)
- **PCI DSS**: Payment card industry security standards

## Resources

- [GitHub Security Features](https://github.com/features/security)
- [Rust Security Guidelines](https://forge.rust-lang.org/security.html)
- [AWS Security Best Practices](https://aws.amazon.com/security/security-resources/)
- [OWASP Secure Coding Practices](https://owasp.org/www-project-secure-coding-practices-quick-reference-guide/)

---

**Thank you for helping keep obsctl secure!**

*Last updated: January 2025* 