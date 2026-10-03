# Pending Frame Forwarding

## Scope

This checkpoint extends `2b84c72bd` with pending segmented destination frames
feeding checked scalar peer windows, followed by guarded whole-target D2H.
The default-false `supports_pending_segment_frame_peer_copy_v1` capability
requires the Context version journal. The KFD backend retains exact latest-list
identity, event dependencies, immutable frame ancestry and original endpoints.
A frame is a separate source origin, never a fabricated compute producer.

Ordered overlapping lists preserve initialized gaps and bytes outside their
descriptor envelope. The successor target must be initialized with no pending
writer; its untouched complement remains available to the full-target readback.
Only successful, quiescent parents with restored original owners can publish
the successor. Public event release does not release dependency custody. Late
admission authenticates both native endpoint markers. Without the journal,
opted-in pending list-source admission is rejected before backend effects.

Corrupt successor routes quarantine immutable admitted endpoints and retained
frame ancestry, including a third device, without indexing untrusted route
fields or restoring uncertain ownership. Ordinary scalar behavior is preserved.
This profile does not add staged fallback, pending compute consumers of the
new frame-derived scalar, list-to-list forwarding or native-fault recovery.

## Native Qualification

All eight cases pass on MI300X GPUs 1/6/7, with forward and reversed rosters:
overlap and packet-tail shapes, each prequeued and late-admitted. Each process
uses production deny-all kernel authority, one Context and two changed rounds
on the same allocations. Each round admits two ordered source-to-frame lists,
one frame-to-target scalar window and one full-target D2H. Only the final
readback stream advances prequeued work. Late admission first observes the
latest list's retained native publication using the ordered roster, not a
direct native publication-ID observation.

Every source, frame, initialized complement, target and host guard byte is
checked. An independent Python byte oracle reconstructs both domain-separated,
length-prefixed round digests. Exact callbacks and native logical counts pass.
Across the matrix this covers 48 logical native peer copies, 112 peer SDMA
packets, 16 dependent D2H copies and 64 exact completion callbacks. No kernels
or modules are admitted. These rosters do not cover every directed device pair.

The selected-device controller pins the ELF by open file descriptor and hash,
bounds each native process and records process-group closure. Fresh UID/BDF,
activity, memory and attachment observations precede and follow every case.
The packet retains 104 command receipts and 48 endpoint observations. These
are point observations, not exclusive reservations or physical-overlap evidence.

The final rebuilt native executable is byte-identical to the tested executable:
`7034e7c5119be2b4082f22683289f8f3a3b0b6c4e77b18a9445d7665a28676da`.
After retrieval, a process census found no remaining owned processes and removed
only `/tmp/fe2o3-forward-window-20261003.VgOtR6J8`. Post-run selected endpoint
observations remained idle and unattached. No device reset or foreign
termination was performed.

## CPU Acceptance

Final-source qualification passes 2337 runtime tests with 32 existing hardware
ignores, 39 example tests and 61 runtime doctests. This includes 21 new Context
and backend tests and four new example tests. Strict Clippy, no-default library
checking, targeted formatting and all 33 source-control workflow commands pass.
The workflow includes 24 controller tests: seven new forwarding tests and 17
existing selected-pair tests.

The aggregate auditor authenticates the complete runtime test roster, exact
selected command arguments, source continuity, source-guard literal changes,
proof-body identity and local/remote executable hashes. Offline native replay
checks all 104 receipts, independently reconstructed byte digests, endpoint
observations, their ordering/freshness and process-group closure. The tested
and final rebuilt ELF are identical despite the narrow test-only path fix.

`qualification.tar.xz` retains the capture and audit tools, raw command receipts,
source identities, metadata audit, candidate source patch, native observations
and diagnostic attempts. Its internal per-file manifest and `SHA256SUMS` bind
the packet. Native ELF contents are identified by hash, not bundled.

## Evidence Boundaries

CPU tests cover exact event/latest-writer admission, no-journal rejection,
source and target leases, cancellation, marker/route corruption, immutable
ancestor quarantine, late admission and source disposal. Compute-backed list
sources have CPU coverage; this native witness uses settled source data only.

Existing proof bodies and predicates are unchanged. Nine source guards change
only 18 hash literals and seven inventory literals; 76 proof-closure files
remain identical to the baseline. Source controls are not solver executions or
a new theorem for the production adapter. Lower-level KFD tests are not freshly
rerun in this packet. No machine-code refinement or HIP/HSA performance parity
is claimed. A3 remains incomplete, particularly ordinary application authority,
broader native failure isolation and eight-device qualification.

## Retained Diagnostics

The initial focused run found six test-fixture errors: pending-result release,
a corruption helper selecting the wrong private reader representation,
restored-only inspection of untouched owners, and incorrect scripted SDMA
progress expectations. The fixtures were corrected without weakening admission
or ownership checks. The initial full suite then passed; subsequent review
strengthened immutable ancestor quarantine against corrupted route indices.

The first example-test build found a nested module-path error. The final change
adds only an explicit `cfg(test)` child path. Final-source checks are rerun;
the normal native executable rebuilds byte-for-byte identically. The auditor
accepts exactly that source difference between the native run and final tree,
not an arbitrary source-generation exception. Superseded attempts are retained
as diagnostics, not selected as final acceptance evidence.
