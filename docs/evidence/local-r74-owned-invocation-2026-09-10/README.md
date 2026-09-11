# Owned Generated Invocation

Local GEN-2A implementation evidence. Baseline:
`16e001f22cb98f61c8b1d1f54ed8d05799d6af30`, branch
`codex/r65-runtime-drain-versions`. The [contract](../../runtime-owned-generated-invocation-v1.md)
defines the nonexecuting boundary and remaining GEN-2B obligations.

## Scope

R74 adds an owner-local generated invocation, consuming the exact authenticated
executable, raw generated arguments and checked device. Existing production
admission remains mandatory, with no qualification fallback. The old borrowed
path retains its original qualification behavior. A shared private constructor
retains the consumed refinement receipt and derives authority coordinates from
the existing admission result; no new unsafe implementation is added.

Charged inputs move into a private storage owner whose complete payload precedes
its decoder in declaration/drop order. Runtime preparation has a closed error
type and cannot return inputs outside that owner. Prepared requests, including
read-only initialization copies, remain guarded through later validation.
The final owner retains storage before authority/device, explicitly prevents
`Send`/`Sync`, and exposes no execution or storage/decoder extraction method.

Six new CPU tests exercise actual packing/preparation helpers: malformed object
rejection, successful identity/retention and disposal, scratch rejection,
post-preparation unwind, witnessed payload-before-credit disposal, and read-only
policy-copy retention. The fixture is structurally valid but deliberately
nonexecutable. Seven downstream negative cases reject cloning, Send, Sync,
private storage, private decoder, executable reuse and execution. A positive
compile-only function type-checks the public consuming interface. Existing
predicate, receipt and identity tests are retained; protected provenance now
has the full currentness/receipt truth table in both tested feature profiles.

Three read-only agents reviewed ownership, admission and downstream type safety.
No blocking issue remained. These tests and reviews do not exercise the complete
constructor with a genuine protected compiler deployment and checked GPU device.
They do not validate Context shutdown, native completion or production execution.

## Final Gates

| Gate | Result |
| --- | --- |
| GNU five-crate runtime all-feature/all-target tests | 2,222 passed; five existing ignores; 48 harnesses |
| musl same runtime gate | 2,222 passed; five existing ignores; 48 harnesses |
| GNU runtime plus host doctests | 86 passed |
| musl runtime / host doctests | 71 / 14 passed |
| GNU all-feature host library | 233 passed; four existing ignores |
| musl default direct-KFD host library | 116 passed |
| Focused default host argument suite | 37 passed, including six new invocation-storage cases |
| Generated macro fixture harness | Seven passed, including seven new invocation negative cases |
| Runner/checker suites | 151 passed |
| Seven-crate all-feature/all-target and no-default library Clippy | Passed with warnings denied |
| Formatting and whitespace | Passed |
| Dependency policy and tests | Passed; 141 members, eight layers, 478 declarations; eight tests |
| CI-local test-gate and standalone lockfiles | Passed; 32 standalone manifests |
| Production musl metadata | Passed; 43 packages and eight permitted build scripts |

All seventeen final source commands, statuses and timings are retained in
`raw/r74-final-source-gate.json`. The gate uses pinned `nightly-2026-04-03`,
locked/offline dependencies, four Cargo jobs, disabled incremental compilation
and an unset stale `XDG_RUNTIME_DIR`. Before/after and retention checks matched
all 5,578 tracked/nonignored non-documentation input identities. Documentation
was finalized afterward. No manifest, dependency or lockfile change was needed.

The metadata audit is not an actual qualifier ELF audit. Its commands were:

```sh
cargo +nightly-2026-04-03 metadata --locked --offline --format-version 1 --no-default-features --manifest-path crates/fe2o3-runtime/Cargo.toml --filter-platform x86_64-unknown-linux-musl
python3 -B scripts/runtime_pure_rust_audit.py metadata --input /home/harsh/.codex-tmp/r74-production-metadata.json --root fe2o3-runtime
```

## Proof Boundary

No Verus source or theorem was added or changed, and the solver was not rerun.
Retention checks confirm runtime-model, resource-accounting and completion
sources are unchanged from signed R73. Its property-specific proof results stay
in the [R73 evidence](../local-r73-charged-results-2026-09-10/README.md); they are
not a proof of this new Rust adapter. A current negative-quality/inventory check
passed all 686 files:

```sh
python3 -I crates/fe2o3-runtime-model/verus/check-negative-quality.py crates/fe2o3-runtime-model/verus/negative crates/fe2o3-runtime-model/verus/verify-verus.sh
```

Rust field-drop ownership, allocation and decoder adapters, concrete device
currentness, native execution and whole-executor refinement remain outside the
claimed proof boundary. No positive production constructor is claimed without
the exact protected-verifier/refinement backend and artifact handoff.

## Earlier Attempt

The first focused run passed 34 tests and failed three new successful-preparation
cases because the new fixture's zero private size disagreed with its enabled
descriptor scratch bit. The separately named fixture variant now clears that
bit as well as metadata/descriptor private size; original fixtures and runtime
validation remain unchanged. The unwind test originally caught this earlier
fixture panic. Moving successful preparation before its panic catcher prevents
that false positive. The second focused run and final gates pass all 37 cases.
Both preliminary logs are retained, not substituted for final acceptance.

## Retention And Remaining Work

`source-files.sha256` is relative to the repository root;
`retained-files.sha256` is relative to this evidence directory. Final source and
retained identities were checked after retention. Verbatim logs are not edited
to satisfy whitespace checks.

No SSH session, MI300X stage, build or workload was started, so no remote cleanup
was required. GEN-2B Context admission/publication, exact native retirement,
charged decoding, wake/retry and shutdown remain open. R73 result credits still
exclude executable images, full/hidden kernargs, read-only initialization copies
and complete metadata/account-domain closure. No hardware acceptance, full
HIP/HSA parity or performance improvement is established by this packet.
