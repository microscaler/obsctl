# GitHub Actions Workflow Architecture

This directory contains the comprehensive CI/CD pipeline for obsctl, designed around the principle of "duplication is the mother of fragility" with a single controlling workflow orchestrating all operations.

## 🏗️ Architecture Overview

```mermaid
graph TD
    A[main.yml - Controller] --> B[Conventional Commits - Embedded]
    A --> C[ci.yml]
    A --> D[release-please.yml]
    B --> C
    C --> D
    D --> E[GitHub Release]
    
    A --> F[Status Report]
    A --> G[Failure Notifications]
    
    style B fill:#e1f5fe
    style A fill:#f3e5f5
```

## 📋 Workflow Files

### 🎯 main.yml - Master Controller
**Purpose**: Orchestrates all CI/CD operations with intelligent conditional logic
**Triggers**:
- Push to `main`, `master`, `develop` branches
- Pull requests to `main`, `master`, `develop` branches  
- Manual dispatch with options

**Key Features**:
- **Intelligent Routing**: Determines which workflows to run based on branch and event
- **Release Control**: Only runs releases on PR merges to main/master or manual dispatch
- **Emergency Options**: Skip tests for emergency releases
- **Status Reporting**: Comprehensive pipeline status with failure notifications
- **Issue Creation**: Automatically creates issues for failed releases
- **Embedded Validation**: Includes conventional commits validation directly (no separate workflow)

### 🔍 Conventional Commits Validation - Embedded Job
**Purpose**: Validates conventional commit format and standards
**Location**: Embedded within main.yml workflow
**Dependencies**: None (runs first after controller)

### 🧪 ci.yml - Continuous Integration
**Purpose**: Comprehensive testing, linting, and quality assurance
**Triggers**: Called by main.yml controller
**Dependencies**: Conventional commits validation must pass

**Features**:
- Pre-commit hooks validation
- Cross-platform compilation tests
- Cargo clippy linting
- Security audits
- Integration testing

### 🚀 release-please.yml - Release Pipeline
**Purpose**: Complete release automation with multi-platform builds
**Triggers**: Called by main.yml controller (PR merges to main/master only)
**Dependencies**: Conventional commits validation + ci.yml must pass

**Features**:
- Release-please automation
- Release configuration testing
- Multi-platform builds (6 architectures)
- Package creation (Debian, Chocolatey, Universal Binary)
- GitHub release creation

## 🔄 Execution Flow

### Pull Request Flow
```
PR Created/Updated → main.yml → [Conventional Commits] → ci.yml → Status Report
```

### Development Branch Flow
```
Push to develop → main.yml → [Conventional Commits] → ci.yml → Status Report
```

### Release Flow (PR Merge to main/master)
```
PR Merge to main → main.yml → [Conventional Commits] → ci.yml → release-please.yml → Status Report
```

### Direct Push Flow (main/master)
```
Direct Push to main → main.yml → [Conventional Commits] → ci.yml → Status Report (Release Skipped)
```

### Manual Release Flow
```
Manual Dispatch → main.yml → [Conventional Commits] → ci.yml → release-please.yml → Status Report
```

## 🎛️ Control Logic

### When CI Runs
- ✅ All pushes to any tracked branch
- ✅ All pull requests
- ✅ Manual dispatch (unless skip_tests=true)

### When Release Runs
- ✅ PR merge to main/master branch (after CI passes)
- ✅ Manual dispatch with force_release=true
- ❌ Direct pushes to main/master
- ❌ Pull requests
- ❌ Development branches
- ❌ CI failures

## 🚨 Failure Handling

### Automatic Issue Creation
When releases fail on main/master, the pipeline automatically creates GitHub issues with:
- Failure details and logs
- Commit information
- Next steps for resolution
- High-priority labels

### Status Reporting
Every pipeline run produces a comprehensive status report showing:
- Branch and event context
- Individual job results
- Overall pipeline status
- Failure reasons

## ⚡ Concurrency Control

### Automatic Cancellation Strategy
All workflows implement intelligent concurrency control to optimize resource usage:

- **Development Branches**: Previous builds are automatically cancelled when new pushes occur
- **Pull Requests**: Concurrent builds for the same PR are cancelled in favor of the latest
- **Main/Master**: Release builds are **NOT** cancelled to prevent incomplete releases
- **Manual Dispatch**: Can override concurrency for emergency situations

### Concurrency Groups
```yaml
# Standard workflows (CI, conventional-commits, main controller)
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

# Release workflow (protected main/master)
concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: ${{ github.ref != 'refs/heads/main' && github.ref != 'refs/heads/master' }}
```

### Benefits
- **🚀 Faster feedback**: No waiting for outdated builds
- **💰 Cost optimization**: Reduces unnecessary compute usage
- **🔧 Developer experience**: Latest changes get priority
- **🛡️ Release safety**: Main/master builds complete fully

## 🔧 Manual Controls

### Emergency Release
```bash
# Force release on any branch (use with caution)
gh workflow run main.yml -f force_release=true
```

### Skip Tests
```bash
# Skip CI tests for emergency releases
gh workflow run main.yml -f skip_tests=true -f force_release=true
```

### Individual Workflow Testing
```bash
# Test individual workflows (conventional commits now embedded in main.yml)
gh workflow run main.yml  # Includes conventional commits validation
gh workflow run ci.yml
gh workflow run release-please.yml
```

## 📊 Benefits

### Single Source of Truth
- All CI/CD logic centralized in main.yml
- Conventional commits validation embedded (no separate workflow)
- No duplicate workflow definitions
- Consistent execution patterns

### Intelligent Execution
- Conditional logic prevents unnecessary runs
- PR-only release control prevents accidental releases
- Resource optimization through smart concurrency
- Clear execution paths

### No Concurrency Conflicts
- Embedded validation eliminates workflow deadlocks
- Single workflow controls all execution
- No competing concurrency groups

### Comprehensive Reporting
- Full pipeline visibility
- Automatic failure notifications
- Status tracking
- Clear release skip messaging

### Emergency Capabilities
- Manual override options
- Skip mechanisms for urgent fixes
- Force release for emergency situations
- Flexible execution control

## 🛠️ Maintenance

### Adding New Workflows
1. Create workflow file with `workflow_call` trigger
2. Add call to main.yml controller
3. Update dependencies as needed
4. Test with manual dispatch

### Modifying Execution Logic
1. Update controller job in main.yml
2. Adjust conditional statements
3. Test with different branch scenarios
4. Update documentation

### Troubleshooting
1. Check main.yml controller logs first
2. Review individual workflow results
3. Check GitHub Issues for automatic failure reports
4. Use manual dispatch for testing

---

*This architecture follows the principle: "Duplication is the mother of fragility" - one controller, many specialized workers.* 