# Uninitialized Coherent Insertion Custody V1

## Status

R111 locally accepts N3-L3-C uninitialized coherent insertion above accepted R110
`356523e6ea8c61c70c6812762aeaab33506de6e2`, with
[retained evidence](evidence/local-r111-uninitialized-coherent-insertion-2026-09-12/README.md).
Acceptance covers the shared settlement path, constructed scripted-engine
fixtures and two direct-session APIs' preflight/missing-engine boundaries. It
does not establish authenticated formal correspondence, native execution,
performance or aggregate-memory qualification.

Primary owns edits, integration and serialized validation. Native and Resources
workers reviewed the production path and constructed oracles; Admission supplied
the public preflight matrix. Their next independent packets remain
[N3-L3-D, C1 and V2](runtime-swarm-next-packets.md).

## Production Boundary

The two existing direct-session APIs now share `data_insertion.rs` settlement:

- `insert_host_visible_fixed_dispatch_data(index, requested_bytes)` selects
  explicit insertion.
- `allocate_host_visible_fixed_dispatch_data(requested_bytes)` requires the
  remembered hole. It does not append when no hole exists.

There are no corresponding selected-lane facade methods. This packet does not
add public APIs, change device-memory insertion, publish a kernel or adopt data
into a generated submission.

`CoherentAllocationCustodyV1` holds a one-shot flag and the actual optional
mapped coherent allocation. Preparation invokes existing allocation followed by
mapping. It does not invoke the initialized-memory helper, copy source bytes,
read storage or fabricate initialized authority. Extraction returns only
`HostVisibleUninitialized` storage.

The lower allocation transition still owns size validation and the released-entry
model checkpoint. In particular, a zero-size request enters allocation before
being rejected. The outer root must not hoist that rejection. Earlier validation,
ledger capacity, index selection, reservation and loan-entry precedence remain
the shared sequencer's responsibility.

## Custody And Ordering

The completed mapped token stays in the root outside the original memory-model
loan. The sequencer retakes the model, borrows the completed identity, commits
the ordered identity ledger and count, and only then extracts the data owner.

Before completion, lower pending-allocation or typed-transition custody owns any
admitted native prefix. An empty outer root does not replace that custody. After
completion, failed retake or commit entry transfers the actual mapped output to
the original engine's terminal transition. The shared sink accepts the raw
mapped token, preserving both R110's initialized and R111's uninitialized paths.

Terminal retention is output-only `LiveInsertion` with default native progress:
retention itself performs no native operation or cleanup. Existing occupied
terminal custody is preserved, the engine remains quarantined, and no per-call
allocation or additional model retake is introduced. Direct wrappers retain the
whole terminal parent before exposing an error or resuming the original panic.

## Validation Boundary

R111 reuses R110's original constructed primary/auxiliary engine,
preparation loan, scripted native backend, accounts and independent model oracle.
A typed test bridge selects initialized or allocation-only preparation; it does
not implement a replacement backend or turn copy into a no-op.

The allocation-only matrix covers successful explicit and remembered-hole
insertion, native/currentness prefixes, projection failures, exact lower owners,
retake and commit failure, capacity and validation precedence, opening failure,
missing Complete, denied retry, and first-panic versus closing-error precedence.
Root tests cover both accounting modes, one-shot preparation/extraction,
zero-size lower entry, and earlier map-failure custody. Public tests separately
exercise preflight and missing-engine classification through the two real APIs.

Oracles distinguish exact requested extent, mapped/unmapped state, identity,
native arguments, no copy/readback, ledger order, model transitions, original
records and retained charges. Scripted backing zeros are not initialization
authority. Later ordinal cases relocate the same actual auxiliary; they do not
qualify three independent native lanes.

Seventeen source and ten auxiliary gates pass. GNU/musl each pass 2,657 tests
with five ignored across 48 harnesses. Six frozen/restored suites pass
19/19/14/10/8/4; fourteen compiled negatives fail their exact behavioral oracles,
and all 5,676 source identities match after restoration. The closed collector
and independent audit verify 217 raw artifacts. Nineteen new test functions
comprise eighteen dynamic tests and one wiring guard; two negatives exercise
reused model-loan substrate. Dynamic commit-entry custody and source-guarded
full commit ordering remain distinct evidence. No solver, SSH or GPU work was run.

## Preliminary Attempts

The initial test builds exposed a missing test-trait import (E0405) and two
boolean gate markers incorrectly tested as Options (E0599). They ran no tests;
both failed attempts and their source snapshots are preserved in the evidence
archive, not counted as passing qualification.

The subsequent root/public run passed seven tests. The first constructed run
passed nineteen but is excluded from qualification: `r111-matrix-format` ran
from `2026-09-12T21:46:18.189Z` to `21:46:41.235Z`, while
`r111-constructed-tests` started at `21:46:28.701Z`, a 12.534-second overlap.
The unchanged test-run endpoint hashes do not establish continuously immutable
source. This exclusion does not assert that the compiler consumed mixed versions.
Preserve both attempts rather than replacing their raw timestamps or results.

Review subsequently added 1/17/4097-byte successful requests, explicit
4096/4096/8192-byte backing charges, and before/after-retake panic cases. A third
preliminary failure was strict Clippy's unused model-oracle helper finding;
the existing wrapper was restored without suppressing the warning. Fresh
serialized tests and strict linting subsequently passed on the frozen source.

The raw source-campaign UTC interval is 3,024.820 seconds, while its recorded
monotonic elapsed duration is 3,182.442 seconds. Preserve both observations;
they do not establish a clock cause or performance result. The later evidence
chain separately checks actual UTC floors, predecessor hashes and monotonic
ordering. Source hashes exclude documentation and establish checked snapshots,
not a proof of continuously immutable bytes.
