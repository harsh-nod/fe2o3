# Gfx950 Model Admission And Read-Only Planning

Base: `2f6b4ec991a92ddc5550b3ee3d16b59d9048511c`.
Qualified on 2026-10-06 UTC, October 5 in America/Los_Angeles.

Three native agent lanes implemented model integration, queue geometry and
proofs; the primary reviewed, integrated and performed live observations. This
checkpoint adds no native gfx950 device capability, allocation, mmap, queue,
dispatch or SDMA authority. No GPU workload, HIP/HSA comparison, whole-runtime
formal proof or A0-A7 milestone exit is claimed.

## Implemented Scope

- A closed device-target profile remains attached to model-only correlation and
  admission. Separate exact gfx950 projection tuples do not broaden gfx942.
  Caller-supplied digests and disabled XNACK remain untrusted model premises.
- Queue-target/device-profile equality is checked both during admission and
  invariant replay. Tests reject reciprocal substitution even when resource,
  domain and generation identities agree. Both profiles have positive controls.
- `Gfx950AqlQueueResourcePlanV1` is an inert, separately branded topology plan.
  It admits only the observed fallback sizing policy, computes checked CWSR,
  header/debug/shadow offsets and cannot substitute for a gfx942 plan. The public
  parser rejects unreviewed kernel size properties. Independent positive kernel
  override precedence is exercised only in private derivation tests and the C
  oracle, not admitted by the public profile.
- The independent C oracle validates retained KFD/ROCr source hashes, queue and
  AQL layouts, all eight header byte images and 24 shadow offsets. The retained
  geometry is 12,288 control bytes and 22,687,744 context bytes per XCC, with
  181,829,632 total mapping bytes. This is not a production header writer.
- Existing gfx942 resource types, planner source, manifest and C oracle remain
  unchanged. Source hashes identify reviewed text, not the loaded kernel/ROCr
  binaries or a source-to-machine refinement.

New geometry manifest SHA256:
`e838edc8d388cf4339ef9427175f4074ec2944af19394dd559bcbcc729fc0cbc`.
Preserved gfx942 geometry manifest SHA256:
`37d45132916d2ecefdec8f53ecab817cbdbaa9b9863440353163bd460626ab02`.

## Qualification

| Check | Result |
| --- | --- |
| Runtime-model library | 1,138 passed, 19 existing ignored |
| Runtime-model doctests | 29 passed |
| Runtime-model all-target Clippy | Passed with `-D warnings` |
| Full KFD library | 1,979 passed, 2 live tests ignored; 837.07 seconds |
| KFD doctests | 4 compiled examples and 50 compile-fail controls passed |
| KFD library Clippy | Passed with `-D warnings` |
| KFD UAPI suites | 56 passed |
| Independent C oracle | Passed; source-hash tampering rejected |
| Runtime and host libraries | Locked, offline downstream checks passed |
| Scoped Verus positives | Identity 8, projection 11, queue 16: 35 obligations passed |
| Affected Verus negatives | All 22 rejected with the required diagnostics |
| Proof audits | Source checks, negative-quality self-test, 694-control roster and pinned verifier closure passed |
| Live MI350 read-only planner | 1 passed, all eight GPUs checked |

The model proofs establish the stated predicates and transition invariants, not
whole-Rust-adapter or native executable refinement. They retain exact target
binding through six modeled queue transitions. Existing verifier flags, limits,
classifiers and diagnostic acceptance rules were not weakened.

The pinned Verus is `0.2026.08.09.92f466f`. Its closure check covered 190 files
and 129,019,839 bytes. `TRANSCRIPT_SHA256` pins the expected future full-run
transcript; it is not evidence that the broad runner completed.

## Live Observations

