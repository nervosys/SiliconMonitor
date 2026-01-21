# Security Quick Reference - simon

## 🚨 Critical Vulnerabilities (Fix Immediately)

### 1. Command Injection in swap.rs (CVSS 9.8)
**File:** `src/utils/swap.rs:203`  
**Risk:** Remote Code Execution via path injection  
**Fix:** Use file API instead of shell command

### 2. Path Traversal in swap.rs (CVSS 8.1)
**File:** `src/utils/swap.rs:89`  
**Risk:** Arbitrary file creation/overwrite  
**Fix:** Implement path whitelisting and canonicalization

### 3. Unchecked Sudo Usage (CVSS 7.2)
**Files:** `utils/clocks.rs`, `utils/swap.rs`, `utils/power_mode.rs`  
**Risk:** Privilege escalation without validation  
**Fix:** Verify sudo availability and permissions

## 📊 Security Scorecard

| Category       | Score           | Status       |
| -------------- | --------------- | ------------ |
| **Overall**    | **C- (58/100)** | ⚠️ Needs Work |
| Memory Safety  | A+ (100/100)    | ✅ Excellent  |
| Dependencies   | A (95/100)      | ✅ Good       |
| Monitoring     | A (90/100)      | ✅ Safe       |
| **Utilities**  | **F (15/100)**  | ❌ Critical   |
| Access Control | F (0/100)       | ❌ Missing    |
| Audit Logging  | F (0/100)       | ❌ Missing    |

## 🎯 Deployment Decision Matrix

| Use Case                  | Recommendation | Notes                                    |
| ------------------------- | -------------- | ---------------------------------------- |
| **Read-only monitoring**  | ✅ **APPROVED** | CPU/GPU/memory/temp monitoring is safe   |
| **System utilities**      | ❌ **BLOCKED**  | Command injection + path traversal risks |
| **CMMC 2.0 environments** | ❌ **BLOCKED**  | Major compliance gaps                    |
| **Development/Testing**   | ⚠️ **CAUTION**  | Okay with proper network isolation       |

## 🛡️ Safe to Use (Production Ready)

- ✅ `simon::new()` and `snapshot()` - Core monitoring
- ✅ CPU monitoring - Read-only sysfs
- ✅ GPU monitoring - Read-only sysfs/NVML
- ✅ Memory stats - Read-only /proc
- ✅ Temperature - Read-only thermal zones
- ✅ Process monitoring - Read-only /proc
- ✅ Engine stats - Read-only sysfs
- ✅ Platform detection - Read-only system info

## ❌ Unsafe for Production (Security Risks)

- ❌ `utils::swap::create()` - Command injection + path traversal
- ❌ `utils::swap::remove()` - Arbitrary file deletion
- ❌ `utils::clocks::enable()` - Unchecked sudo
- ❌ `utils::clocks::disable()` - Unchecked sudo
- ❌ `utils::power_mode::set_mode()` - Unchecked sudo
- ❌ Any function calling `std::process::Command` with user input

## 🔧 Quick Mitigation (Temporary)

### Option 1: Feature Flag (Recommended)
```toml
# Cargo.toml
[features]
default = ["monitoring"]
monitoring = []  # Safe read-only features
utilities = []   # Unsafe write operations
unsafe-utils = ["utilities"]  # Explicit opt-in
```

### Option 2: Runtime Check
```rust
// Add to each utility function
fn require_explicit_consent() -> Result<()> {
    if std::env::var("simon_ALLOW_UNSAFE").is_err() {
        return Err(simonError::PermissionDenied(
            "Utility functions disabled. Set simon_ALLOW_UNSAFE=1 to enable (NOT RECOMMENDED)".into()
        ));
    }
    eprintln!("⚠️  WARNING: Using unsafe utility functions. Proceed with caution.");
    Ok(())
}
```

## 📋 Priority Fix List

### Week 1 (Critical)
- [ ] Fix command injection in `swap.rs:203`
- [ ] Implement path validation in `swap.rs:89`
- [ ] Add sudo verification before all privileged ops

### Week 2 (High)
- [ ] Replace all 15 `.unwrap()` calls
- [ ] Add input validation (size limits, path whitelisting)
- [ ] Implement audit logging

### Week 3 (Medium)
- [ ] Add access control checks
- [ ] Implement timeout for external commands
- [ ] Add rate limiting

### Month 1 (CMMC Compliance)
- [ ] Implement RBAC
- [ ] Add structured audit logs
- [ ] Create security documentation
- [ ] Establish baseline configs

## 🔍 CVE Status

**Last Scanned:** October 28, 2025  
**Tool:** cargo-audit v0.21.2  
**Database:** RustSec (861 advisories)

**Results:**
- ✅ 0 Critical vulnerabilities
- ✅ 0 High severity
- ✅ 0 Medium severity
- ✅ 0 Low severity
- ⚠️ 1 Warning (unmaintained `paste` via `ratatui`)

**Next Scan:** Run `cargo audit` before each release

## 📞 Security Contact

For security issues, DO NOT open public issues.

**Report vulnerabilities to:**
- Email: security@nervosys.dev (if applicable)
- GitHub Security Advisory (private)

## 📚 Related Documents

- Full audit: `SECURITY_AUDIT.md`
- CVE tracking: `cargo audit`
- MITRE ATT&CK mapping: See SECURITY_AUDIT.md §2
- CMMC 2.0 compliance: See SECURITY_AUDIT.md §3

