# Pending Frame List Forwarding

## Scope

This checkpoint extends `a2951fe9d8191e757198574d9f12b80535712c1d` with a
copy-only three-GPU pipeline:

```text
A --ordered lists--> B --ordered list--> C --whole-target D2H
```

The independent default-false
`supports_pending_segment_frame_peer_copy_segments_v1` capability requires the
version journal, exact latest source event and immutable retained frame. A frame
is a distinct source origin, not compute authority. Source and destination
bounding envelopes may differ. Initialized gaps outside earlier descriptor
envelopes remain readable. The target must be initialized with no pending writer;
pending compute consumption of this new frame-derived list remains excluded.

The backend reuses the existing native list queue and packet planner. Exact plan
identity, decreasing dependency ranks, original owner pairs and both occupancy
markers are authenticated. Ancestor child-resource occupancy does not grant
destination-write ordering permission. Only successful parent completion and
original-owner restoration permit successor publication. Settled receipts may
outlive disposed operational ancestors; uncertain effects remain retained.
Validation avoids duplicate recursive walks; a 16-hop CPU counter regression
checks one common validation visit per node. No lower-level KFD transport or
packet-arithmetic implementation is changed.

## Native Qualification

All eight cases pass on MI300X GPUs 1/6/7: overlap and packet-tail shapes,
prequeued and late admission, with forward and reversed routes. Each process
uses production deny-all kernel authority, one Context and two changed-content
rounds on reused allocations. Each round has two source-to-frame lists, a
four-descriptor frame-to-target list and guarded full-target D2H. Prequeued work
advances only through final readback. Late cases first infer latest-list native
publication from the ordered roster, then use final-readback-only progress;
this is not direct physical fence-ID evidence.

Every original source, frame, initialized complement, target and host guard byte
is checked. Overlapping successor descriptors select nonuniform parent-written
bytes; reversing descriptor order changes the expected target. Caller descriptor
mutation after admission cannot change the retained list. An independent Python
oracle reconstructs both domain-separated, length-prefixed round digests.
The matrix checks 48 logical native lists, 160 peer SDMA packets, 16 dependent
D2H copies and 64 exact original completion callbacks. No kernels are launched.

The selected-device controller pins the ELF by file descriptor and hash, bounds
every process and records process-group closure. It retains 104 command receipts
and 48 fresh before/after endpoint observations. GPU 0 was not selected. These
are point observations on a shared host, not exclusive reservations, every-pair
coverage or physical-overlap measurements.

The native ELF is built from exactly the final qualified source:
`c198b024e0f04b5140f65a838c616cadec18e81d227b2033a48b464bbb97c79e`
(20848664 bytes). Results were retrieved before cleanup. A hash-bound cleanup
helper found no remaining owned processes and removed only
`/tmp/fe2o3-frame-segments-20261003.KMkzCpOq`. Selected post-run endpoint
observations were idle and unattached. No resets or foreign termination occurred.

## CPU Acceptance

Final-source qualification passes 2352 runtime tests with 32 hardware ignores,
40 example tests and 61 runtime doctests. Fifteen new runtime tests cover
Context admission and native custody, immutable plans, event/rank/lease drift,
source disposal, multi-hop child occupancy, cancellation, failed/Unknown parents,
queued-reader submission rollback and bounded validation. Strict Clippy,
no-default library checking, targeted formatting and all 33 source-control
workflow commands pass. The workflow includes 25 smoke-controller tests.

The auditor checks the complete current source inventory, exact commands and
environments, artifact target/profile/path identities, complete runtime roster,
native executable hash, independent byte digests and every native receipt.
Observation ordering/freshness, process-group closure, uploaded cleanup-helper
identity and campaign/retrieval/cleanup chronology are replayed locally. Two
abort-isolation tests print a doubled inherited test banner; the auditor accepts
only those exact named doubled successful lines before checking roster equality.

`qualification.tar.xz` retains raw captures, tools, source identities, metadata
audits, the candidate source patch, native observations and diagnostic attempts.
Its internal manifest and `SHA256SUMS` bind the packet. ELF contents are not
bundled. Native agents independently reviewed Context, backend and witness/audit
changes; primary owned edits, integration and execution.

## Evidence Boundaries

Nine source guards change only 18 hash literals and seven inventory literals.
All 76 executable proof-closure files remain identical to the baseline. The
four producer-journal observer bodies and shared input fold/validation bodies
are unchanged. Source controls and metadata rebinding are not new solver runs
or a proof of the new Context/backend adapter. Lower-level KFD tests are not
freshly rerun in this packet. No general application-kernel authority,
machine-code refinement, post-arm native fault recovery or HIP/HSA performance
parity is claimed. A3 remains incomplete.

## Retained Diagnostics

Early compilation caught a missing plan import and a test attempting to clone a
deliberately non-Clone plan. The latter now reconstructs an equal plan under a
fresh Arc to test identity rejection. An initial focused test incorrectly
unwrapped legitimate quiescent cancellation; the fixture now checks its Failed
status. The first full run passed 2349 tests and failed one compatibility fixture
that reused allocation 4, which its later compute launch already consumed.
Moving the new copy to unused allocation 6 restored that contract. The next
full run passed 2350 tests. Two final queued-reader tests bring the accepted
final-source run to 2352. Superseded attempts are diagnostics, not acceptance.
