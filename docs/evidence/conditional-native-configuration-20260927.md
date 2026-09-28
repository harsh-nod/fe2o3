# Native Cargo Configuration Preparation

Date: 2026-09-27. Base: `908feabb77405ab2609fb1916e12577fa40a8168`.
Follow-up to [Cargo continuation](conditional-native-cargo-continuation-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).

**No complete M0-M7 milestone, production activation, protected execution or
additional end-to-end GPU kernel is claimed.**

## Changes

V1 and V2 configuration preparation now share one parser body, retaining their
existing exact schemas, identity domains and ordinary entry points. The native
entry prepays manifest reads, parsing/canonicalization and aggregate provider
reads/hashing on the caller's existing account. The shared bounded reader stops
at the initial file length plus one before checking the same inode's metadata;
file growth cannot extend that read to the larger global cap.

The resulting move-only recipe checks an independently supplied expected
configuration identity and records the original budget address, work ledger and
retained input floor. It cannot be constructed by converting an unmetered parsed
configuration. Transfer rejects another account, relocation or retired storage.
Its private configuration cannot be extracted to invoke the older continuation.
The conditional Cargo continuation now requires this recipe, moves its payloads
without cloning them and keeps the configuration reservation with the final
artifact/readiness owner.

All preparation reservations remain on error/unwind and success. This
conservative schedule deliberately retains temporary charges until the enclosing
attempt's owners are retired. Work measures logical byte/row visits, not CPU
instructions; storage measures logical buffers, not allocator capacity or RSS.
The schedule depends on pinned serde_json and nightly collection behavior.
Worker image capture, OS I/O retries and process supervision remain the existing
separate bounded domain. This is not an aggregate process-memory guarantee.

Review also found three unconditional arithmetic rejections in the earlier
unselected Cargo continuation: its prepaid scopes specified entry work 8 but
total work 0. The shared account-floor check now correctly includes entry work
in the total, and has a positive/exact/one-short regression. This fix does not
establish successful execution of the entire conditional continuation.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded
command at a time, compiled inputs frozen during each build:

| Command / Filter | Result |
| --- | --- |
| Cargo binary: `build_config::tests` | 15 passed, none ignored |
| Cargo binary: `compiler_execution_boundary::native` | 5 passed, none ignored |
| Cargo integration: `production_build_config` | 18 passed, 1 failed, none ignored |
| `cargo check -p cargo-fe2o3 -p rustc-codegen-fe2o3 -p fe2o3-hsaco-finalize --all-targets` | Passed; warnings remain |

New tests cover both schemas, exact/one-short work and storage, quota rejection
before the relevant I/O or JSON parsing, expected-identity/schema mismatch,
terminal reservations, original ledger/floor/address, recipe movement and quote
overflow. Portable fixtures use a measured `/bin/true` image for pinning only;
they do not execute an LLVM worker or establish protected provenance.

The integration failure is the existing
`production_runner_rejects_no_envelope_marker`: supervisor startup rejects socket
inspection before the expected missing-envelope diagnostic. A fresh independent
probe creates a socket pair successfully, then gets `EPERM` from
`getsockopt(SOL_SOCKET, SO_TYPE)`. Checks were not relaxed and this failure is not
counted as a pass. Rustfmt and whitespace checks passed. Every command terminated
and the private scratch directory was removed; no remote jobs/files were created.

Logs under `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
deb59a2aa0ca95ddbcf276ebcabe93132e9304ecb6e0714571770b3af6e2cea7  cargo-native-configuration-tests-r2-20260927.log
20eff4de23503fffdbdbc282a101ce05e3c7a9c3ef55cfc2887e9b31d40308e6  cargo-native-account-floor-tests-r1-20260927.log
e7612740f867f491de15050286e78440087a8c0bc3b65caeb260bf0ab10f2b4b  cargo-native-configuration-production-regression-r1-20260927.log
b2bcb22befc08f5859f41eab74c8cfccbe672130484fa00513b04d671c59aed2  cargo-native-configuration-check-r1-20260927.log
```

## Remaining Gates

The outer broker still must admit the schema/expected identity and invoke native
preparation before the selected child borrows the stable attempt account. Live
environment/argv/cwd admission and Command/spawn accounting remain separate work.
No outer production selector changed. Positive protected conditional recovery,
conditional artifact publication/recovery, sealed authority and generated safe
host consumption, machine/numerical refinement, the target-matched 47/47 matrix,
migration/removal and release CI remain incomplete.

Native agent creation failed at the service thread limit, so this checkpoint was
implemented and reviewed locally; no Qwen worker was invoked. All three SSH
aliases and the origin fetch failed DNS. Push and issue-update outcomes must be
reported separately from local validation.
