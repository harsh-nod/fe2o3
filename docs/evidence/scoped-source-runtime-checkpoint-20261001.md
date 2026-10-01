# Scoped Source and Protected Runtime Checkpoint

This records progress on [#272](https://github.com/harsh-nod/fe2o3/issues/272),
not a release qualification. **M0 is complete; M1-M7 remain incomplete.**
Strict production compiler -> required proof -> safe GPU launch coverage remains
**0/47**. Component tests, authored proof adapters and runtime provisioning do
not advance that count. This does not reclassify independently runnable legacy
examples as qualified through the new production path.

## Latest Verified Checkpoint

The latest completed full kernel-IR library run, r283, passed **1,480 tests**,
with zero failures, ignored tests or filtered tests. Its candidate was
`d7b40521fc828f7eba1f5e95a02e5725de9a3c67`. Source/tool inventories stayed
unchanged, and the primary independently checked the raw log digest:
`db444eee35f7f11014c6862aa0b7d45cf107124026570ac6b4c0cba498eb1cee`.

This implements scoped pointer forwarding through exact SSA block arguments,
retaining the original producer and scope lifetime. Presence guards
propagate in the direction of actual CFG edges. Mixed origins, unconstrained
entry parameters, closed scopes and unsupported escapes remain rejected. This
is a canonical-IR result, not a genuine Rust frontend or GPU result.

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
generic-core subsequently failed, so the overall run failed. Diagnosis of that
remaining job is pending. This is not a passing release gate or publication of
the local compiler candidate.

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
