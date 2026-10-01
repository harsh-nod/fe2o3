# Scoped Source and Protected Runtime Checkpoint

This records progress on [#272](https://github.com/harsh-nod/fe2o3/issues/272),
not a release qualification. **M0 is complete; M1-M7 remain incomplete.**
Strict production compiler -> required proof -> safe GPU launch coverage remains
**0/47**. Component tests, authored proof adapters and runtime provisioning do
not advance that count. This does not reclassify independently runnable legacy
examples as qualified through the new production path.

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

## Remaining Acceptance Work

The immediate gate is M1: one genuine vecadd source must complete source/memory/
control verification, protected proof, artifact binding and generated safe launch
in one production transaction, including its required negative tests. M2-M5 add
hierarchy/synchronization, general memory/control, structured compute and advanced
kernels through that same path. M6 requires complete authority composition and
legacy-path retirement; M7 requires coverage-enforcing CI, qualified manifests,
accurate documentation and identical final publication to both repositories.
