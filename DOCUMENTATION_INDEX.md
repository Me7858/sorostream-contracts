# SoroStream Documentation Index

Complete guide to all SoroStream documentation for developers and integrators.

## 📚 Official Documentation

### Getting Started
- **[QUICK_START_GUIDE.md](./QUICK_START_GUIDE.md)** - 5-minute introduction with examples
  - Installation
  - Create your first stream
  - Basic operations (withdraw, cancel, top-up)
  - Common use cases (salary, vesting, milestones)
  - Batch operations

### API Reference
- **[COMPREHENSIVE_API_REFERENCE.md](./COMPREHENSIVE_API_REFERENCE.md)** - Complete function reference
  - Core concepts (stroops, flow rates, stream IDs)
  - All 50+ error codes with explanations
  - Every public function signature, parameters, returns, errors
  - Data type specifications
  - Real-world examples for each feature
  - Best practices and patterns

### Parameter Validation
- **[PARAMETER_VALIDATION_GUIDE.md](./PARAMETER_VALIDATION_GUIDE.md)** - Constraints and edge cases
  - Each parameter explained with constraints
  - Common pitfalls and how to avoid them
  - Timestamp handling
  - Fee calculations
  - Validation rules for all stream types

## 🏗️ Architecture & Design

- **[ARCHITECTURE.md](./ARCHITECTURE.md)** - System design and state machine
  - Stream lifecycle state machine
  - Contract layers and responsibilities
  - Data persistence strategies
  - On-chain storage optimization

- **[STORAGE.md](./docs/STORAGE.md)** - Storage schema and persistence
  - Stream record structure
  - Index organization
  - TTL management
  - Storage optimization

## 🐛 Known Issues & Fixes

- **[ROUNDING_DUST_BUG_SUMMARY.md](./ROUNDING_DUST_BUG_SUMMARY.md)** - Fixed rounding discrepancy bug
  - Issue description and root cause
  - Fix implementation
  - Impact assessment
  - Examples

- **[PAUSE_STREAM_EVENT_FIX_SUMMARY.md](./PAUSE_STREAM_EVENT_FIX_SUMMARY.md)** - Fixed event emission bug
  - Event logging issue with multiple streams
  - Root cause analysis
  - Applied fix

## 🔧 Advanced Features

- **[AUTO_RENEW_RENEW_COUNT_USAGE.md](./AUTO_RENEW_RENEW_COUNT_USAGE.md)** - Auto-renewal with limits
  - Renew count parameter explanation
  - Limited vs unlimited renewals
  - Examples and use cases
  - Migration guide

- **[ADMIN_DOCS_README.md](./ADMIN_DOCS_README.md)** - Administrative functions
  - Admin override system
  - Rate limiting
  - Whitelisting and blocklisting
  - Fee management
  - Emergency controls

## 📋 Examples

### Real-World Scenarios
1. **Salary Payments** - Monthly recurring streams with auto-renewal
2. **Vesting Schedules** - Multi-year vesting with cliffs and linear unlocking
3. **Milestone-Based Grants** - Discrete token releases at specific milestones
4. **Batch Payroll** - Creating and managing streams for multiple employees
5. **Subscription Services** - Monthly/yearly recurring revenue streams
6. **Time-Decay Vesting** - Front-weighted token release curves

## 🔍 Troubleshooting

### Error Resolution
- **StreamNotFound** - Check stream ID is correct
- **NotRecipient** - Verify caller is the recipient
- **ZeroFlowRate** - Increase amount or decrease duration
- **DuplicateStream** - Use unique nonce for sender+recipient combo
- **RateLimitExceeded** - Wait for rate limit window to expire
- **RecipientNotWhitelisted** - Recipient must be added to allowlist

### Common Issues
- **Unable to withdraw** - Check if stream is Active, not Paused/Completed
- **Withdrawal less than expected** - May be subject to dust threshold (≤1 stroop)
- **Stream ending unexpectedly** - Check auto_renew_count hasn't been reached
- **Can't change recipient** - Stream marked non_transferable at creation
- **Storage TTL warning** - Use bump_stream_ttl() to extend

## 📊 Feature Matrix

