# Charged Result Staging and Native Peer Transport

Development checkpoint on baseline
`37ecc0405a341482df4aeac0980a75ad014d6589`. This closes completed-value staging
and qualifies its native transport composition, not A3 or production Worker
application admission. Primary integrated changes; three read-only agents reviewed
ownership, failure coverage and the next multi-device integration restriction.

## Implementation

- `ChargedTypedResultV1::{encode_into_v1,write_staging_v1}` exports data without
  consuming the original typed allocation or result credit. All ten sealed scalar
  types use exact little-endian encoding, checked lengths and caller-owned scratch.
- `GeneratedRuntimeChargedResultV1::{encode_completed_into_v1,try_stage_completed_v1}`
  requires the original completion's exact result gate and committed output.
  Contention returns `None`; foreign/unready/taken/poisoned output rejects before
  Context effects. Backend code runs only after releasing the output mutex.
- `RuntimeContextV1::write_host_visible_allocation_v1` requires one complete live
  HostVisible allocation and delegates to ordinary journal/backend settlement.
  Wrong kind, extent or identity rejects before effects. NoEffect remains distinct
  from Unknown/quarantined writes; the caller keeps the destination handle.

Scratch is outside the original result-credit budget. No encoding buffer is
silently allocated, and staging does not relabel generated DATA as PUBLIC memory.
For owner-engine composition, authenticate/encode on the caller and enqueue only
scratch/staging handles. Moving the original output into a rejected queue closure
would drop it. Enqueue is not copy admission; observers/timeouts do not establish
quiescence. Require successful H2D completion before ordinary peer admission.

## Qualification

The archive contains exact commands, environment, stdout/stderr, before/after
source inventories, executable identities, candidate source patch, metadata audit,
agent review and replay audit. Rust uses `nightly-2026-04-03`, four build jobs,
single-threaded tests, test opt-level 1 with debug/overflow checks, no incremental
build or debug info, and `FE2O3_HIP_SYS_DISABLE=1`.

`audit.py` accepts 55 receipts against the same 6382-file final source inventory.
The archived earlier `controls-final-*` run predates final fixture changes;
`controls-02-*` is the accepted final-source guard run.

- Host library: **313 passed, 5 ignored**; **33 doctests**. Ignores cover native/HIP
  hardware and environment-pinned parser fixtures.
- Runtime library: **2392 passed, 34 ignored**; **71 doctests**.
- Runtime examples: **170 tests**. Both libraries pass strict Clippy and
  no-default-feature checks. Package formatting and whitespace checks pass.
- **33 source-control commands** pass on the final source. Nine guard files change
  only 16 identity hash constants; all 76 selected proof/declaration/shared-body
  files are unchanged. No solver ran and no whole-adapter refinement is claimed.

Six new host tests cover all scalar widths, signed extrema, float payload bits,
wrong scratch/destination identities, contention and unavailable/poisoned output,
error/panic retention, NoEffect retry, retained readers, failed H2D rejection,
guard preservation, and dropping the original result before upload/peer completion.
Two runtime tests preserve exact diagnostic identity and journal settlement.
CPU gate fixtures exercise the shared matching body without fabricating a native
completion receipt; the two public authenticated entry points have compile doctests.

## Native Scope

MI300X GPUs 6/7 were independently observed idle before and after each direction:
`0000:c6:00.0` / `0x10a254ce4987e716` and
`0000:e5:00.0` / `0x53691ef168a0147d`. The reviewed selected-pair controller
retains eight endpoint observations and 18 bounded command receipts. Every owned
process group is absent after reaping. This is shared-host functional testing,
not an exclusive performance reservation.

The ignored host test uses the public native-peer constructor with deny-all
application compute authority. Its explicitly synthetic charged host result has
4097 varied u32 values. Per direction it performs two settled H2D uploads, one
native XGMI peer copy, complete source/destination readback with 32-byte guards,
unchanged original result/storage/credit checks, explicit logical/native shutdown,
and final result-credit refund. Both directions pass using the same 24,908,352-byte
ELF, SHA-256:
`d226bcf431b9fa182dd58b9f133a685667c0f1c18b463fc5fa7e19f2b29d70eb`.

Owned `/tmp/fe2o3-result-staging-20261003.DNKWEUXA` was removed after retrieving
evidence; cleanup found no owned processes. No other host files, jobs or GPUs
were cleaned or reset.

This does not qualify protected Worker execution, authenticated native completion
through staging, tracked owner-engine cancellation, pending H2D-to-peer admission,
zero-copy generated DATA promotion, physical overlap, or HIP/HSA speedups.
The [critical path](../../runtime-multi-gpu-critical-path.md) next removes the
single-backend restriction in normal host preparation/validation helpers while
preserving the original protected-evidence gate.
