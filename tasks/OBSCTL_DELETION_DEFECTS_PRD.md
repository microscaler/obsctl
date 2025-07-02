# obsctl Deletion Defects Fix - Product Requirements Document

**Document Version:** 1.0  
**Date:** January 2025  
**Priority:** Critical Release Blocker  
**Team:** obsctl Core Development  
**Estimated Timeline:** 5-7 days  

## Executive Summary

During "eat our own dog food" testing, we discovered **multiple critical defects** in obsctl's core deletion functionality that prevent reliable storage management. These defects block the fundamental promise of obsctl as a storage management tool and must be fixed before any release.

**Key Finding:** obsctl cannot reliably clear storage using its own commands, violating the "eat our own dog food" principle and requiring users to resort to external tools.

# Key mandatory rule:

Under no circumstance may you delete the docker volume to solve the problem, this will mean we have a significant defect that prevents us shipping.
```
docker compose down minio && docker volume rm obsctl_minio_data 2>/dev/null || true
```

## Problem Statement

### Critical Defects Discovered

#### 1. **CRITICAL: MissingContentMD5 Error in Batch Deletion**
- **Symptom:** `MissingContentMD5: Missing required header for this request: Content-Md5`
- **Impact:** Recursive deletion fails partway through operations
- **Root Cause:** AWS SDK batch deletion requires checksum algorithm for MinIO compatibility
- **Evidence:** Fails on `obsctl rm s3://bucket/prefix --recursive` with 50+ objects

#### 2. **CRITICAL: Phantom Deletion Success**
- **Symptom:** Shows "delete: s3://bucket/object" messages but objects remain in storage
- **Impact:** False positive deletion results create data integrity concerns
- **Evidence:** 69 objects "deleted" but all 75 objects still present after operation

#### 3. **HIGH: Wildcard Pattern Matching Failure**
- **Symptom:** `obsctl rm 's3://bucket/*' --recursive` deletes 0 objects
- **Impact:** Users cannot use intuitive wildcard patterns
- **Evidence:** Pattern matching completely broken across all test scenarios

#### 4. **HIGH: Bucket vs Object Deletion Logic Confusion**
- **Symptom:** `obsctl rb s3://bucket --force` fails with service errors
- **Impact:** Bucket deletion operations fail even with force flag
- **Evidence:** Same MissingContentMD5 errors in bucket deletion path

#### 5. **MEDIUM: Inconsistent Error Reporting**
- **Symptom:** Mixed success/failure messages with unclear final state
- **Impact:** Users cannot trust operation results
- **Evidence:** Operations report partial success but leave storage unchanged

## Business Impact

### Current State
- **obsctl CANNOT reliably clear storage** using its own commands
- **Users must resort to external tools** (docker, mc, etc.) for basic operations
- **"Eat our own dog food" principle completely violated**
- **Core storage management functionality fails**

### Risk Assessment
- **Release Blocker:** Cannot ship with broken core functionality
- **User Trust:** Phantom success undermines confidence in all operations
- **Competitive Position:** Basic storage management is table stakes

## Technical Analysis

### Root Cause Investigation

Based on research and testing, the primary issue is **AWS SDK Rust compatibility with MinIO**:

1. **Checksum Algorithm Requirement:** MinIO requires `checksum_algorithm` parameter for batch deletion operations
2. **SDK Default Changes:** Recent AWS SDK versions enforce integrity checksums by default
3. **S3-Compatible Service Breakage:** This affects many S3-compatible services (Cloudflare R2, Tigris, MinIO)

### Evidence from Research
- **AWS SDK Documentation:** `checksum_algorithm` field supports CRC32, CRC32C, CRC64NVME, SHA1, SHA256
- **Industry Impact:** Apache Iceberg, Trino, and other projects experiencing same issues
- **MinIO Documentation:** Confirms batch deletion requires Content-MD5 or checksum headers

## Solution Requirements

### Must Have (P0)
1. **Fix Batch Deletion MissingContentMD5 Error**
   - Add `checksum_algorithm` parameter to `delete_objects()` calls
   - Support multiple checksum algorithms (CRC32, SHA256, etc.)
   - Maintain backward compatibility with AWS S3

2. **Fix Phantom Deletion Success**
   - Ensure deletion operations actually remove objects
   - Add verification step after batch operations
   - Provide accurate success/failure reporting

3. **Fix Wildcard Pattern Matching**
   - Implement proper glob pattern parsing
   - Support standard shell wildcards (*, ?, [])
   - Test against various pattern scenarios

4. **Fix Bucket Deletion Logic**
   - Separate bucket vs object deletion code paths
   - Handle empty bucket verification properly
   - Implement proper force deletion sequence

### Should Have (P1)
1. **Enhanced Error Handling**
   - Distinguish between different failure types
   - Provide actionable error messages
   - Implement retry logic for transient failures

2. **Operation Verification**
   - Add post-operation verification steps
   - Implement object count validation
   - Provide detailed operation summaries

3. **Compatibility Testing**
   - Test against multiple S3-compatible services
   - Validate checksum algorithm support
   - Ensure AWS S3 compatibility maintained

### Could Have (P2)
1. **Performance Optimization**
   - Implement batch size optimization
   - Add progress reporting for large operations
   - Optimize for different storage backends

2. **Configuration Options**
   - Allow checksum algorithm selection
   - Provide compatibility mode settings
   - Enable verbose operation logging

## Technical Implementation Plan

### Phase 1: Core Fixes (Days 1-3)

#### 1.1 Fix MissingContentMD5 Error
**File:** `src/commands/rm.rs` lines 207-217

