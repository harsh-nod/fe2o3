# Context Generated Preparation

Local R76 / GEN-2B-2 source evidence. Baseline:
`008486767dff8fdce3242d6ae6cc2dc4635b235c`, branch
`codex/r65-runtime-drain-versions`. The
[contract](../../runtime-context-generated-preparation-v1.md) records the exact
nonexecuting scope and remaining native/generated acceptance.

## Scope

Immutable device callbacks now traverse retained checked-device, shared-memory
and live-queue ownership inside full opening/closing currentness checks.
Currentness errors/panics poison before candidate disposal; callback panic skips
closing observation. Session/queue guards bind exact model admission and VM,
and reject inactive or poisoned queues. There is no device extraction,
readmission, queue creation, model loan or native publication.

Runtime binds an inert owner-local payload to exact Context/runtime-device/
backend-device/model-device identities. Lazy VM/queue absence is not frozen.
Host adds a Context-bound generated invocation sharing the standalone path's
private preparation and protected production admission. Storage precedes decoder
and authority disposal; the existing R73 debit is reused, not reserved again.

Three read-only agents reviewed native guards, runtime/host identity, API privacy
and resource lifetime. They identified terminal failure classification, the
authority audit comment and the terminal mock's intentional-abort destructor.
The corresponding changes are included. Agents ran no builds or hardware jobs.

Fourteen new CPU tests exercise the production-used envelope and scalar guards,
model identity/lazy-VM preservation, Context rejection and inert shutdown
storage. Five runtime compile-fail doctests and five generated-host negative
binaries cover lifetime, mutable extraction, auto-trait and privacy restrictions.
One compile-only (`no_run`) host doctest type-checks the new interface without
retaining a Context borrow; it does not execute preparation. Model-only/fake owners
are not represented as actual checked devices.

## Final Gates

All seventeen corrected-source gates in `r76-final2` pass. The gate runner
checks the same 5,590 non-documentation source identities before and after the
sequence; retention independently rechecks them. No manifest or lockfile changed.

| Gate | Result |
| --- | --- |
| GNU five-crate runtime all-feature/all-target tests | 2,252 passed, five existing ignores |
| musl same runtime profile | 2,252 passed, five existing ignores |
| GNU runtime and host doctests | 92 passed |
| musl runtime / default host doctests | 76 / 15 passed |
| GNU all-feature host library | 233 passed, four existing ignores |
| musl default host library | 116 passed |
| Generated macro fixture harness | Seven passed, including the new rejection binaries |
| Seven-crate all-feature and production-profile Clippy | Both pass with warnings denied |
| Python runner/checker suite | 151 passed |
| Formatting, source whitespace, dependency policy and policy tests | Pass |
| CI-local test gate and standalone lockfiles | Pass; 32 lockfiles checked |

Commands, timings and exit codes are retained in
`raw/r76-final2-source-gate.json`; verbatim logs and the source identity snapshot
are alongside it. Rust gates use `nightly-2026-04-03`, locked/offline dependencies,
four Cargo build jobs, incremental compilation disabled and `XDG_RUNTIME_DIR`
unset. These CPU/type checks do not execute the positive `no_run` host example.

## Earlier Attempts

An initial host all-feature check passed. A focused preparation-filtered run
passed five new Context tests and one existing KFD test; its raw terminal output
is not retained and is not substituted for the final source gates.

The first broader attempt, `r76-final`, failed its GNU gate because the new
backend rejection test dropped a deliberately terminal mock. The backend's
intentional terminal-custody destructor aborted that test binary with SIGABRT.
The test now retains the terminal fixture after assertions, matching the
existing retention policy. This was a test-harness error, not permission to
weaken terminal disposal. The failed log remains verbatim in this packet.

## Proof And Native Boundaries

Model, resource-accounting and completion sources remain unchanged from signed
R73; no theorem was added and the Verus solver was not rerun. The current
expected-negative quality/inventory check passes all 686 files. R73's existing
property-specific results are not a proof of the new callback/Context/host
adapter. External currentness/reset and kernel/firmware contracts remain.

The production musl metadata audit passes 43 packages and eight permitted build
scripts. This checks the dependency graph, not an actual qualifier executable.
Commands and verbatim metadata/audit output are retained under `raw/`.

No genuine compiler-backed generated constructor, concrete Linux retained-owner
success, native output readback, charged result-driver integration, whole-executor
refinement, HIP/HSA parity or performance improvement was validated here.

## Retention

`source-files.sha256` is relative to the repository root;
`retained-files.sha256` is relative to this evidence directory. Verbatim logs
are not edited to satisfy whitespace checks. The post-retention staged whitespace
check excludes `raw/`, whose tool-generated trailing whitespace and EOF blank
lines are preserved.

No SSH session, MI300X stage, build or workload was started for R76. No remote
cleanup was needed. GEN-2B-3 exact persistent projection/publication is the next
integration packet; native bootstrap-scope and production compiler acceptance
remain separate cells.
