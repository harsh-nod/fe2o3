# Static Publication Terminals in Semantic MIR V31

V31 adds two inert compiler-terminal records for the reviewed, private
`fe2o3_device::static_publication` functions. It adds no kernel argument kind,
host descriptor, allocation identity, or runtime admission. The payload keeps
the existing exclusive `DisjointSlice<f32, Index1D>` read/write ABI; flags keep
the shared `&[AtomicU32]` read/write, shared-atomic ABI introduced by V29.

## Source Contract

The public `publish_once_128` facade consumes the payload, checks that both
allocations have exactly 128 elements and the launch has 256 invocations,
and assigns one producer and one consumer to each cell. Admission must also
prove the exact 128-invocation workgroup layout, roles, bounds, exclusive
payload custody, and closed effect roster. These properties do not follow
from a matching signature or from the serialized records alone.

The private producer performs an ordinary payload write followed by a
System-scope Release store of READY (2). The private consumer first performs
its own System-scope Release store of REQUEST (1), then a System-scope Acquire
load of the same flag. Only a load of READY enables the ordinary payload
read. Otherwise the consumer returns without reading the payload.

The consumer's preceding REQUEST prevents an initially READY flag from
authorizing a read. For a successful attempt, the required proof combines
atomic coherence with the closed current-writer roster: the observed READY
must come from this attempt's producer, whose Release publishes its earlier
payload write. There is no new zero-initialization or epoch premise. Existing
initialized-memory, atomic eligibility, lease, lifecycle, and quiescence
requirements remain in force. An attempt need not succeed; this is neither
a progress guarantee nor a workgroup residency assumption.

## Retained Records

| Tag | Operation | Retained Types | Source Arguments |
| --- | --- | --- | --- |
| 72 | `StaticPublication128PublishF32` | `payload`, `flags`, `result` | payload, flags, cell, value |
| 73 | `StaticPublication128TryReadF32` | `payload`, `flags`, `result` | payload, flags, cell |

The cell is the admitted 64-bit `usize`. `flags` is the full immutable
reference-to-slice type, not the unsized slice type. Its element must carry
the retained `CoreAtomicU32` marker and pass the existing exact V29 storage
layout validation. This immutable Rust borrow is not readonly allocation
authority and does not imply nonaliasing with other shared arguments.

Both terminals return `PublicationAttemptF32`, an ordinary `repr(C)` pair
of `status: u32` and `value: f32`, size 8, alignment 4, and offsets 0 and 4.
Statuses are NotReady (0), Published (1), Ready (2), and Invalid (3).
Every status other than Ready carries positive zero. The status itself
grants no permission for a later memory access. Retained field identities,
scalar-pair representation, size, alignment, and offsets are checked; actual
function ABI passing remains separately retained and validated.

The importer authenticates the actual diagnostic-item `DefId`, exact source
path, and complete reviewed device-provider closure for both private
terminals and the result type. The atomic element is authenticated against
the pinned core `Atomic<u32>` definition, generic argument, and layout, not
its spelling or interior-mutability properties. User-defined lookalikes
cannot manufacture these producer identities.

## Compatibility and Validation

V31 allocates only tags 72 and 73. Tags 0 through 71 retain their existing
encodings, and requests without publication terminals retain their previous
minimum wire version. V30 and earlier reject the new terminal tags; exact
version decoders reject mismatched envelopes. V16 through V26 remain retired
and V27 remains independently reserved.

Focused schema tests cover producer and consumer signatures, atomic
nominality, pointer mutability and metadata, address space, result layout,
argument order and arity, exact tags, truncation, unknown tags, old-version
rejection, and preservation of V30 bytes. These tests concern inert records;
actual Rust source identity, ranked custody, exact effect correspondence,
and the happens-before conflict proof are separate obligations.

