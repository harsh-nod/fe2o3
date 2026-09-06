# MI300X R38 bounded persistent-compute wait, 2026-09-05

Status: `Qualified correctness; no demonstrated performance gain`. R38 adds a
bounded persistent-compute wait and routes Ready through the fused R36 recycle
path with explicit failure custody. In the exact R26 V4 comparison below, KFD
E2E movement was mixed and small: 0.235% lower, 0.321% higher, and 1.183%
lower across the three slots. This does not demonstrate a performance gain.

## Provenance

- Exact baseline:
  [`f81d67fa603ecf23ebd101556b327ae80f13c5ec`](https://github.com/harsh-nod/fe2o3/commit/f81d67fa603ecf23ebd101556b327ae80f13c5ec).
- Exact R38 production:
  [`a1ea30cffbd24a5714a5fe0318b4231f42e98727`](https://github.com/harsh-nod/fe2o3/commit/a1ea30cffbd24a5714a5fe0318b4231f42e98727).
  Its production delta is confined to KFD `persistent_compute.rs` and
  `queue_live.rs`, plus runtime `kfd_backend.rs`.
- Exact R38 proof:
  [`5bd8c86aaf1ea249bdb342eaa503cf989ff0a733`](https://github.com/harsh-nod/fe2o3/commit/5bd8c86aaf1ea249bdb342eaa503cf989ff0a733).
- Host and workload: `sharkmi300x-1`, Linux `6.8.0-124-generic`, ROCm
  `7.2.4`, GPU 2 (`gfx942:xnack-`, unique ID `0xd2e26fef80cf5c33`), one
  1 MiB in-place `u32` transform, 10 warmups, 30 samples per backend per
  slot, and 10 iterations averaged into each sample.

The external baseline archive is
`/tmp/fe2o3-r38-baseline-f81d67fa-r26-20260905-evidence.tar.gz`, SHA-256
`05fc524c417a5d24d4d90dc39ae0118cd30e188b8c5c08b4929de88451e2a13a`,
size 104,268 bytes. Its set ID is
`78e29db541912fe8d723da52ce1d92e0148aeed0d38f4de9d15dff81424a58ed`;
manifest SHA-256 is
`fab61d2efc341588195da3f301cb6a22b6fe771c687f824fb2f638b96d38e75a`.

The external R38 archive is
`/tmp/fe2o3-r38-a1ea30cf-r26-20260905-evidence.tar.gz`, SHA-256
`bd29435f13f26081ae7f5ceb44430650c98ebb272087a5d666044d84ba5b9215`,
size 104,494 bytes. Its set ID is
`507f0eb73fd2f4d138c9f9105cd03fd2d62d5baec625d8b70af8d8e68dd66c85`;
manifest SHA-256 is
`d25921efdb1aff2d7bdf55f0398d6a71a9a712556a36f090bdc1aa5d1f76eb60`.
Both archives carry clean source/diff records and passing three-slot set
validation. They are external and non-durable.

## Measured path

The R26 compute interval launches the retained persistent dispatch, flushes,
and waits through the runtime. R38 gives that wait a bounded native path:
Pending stops at the deadline without recycle, timeout restores the active
Published state, and Ready enters the R36 completion/recycle composition.
Prepared, Materialized, Poll, and the residual non-SDMA routes remain outside
the new persistent-compute transition.

## Results

All values are host-monotonic p50 nanoseconds. Lower is better.

| Slot | Revision | KFD H2D | KFD compute | KFD D2H | KFD E2E |
| --- | --- | ---: | ---: | ---: | ---: |
| 0 | baseline | 207786 | 85145 | 46225 | 339615 |
| 0 | R38 | 206042 | 84952 | 46585 | 338817 |
| 1 | baseline | 209235 | 84555 | 46832 | 340654 |
| 1 | R38 | 209259 | 85518 | 46323 | 341746 |
| 2 | baseline | 207679 | 85448 | 50458 | 343551 |
| 2 | R38 | 208145 | 84555 | 46218 | 339488 |

The slotwise KFD compute changes were -0.227%, +1.139%, and -1.045%; E2E
changes were -0.235%, +0.321%, and -1.183%. The slot-2 baseline D2H value is
visibly unlike the other baseline slots, so its reduction is not treated as an
R38 effect. R38 remained 3.421x-3.455x slower than HIP E2E and
3.363x-3.398x slower than HIP compute.

## Correctness boundary

The production path observes completion before the deadline decision, restores
Published custody on timeout, and routes Ready through completion settlement
and recycle. Its typed failures retain Published, Completed, Recycled, or
teardown custody according to the stage. The benchmark validates every element
on every iteration and all three archived slots pass. That is bounded execution
evidence, not a proof of the native implementation.

The independent R38 finite model proves 19 obligations and rejects six pinned
mutations. It covers first observation at zero deadline, bounded Pending
histories, exact timeout restoration, Ready composition through R36, defensive
preflight handling, all eleven internal failure stages, missing-queue custody,
and residual route selection. The executable model checks 756 model-admitted
present-queue cases, not 756 proven production-reachable histories.

The authenticated runner at `5bd8c86a` reports 857 obligations and 339
rejected mutations; transcript SHA-256 is
`c5041f339995a13b67128c5f8a35c1172199bfa0306ed3ddf356be8b3a37f915`.
The model is affine at its valid executable boundary. It does not preserve or
prove custody for explicit drop or invalid model inputs.

## Archive integrity

The retained external containers can be checked without rebuilding or rerunning
the workload:

```bash
printf '%s  %s\n' \
  05fc524c417a5d24d4d90dc39ae0118cd30e188b8c5c08b4929de88451e2a13a \
  /tmp/fe2o3-r38-baseline-f81d67fa-r26-20260905-evidence.tar.gz \
  bd29435f13f26081ae7f5ceb44430650c98ebb272087a5d666044d84ba5b9215 \
  /tmp/fe2o3-r38-a1ea30cf-r26-20260905-evidence.tar.gz | sha256sum --check
gzip -t /tmp/fe2o3-r38-{baseline-f81d67fa,a1ea30cf}-r26-20260905-evidence.tar.gz
```

## Claim limits

The revisions ran sequentially with three slots on one MI300X and one fixed
workload. There is no randomized revision order, confidence interval, size
sweep, concurrent application workload, multi-device workload, or hardware
counter study. The proof has no Rust-to-Verus or native refinement theorem and
proves no real timing, driver, firmware, hardware, progress, parity, or
performance property. R38 is a correctness tranche; these measurements do not
support a speedup claim.
