# Authenticated Completed Compute Data Import

Development evidence for persistent KFD ownership, not #182, A1/A2, full HIP/HSA
parity or performance acceptance. A sealed storage-to-compute input conversion
and pending-peer lifecycle integration remain open.

## Source

Implementation: `acde22f412b98405b4c39229eb4f5c8a43460634`, SSH-signed by
`harmenon@amd.com`. Root implemented and tested; a read-only agent reviewed the
production custody boundary and the genuine public-path fixture design.

Raw directory: `/home/harsh/.codex-tmp/fe2o3-compute-import-20260927-HMjuqKWr`.
Frozen archive: `receipts.tar.xz`, SHA-256
`70e56c0b258341baef363c9a8feb54f26f94f3db30b598e09bd47691c1ac4f5e`.
Archive comparison passed; source and test-binary identities are recorded.

## Qualification

All final commands ran against the same signed source:

| Lane | Result |
| --- | --- |
| Full KFD library suite | 1659 passed, 1 existing failure |
| Runtime default features | 1512 passed, 3 existing failures, 5 ignored |
| Runtime all features | 1621 passed, 3 existing failures, 28 ignored |
| KFD/runtime doctests | 89 passed |
| Strict all-target/all-feature Clippy | Passed |
| Minimal runtime, formatting, whitespace | Passed |
| Signature, source continuity, scalar-source continuity | Passed |
| Layout | Passed without changing any limit |

The KFD failure is
`target_debug_telemetry_v2::tests::credential_bound_channel_accepts_typed_failure_before_publication`
at `target_debug_telemetry_v2.rs:1173`, with `SocketAdmission`. Both runtime lanes
reproduce `cooperative_debug_telemetry_emits_only_bounded_logical_records`,
`failed_session_end_is_explicit_and_terminal` and
`pre_native_telemetry_failure_is_returned_and_poisoned`, each at
`authorized_execution.rs:1317` with `InspectSocket(PermissionDenied)`.
Both source files are unchanged from the preceding backing checkpoint.

Layout remains owner inline 136 bytes, owner heap 3328 bytes, ledger 3072 bytes,
and queue inline 38976 bytes. This is a layout bound, not a performance result.
MI300X DNS failed before connection, creating no remote artifacts. The final
issue-state probe also failed to connect to GitHub; it is retained as a failed
observation, not issue acceptance. Earlier in-turn readback found #182 open.

## Behavior

- Completed restoration consumes the original `Gfx942FixedDispatchDataV1` only
  after authenticating the completed owner/use, detaching slot and generation,
  exact storage identity, physical extent and original queue/pool/logical scope.
- An opaque owner-minted permit connects preflight to backing import. It is
  neither cloneable nor publicly constructible. Import has no raw-lease plus
  initialization-boolean/prefix API. Public ledger completion cannot mint it.
- Full typed initialization restores full logical coverage; cold returned data
  discards any earlier prefix. Neither path manufactures a content hash. Existing
  read/write digest rules remain separate.
- Three-binding restoration borrows and preflights the complete roster before
  consuming any member. Failure retains the original typed objects and leaves
  all native mappings detached, with every use quarantined rather than settled.
- Single-buffer failure now also retains typed data, not a stripped raw lease.
  Raw completed-restoration helpers are test-only reset controls.
- This is bounded metadata work with no additional allocation in the restoration
  commit path. No latency, throughput or comparison result is inferred from that
  source property.

## Test Scope

Five new test groups cover cold/authenticated import, stale-prefix invalidation,
exact completed-use substitution, foreign host/device storage, queue occurrence,
pool generation, zero/changed/oversized logical extents and unchanged rejected
content descriptors. Registered fixture storage is explicitly released.

The public single/three-buffer tests use real loader/planner preparation,
registered storage, dispatch generation and completion bookkeeping, and the
production public detach methods. The completion observation is CPU-injected.
Read digests survive valid read recovery; write digests do not. Every three-buffer
failure position is exercised, plus both cold and initialized single failures.
Healthy cleanup uses the normal retained-control release path. Terminal cleanup
uses an explicitly test-only simulated process teardown without resetting poison.
Each completed case verifies fixture storage/accounting release.

An early fixture execution rejected a mismatched queue VM before restoration;
the fixture was corrected to use its registered memory VM. Development compiler
and fixture failures are summarized separately in `development.md` in the raw
packet. Production checks and constructor visibility were not weakened.

## Verification Boundary

These tests do not execute GPU copies or kernels, establish firmware/machine-code
refinement, or prove full native adapter correctness. The existing initialized
prefix arithmetic proof sources are unchanged; their preceding campaign is in
the [backing packet](../dev-persistent-sdma-backing-2026-09-27/README.md).
That scalar proof is not a proof of this new concrete typed-data import.

Remaining composition work includes sealed quiescent InitializedStorage input,
origin-preserving bind/replay/cancellation, exact active predecessor DMA admission,
router-driven gates, durable mixed-depth history, native XGMI composition and
matched hardware qualification. Broader Worker, device-language, collective,
distributed and release gates are unchanged.
