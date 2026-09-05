# MI300X Worker V5 Portable Progress Qualification, 2026-09-04

Status: `Measured` for the exact bounded test below. This is native liveness and
data-correctness evidence for one operation, not a HIP/HSA parity or performance
result.

## Provenance

- Commit: `0631c5be944db6ee42f178345df2e9078fb69ec8`
- Host: `sharkmi300x-1`, Linux `6.8.0-124-generic`
- Device: GPU 6, AMD Instinct MI300X `gfx942`, unique ID
  `10a254ce4987e716`
- Device load immediately before the run: 0% GPU use, 0% allocated VRAM
- Device load after cleanup: 0% GPU use, 0% allocated VRAM
- ROCm: `7.2.4`
- Rust: `1.96.0-nightly (55e86c996 2026-04-02)`, installed as the pinned
  `nightly-2026-04-03` toolchain
- Test: `kfd_worker_v5_qualification::worker_v5_kfd_composite_progresses_sixty_three_plus_two_d2d_without_manual_flush`

## Result

The ignored test passed once in 20.40 seconds after compilation. It created a
copy-only KFD Worker V5 child, submitted one 256 MiB same-device D2D copy, and
used `event_future_with_progress` without a caller `flush_stream`. The transfer
plan was exactly 65 SDMA packets in a 63-packet window plus a two-packet
continuation, with a 2,048-byte final packet. After completion, untimed readback
matched the deterministic absolute-offset pattern in both the source and
destination physical allocations.

The run used a unique checkout and Cargo target directory below `/tmp`, an
outer 900-second timeout, and an exit/signal cleanup trap. The checkout and
target directory were absent after the command returned, and the selected GPU
returned to 0% use and 0% allocated VRAM.

This result does not measure latency or bandwidth, compare HIP or HSA, prove
fairness or general liveness, authenticate firmware semantics, or refine the
Rust implementation to the independent Verus model.
