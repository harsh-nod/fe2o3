# Native Peer Subrange Qualification

Status: CPU and finite native qualification passed on 2026-10-02 UTC.
Implementation baseline: `bdc9b7b3c000b4b9094b7e4226e6eb198b6d57d6`.

## Implementation

- Native peer copies admit independently checked source and destination logical
  windows, including unequal allocation sizes and nonzero, different offsets.
  Both complete original owners must remain initialized PUBLIC DeviceLocal
  allocations. A range does not create a new owner or grant access to physical
  padding outside the logical allocation.
- An immutable `Gfx942ComputeXgmiCopyWindowV1` binds both logical extents, offsets
  and positive transfer length to the existing bounded packet plan. Admission
  checks logical bounds before ownership extraction; native packet submission
  independently checks each mapped GPU address and packet end for overflow.
- Persistent transfer progress retains whole mappings, original owner identities,
  model loans and terminal custody. Packet completion does not restore either
  owner early; restoration follows retirement of the complete requested range.
  Existing equal, full-extent entry points remain wrappers with their original
  admission requirements.
- Runtime ordinary and directed native selection retain and revalidate the exact
  window without staged fallback after native selection. Existing resource-owner
  exclusion, dependency, cancellation and fail-stop rules remain in force.
  Pending compute-output peer admission still requires the existing full-extent
  profile; this increment does not broaden kernel or compute authority.

## CPU Qualification

| Check | Result |
| --- | --- |
| Final-source combined KFD/runtime library test build | Accepted |
| Focused KFD compute-XGMI group | 68 passed |
| Focused runtime compute-XGMI group | 88 passed |
| Complete runtime library suite | 2,145 passed, 32 ignored |
| Complete KFD library suite | 1,934 passed, no ignores |
| Directed witness unit tests | 11 passed |
| Deferred/live-sharded witness unit tests | 6 and 5 passed |
| Strict Clippy, feature, formatting and witness build receipts | All accepted |
| Source-control workflow | All 32 commands accepted |

The complete KFD roster is partitioned into four disjoint heavy construction
shards and one remainder; the runtime suite has a separate complete roster.
Qualification requires exact test-name multisets, unchanged executable hashes,
unchanged source snapshots and terminal owned process groups, not only summary
counts. Separate Cargo and direct-test locks permit checks alongside immutable
test executables; selected executable hashes must also match at final closure.

New CPU coverage includes independent logical bounds, physical-padding rejection,
address overflow, unequal extents, packet tails and ring wrap, exact original
owner restoration, retained native prefixes, and error/unwind boundaries.
Scripted native completion tests qualify real ownership and packet construction
paths, not physical DMA or numerical GPU execution.

Two initial runtime test failures were fixture assumptions corrected without a
production change. Those failures and diagnostic captures with source drift are
retained but excluded from final-source acceptance. The final CPU selection is
`attempt-01/{build,tests,checks}`; it contains 4,101 passing library/example tests
and the unchanged 32-test hardware-ignore roster, authenticated against the prior
accepted campaign. Focused reruns are not added to that total.

## Arithmetic Proof

The new window campaign checks three arithmetic functions using the executable
macro bodies invoked by production: independent window bounds, exact projected
packet offsets and their admitted logical end bounds. Both positive runs verify
all three functions, eleven required mutants fail their proof obligations, and
all seven runner controls pass. The selected packet is `proof/proof-attempt-02`.
The existing bounded packet planner remains the relative-offset authority.

This is an arithmetic proof, not formal refinement of the complete Rust wrapper,
runtime adapter, Linux route-currentness checks, mapping transitions, DMA engine,
model loans or cleanup. Source identity and closure checks bind the evidence to
the recorded implementation; they do not replace semantic proof. The verifier's
190-file release closure is checked before and after. The separate source-control
replay is `source-ci/attempt-02-after`: all 32 commands pass. Its reviewed metadata
refresh changes only 20 hash literals and seven inventory counts in 12 files;
76 historical proof-closure files are unchanged. No historical proof transfer to
the new native adapter is claimed.

## Native Qualification

All 28 MI300X cases pass: 12 new subrange cases and 16 current-source controls.
The campaign runs from 16:06:41 to 16:20:49 UTC on 2026-10-02, using GPUs 5/6/7
for directed copies and GPUs 6/7 for the two-device compute controls.

The new cases cover chain and fanout in both device orders, with and without
dependent D2H, including late peer admission after observed producer publication.
Each uses two changed-content rounds in one Context, 8,388,581-byte copies with
a three-packet plan and 37-byte tail, logical owner lengths
8,388,838/8,389,350/8,389,734, peer offsets 17/131/509, and host readback offset 97.
Every logical source/destination byte is compared, including untouched destination
guards; D2H cases also compare the complete host allocation and its guards.
The separate Python oracle checks the length-framed full-buffer digests. It is
independently implemented from the Rust witness, not a second GPU observation.

The directed executable SHA-256 is
`b301c4332d7bfb7a19bf3d24cd038fac29cc18aae2d7c6515e66f1f6bd32dcda`.
Local and remote executable hashes match before and after execution. The native
completion count advances 0/2/4 in each directed case; this is a runtime logical
copy counter, not an independently measured hardware packet counter.
Fresh UID/BDF, PID/attachment, activity, memory and host-memory checks bracket
every case. Owned processes are absent, all three uploaded executables and
`/tmp/fe2o3-native-peer-subranges-20261002-POshuA` are removed, and the final GPU
memory/process observations match the initial baseline. No fault was injected.

The host is shared. Point-in-time admission is not an exclusive reservation or
performance isolation. Only owned files/processes may be removed; no device
reset or foreign-work termination is part of this campaign.

## Provenance

Working evidence root:
`/home/harsh/.codex-tmp/fe2o3-native-peer-subranges-20261002`.
The `raw.tar.xz`, `raw-manifest.json` and `SHA256SUMS` retain controllers, exact
commands and clean environments, source/ELF manifests, complete roster partitions,
outputs and exits, arithmetic positives/negatives, source-control receipts,
hardware admissions and owned cleanup. `candidate.patch` records source changes
against the baseline above. Final retained-receipt selection is `audit-02.json`,
which also checks per-case admission and cleanup chronology. Earlier failed or
superseded attempts remain in the bundle; they are not added to acceptance totals.

This increment does not establish general application-kernel authority, arbitrary
partial compute-output pipelines, device concurrency, eight-device qualification,
fault isolation or HIP/HSA performance parity. Historical proofs and prior native
campaigns remain separate evidence with their original scope.