| Feature | Status | Doc Reference |
|---------|--------|---------------|
| Linear Streams | ✅ Core | QUICK_START_GUIDE |
| Auto-Renewal | ✅ Core | AUTO_RENEW_RENEW_COUNT_USAGE |
| Vesting with Cliff | ✅ Core | COMPREHENSIVE_API_REFERENCE |
| Step-Vesting (Tranches) | ✅ Core | COMPREHENSIVE_API_REFERENCE |
| Milestone-Gating | ✅ Core | COMPREHENSIVE_API_REFERENCE |
| Time-Decay Curves | ✅ Advanced | COMPREHENSIVE_API_REFERENCE |
| Batch Operations | ✅ Advanced | QUICK_START_GUIDE |
| Stream Pause/Resume | ✅ Advanced | COMPREHENSIVE_API_REFERENCE |
| Recipient Redirect | ✅ Advanced | COMPREHENSIVE_API_REFERENCE |
| Dual-Token Streams | ✅ Advanced | COMPREHENSIVE_API_REFERENCE |
| Rate Limiting | ✅ Admin | ADMIN_DOCS_README |
| Whitelisting | ✅ Admin | COMPREHENSIVE_API_REFERENCE |
| Admin Override | ✅ Admin | ADMIN_DOCS_README |
| Delegation | ✅ Advanced | COMPREHENSIVE_API_REFERENCE |
| Non-Transferable | ✅ Security | COMPREHENSIVE_API_REFERENCE |
| Recipient Approval | ✅ Security | COMPREHENSIVE_API_REFERENCE |
| Sender Lock | ✅ Security | COMPREHENSIVE_API_REFERENCE |
| Stream Health Monitoring | ✅ Operations | COMPREHENSIVE_API_REFERENCE |

## 🚀 Quick Links

### For Developers Integrating SoroStream
1. Start with [QUICK_START_GUIDE.md](./QUICK_START_GUIDE.md)
2. Reference [COMPREHENSIVE_API_REFERENCE.md](./COMPREHENSIVE_API_REFERENCE.md) for each function
3. Check [PARAMETER_VALIDATION_GUIDE.md](./PARAMETER_VALIDATION_GUIDE.md) before creating streams
4. Use examples from [COMPREHENSIVE_API_REFERENCE.md](./COMPREHENSIVE_API_REFERENCE.md)

### For Contributors to SoroStream
1. Read [ARCHITECTURE.md](./ARCHITECTURE.md) for system design
2. Study [STORAGE.md](./docs/STORAGE.md) for data persistence
3. Review known fixes: [ROUNDING_DUST_BUG_SUMMARY.md](./ROUNDING_DUST_BUG_SUMMARY.md), [PAUSE_STREAM_EVENT_FIX_SUMMARY.md](./PAUSE_STREAM_EVENT_FIX_SUMMARY.md)
4. See [CONTRIBUTING.md](./CONTRIBUTING.md) for contribution workflow

### For Contract Administrators
1. [ADMIN_DOCS_README.md](./ADMIN_DOCS_README.md) - Full admin reference
2. [PARAMETER_VALIDATION_GUIDE.md](./PARAMETER_VALIDATION_GUIDE.md) - Constraint enforcement
3. [COMPREHENSIVE_API_REFERENCE.md](./COMPREHENSIVE_API_REFERENCE.md) - Admin-only functions

## 📞 Support Resources

