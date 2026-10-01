# Scoped Source and Protected Runtime Checkpoint

This records progress on [#272](https://github.com/harsh-nod/fe2o3/issues/272),
not a release qualification. **M0 is complete; M1-M7 remain incomplete.**
Strict production compiler -> required proof -> safe GPU launch coverage remains
**0/47**. Component tests, authored proof adapters and runtime provisioning do
not advance that count. This does not reclassify independently runnable legacy
examples as qualified through the new production path.

## Latest Compiler Checkpoint

The latest completed six-package `cargo check --tests`, r313 at
`683a2724f9b9a84dd4bf19982c5b2d39a7d4d46e`, ended at 12:13 UTC with one
lowerer test borrow-checking error. No tests ran. Its narrow fix drops the
borrowed descriptor view before its backing bytes; that fix is local.

The latest actual Rust source run, r312 at
`414549a7f201f6d37e179da2307fef0dadf19e08`, built the backend test binary but
finished with **zero passed, one failed**, none ignored and 4,213 filtered:

- Fill: gfx942 opt0/MIR0 and gfx950 opt3/MIR2 stop during original source
  assembly. The other two configurations reach the native-policy callback,
  then refuse an uninterpreted execution intrinsic before completed-source use.
- Vecadd: both targets at opt0/MIR0 reject execution availability against the
  original SSA instance; both at opt3/MIR2 reject inconsistent scopes at a
  control-flow join.

All ordinary configurations failed. Later planned negative controls are not
credited. No source clauses, Verus execution, protected proof or GPU run were
reached. The strict source-clause extractor was not run for r312.

Local repairs now join source events to exact value IDs independently of event
ordering, and bind genuine context-derived coordinates into scalar expressions.
The attachment census releases only its own temporary bookkeeping before
emitting retained output. Coordinate interpretation uses the already-global
cell instead of applying the invocation mapping twice. New regression tests
are authored but not yet executed. Control-condition coordinate binding and
the proof-client/runtime changes also remain local, not end-to-end acceptance.
Complete source-bound proof execution, paired publication/finalization and
continuous protected compiler enforcement remain implementation work.

The r314 static musl runtime-test build at the same r313 revision failed because
`libc` does not expose `PTRACE_GET_SYSCALL_INFO` on musl. A local repair uses
the unchanged Linux UAPI request value. The twelve new native process-tree
controls have **not run**. Their launcher's seven local input/result tests
passed, which does not qualify native execution. MI350 is reachable; read-only
probes identified GCC 13 and the expected login account. No remote test scratch
was created by these probes.

The primary independently audited source inventories, tool identities, raw logs
and runner before moving each validation worktree. Raw log SHA-256 values:

- r312: `482e16514b9452bc22a9f45f964ebe6a98a72c6e474d859d1bb1a4e56603ea03`
- r313: `01cc23abaaae6b60c8537da43bdf6c6ab115623b85b9d7463e1c318e3795649d`
- r314: `e13e2c722b2af41947160143cff05885cb6320b5afe2e2723a5bebdb9f58e86a`

Both public mains were read at
`f0145ab1a71a2dcaa888623e6035c47144f59c38` before this update. This publishes
evidence, not the unfinished compiler candidate. **M0 alone is complete;
M1-M7 remain incomplete; strict coverage remains 0/47.**

## Previous Compiler Checkpoint

At 11:34 UTC, r310 completed the five-package `cargo check --tests` at
`3dabc6f3791f6cf876f609f4be6d2ffa854d670b` with **five verifier compilation
errors**: three unavailable formation-type imports/uses and two mutable-budget
argument mismatches. No tests ran. The preceding r309 failed on three
scalar-query helper visibility errors; its narrow visibility repair is included
in r310. These are completed failed checks, not observation timeouts.

The latest actual source execution, r308 at
`87f35a3a9a0415d05333061d3425ca917daaedf5`, built the backend test binary and
finished with **zero passed, one failed**, none ignored and 4,114 filtered.
The fill parent exercised gfx942/gfx950 source configurations. gfx942 opt0/MIR0
stopped at unsupported original private-expression derivation; gfx950 opt0/MIR0
stopped at execution lifecycle during pending root emission. Both targets at
opt3/MIR2 stopped at an execution-lifecycle check during a producer call.
The parent failure does not establish that every planned negative control ran.
No actual source-clause frame was emitted; the strict extractor refused the log
and created no proof input. No Verus, protected proof or GPU execution occurred.

Original integer-cast reconstruction and source-proven consuming-Copy handling
are now integrated locally, but an actual source rerun has not yet confirmed
these repairs. Multi-effect memory tracking, scalar/read symbol binding and
typed original-source borrowing are also local work, not end-to-end acceptance.
The complete source-bound proof request, authenticated receipt consumption,
publication within the original owner lifetimes and continuous protected
execution remain implementation work.

Both source inventories, tool identities, raw logs and runner were independently
checked before changing each validation worktree. Raw log SHA-256 values:

- r308: `7a11d50b450bdb64ad2ec2eb64cbb7b23e1cd3127d03d929458580cda65c2fff`
- r309: `1d5677e13c7b3070f1c4730c56403cc4ce477c4efeb77c3baa037506cdfe4d3b`
- r310: `62fd050e64e583aca767a412b49fa5731a8dda8a8283736131e29109a495c174`

Both public mains were independently read at
`e9d283947e171346b55dc6112bc2dff568fbe135` before this documentation update.
The unfinished compiler candidate remains local. **M0 alone is complete;
M1-M7 remain incomplete; strict coverage remains 0/47.**

## Earlier Actual Source Checkpoint

At 10:54 UTC, r305 completed the actual Rust fill/vecadd parent tests at
`cb4a7f048677b89d61ea4e0189194e4055cbdbd7`: **zero passed, two failed**, none
ignored and 4,101 filtered. The backend test binary built successfully, but both
tests failed before the expected original-source consumer was reached. Their
assertion hid the underlying compiler refusal, so its cause is not yet established.
A subsequent diagnostic-only change preserves that error in the assertion;
it has not been rerun and is not a fix for the refusal.

No generated source-clause frames were reached. The strict extractor refused
the log rather than substituting proof text. No Verus, protected proof or GPU
execution is credited. Source/tool inventories stayed unchanged, and the
primary independently checked both source inventories, raw log and runner:

- r305 raw SHA-256: `775cc9ee2c8c188ad69e462f28ea15f82e04d84bace473abd378a0c481914ff9`

The preceding r304 four-package test-binary build failed with ten diagnostics:
nine from an ambiguous coordinator callback error type, and one from a scalar
test expectation for a tuple-valued receive API. Corrections are committed
locally but have not been rebuilt. r305 selected the backend alone and does
not validate those corrections. Its predecessor's independently checked raw
log SHA-256 is `967d534d369ced484db3fb3e763680259f079df0a5fb58a8bfc96decf90eed5f`.

Newer reviewed proof-client cleanup, native source-admission compatibility,
original typed-root borrowing and target/descriptor/Worker integration remain
local and untested. Full source-proof request construction and continuous
protected compiler enforcement are still implementation gaps, not merely GPU
testing tasks. Both public mains were read back at
`7cfa074cf97c51176f042fe33fd531f7ddefcb81` before this documentation update.
**No milestone advances: M0 alone is complete; strict coverage remains 0/47.**

## Focused Component Checkpoint

At 10:19 UTC, r303 passed **75 selected descriptor, KFD and host tests** at
`aa7780fc31e692e64bb1896824f9aa26dce1c418`: descriptor 29, KFD 26 and host 20,
with zero failures or ignored tests and 625 filtered tests. These are component
tests, not GPU runs, complete crate suites or protected-proof execution.

Ordinary pointer formation and its eventual memory access now retain separate
source coordinates and address envelopes in the V28 contract. Shared formation,
type and link requirements remain checked. Both envelopes are checked against
the live mapping, including zero-access formations and independent overflow
cases. The codec itself does not authenticate source correspondence or grant
compiler, artifact or launch authority.

The preceding r302 selection had 74 passes and one failure: a newly authored
test incorrectly expected an unchanged formation identity when another contract
row changed. The existing hash intentionally binds each row to the whole
contract. Only that assertion was corrected; production hashing was unchanged,
and the test still requires the decoded formation row to remain identical.

The primary independently checked both runs' source/tool inventories and raw
log hashes:

- r302: `95ffeef3b43586390dbf52c031a8775392fedaf7628f97d7e875da5465ebf2b7`
- r303: `adf15b3b6b30c79c9b58f5063b00d8f67b8cb2e5a657cf41925b071cf2610918`

The broader compiler integration is not yet validated. The r300 lowerer-test
build exhausted its unchanged 16 GiB virtual-memory allowance; no tests ran.
The r301 native-backend build then stopped with three invalid block-ID method
calls in source-control verification; no tests ran. Those calls were corrected
to use the original source block ordinals. New original-formation, borrowed
proof-client and target-consumer integration requires its own build and actual
Rust fill/vecadd execution; the component pass above does not validate it.

Protected execution still needs the complete original compiler/proof connection
and continuous enforcement. Provisioning, endpoint association, process parking
and ordinary proof-text diagnostics are not protected proof results. No required
missing source obligation is waived. Both public mains were read back at
`978379990f45c126c6191ed9517288ee695ba4a6` before this documentation update;
the unfinished compiler integration remains local. Full-range publication gates
also remain unresolved. **M0 alone is complete; strict coverage remains 0/47.**

## Earlier Compiler Checkpoint

The completed r294/r295 runs used the local candidate
`0d4f37f1e2b8d963aeb28891dc6c872c52d980fd`. The primary independently checked
the source/tool inventories and raw logs. Neither run executed a GPU kernel
or earned protected-proof credit.

| Run | Executed scope | Result |
| --- | --- | --- |
| r294 | Selected kernel-IR, lowerer, Pliron and verifier library tests | 884 passed, 47 failed, 10 ignored |
| r295 | Complete kernel-IR library suite | 1,501 passed, none failed, ignored or filtered |
| r296 | Four-crate `cargo check --tests` on the newer integrated candidate | Passed; no tests executed |

r294 includes kernel-IR 37/0, lowerer 596/47, Pliron 73/0 and verifier 178/0
(pass/fail). Its ten ignored tests include checks requiring actual installed
protected-runtime prerequisites. These are **test counts, not kernel counts**;
the selected roster is broader than r285, so totals are not a same-selection
comparison. Remaining lowerer failures include access/formation correspondence,
parent cleanup accounting and insufficient fixture work allowances. The latter
require diagnosis against actual optimizer charges, not arbitrary cap increases.

r296 checked `fe2o3-kernel-ir`, `fe2o3-lower-mir-kernel`, `fe2o3-pliron` and
`fe2o3-verifier` at `bfb609bb5b9b5ead0b6ef048f8849c8c085b031c`. That candidate
includes the completed-source clause-input adapter, read-binding/formula
components and a fix distinguishing the original pointer formation from its
forwarded memory-access operand. This passing compile check does not validate
their behavior or Rust backend tests, which need their own build and execution. Required missing
source/memory obligations still refuse before aggregate proof execution.

Independently checked raw log SHA-256 values:

- r294: `514b80f449a17d220a9083e30dd1ccde86bfc80252a77c6d244100a250a9faaf`
- r295: `7c5a483f7e879e5586a587d9c992736757ac409a60afac92028a97b858802d99`
- r296: `2e12b3aed5f03f17268b0043735349c97a87936d3ae483f87e2e0e58ac50de37`

Further expression-pair, scoped native lookup and parent-cleanup fixes are
integrated locally but have not passed an execution rerun. The genuine Rust
fill/vecadd results remain the failed r272/r273 runs below. The protected client
and helper transport are still unqualified, and the active driver lacks the
complete confirmed-execution and continuous-enforcement connection required
for protected proof execution. An endpoint, handshake or process-output record
does not replace that connection or a verified proof.

The unpublished integration also has unresolved full-range hygiene and inherited
commit sign-off findings. Passing checks on individual patches do not establish
a passing publication gate. Both public mains were independently read back at
`ca8647fc1932dece0a5b34e1b75721d482225d7e` before this documentation update;
the unfinished compiler integration is not part of that publication.

## Earlier Checkpoints

The completed full kernel-IR library run r286 passed **1,497 tests**,
with zero failures, ignored tests or filtered tests. Its candidate was
`dce057115143ba327739877aab424ff56eb96eea`. Source/tool inventories stayed
unchanged, and the primary independently checked the raw log digest:
`f7dcd87666278c67be120740e340a72d8939239ff5490b3926200e5c0ce3a51b`.

This implements scoped pointer forwarding through exact SSA block arguments,
retaining the original producer and scope lifetime. Presence guards
propagate in the direction of actual CFG edges. Mixed origins, unconstrained
entry parameters, closed scopes and unsupported escapes remain rejected. This
is a canonical-IR result, not a genuine Rust frontend or GPU result.
The scoped-memory projection now distinguishes the actual access operand from
its original pointer-formation result and checks their typed, unique SSA origin.

The selected compiler integration run r285 completed at
`820f953e1f4598ac967b3c55963ad2f2e730cedb`: **455 passed and 47 failed**,
none ignored (lowerer 410/47, Pliron 20/0, verifier 25/0). These are test counts,
not kernel runs, and the selected roster differs from earlier runs. Source/tool
inventories and the independently checked log digest remained stable:
`bc3ae6748de815c7be6e23a9cfe6765e215a1306ee7d04f8ab574de125033c74`.
Failures include source/native pointer correspondence and fixture assumptions
about generated abort edges, deferred cleanup flags and query-counter scopes.

The successor combined `cargo check --tests`, r287 at the r286 candidate,
failed with **88 lowerer compilation errors**; no tests executed. The errors
include generic lifetime coupling, source-block identifier types and a helper
still restricted to the older source profile. These are being repaired, not
waived. Source/tool inventories stayed unchanged; raw log digest:
`5987c1c01e7c71bf8573490e379d758005c3a66ef388688761ef2329cd5ac47c`.
The full kernel-IR pass does not validate this unfinished compiler integration.

The reviewed lifetime/interface repairs were integrated at
`79d670124cc2b61151bbfecac29d1ac0a6369740`. The next combined check, r288,
stopped with **three compilation errors**: two scoped-wrapper lifetime arities
and one shared native-view outlives bound. No tests ran. Further errors may
be exposed after these type errors are fixed; this is not a passing build or
evidence that every earlier error is resolved. Source/tool inventories remained
unchanged, and the independently checked raw log SHA-256 is
`7b8c7f6e85836575bbe686092c7145f864df470501f5f8627ec931258118ec76`.

The successor r289 at `db6f0c379c10a477d00c4678216eea4593f87cb2`
includes the scoped lifetime repairs and exact original-read symbol interface.
It stopped with **two lowerer lifetime errors** in the final LICM native
handoff; no tests ran. Its seven new source-read tests remain unrun, and the
complete memory theorem consumer is still being implemented. Source/tool
inventories remained unchanged; independently checked raw log SHA-256:
`fce0b69e3bda660e2e486e6b324deb575d70ed76be43a54830b7b4d22ecd114f`.

The preceding combined compiler run, r279, finished at
`af11b1f931ee622dfb2029e551660fbf05ae79b6` with **402 passed and 93 failed**:
lowerer 357/93, Pliron 20/0 and verifier 25/0. No tests were ignored. Remaining
failures included scoped-pointer CFG transport, transient enum projection and
test observers that did not handle the emitted Switch terminator. Its log digest
is `8d6510fe6aa2c3c10ac0b9c4089bdb765cf2fe0d9f4b1548e48b4f84ad3f5de9`.
The successor integration at `820f953e1f4598ac967b3c55963ad2f2e730cedb`
includes repairs for those boundaries. Its r284 build stopped with **no space
left on device** while compiling the lowerer library test; **no tests ran**.
The runner also failed to write its final inventory/report, so this run has no
source/tool-stability attestation. The retained partial log digest is
`167a3f163c46a107a72f2437b7c2804083170764e9ccad3df2d25c69005184ae`.
The latest genuine Rust fill/vecadd results remain the failed r272/r273 runs
described below. Disk recovery is not validation of the new candidate.

Both public main branches were independently read back at
`3c1205861765a99459f9a4b4513760ff17bcceac` before this documentation update.
That publication fixes target-scoped dependency policy, the missing codegen test
shard assignment, and workspace formatting. All five codegen shards passed in
the [CI run](https://github.com/harsh-nod/fe2o3/actions/runs/36826853167);
generic-core subsequently failed, so the overall run failed. Its `cargo-fe2o3`
suite had 470 passes, two failures and five ignored tests: both failures rejected
the stale reviewed workspace macro-tree pin. Review found exactly four changed
files since the accepted tree: generic typed-entry validation and its tests in
`54585b104`, and a nested fixture lockfile synchronization in `ce7bed431`.
The complete 59-file tree was independently hashed and its local pin updated;
the former pin and intermediate source-only pin are explicit negative controls.
External-source and device-source pins and all closure checks remain unchanged.
The pin repair was published as `65d0910b36c222e09598ae4d5817ee6004fdf190`.
Its five codegen shards and parity gate passed, but generic-core was cancelled;
that run does not establish a passing release gate. Both public mains were
subsequently read back at `a209ae259299069479570cff7726cec7ff0b448e`, preserving
concurrent literal-repeat and retained-storage work. That candidate's
[CI](https://github.com/harsh-nod/fe2o3/actions/runs/36831694594) has five passing
codegen shards and a passing parity gate; generic-core was subsequently
cancelled. Neither cancelled run establishes a passing release gate. The
unfinished local compiler candidate has not been published.

Subsequent independent readbacks found both public mains at
`6ea0940fb67352bbc6a49c2ffab62e1f8ce803e9`, retaining concurrent context,
descriptor and reference-storage work. Its
[CI](https://github.com/harsh-nod/fe2o3/actions/runs/36833714801) was cancelled.
The next readback matched `cefb2f6426346a63a55b25dfac2e350d0a76c406` on both
mains; its [CI](https://github.com/harsh-nod/fe2o3/actions/runs/36834002610)
is in progress at this checkpoint. No complete release gate is claimed. Those
published changes do not include the unfinished scoped-source integration
candidate.

## Compiler Evidence

The guarded runs used pinned nightly-2026-04-03, locked/offline dependencies,
one Cargo job, serial tests and disabled GPU visibility. Their complete source
and tool inventories remained unchanged. The candidate was
`3fe6788ccb28d49f3eeb5254c96c3e9a6d55472d`, not the public release branch.

| Run | Executed scope | Result |
| --- | --- | --- |
| r270 | Selected lowerer, Pliron and verifier library tests | 167 passed, 82 failed, none ignored |
| r271 | Fresh Rust compiler-backend test binary, `--no-run` | Build passed; no tests executed |
| r272 | Genuine Rust fill, fresh compiler sessions | Failed before proof or GPU execution |
| r273 | Genuine Rust vecadd, fresh compiler sessions | Failed before proof or GPU execution |

r270 includes lowerer 136/81, Pliron 14/0 and verifier 17/1 (pass/fail).
Most scoped source consumers fail at original live guarded-memory admission.
All 14 source-census tests pass; the remaining formula failure is an invalid
fixture identifier, not an observed semantic-formula mismatch.

Each genuine Rust parent tests gfx942 and gfx950 at opt0/MIR0 and opt3/MIR2,
with two fresh sessions per combination. No prior backend binary or saved child
request was substituted. The observed failures are:

| Source | gfx942 opt0 | gfx950 opt0 | Both targets opt3 |
| --- | --- | --- | --- |
| Fill | Lifecycle correspondence | Original borrow not emitted | Index operand is `Copy`, not `Move` |
| Vecadd | Execution availability | Lifecycle correspondence | Index operand is `Copy`, not `Move` |

The consuming-index repair must establish an authenticated, single-use witness
and preserve original source/SSA identity and live-borrow checks. Simply accepting
copies or fabricating move events would not establish that invariant. Rust's
[runtime MIR operand rules](https://doc.rust-lang.org/nightly/nightly-rustc/rustc_middle/mir/syntax/enum.Operand.html#variant.Copy)
do not require the `Copy` trait for every post-drop-elaboration `Copy` operand.

Log SHA-256 values:

- r270: `cdf0e1e04c715d7921a43800227843f0d1166f739bb693aa9f5f1c41f0d77dbf`
- r271: `a4cff1dabe50da409e8a6e883f876cce6aefe9680e8dbfd84656b157ada7eb5f`
- r272: `978a6aab04c7110f4adff829a4618bd99db7efae6be6ea0aef8a30b872819b33`
- r273: `856f784812a125e9c96eb99a5a4b03eb2fb14cd41197a12441b20ca12c806c77`

The subsequent r274 candidate,
`2b80425ad549bd42a29428f51b84ad3f2bb0736c`, integrates scoped private-source
completion, precise physical-access diagnostics and fixture corrections. Its
six commits pass DCO and hygiene checks. The build stopped at one inaccessible
type alias in a new test's independent frame-size oracle; **no tests executed**.
The one-line test correction preserves the exact size/alignment assertions and
needs its own rerun. r274 retained unchanged source/tool inventories; log SHA-256:
`245b06bfa3513246d70dc4c5bc9528691cb3f9eb555990b241bda7f0132af8e3`.

## Protected Provisioning

One fresh isolated MI350 attempt completed installation, genuine V3 protected
provisioning/readback and lower revalidation. All three actual stages returned
exit 0. Readback checked fresh records, key ownership, signatures, image identity
and service identity. Prior protected state was neither reused nor changed.

The original container failed the unchanged one-byte `flistxattr` check on
`/usr`. A private derived image precreated the mount targets, avoiding OverlayFS
copy-up there. No attributes were stripped and no acceptance rule was relaxed.

The executed private runner changed exactly the fixed image literal from the
committed `088d41dfecca8b3a7343331ac8854d2684b63bea` runner. It was not
byte-identical to that committed runner or a silently changed production default.

- Executed image: `sha256:bb06b7d78df7bf67a916ecaabcdf2b232bf3855cbc636949b796ef9497e07926`
- Executed runner: `5e8387951c51c93ee94ac8d383f4c3a19f3e4d705f9a7862918ef89bd94fe4da`
- Unchanged native readback: `4702554bf9f99fe799a41468a8e60702c05fec00426b8b154f2c8e710f753f7b`
- Public report archive: `21cafc7ec30d3b8800079db8127da3cd76b85d8e5295e1b84ed0c89c5349f6f2`

The primary independently verified all 47 public archive payload/manifest hashes
and six actual container snapshots. Those are **47 archive checks, not kernels**.
The snapshots retain the exact image, private PID namespace, read-only root,
no network, restricted capabilities, two CPUs, 2 GiB and 64-process limit.
Independent cleanup records confirm all owned processes and three containers
are absent; protected records and reports remain retained. No secrets were exported.

**Not executed:** service activation, protected-runtime assembly, protected
compiler/proof execution, simulator qualification or GPU validation. The image
recipe, fixed production pin and regression tests still require source integration.
The experiment does not establish a bit-identical Docker rebuild.

## Subsequent Runtime Evidence

The actual static proof-helper build from exact
`3c505a13f780c0f8703f5128bb724edefe0644c3` completed on MI350. Preparation,
build and actual ELF inspection returned zero. The 7,193,784-byte executable
has the expected secure entry, a nonexecutable stack, no dynamic dependencies
and no undefined symbols. Artifact SHA-256:
`f548930c6a3cce88c1974c08f48e6f82df20ca7b2f375481ae08c9d0e436a8be`.

All 42 public archive payload hashes were independently verified. Owned build
processes were drained and disposable caches removed; the artifact and reports
remain. **The helper was not executed or admitted as a production runtime role.**
Protected proof requests, compiler enforcement and complete runtime assembly
remain unfinished. Successful helper compilation grants no kernel coverage.

The committed image-admission gates at the same source revision also passed
for the admitted base and derived image, and rejected the old base-as-executable
substitution. This supersedes the earlier image-integration-pending note for
that local candidate only; it does not establish service or compiler activation.

### Deployment Tests on MI350

Exact deployment source `db78a9c36abc8d8a11dfe8b2a0f79205fd9bb7a0` passed a
nonroot MI350 run in private scratch with pinned nightly-2026-04-03, locked
offline dependencies, one build job and serial tests:

- Deployment library: **143 passed, zero failed, two ignored**.
- Qualification executable: built successfully; **four unit tests passed**.
- Compile-fail doctests: **19 passed**.

The two ignored tests require native root-owned filesystem authority and remain
unvalidated. All source inventories before and after the run matched. The
primary independently checked the exact source archive, selected raw report
contents and matching local/remote report-archive SHA-256:
`d0a305b99ae6acb98932706954bd6701daa36f2a5e001d742017ee86ffa41c2e`.
All owned sessions terminated. Owned build/dependency caches and temporary
source were removed; evidence and reports remain. No other user's work was
removed.

This run does **not** validate the newer integrated compiler/helper candidate,
native service activation, the fourteen-role descriptor/main-PID join, actual
proof requests, or GPU execution. The cancellation tests exercised the existing
flag logic, not injected cancellation during admission/recovery. Those acceptance
cases remain required; these component passes do not complete M1.

## Milestone Status

| Milestone | Status | Remaining acceptance |
| --- | --- | --- |
| M0: Contracts | Complete | None |
| M1: Minimal production slice | In progress | Genuine vecadd through source verification, protected proof, artifact binding and safe launch, with required negatives |
| M2: Hierarchy and synchronization | Incomplete | Wave/LDS reductions and scan, atomics, barriers, convergence and reuse-epoch matrices |
| M3: General memory and control | In progress | Integrated memory semantics, loops, helpers, cross-crate generics, dynamic layouts and multiple kernels |
| M4: Structured compute and targets | Incomplete | Generic GEMM, numerical/resource checks and target-matched validation |
| M5: Advanced kernels | Incomplete | Softmax, attention, MoE and remaining entries through the complete path and test matrices |
| M6: Authority and migration | In progress | Complete source/optimized/artifact evidence composition and retirement of superseded paths after validation |
| M7: Documentation and release | In progress | Complete coverage gates, qualified manifest, accurate tutorials and identical final implementation on both mains |

## Remaining Acceptance Work

The immediate gate is M1: one genuine vecadd source must complete source/memory/
control verification, protected proof, artifact binding and generated safe launch
in one production transaction, including its required negative tests. M2-M5 add
hierarchy/synchronization, general memory/control, structured compute and advanced
kernels through that same path. M6 requires complete authority composition and
legacy-path retirement; M7 requires coverage-enforcing CI, qualified manifests,
accurate documentation and identical final publication to both repositories.
