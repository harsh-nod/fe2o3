# Pending-Source Ordered Destination Lists

## Scope

This checkpoint extends `8191d0081` to compose pending full-write compute sources
with ordered native peer-copy lists into the same initialized destination.
The default-false `supports_ordered_pending_compute_peer_copy_segments_v1`
capability is separate from pending-source and settled-destination ordering.
Supporting those two operations independently does not imply safe composition.

Each list retains its exact source producer independently of its exact latest
same-stream destination predecessor. Immutable source and destination-frame
identities remain distinct. Source reads and whole-destination queued writes
keep their existing journal rules. Settled and compute-backed origins can mix;
neither list bounding envelopes nor dependency events become scalar write
coverage or application-kernel authority.

Failed or cancelled dependencies cannot promote a destination version. Unknown
journal lineage and uncertain-native fail-stop custody remain conservative.
No new source-language, machine-code or whole-adapter proof is claimed.

## Native Design

The unchanged finite R57 authority supplies two different source payloads:
source 0 produces C with the first gate, and source 1 produces D with the second.
The selected source gates remain pending when the two destination lists are
admitted. Unselected source gates complete and release their results during
setup. No expected source output is installed from the host. Setup readbacks
inspect only settled source outputs: reading a selected pending output would
consume the native H2dReady state required by its future compute gate. Exact
completed uploads and empty setup streams are still checked for every source,
and final full-byte source verification is unchanged.

The latest destination list feeds a full-frame Read-only compute consumer,
then a windowed native return peer and guarded full-frame D2H. All public events
are released after dependent admission; caller descriptor storage is overwritten
after submission. Only the final readback stream drives a prequeued pipeline.
Late modes first observe the oldest retained native list before admitting the
consumer. Selected source results may settle during that explicit seeding.

The witness adds these modes to `gfx942-runtime-deferred-peer-chain-smoke`:

```text
--pending-destination-segments-compute <first|second|both> <4|65|4096> <S0> <S1> <D>
--late-pending-destination-segments-compute <first|second|both> <4|65|4096> <S0> <S1> <D>
```

For one or two selected pending sources, setup launches decrease from four to
three or two, pipeline launches increase from one to two or three, and exact
completion callbacks increase from five to six or seven. Native logical copies
still advance from zero to three. Independent oracles check both full source
buffers, gathered frame, computed result, returned owner and host guards.

The accepted 21-case matrix covers each source selection, prequeued/late consumer
and forward/reversed three-device roster at four descriptors per list; two
65-descriptor cases and one two-list 4096-descriptor case; and six settled copy,
disposal, packet-spanning, compute and scalar-gather controls. It is not every
directed device pair or a matched performance campaign.

## Qualification Status

CPU generation 04 and metadata proposal 05 are accepted by independent readback.
All 2,316 runtime tests pass with 32 unchanged hardware ignores. Four examples
pass 42 tests; 61 runtime doctests, strict Clippy, default/no-default feature
checks, formatting and all 32 source-control workflow commands pass. The runtime
roster adds exactly 13 Context and 9 backend tests to the prior checkpoint.
Both normal hardware witnesses are built and identified from this same source.

The prior 1,950-test KFD qualification is reused through exact executable,
dependency-source, shared-body and archived-receipt identity; it is not a fresh
KFD build or test run. The metadata rebind changes 16 hash literals and 7
inventory constants in 9 guards. All 76 associated proof files are unchanged.
No new solver execution or whole-adapter refinement is claimed.

All 21 native cases pass on MI300X GPUs 1/6/7 in forward and reversed order.
The 189 transport receipts bind fresh UID/BDF, activity, memory and process
checks, exact executable hashes, independent report/byte oracles and explicit
cleanup. Both owned binaries and their scratch directory were removed, no owned
process remained, and selected-device memory and whole-host process observations
returned to baseline. GPU 0's foreign workload was not disturbed. These checks
are not an exclusive reservation, physical-overlap proof or performance result.

The 4096-descriptor command took 231.025 seconds including setup, compute,
transfers, full-byte verification and shutdown, within its original bound. This
is not isolated copy throughput. Read-only inspection found cached descriptor
plans, but serial per-packet publication/fencing and full paired currentness
checks. Profiling and verified shared-owner packet windows remain follow-up work.

The normal deferred witness SHA-256 is
`01fabca2e33fdc10f792e960598d31a459aec6066669d26c9004a58fa7311b75`;
the copy-only witness SHA-256 is
`7ae3a7fefdbf17d672394b54b84c8499c1c8ecb14c1c5cbe6772365de4154de8`.
The raw packet contains selected receipts, independent CPU/native/aggregate
auditors, controllers, failed diagnostics and the exact candidate source patch.

## Development Diagnostics

The initial full runtime suite recorded three failures, all in new fixtures:
a source event from neither endpoint correctly rejected with WrongDevice, a
return target still retained as a compute input correctly rejected Busy, and a
producer result still retained by stream order could not be released. Fixture
corrections preserve those rejections, use an independent return target and
dispose only the restored source allocation until the result retain settles.
Production custody rules were not weakened.

CPU generation 02 was deliberately interrupted after independent review found
the pending-output setup-readback problem above. Its nonzero receipt is retained
as a diagnostic, not a test pass. Earlier compiler/type and source-metadata
attempts are likewise preserved separately from final-source qualification.

## Remaining Work

General application-kernel admission, source-to-machine refinement, native
post-arm fault isolation, all-device coverage and matched HIP/HSA performance
remain open. The finite R57 authority and CPU argument encoding do not discharge
the production application contract. A3 and the broader parity goal remain
incomplete.