**Current Code:**
```rust
config
    .client
    .delete_objects()
    .bucket(&s3_uri.bucket)
    .delete(delete_request)
    .send()
    .await?;
```

**Fixed Code:**
```rust
config
    .client
    .delete_objects()
    .bucket(&s3_uri.bucket)
    .delete(delete_request)
    .checksum_algorithm(aws_sdk_s3::types::ChecksumAlgorithm::Sha256)
    .send()
    .await?;
```

#### 1.2 Add Checksum Algorithm Configuration
**New:** Add checksum algorithm selection to Config struct
**Location:** `src/config.rs`

#### 1.3 Fix Pattern Matching Logic
**File:** `src/commands/rm.rs` recursive deletion logic
**Action:** Implement proper prefix handling and glob pattern support

### Phase 2: Verification & Testing (Days 4-5)

#### 2.1 Add Operation Verification
- Implement post-deletion object count checks
- Add verification for successful operations
- Ensure accurate reporting

#### 2.2 Comprehensive Testing
- Test against MinIO, AWS S3, and other S3-compatible services
- Validate all checksum algorithms
- Test various deletion patterns and scenarios

### Phase 3: Integration & Documentation (Days 6-7)

#### 3.1 Integration Testing
- Run full test suite against live MinIO instance
- Validate "eat our own dog food" scenarios
- Test traffic generator cleanup operations

#### 3.2 Documentation Updates
- Update CLI help text with new options
- Document checksum algorithm choices
- Add troubleshooting guide for S3-compatible services

## Testing Strategy

### Test Scenarios
1. **Batch Deletion Test:** Delete 100+ objects recursively
2. **Pattern Matching Test:** Use wildcards and verify correct object selection
3. **Bucket Deletion Test:** Delete non-empty bucket with force flag
4. **Cross-Service Test:** Validate against MinIO, AWS S3, and other services
5. **Phantom Deletion Test:** Verify objects are actually removed

### Test Environment
- Use existing MinIO docker-compose setup
- Generate test data with traffic generator
- Implement automated test verification

### Success Criteria
- ✅ All deletion operations complete successfully
- ✅ Objects are actually removed from storage
- ✅ Pattern matching works as expected
- ✅ Error messages are clear and actionable
- ✅ Operations work across S3-compatible services

## Implementation Constraints

### Technical Constraints
- **Maintain AWS S3 Compatibility:** Cannot break existing AWS S3 users
- **Backward Compatibility:** Existing commands must continue working
- **Performance:** No significant performance degradation
- **Dependencies:** Minimal new dependencies

### Forbidden Actions
**CRITICAL:** Under **NO circumstances** may the implementation include:
```bash
docker compose down minio && docker volume rm obsctl_minio_data
```
This destroys the test environment and prevents proper debugging/testing.

### Testing Environment Preservation
- **Maintain MinIO instance** throughout development
- **Preserve test data** for validation
- **Use obsctl commands only** for storage operations
- **Document all test scenarios** for reproducibility

## Risk Mitigation

### Technical Risks
1. **AWS SDK Version Compatibility**
   - **Risk:** Different SDK versions may behave differently
   - **Mitigation:** Pin specific AWS SDK version, test across versions

2. **S3-Compatible Service Variations**
   - **Risk:** Different services may have different requirements
   - **Mitigation:** Implement configurable checksum algorithms

3. **Performance Impact**
   - **Risk:** Additional checksum computation may slow operations
   - **Mitigation:** Benchmark performance, optimize if needed

### Project Risks
1. **Timeline Pressure**
   - **Risk:** Fixes may be rushed and introduce new bugs
   - **Mitigation:** Implement comprehensive testing at each phase

2. **Scope Creep**
   - **Risk:** Additional features may delay critical fixes
   - **Mitigation:** Focus strictly on P0 requirements first

## Success Metrics

### Primary Metrics
- **Deletion Success Rate:** 100% for all test scenarios
- **Operation Accuracy:** 0% phantom successes
- **Pattern Matching:** 100% correct object selection
- **Cross-Service Compatibility:** Works with MinIO, AWS S3, and 2+ other services

### Secondary Metrics
- **Error Clarity:** All error messages provide actionable guidance
- **Performance:** No more than 10% performance degradation
- **Test Coverage:** 100% coverage of deletion code paths

## Acceptance Criteria

### Definition of Done
- [ ] All P0 requirements implemented and tested
- [ ] Comprehensive test suite passes 100%
- [ ] "Eat our own dog food" scenario works end-to-end
- [ ] Documentation updated and reviewed
- [ ] Code review completed by 2+ team members
- [ ] Performance benchmarks meet requirements

### Release Readiness
- [ ] All critical defects resolved
- [ ] No regressions in existing functionality
- [ ] Cross-service compatibility validated
- [ ] User acceptance testing completed

## Conclusion

These deletion defects represent a fundamental failure in obsctl's core functionality. The fixes are well-understood, technically feasible, and critical for product credibility. With focused effort and proper testing, we can resolve these issues and restore confidence in obsctl's storage management capabilities.

The key insight is that this is primarily an **AWS SDK compatibility issue** with S3-compatible services, not a fundamental design flaw. The solution involves adding proper checksum algorithm support while maintaining backward compatibility.

**Next Steps:**
1. Begin Phase 1 implementation immediately
2. Set up continuous testing against MinIO
3. Coordinate with team for code review and testing
4. Plan release timeline after successful completion

---

**Document Owner:** obsctl Core Team  
**Review Required:** Architecture Team, QA Team  
**Approval Required:** Engineering Lead, Product Owner 