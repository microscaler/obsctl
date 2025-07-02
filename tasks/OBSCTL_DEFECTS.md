# obsctl Defect Tracking Report

**Date:** January 2025  
**Context:** MinIO storage cleanup using obsctl (eating our own dog food)  
**Goal:** Clear all MinIO storage using only obsctl commands  

## Critical Defects Discovered

### 1. **CRITICAL: MissingContentMD5 Error in Batch Deletion**
- **Command:** `obsctl rm s3://bucket/prefix --recursive`
- **Error:** `MissingContentMD5: Missing required header for this request: Content-Md5`
- **Impact:** Recursive deletion fails partway through, leaving storage partially cleaned
- **Status:** Blocks storage cleanup operations
- **Expected:** Recursive deletion should complete successfully
- **Actual:** Fails with MD5 header error during batch operations

### 2. **HIGH: Recursive Delete Pattern Matching Issues**
- **Command:** `obsctl rm 's3://bucket/*' --recursive`
- **Result:** "Successfully deleted 0 objects" but objects remain
- **Impact:** Wildcard patterns don't work as expected
- **Expected:** Should delete all objects matching pattern
- **Actual:** No objects deleted despite success message

### 3. **HIGH: Bucket vs Object Deletion Confusion**
- **Commands:**
  - `obsctl rm s3://bucket --recursive` → "To delete a bucket, use --force flag"
  - `obsctl rm s3://bucket/ --recursive` → "To delete a bucket, use --force flag"
- **Issue:** Unclear distinction between bucket deletion and object deletion
- **Expected:** Should delete objects within bucket, not bucket itself
- **Actual:** Treats any bucket-level URI as bucket deletion attempt

### 4. **MEDIUM: rb Command Service Errors**
- **Command:** `obsctl rb --all --force --confirm`
- **Error:** "service error" when deleting objects in buckets
- **Impact:** Bulk bucket deletion completely fails
- **Expected:** Should delete all buckets and their contents
- **Actual:** Fails to delete any buckets due to object deletion errors

## Working Functionality

### ✅ Individual Object Deletion
- **Command:** `obsctl rm s3://bucket/object`
- **Status:** Works correctly
- **Evidence:** Successfully deleted test.txt (76→75 objects)

### ✅ Prefix-Based Recursive Deletion (Partial)
- **Command:** `obsctl rm s3://bucket/prefix --recursive`
- **Status:** Works until batch size limit hit
- **Evidence:** Successfully deleted 69 objects before MD5 error

## Impact Assessment
- **Severity:** HIGH - Blocks core storage management functionality
- **User Experience:** Poor - Multiple failed attempts required
- **Production Readiness:** BLOCKED - Cannot reliably clear storage

### 6. **CRITICAL: Phantom Deletion Success**
- **Command:** `obsctl rm s3://alice-dev-workspace/alice-dev --recursive`
- **Behavior:** Shows "delete: s3://bucket/object" for 69 objects, then fails with MD5 error
- **Issue:** Objects appear to be deleted but are still present when listing bucket
- **Impact:** False positive deletion results - data not actually removed
- **Expected:** Objects should be permanently deleted when "delete:" message shown
- **Actual:** Objects remain in bucket despite success messages

### 7. **CRITICAL: Inconsistent State After Failed Batch Operations**
- **Evidence:** All 75 objects still present after "successful" deletion of 69 objects
- **Impact:** Users cannot trust deletion operation results
- **Severity:** Data integrity issue - operations appear successful but fail silently


## Final Status

### ✅ Storage Cleared Successfully
- **Method:** Docker volume deletion (workaround)
- **Command:** `docker compose down minio && docker volume rm obsctl_minio_data`
- **Result:** All MinIO storage completely cleared
- **Verification:** `obsctl ls` returns empty (no buckets)

### ❌ obsctl Deletion Functionality Assessment
- **Individual Object Deletion:** ✅ Works reliably
- **Batch/Recursive Deletion:** ❌ Multiple critical failures
- **Bucket Deletion:** ❌ Service errors prevent operation
- **Pattern Matching:** ❌ Wildcard patterns don't work

### Critical Production Impact
- **obsctl cannot reliably clear storage** using its own commands
- **Users must resort to external tools** (docker, mc, etc.)
- **"Eat our own dog food" principle violated** - obsctl fails at basic storage management
- **Data integrity concerns** - phantom deletion success creates false confidence

### Immediate Action Required
1. **Fix MissingContentMD5 error** in batch deletion operations
2. **Fix phantom deletion issue** - ensure delete operations actually remove objects
3. **Improve error handling** in rb command for bucket deletion
4. **Add integration tests** for storage cleanup scenarios
5. **Consider this a release blocker** until core deletion functionality works

### Workaround for Users
Until fixes are implemented, users should:
1. Use individual object deletion for small numbers of objects
2. Use external tools (mc, aws cli) for bulk operations
3. Restart MinIO/S3 service for complete cleanup when possible

**Priority:** CRITICAL - Core functionality failure
**Assigned:** Development team
**Target:** Next release cycle
