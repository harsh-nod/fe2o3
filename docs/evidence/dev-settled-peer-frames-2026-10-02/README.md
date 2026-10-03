# Settled Source Peer List Frame Qualification

This checkpoint extends ordered native XGMI lists to prequeued compute and
full-frame D2H consumers when the source is already settled and its producing
result has been released. No source producer event is required. The earlier
[pending-source checkpoint](../dev-pending-peer-segments-2026-10-02/README.md)
remains a separate supported origin of the same list-frame abstraction.

Baseline: `f9e8581ff77d210972ac733ae9e4fcf267234c0c`. Frozen-source CPU checks,
all 14 native cases and independent receipt audits pass on 2026-10-03 UTC.
The directory date identifies the local work date, not the UTC execution date.

## Ownership and Composition

An additive, default-false backend capability enables the Context profile.
The version journal retains an ordinary current-version source read lease and
a whole-destination writer. The private immutable list identity binds whether
the source was settled or retained an exact compute producer. A missing
producer never turns a Pending or Unknown source into a settled source.

The unified backend retains an immutable destination-frame record with the
original endpoint identities, stream, bounded dependency rank and exact shared
descriptor plan. It does not fabricate a compute producer. Read-only consumers
may include initialized destination bytes outside the copied segments, but no
consumer can observe list success before whole-list completion and owner
restoration. Source/destination envelopes are not scalar produced coverage.

Existing accepted transfers outside the frame profile keep their legacy
behavior without receiving new consumer authority. In particular, depth limits
on the new provenance must not turn an otherwise valid legacy transfer into
an admission failure. Pending destination-writer chaining is not added here.

## Native Qualification

The bounded two-GPU matrix passes 14 cases, in both directions:

- Settled list -> compute -> scalar peer return -> D2H with 4, 65 and 4096
  ordered descriptors, including duplicates and overlapping writes.
- A four-descriptor compute consumer admitted after observed list publication.
- Settled list -> full-frame D2H with four descriptors.
- Pending-compute source regressions with four descriptors.
- Existing settled-list packet-spanning regressions.

In the new settled-source modes, the source setup launch is joined and its
result released before list admission. The source stream must have zero
retained results before and after admission; no source event is created or
supplied. The full pipeline has four
results and the direct readback has two. Public dependency events are released
after admission, and only the final readback stream drives a prequeued chain.

The new witnesses emit 52 exact fields, including explicit settled-source
provenance. Independent integer-quarter byte oracles retain the preceding
length-prefixed digest framing, so physically identical outputs have identical
digests across the two admission profiles. All logical source, destination,
compute, return and host bytes applicable to each mode are checked, including
untouched destination bytes and guards. The finite R57 V2 qualification
authority is unchanged; no application kernel authority is introduced.

Fresh admission selected MI300X GPUs 6 and 7, unique IDs
`0x10a254ce4987e716` and `0x53691ef168a0147d`. Each case has exact source-bound
local/remote executable hashes, byte-oracle comparison and pre/post endpoint,
memory and process-attachment checks. The independent native auditor replays
all 133 command receipts. This is point-admitted shared-host execution, not an
exclusive reservation or a performance comparison.

Both uploaded executables and `/tmp/fe2o3-settled-frames-20261002-xzONQD`
were removed. No owned executable process remains; selected-device VRAM and the
process roster returned to baseline. No device reset or foreign-process
termination occurred.

## CPU Qualification

On 2026-10-03 UTC the frozen candidate passes 2266 runtime tests, with no
failures and the same 32 hardware-dependent ignores; 20 deferred-chain and
three settled-list example tests; and 61 runtime doctests. Strict Clippy, the
no-default library check,
changed-file formatting, whitespace checks and all 32 source-control workflow
commands pass. The 21 new runtime tests comprise 11 Context and 10 backend
tests. The old envelope-rejection test now specifically checks that an envelope
without a retained frame cannot authorize a consumer.

Nine guard files change only 18 hash literals and seven source-inventory
literals. Independent AST checks preserve all other predicates and all four
extracted observer forwarding methods. All 76 referenced proof files and the
lower KFD crate are unchanged. Neither a solver nor the prior 1943-test KFD
suite was rerun. These source controls do not prove the new adapter.

The first diagnostic compile exposed test-only access to private provenance
fields. A narrow test accessor fixed those errors before the final source was
qualified. Its rejected output and superseded metadata proposals are retained;
they are not selected acceptance evidence.

## Evidence and Limits

`raw.tar.xz` contains commands, source manifests, outputs, selected receipts,
the source-guard metadata proposal and verifier, byte oracles, independent
auditors, and the staged candidate patch. `raw-manifest.json` authenticates
every archived file; `SHA256SUMS` covers this README, manifest and archive.
The aggregate audit checks exact test rosters against the preceding accepted
checkpoint, unchanged ignores and proofs, source identity, all native cases,
and owned cleanup. No executable files are distributed in the packet.

This increment does not complete A3, prove the whole native adapter, qualify
physical overlap or establish HIP/HSA performance parity. Application-kernel
evidence, pending destination/list chaining, native partial-failure isolation,
eight-GPU coverage and matched performance remain separate work.
