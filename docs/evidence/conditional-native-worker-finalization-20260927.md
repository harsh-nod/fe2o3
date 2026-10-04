# Conditional Native Worker and Finalization

Date: 2026-09-27. Follow-up to
[Cargo readiness custody](conditional-native-cargo-custody-20260927.md) for
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
Base: `99da9d22924390272d27da25a37af94b9fdd6510`.

**Component integration only. No complete M0-M7 milestone, production version
switch, protected proof run, or additional end-to-end GPU kernel is claimed.**

## Implemented Boundary

The existing reproducible worker engine now accepts the actual recovered
conditional V5 source family through a typed adapter. Preflight takes a locked
V5 token containing `RecoveredCompilerConditionalNativeSemanticHandoffV5`,
checks currentness before and after staging, and binds the complete occurrence,
compiler closure, actual final graph, contract catalog and target profile.
Execution requires that same consumed occurrence and pinned worker. Its result
retains the original conditional source allocation and both complete worker
exchanges; it cannot substitute an ordinary V4 source or a legacy V3 projection.

Native V4 and conditional V5 staging/execution share the same resource quote,
module decoder, request construction, candidate/replay process execution,
transcript validation and evidence hashing. Existing V3/V4 identity domains are
unchanged; conditional binding/request/evidence domains are distinct. The
original source/preflight reservations stay paid and returned storage is
additional and unreserved, following the existing native accounting contract.

The conditional finalizer retains that worker owner, uses the existing lineage
and physical-ABI inspectors, requires the exact source-carried V5 descriptor,
and calls the existing nominal V5 artifact finalizer. Mandatory CPU invocation
contracts are not converted into an older ABI receipt. Finalization commits
the exact source, worker, inspection and finalized bytes under a distinct
domain. It grants no protected origin, machine refinement or launch authority.

Descriptor traversal and final hashing use the caller's resource account.
Worker-wire decoding, artifact inspection and artifact payloads retain their
existing separately bounded accounting domain. This is not an aggregate
artifact-work, allocator-capacity or RSS guarantee.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time, with compiled inputs frozen during builds:

| Command / Target | Result |
| --- | --- |
| `cargo test -p fe2o3-hsaco-finalize --lib` | 188 passed; none failed or ignored |
| `cargo test -p fe2o3-hsaco-finalize --doc` | 60 passed, including six new compile-fail examples |
| Finalizer `reproducible_first_build_worker_v3` | 12 passed; one real LLVM-worker test explicitly ignored |
| Verifier `export_native_first_build_worker_v4_fixtures -- --ignored` | 1 passed; all four private fixture files exported |
| Finalizer `native_first_build_worker -- --ignored` | 5 passed; none failed or ignored |
| Finalizer `native_worker_finalization_tests -- --ignored` | 18 passed; none failed or ignored |
| `cargo check -p fe2o3-hsaco-finalize -p cargo-fe2o3 -p rustc-codegen-fe2o3 --all-targets` | Passed; warnings remain |

The four new artifact-component tests cover gfx942/gfx950 V5 bytes, digest-only
patching, coherently resealed CPU-contract substitution, schema downgrade,
exact/one-short resource limits, original-ledger preservation and typed resource
errors. The new compile-fail examples reject duplication, default construction,
ordinary-source conversion and bypass of recovered-token typing.

The opt-in regressions exercise real public V4 recovery, transaction consumption,
measured fixture processes, finalization and restart rejection. Their keys,
invocations, worker derivations and ELF payloads are synthetic test inputs. V4
here names the ordinary source transport, not successful conditional V5 source
admission. These runs grant no protected proof, LLVM refinement or GPU credit.
No positive complete conditional V5 source-to-worker run was executed.

The first all-target command used an incorrect underscore package name and
failed package selection; the corrected hyphenated command above passed.
Private exported fixtures and the empty scratch directory were removed after
all test sessions terminated. No remote jobs or files were created.

Logs in `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
7c345282c823f8592041f2fa040fa747ce177520bb84a93471d2deee95cfa135  conditional-worker-lib-tests-r2-20260927.log
764ed9d042a0adf05be82f7b39c12230eba5a60930f2ccc9c89ed13d496de912  conditional-worker-doctests-20260927.log
5eedef673e07ae3333c0474e6561b5d00983a1192cea8c299df268b0e9a333a1  conditional-worker-all-targets-r2-20260927.log
dc7017571b2d0040d5136ee0a780c57f4e9d71c5807ec598c4213c0346713805  conditional-worker-engine-regression-20260927.log
5a0e3e8875c72cb6ea608aa8e7dc8e7debc5f3def372749f9d5d7e85ab323aa4  conditional-worker-v4-export-20260927.log
8526dca5c7d362e5086f4ef720e2459c1df21a0ea860dc8b1e67eed3afcd5d17  conditional-worker-native-regression-20260927.log
e725a90349e7ebfd4d3a19e7a7b14cd4080fbe0d51377bf550d8d1d87b8b1d70  conditional-worker-finalizer-regression-20260927.log
```

## Remaining Integration

Cargo still selects the older production handoff. Its native readiness owner
cannot yet drive this entire continuation. The next integration must retain
independently admitted parent invocation/configuration custody and use the
original account to map the locked raw V5 token through conditional recovery,
preflight before consumption, execute, and retain the native execution receipt
through finalization and sealed authority admission. Recovery's terminal error
and panic reservations/lock must not be enclosed in a blanket-refund scope.

The compiler's bounded live invocation admission, coherent broker/deployment
selection, conditional artifact publication/recovery and safe generated host
consumer also remain gates. The existing legacy-worker nominal V5 finalizer is
not a substitute for this actual conditional source family. Protected vecadd,
advanced-kernel mutation matrices, machine/numerical refinement, target-matched
47/47 GPU runs, legacy retirement, release CI and identical public repository
heads have not been established. No manifest entry changes production status.

Native agent creation failed at the service thread limit, so the primary agent
implemented and reviewed this checkpoint; no Qwen worker was invoked. All three
SSH aliases and the origin Git fetch failed DNS resolution. Subsequent push and
issue-update results must be reported separately, not inferred from local tests.