| Resource | Purpose |
|----------|---------|
| [GitHub Issues](https://github.com/SoroStream/sorostream-contracts/issues) | Bug reports, feature requests |
| [CONTRIBUTING.md](./CONTRIBUTING.md) | Contribution guidelines |
| [Stellar Developer Docs](https://developers.stellar.org/) | Soroban SDK reference |
| This Index | Documentation navigation |

## 🎯 Learning Paths

### Path 1: Quick Start (2 hours)
1. [QUICK_START_GUIDE.md](./QUICK_START_GUIDE.md) - 30 min
2. Run examples - 30 min
3. Try batch operations - 30 min
4. Review [PARAMETER_VALIDATION_GUIDE.md](./PARAMETER_VALIDATION_GUIDE.md) (skim) - 30 min

### Path 2: Comprehensive Integration (8 hours)
1. [QUICK_START_GUIDE.md](./QUICK_START_GUIDE.md) - 1 hour
2. [COMPREHENSIVE_API_REFERENCE.md](./COMPREHENSIVE_API_REFERENCE.md) - 3 hours
3. [PARAMETER_VALIDATION_GUIDE.md](./PARAMETER_VALIDATION_GUIDE.md) - 2 hours
4. Advanced features ([AUTO_RENEW_RENEW_COUNT_USAGE.md](./AUTO_RENEW_RENEW_COUNT_USAGE.md), redirects, etc.) - 2 hours

### Path 3: Deep Dive (Full Day)
1. [ARCHITECTURE.md](./ARCHITECTURE.md) - 1 hour
2. [STORAGE.md](./docs/STORAGE.md) - 1 hour
3. [COMPREHENSIVE_API_REFERENCE.md](./COMPREHENSIVE_API_REFERENCE.md) - 3 hours
4. Bug fixes and known issues - 1 hour
5. Admin functions and security - 1 hour
6. Hands-on implementation - 2 hours

## 📝 Document Descriptions

### QUICK_START_GUIDE.md
**Purpose:** Get developers up to speed in 5 minutes  
**Audience:** New integrators, quick reference  
**Content:** Installation, first stream, common patterns, batch ops  
**Length:** ~280 lines  

### COMPREHENSIVE_API_REFERENCE.md
**Purpose:** Complete function reference for all public endpoints  
**Audience:** API users, integrators needing detailed specs  
**Content:** 50+ functions, data types, error codes, examples  
**Length:** ~1,450 lines  

### PARAMETER_VALIDATION_GUIDE.md
**Purpose:** Explain parameter constraints and edge cases  
**Audience:** Stream creators wanting to avoid errors  
**Content:** Each parameter explained, constraints, common pitfalls  
**Length:** ~570 lines  

### ARCHITECTURE.md
**Purpose:** System design and high-level overview  
**Audience:** Contributors, contract auditors  
**Content:** State machine, design patterns, layer structure  
**Length:** ~500 lines  

### AUTO_RENEW_RENEW_COUNT_USAGE.md
**Purpose:** Auto-renewal feature usage guide  
**Audience:** Users implementing recurring streams  
**Content:** Renew count mechanics, examples, migration  
**Length:** ~260 lines  

### ROUNDING_DUST_BUG_SUMMARY.md
**Purpose:** Document and fix rounding discrepancy issue  
**Audience:** Contributors, auditors  
**Content:** Bug analysis, fix implementation, test cases  
**Length:** ~180 lines  

### PAUSE_STREAM_EVENT_FIX_SUMMARY.md
**Purpose:** Document pause/resume event fix  
**Audience:** Contributors, indexer maintainers  
**Content:** Event bug, fix, impact  
**Length:** ~130 lines  

## 🔗 Cross-References

### Stream Creation
- See [QUICK_START_GUIDE.md](./QUICK_START_GUIDE.md) for basic examples
- See [COMPREHENSIVE_API_REFERENCE.md](./COMPREHENSIVE_API_REFERENCE.md) for all variants
- See [PARAMETER_VALIDATION_GUIDE.md](./PARAMETER_VALIDATION_GUIDE.md) for constraints

### Withdrawals
- See [QUICK_START_GUIDE.md](./QUICK_START_GUIDE.md) for basic pattern
- See [COMPREHENSIVE_API_REFERENCE.md](./COMPREHENSIVE_API_REFERENCE.md) for edge cases
- See [ROUNDING_DUST_BUG_SUMMARY.md](./ROUNDING_DUST_BUG_SUMMARY.md) for rounding info

### Error Handling
- See [COMPREHENSIVE_API_REFERENCE.md](./COMPREHENSIVE_API_REFERENCE.md) for all error codes
- See [QUICK_START_GUIDE.md](./QUICK_START_GUIDE.md) for error patterns
- See [PARAMETER_VALIDATION_GUIDE.md](./PARAMETER_VALIDATION_GUIDE.md) for validation errors

### Admin Functions
- See [ADMIN_DOCS_README.md](./ADMIN_DOCS_README.md) for admin reference
- See [COMPREHENSIVE_API_REFERENCE.md](./COMPREHENSIVE_API_REFERENCE.md) for signatures

## 📦 Documentation Organization

```
/workspaces/sorostream-contracts/
├── QUICK_START_GUIDE.md                    ← Start here
├── COMPREHENSIVE_API_REFERENCE.md          ← Complete reference
├── PARAMETER_VALIDATION_GUIDE.md           ← Constraints & validation
├── DOCUMENTATION_INDEX.md                  ← This file
├── ARCHITECTURE.md                         ← System design
├── AUTO_RENEW_RENEW_COUNT_USAGE.md         ← Feature documentation
├── ROUNDING_DUST_BUG_SUMMARY.md            ← Known issue
├── PAUSE_STREAM_EVENT_FIX_SUMMARY.md       ← Known issue
├── ADMIN_DOCS_README.md                    ← Admin functions
├── CONTRIBUTING.md                         ← Contribution guide
├── docs/
│   └── STORAGE.md                          ← Storage schema
└── contracts/
    └── stream/
        └── src/
            ├── lib.rs                      ← Implementation
            ├── interface.rs                ← Public interface
            ├── types.rs                    ← Data structures
            └── ...
```

## ✅ Documentation Checklist

- [x] Quick start guide for new users
- [x] Comprehensive API reference with examples
- [x] Parameter validation guide with constraints
- [x] Error code documentation
- [x] Architecture documentation
- [x] Feature documentation (auto-renewal, dual streams, etc.)
- [x] Known issues documented and fixed
- [x] Admin function reference
- [x] Best practices guide
- [x] Troubleshooting guide
- [x] Cross-references between documents
- [x] Documentation index (this file)

## 🎓 Learning Resources

- **Stellar Developer Docs**: https://developers.stellar.org/
- **Soroban SDK Reference**: https://github.com/stellar/rs-soroban-sdk
- **Sorostream GitHub**: https://github.com/SoroStream/sorostream-contracts
- **Examples**: See examples/ directory in repository

---

**Last Updated:** August 2026  
**Version:** 1.0.0  
**Status:** Complete and maintained