An observation-only C probe opened KFD and each DRM render node read-only,
issued version/device-information queries and closed the handles. All eight
devices reported KFD 1.18, DRM 3.64.0, acceleration enabled, PCI `0x75a0`, chip
revision 0, external revision 80, PCI revision 0 and family 141. Sysfs confirmed
kernel `6.8.0-124-generic`, amdgpu `6.16.13`, source version
`703B1127E578BC5D4BD6615`, SPX/NPS1 and `mes=0`, `sched_policy=0`, `cwsr_enable=1`.
This probe did not observe XNACK, apertures, reset subscriptions or native
currentness, and did not allocate, map or submit work.
Raw device observations are retained in [device-observation.json](device-observation.json).

Separately, the exact ignored test
`gfx950_queue_resources::tests::live_gfx950_plans_all_eight_devices_without_native_authority`
passed in 0.01 seconds, checking all eight plans and rejecting the gfx942 planner
for each. That test used only sysfs/procfs discovery, with no device opens or
ioctls. Its stripped executable matched locally and remotely at SHA256
`a06d20ca6ea8952b0b1f2407636b7665aff8108eab8e327f4e63faef578b7b95`.
An empty KFD process snapshot is only a point-in-time observation, not a GPU
reservation. No hardware performance result follows from these checks.
The exact command and output are in [live-planner.json](live-planner.json).

## Broad Proof Blocker

The full runner stopped before broad verification at an existing
`context_read_invariant_v1.rs` source-pin mismatch. All 776 source checks were
audited; this was the sole mismatch, and both that source and its pin are
unchanged from the base. Actual source SHA256 is
`1071534ab340e26aa54c2f456b16cc01677982cd4b042f2c821359e1f579b29c`;
the stale pin is
`ec1c25f0948adefa79306ea39a5e93da66f324e4a465c2aafdb4434b32d6d779`.
History attributes the difference to the previously added `spinoff_prover`
attribute, not this change.

A private staging copy with the candidate pin was used for requalification.
The unchanged reader proof passed 156 obligations, checker self-tests passed,
and the campaign passed its 157-obligation positive-before control and two
mutations. The third mutation, `reader_count`, timed out under the unchanged
120-second child/130-second wrapper limits. Thirteen further mutations and the
positive-after control were not reached. The timeout process group was absent
after cleanup. The repository pin remains untouched; no broad-suite pass is
claimed. Retained logs include this incomplete attempt and its source comparison.

Earlier development logs also retain the fixed SHA digest-format compilation
error and projection-proof resource failures. The final proof refactors
equivalent predicates and explicit instantiations without increasing solver
limits or dropping the original history obligation.

Changed Rust files pass scoped formatting, shell syntax checks pass and
`git diff --check` passes. A package-wide formatting check reports unrelated,
unchanged retained-pair examples/tests; its raw output is retained. Those files
were not reformatted, and no package-wide formatting-clean claim is made.

## Evidence And Next Work

`qualification.tar.gz` retains raw CPU/proof logs, exact commands, live JSON
observations, the C probe source, changed-source capture, reviewed source files
and the next-slice doorbell audit. Cargo outputs, uploaded executables and the
compiled C oracle are excluded. Its SHA256 is recorded in `SHA256SUMS`.

The exact owned MI350 scratch directory was removed and a separate absence check
passed. No native allocation or queue was created. All three exact owned local
build/source scratch directories were removed after archive verification and
their absence checked. Re-extracted queue sources passed the oracle again, and
all 35 changed-source hashes matched. The installed verifier and unrelated user
files were retained.
See [remote-cleanup.json](remote-cleanup.json) for remote removal evidence.

Next: separately branded native gfx950 device/currentness, memory and queue
custody, followed by a bounded barrier-only completion/destroy probe. The
[native work order](../../runtime-gfx950-native-admission-work-order.md) splits
these lanes and preserves target agreement before effects. The broad reader
proof campaign also needs independent repair/requalification before its pin or
the full-suite status can change. Peer/SDMA, machine semantic authority and the
admitted two-GPU application remain subsequent gates.
