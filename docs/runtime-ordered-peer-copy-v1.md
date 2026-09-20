# Ordered Peer Copy V1

## Contract

`RuntimeContextV1::peer_copy_segments` is an optional local SPI extension, not a
Worker wire operation. It accepts one source allocation, one destination
allocation, and 1 through 4096 ordered descriptors. Each descriptor contains
source-relative offset, destination-relative offset, and nonzero length. The
source and destination bounding envelopes may have different lengths.

The facade validates the complete list before backend entry. The KFD adapter
also validates the complete list against allocation extents, endpoint access,
device/stream binding and the native linear packet limit. No large-segment
splitting is implicit. Order and duplicates are preserved. Destination overlaps
use serial last-writer-wins semantics; untouched bytes must remain unchanged.
This is not an atomic transaction: failure can leave an applied prefix.

4096 is a host metadata policy bound, not the native 63-ticket ring bound. One
three-u64 descriptor vector has at most 96 KiB of payload. The facade and backend
temporarily hold separate snapshots during admission; the backend retains one.
Admission and descriptor execution are linear in descriptor count. Existing
backend custody validation additionally examines logical ownership indexes.

One logical submission retains one source-envelope read lease and one
whole-destination journal writer. Its domain-separated identity includes the
branded stream, both branded envelopes, and the entire ordered descriptor list.
An intermediate native completion does not settle the writer, deliver completion
callbacks, or wake dependent submissions. Cancellation is possible only before
the first publication attempt, never between completed segments.

## Native Execution

The gfx942 XGMI adapter uses existing native batch-scope APIs. It submits one
segment, waits for that exact singleton ticket, recovers the same mapping pair,
then proceeds to the next descriptor. At most one native ticket is outstanding.
An invocation performs full opening and closing currentness checks; each native
submit and wait retains the existing operational checks inside that scope.
Neither whole-host topology discovery nor endpoint observations are skipped.

`poll` performs at most one segment's publication/completion step. `flush`
progresses the current eligible segment; later segments become eligible only
after predecessor completion. First-segment publication is the logical
operation's publication point. A failed step is reported as a quiescent flush
error, not success. No background progress is introduced.

`wait` uses one absolute deadline, including preparation, for the entire call.
One initial publication/completion scan is allowed at expiry, matching the
existing aggregate progress convention. After a successful segment, expiry
prevents publication of the next descriptor. Mandatory closing checks can finish
after the deadline. Pending returns only after closing, retaining either an exact
ticket or the mapped pair and completed-prefix cursor. Retrying an outstanding
ticket waits for it rather than republishing the segment.

Success requires the complete prefix, successful closing currentness, reusable
mapping restoration, and only then logical settlement. Recoverable submission
failure restores mappings and settles a failed logical result, even when an
earlier prefix exists. Retained publication errors, failed currentness, or
ambiguous completion quarantine custody. Unwinding while native authority is
moved is fail-stop, following the existing aggregate adapter.

The initial implementation reserves its directional logical submission domain:
it requires no active work in that direction, and ordinary copies in that
direction are rejected until it settles. The opposite direction remains
independent subject to existing allocation/dependency ownership checks. Ordinary
aggregate APIs reject sequence handles. Existing diagnostic capture formats are
invalidated for sequence progress rather than populated with misleading records.

## Verification Scope

Production admission and cursor transitions import the pure R74 Rust model.
Tests explore all reachable abstract states for counts 1 through 4, boundary
counts including 63/64/65/4096, every relevant range/overflow rejection, and
scripted publication/wait/close faults at each position in a four-segment list.
Facade tests cover snapshot immutability, unequal envelopes, duplicates, ordered
overlaps, canaries, source immutability, one writer/reader, event settlement,
callbacks, prepublication cancellation, and partial-effect failure.

The independent Verus artifact has ten parameterized obligations for arbitrary
accepted counts and five expected-negative controls. It proves abstract cursor
ordering, irreversible publication history, cancellation exclusion, no reopening
or publication after recovery, and successful-close gating under modeled observations. Its ghost
successful-close flag records the observation supplied by the adapter. Invalid
Rust transitions return `None`; the specification stutters on invalid actions.
Rust/Verus correspondence is reviewed, not an executable refinement proof.

This does not prove ticket authenticity, actual currentness observations, DMA
effects, Rust/native refinement, hardware liveness, or performance. The native
smoke example checks both directions, 1/65/4096 segments, poll/flush continuation,
cancellation, byte-exact canaries and explicit native shutdown. It must run only
after fresh shared-host identity/activity admission.

## CPU Qualification

With pinned nightly `2026-04-03`, all features, optimization level 1, debug
assertions and overflow checks enabled, the GNU and musl runtime library suites
each passed 1190 tests, with 20 ignored hardware-specific tests. The musl smoke
example's two CPU tests also passed. The no-default-features context/XGMI subset
passed 195 tests. Warnings-denied all-features Clippy passed for the runtime
library, tests, and smoke example.

The three focused R74 Rust model tests passed. The independent R74 Verus artifact
verified ten obligations, and each of its five expected-negative controls failed
its named obligation. The registered 691-file negative-quality audit passed.
The full authenticated proof runner and native hardware qualification are still
pending at this source checkpoint; targeted results do not substitute for them.

## Remaining Work

- Native hardware qualification and a source-bound evidence packet.
- Matched useful-segment workloads against HIP/HSA, including admission and
  completion costs. No new speedup or parity claim follows from this API.
- Owner-engine convenience wrappers with descriptor budget accounting, graph
  sequence nodes, and explicit negotiated Worker transport support.
- More permissive scheduling-domain coexistence and native multi-packet
  publication are separate optimizations. This serial version still constructs
  singleton wait rosters and does not claim optimal packet throughput.
