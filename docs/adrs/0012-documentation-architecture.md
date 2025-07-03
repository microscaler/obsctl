# ADR-0012: Documentation Architecture Strategy

## Status
**Accepted** - Implemented (July 2025)

## Context

obsctl required a clear, maintainable documentation strategy that serves different user types and use cases. The project needed to balance comprehensive technical documentation with user-friendly guides while avoiding duplication and maintenance overhead.

## Decision

Implement a three-tier documentation architecture with clear separation of concerns and targeted content for different audiences.

### Core Strategy
- **README.md** - Primary user interface and quick start guide
- **docs/adrs/** - Architecture Decision Records for technical decisions
- **Man Page** - Comprehensive CLI reference documentation

## Documentation Architecture

### Tier 1: README.md (User Interface)
**Purpose:** Primary entry point for all users
**Audience:** End users, evaluators, new contributors
**Content Strategy:**
- Project overview and value proposition
- Quick installation instructions
- Essential usage examples
- Key features highlights
- Links to detailed documentation

**Scope:**
- What obsctl does and why it matters
- Installation methods (Homebrew, apt, rpm, Chocolatey)
- Basic usage patterns and examples
- Advanced filtering overview with key examples
- S3 provider compatibility matrix
- Contributing guidelines and community links

### Tier 2: docs/adrs/ (Technical Architecture)
**Purpose:** Technical decision documentation for maintainers
**Audience:** Developers, architects, technical contributors
**Content Strategy:**
- Tightly constrained architectural decisions
- Implementation rationale and alternatives
- Technical consequences and trade-offs
- Migration notes and validation results

**ADR Categories:**
```
Core Features:
- 0001: Advanced Filtering System
- 0002: Pattern Matching Engine
- 0003: S3 Universal Compatibility
- 0004: Performance Optimization Strategy

Observability Stack:
- 0005: OpenTelemetry Implementation
- 0006: Grafana Dashboard Architecture  
- 0007: Prometheus and Jaeger Infrastructure

Infrastructure:
- 0008: Release Management Strategy
- 0009: UUID-Based Integration Testing
- 0010: Docker Compose Architecture
- 0011: Multi-Platform Packaging
- 0012: Documentation Architecture (this ADR)
```

### Tier 3: Man Page (CLI Reference)
**Purpose:** Comprehensive command-line reference
**Audience:** Power users, system administrators, automation scripts
**Content Strategy:**
- Complete command reference with all flags
- Detailed examples for every operation
- Advanced filtering documentation
- Configuration methods and precedence
- Exit codes and error handling

**Man Page Sections:**
- NAME, SYNOPSIS, DESCRIPTION
- COMMANDS (all 9 obsctl commands)
- OPTIONS (all CLI flags with detailed descriptions)
- ADVANCED FILTERING (comprehensive filtering examples)
- CONFIGURATION (AWS config, OTEL setup)
- EXAMPLES (real-world usage scenarios)
- EXIT STATUS, FILES, SEE ALSO

## Content Distribution Strategy

### README.md Content Guidelines
```markdown
# What goes in README.md
✅ Project description and value proposition
✅ Installation instructions (all platforms)
✅ Quick start examples (5-10 essential commands)
✅ Key features overview with brief examples
✅ S3 provider compatibility
✅ Links to detailed documentation
✅ Contributing guidelines

# What does NOT go in README.md
❌ Detailed command reference (→ man page)
❌ Technical architecture details (→ ADRs)
❌ Comprehensive examples (→ man page)
❌ Implementation details (→ ADRs)
❌ Historical decisions (→ ADRs)
```

### ADR Content Guidelines
```markdown
# What goes in ADRs
✅ Technical decisions and rationale
✅ Alternatives considered and rejected
✅ Implementation details and consequences
✅ Migration notes and validation
✅ Performance characteristics
✅ Architecture diagrams and data flows

# What does NOT go in ADRs
❌ User tutorials (→ README.md)
❌ Command reference (→ man page)
❌ Installation instructions (→ README.md)
❌ Basic usage examples (→ README.md)
❌ Ongoing task tracking (→ separate files)
```

### Man Page Content Guidelines
```markdown
# What goes in man page
✅ Complete command reference
✅ All CLI flags and options
✅ Comprehensive usage examples
✅ Advanced filtering documentation
✅ Configuration file formats
✅ Exit codes and error conditions
✅ Enterprise use cases

# What does NOT go in man page
❌ Project overview (→ README.md)
❌ Installation instructions (→ README.md)
❌ Technical architecture (→ ADRs)
❌ Implementation decisions (→ ADRs)
❌ Contributing guidelines (→ README.md)
```

## Documentation Workflow

### Content Creation Process
1. **User-Facing Features** → Update README.md examples + man page reference
2. **Technical Decisions** → Create focused ADR with implementation details
3. **CLI Changes** → Update man page with complete flag documentation
4. **Architecture Changes** → Create or update relevant ADR

### Maintenance Strategy
- **README.md** - Keep concise, update for major feature releases
- **ADRs** - Immutable once accepted, create new ADRs for changes
- **Man Page** - Update for every CLI change, comprehensive reference

### Cross-Reference Strategy
- **README.md** → Links to man page and relevant ADRs
- **ADRs** → Reference implementation files and related ADRs
- **Man Page** → Self-contained reference, minimal external links

## Content Examples

### README.md Example Section
```markdown
## Quick Start

# Install obsctl
brew install obsctl  # macOS
sudo apt install obsctl  # Ubuntu/Debian

# Basic operations
obsctl ls s3://my-bucket/
obsctl cp file.txt s3://my-bucket/
obsctl sync ./dir s3://my-bucket/backup/

# Advanced filtering
obsctl ls s3://logs/ --created-after 7d --min-size 1MB
```

### ADR Example Structure
```markdown
# ADR-XXXX: Decision Title

## Status
**Accepted** - Implemented (July 2025)

## Context
[Problem statement and background]

## Decision
[What was decided and why]

## Alternatives Considered
[Other options evaluated]

## Consequences
[Positive and negative impacts]
```

### Man Page Example Section
```
.SH ADVANCED FILTERING
obsctl supports comprehensive filtering with date, size, and result management.

.SS Date Filtering
.TP
.B --created-after DATE
Show objects created after specified date
.br
Formats: YYYYMMDD (20240101), relative (7d, 30d, 1y)
```

## Alternatives Considered

1. **Single Large Documentation File** - Rejected due to maintenance complexity
2. **Wiki-Based Documentation** - Rejected due to version control issues
3. **Separate User/Developer Docs** - Rejected due to duplication overhead
4. **Generated Documentation Only** - Rejected due to lack of narrative structure
5. **GitHub Pages with Multiple Sections** - Rejected in favor of simpler structure

## Consequences

### Positive
- **Clear Separation of Concerns** - Each tier serves specific audience
- **Reduced Duplication** - Content has single authoritative location
- **Maintainable Structure** - Easy to update without breaking other sections
- **Professional Appearance** - Comprehensive yet organized documentation
- **User-Friendly** - README provides immediate value for new users
- **Developer-Friendly** - ADRs provide technical context for contributors

### Negative
- **Multiple Locations** - Users need to know where to find specific information
- **Cross-Reference Maintenance** - Links between documents require updates
- **Consistency Requirements** - Style and tone must be maintained across tiers
- **Learning Curve** - Contributors need to understand documentation strategy

## Implementation Status

### Current State
- ✅ **README.md** - Comprehensive user interface with S3 compatibility
- ✅ **docs/adrs/** - 12 ADRs covering all major architectural decisions
- ✅ **Man Page** - Enhanced with advanced filtering and comprehensive examples
- ✅ **Cross-References** - Proper linking between documentation tiers

### Documentation Metrics
- **README.md** - ~200 lines, focused on user onboarding
- **ADRs** - 12 documents, ~15KB total, tightly constrained
- **Man Page** - ~400 lines, comprehensive CLI reference
- **Total Coverage** - All features and decisions documented

## Validation

### Success Criteria Met
- ✅ Three-tier architecture implemented and functional
- ✅ Clear content guidelines established for each tier
- ✅ No significant duplication between documentation layers
- ✅ Professional appearance suitable for enterprise use
- ✅ Easy navigation between different documentation types
- ✅ Comprehensive coverage of all obsctl features and decisions

### User Feedback Integration
- **New Users** - README provides immediate value and clear next steps
- **Power Users** - Man page serves as comprehensive reference
- **Developers** - ADRs provide technical context for contributions
- **Enterprise Users** - Professional documentation suitable for evaluation

## Migration Notes

Evolved from ad-hoc documentation to structured three-tier architecture:
- Consolidated scattered documentation into clear hierarchy
- Eliminated duplicate content across multiple files
- Created professional ADR structure for technical decisions
- Enhanced man page to serve as authoritative CLI reference

## References
- [Architecture Decision Records](https://adr.github.io/)
- [Unix Manual Page Standards](https://man7.org/linux/man-pages/man7/man-pages.7.html)
- [Documentation Best Practices](https://documentation.divio.com/)
- [README.md](../../README.md)
- [Man Page](../../packaging/obsctl.1) 