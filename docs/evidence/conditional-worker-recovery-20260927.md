# Conditional Worker Recovery Adapter

Date: 2026-09-27. Base: `4e4bdd6f566b1c5a17ce18024094360b25e722a0`.
Follow-up to [compact transcript retention](conditional-compact-replay-20260927.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).

**The conditional replay entrypoint is implemented and compiles. Its transaction,
artifact and resource components pass tests. A complete positive recovered
conditional-source-to-artifact replay has not been executed. This is not native
production activation, protected execution, a completed M0-M7 milestone, or
47/47 GPU qualification.**

## Implementation

`revalidate_conditional_worker_finalizer_v5` consumes the actual independently
recovered conditional source, retaining its final graph and contract catalog.
It checks the transcript's attempt and outer identity, rederives the complete
V5 transaction occurrence, and rebuilds the conditional Worker binding. It
checks provider identities, inspects and reconstructs raw V5 artifact bytes,
reconstructs both canonical Worker exchanges with the existing engine, validates
their complete metadata and binding, and compares the source-evidence identity.
Finally it reruns conditional finalization and compares the finalization identity
and exact finalized artifact bytes. No Worker process is started during replay.

The caller must recover the source against independently admitted policies
before entering this adapter, outside any refund scope around that terminal
admission. The adapter cannot construct a source from transcript coordinates or
substitute ordinary V4 source evidence. Fresh Worker execution and independent
replay retain distinct private source variants. The public `custody()` observation
reports `ConsumedPublication` or `RecoveredTranscript`; neither observation is
authority and no conversion into a consumed token is provided.

V4 and V5 receipt rederivation share the existing schema-driven transaction
hashing implementation. Each uses its own domains and typed coordinates. The
V4 wire and logical quote are preserved. Receipt replay observes no filesystem,
creates no currentness lease, and cannot reopen a consumed publication. Native
and conditional Worker evidence also share the transcript-validation and
identity-hashing join instead of duplicating its checks.

The original budget prepays source backing/metadata/recovery, retained transcript
and borrowed finalized bytes. Provider construction and Worker reconstruction
use that ledger; the common replay quote conservatively accommodates the actual
conditional evidence header. Exact final-byte comparison is charged. Returned
Worker/finalizer storage is additional and unreserved. Original work and denial
history survive temporary-scope cleanup. Artifact parsing/payload allocation
retains the existing separately bounded finalizer domain: this is not a claim
about whole-process RSS, allocator overhead or spare caller capacities.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded
Cargo command at a time, with compiled inputs frozen during each command:

| Command / Filter | Result |
| --- | --- |
| Artifact-transaction library: `replay` | 15 passed |
| Artifact-transaction library: `compiler_module_handoff` | 158 passed |
| Complete finalizer library | 197 passed |
| Complete artifact-transaction doctests | 31 passed |
| Complete finalizer doctests | 64 compile-fail and 2 compile-pass passed |
| Five-package `--all-targets` check | Passed; warnings remain |

The handoff and replay selections overlap; counts are not additive. The build
check covers `cargo-fe2o3`, `rustc-codegen-fe2o3`, `fe2o3-hsaco-finalize`,
`fe2o3-compiler-execution-client` and `fe2o3-artifact-transaction`.

New tests cover inert receipt replay before publication and after consumption,
producer/attempt/target/source/transaction/slot-domain substitution, exact and
short resource budgets, original-account preservation, header-quote overflow,
V5 artifact round trips on gfx942/gfx950 fixtures, corrupted final digests,
unfinalized bytes and wrong descriptor families. These are structural fixtures,
not fabricated semantic proof owners or successful protected GPU executions.
The first receipt test build exposed a private fixture-helper import; the test
now acquires and reserves its token through the existing lease API instead.

Rustfmt and whitespace checks passed. No unsafe code was added or inventory
allowance changed. The preexisting unsafe inventory failure recorded in
[the earlier checkpoint](shared-compiler-slot-admission-20260927.md) remains
unresolved and was not rerun here. All build/test processes terminated and
private `/tmp/fe272recover.Vz8JsFq8` scratch was removed.

Logs under `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
c5a267d089e1f3e6d020cc7a6de807d840669876047acccb5608891f7bd95bc6  conditional-recovery-receipt-tests-r1-20260927.log
0e4fbf3ac9f91a729722055034c65019e0f2959fba9d361f8eead4baca9a7232  conditional-recovery-receipt-tests-r2-20260927.log
0db39d789883b93960ac87c50fb0004a35469ab24fd7b261517162fe886476c3  conditional-recovery-finalizer-tests-r1-20260927.log
7588aa283d08fb7b09aa0411d93357c6bf72ec3963d0a79cca4cfe187411cb91  conditional-recovery-doctests-20260927.log
64f5579a473ec3aefdd7a1ceda9fe271e7082f1f11afcd01858f3b1103e2e522  conditional-recovery-handoff-regressions-20260927.log
9107860d6cad00d67dd5772212cecf3a71d2729726cea55ff4940bf216470b57  conditional-recovery-all-target-check-20260927.log
```

## Remaining Work

Integrate conditional durable publication/recovery through the existing journal,
including independent source policy admission and exact fresh/recovered joins.
Do not use nominal V5 structural publication as a source-family downgrade. The
parent/profile/broker/driver transition, protected compiler receipt continuity,
sealed verifier and generated host integration, complete positive replay,
machine/numerical refinement, target-matched 47/47 runs, selector retirement and
release gates remain open.

Native agent spawning again hit the thread limit; implementation and review were
local. This execution environment could not resolve GitHub or any of the three
SSH aliases. No remote job or file was created. Push and issue-update outcomes
are reported separately, not implied by the local test results.
