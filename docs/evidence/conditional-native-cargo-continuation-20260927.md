# Conditional Native Cargo Continuation

Date: 2026-09-27. Base: `d9a8d6f5dae299ee41abf67b5f5013ddff25155a`.
Follow-up to [worker/finalizer integration](conditional-native-worker-finalization-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).

**Compiled, unselected component integration. No complete M0-M7 milestone,
protected execution, production activation, or additional GPU kernel is claimed.**

## Integration

The existing prepared Cargo configuration can now move its worker recipe into
the native readiness owner's V5 continuation. It uses the original account to
acquire the locked publication, match the complete captured/sealed invocation,
admit the native execution carriage, recover the actual conditional source,
preflight, consume once, execute the shared worker and finalize the V5 artifact.
There is no second configuration parser, source projection or worker engine.

The continuation consumes readiness even on failure. Successful output owns
readiness and its exclusive account borrow, the artifact and the exact execution
carriage, and retains a borrow of the parent invocation. It has no detached-parts
escape. Exact policy and publication subject are checked again before return.
These structural owners do not grant publication, load or launch authority.

Parent comparison reuses sealed capability revalidation and compares every
descriptor field, not only a digest or compiler closure. The caller must prepay
the parent capture before readiness borrows the account. The separately admitted
root policies, target and history limits are not inferred from handoff claims.

No blanket-refund scope encloses conditional recovery. Its terminal error retains
the transaction lock; returned source, preflight, worker and finalizer storage is
reserved before further use. Only known success scratch and the dropped lease's
storage are released. Configuration admission and worker/artifact payload bounds
remain separate obligations, not an aggregate allocator or RSS guarantee.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded build
at a time, with compiled inputs frozen during each build:

| Command / Filter | Result |
| --- | --- |
| Cargo binary: `protected_compiler_handoff_v3::native::tests` | 3 passed |
| Cargo binary: `compiler_execution_boundary::native::tests` | 4 passed |
| `cargo check -p cargo-fe2o3 -p rustc-codegen-fe2o3 -p fe2o3-hsaco-finalize --all-targets` | Passed; warnings remain |
| Transaction library: `conditional_transaction_v5_` | 13 passed |

No tests in these selected runs were ignored. Parent tests cover exact agreement,
crate/source/target/cwd/closure substitutions, a replaced sealed capability,
exact and one-short work/storage/input floors, and original-ledger preservation.
Receipt tests check the shared complete-policy/subject comparison. Transaction
regressions cover consume-once, currentness, crash journals, lock retention on
error/panic and terminal resource accounting. They exercise supporting APIs,
**not successful execution of the new complete Cargo continuation**.

Initial parent fixture attempts failed on an `OsStr` argument mismatch and a
noncanonical inherited-backend spelling, then on a private constant import.
The final fixture uses the existing ordinary backend path form and passes;
production parsing and validation were not relaxed.

Logs under `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
15276e87a7d64ae4b34d0667bff746f2599ce637425af7dcae01b058b9525279  cargo-conditional-parent-tests-r4-20260927.log
5924bbb89789713abf3ea902ffa3fbbbf8515ada0e34e79300bbad7c991b8618  cargo-conditional-receipt-tests-r1-20260927.log
1021d8e0b3afeed00f4f8032b175fa700e2add168b3c8d557364b0953281e1e5  cargo-conditional-continuation-check-r1-20260927.log
5e735622bd137cbee97a270097a1287d957ca66ccb7f2b4bb1266c1ec4ae7a3b  cargo-conditional-transaction-regression-20260927.log
```

## Activation Gates

The production wrapper still selects the V1 execution/V3 handoff path. Bounded
configuration preparation must supply usable admission/storage quotes before
native readiness takes the account; a comment requiring prepayment is not that
implementation. Bounded live invocation capture, coherent broker/deployment
selection, positive protected conditional recovery, conditional publication and
recovery, sealed authority admission and generated safe host consumption remain.
Machine/numerical refinement, the target-matched 47/47 matrix, migration/removal
and release CI are not established by this checkpoint.

Native agent creation failed at the service thread limit. The primary agent
implemented and reviewed this change; no Qwen worker was invoked. No remote jobs
or files were created. Push and issue-update outcomes are reported separately.