Decoding V31 does not enable ordinary tensor publication, change generic
race analysis, or qualify a protected runtime launch. In particular, the
existing protected shared-atomic runtime gate remains closed. GPU execution
and numerical evidence require a separately bound, reviewed artifact and
runtime path; neither is established by schema admission.

## Evidence Boundaries

Middle-end evidence V6 uses its own version, domain and policy. Its common
source, ranked, coverage, typed-semantic and eight-pass facts share a bounded
body codec with V5 without constructing a nested V5 admission. A publication
record retains both allocation origins, all six ranked sites (including the
read guard), the full 256-invocation domain, and 128 potential and discharged
cell pairs. The historical V5 live constructor rejects the new publication
operations and proof; its canonical bytes and decoder meaning are unchanged.

Formal-memory evidence V5 separately retains the exact semantic, ranked and
KIR identities; source-to-ranked-to-KIR effect correspondence; full physical
geometry; and every original affine conflict with a closed discharge reason.
The original canonical formal-obligation receipt is embedded byte-for-byte,
using its existing validated V1 or V2 format. Raw, discharged and unresolved
conflict counts remain distinct. Even when affine analysis reports zero raw
conflicts, a publication kernel retains a nonempty publication proof with
the ranked 128-pair obligation. Historical formal V4 remains unchanged and
does not admit publication records.

Canonical decoding is inert. It validates encoding, internal correlations,
receipt identities, exact witnesses and discharge counts; it does not
authenticate a new source program or replace live owner replay. The live
constructors require the original retained owners and reject mismatched
publication presence. Neither evidence version opens protected shared-atomic
runtime admission or attests to a concrete GPU execution.

## Source Validation and Remaining Gaps

The live middle-end V6 and formal-memory V5 constructors are implemented and
reviewed; component proofs and canonical codecs have tests. Four admitted-model
integration tests construct or reject the matched live pair through production
semantic, ranked and KIR owners, and reject cross-wired source, graph, site and
KIR identities. The literal-bound controls admit `flags.len() == 128` with
`cell < 128`, but reject all three flag atomic accesses when the declared
length is 127. These fixtures alone do not authenticate Rust source or
establish a GPU result.

The matched compiler run in `evidence/publication-v31-source-v5` passes the
real Rust publication source checks, including checked LLVM emission with
exact atomic effects and an independent consumed readonly input. The five
publication checks, five readonly-filter checks (which include the readonly
publication case), and three indexed-atomic checks pass. Source, compiler
binary, test executable, HEAD and status postchecks bind that run to its
captured dirty-source snapshot; it is not a clean release or native artifact.
The compiler library passes 723 tests and PLIRON passes 1319 tests.

Two bounded analysis changes connect these source checks to the existing
model proof. Ordinary rank-one bounds can combine an exact true equality
`length == C` with a later true `cell < C`, requiring the equality on every
incoming path and retaining the existing edge, definition and budget checks.
Publication role analysis explores both successors of an exact two-way
analysis split; dependency summaries do not become numeric facts. The exact
five effects, six sites, acyclic control flow and full 256-invocation role
checks remain mandatory. Wrong literals, bypasses, changed block arguments,
cycles and extra payload effects remain negative controls.

The publication source fixtures currently have no
supported functional reference staging, and the required authenticated
Verus runtime is not installed on the test host. An inert invocation record
does not supply either obligation.

The ordinary source positives request checked LLVM only. A separate V3
handoff negative first requires the same source to pass LLVM extraction,
then requires the exact final-lineage rejection for missing authenticated
MIR-to-PLIRON Verus execution. Earlier source or lowering failures do not
satisfy that test. This negative now passes the matched source run; it is not
evidence of a successful V3 publication handoff. Full V3 publication lineage,
a publication HSACO and GPU execution remain unqualified.

Completing this gap requires a supported functional/reference contract for
the conditional publication operation and actual per-compilation proof
execution using the pinned retained runtime. No functional or runtime gate
is bypassed, and the component evidence is not promoted to a production
handoff or GPU claim.
