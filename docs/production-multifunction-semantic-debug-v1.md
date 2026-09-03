# Production multi-function semantic debug V1

Status: implemented additive slice for GitHub issue #215 T1.

Production semantic debugging no longer treats a semantic function index as a
canonical KIR function ordinal. For a singleton kernel whose retained closure
contains ordinary Rust helpers, the compiler now carries an exact function
roster through Source Map V2, the pre-finalization semantic map, protected
lineage, independent finalizer replay, and debugger queries.

## Exact contract

`InertCanonicalMirToKirCorrespondenceEvidenceV5` keeps the frozen lossless V4
correspondence as an exact nested payload and adds one bounded canonical record
for each defined KIR function:

- semantic correspondence owner;
- semantic function index;
- absolute canonical KIR function ordinal;
- closed `KernelEntry` or `InternalHelper` role; and
- complete UTF-8 KIR function identity.

The producer replays the live semantic/KIR owner before encoding. Decode
rejects noncanonical order, duplicate owner/function keys, duplicate KIR
ordinals, unknown roles, nonzero reserved bytes, invalid lengths, truncation,
trailing bytes, and resource-limit violations. Module admission independently
requires every record to name the exact function, role, ordinal, and body, and
requires every defined function to occur exactly once. Sparse ordinals are
accepted only when the intervening module entries are declarations and every
definition is still covered.

V5 grants no compiler, proof, artifact, runtime, load, launch, attach, or
debugger-control authority. Existing V4 evidence and V1 debugger/map wire
formats remain decodable. The verifier validates the exact nested V4 proof
payload; the finalizer additionally consumes and replays the V5 function
roster. Transformation Map V2 identifies the exact V5 evidence bytes with the
additive `mir_kir_correspondence_v5` kind.

## Debugger behavior

Source Map V1/V2 emission uses the live `(correspondence owner, semantic
function)` key for statements, terminators, parameters, and synthetic
operations. KIR block IDs are resolved only inside the corresponding exact
function body. Source scopes and variables retain that function's absolute KIR
ordinal, so identical block IDs in an entry and helper cannot cross-wire.

The ordinary `debug_helper` fixture demonstrates the public workflow:

1. compile an ordinary `#[kernel]` that calls an `#[inline(never)]` Rust helper;
2. decode a compiler-produced Simulation Bundle V2 and observe two distinct
   function ordinals in Source Map V2;
3. stop at an exact helper KIR operation in the CPU simulator;
4. inspect the entry/helper call stack; and
5. resolve the helper KIR operation back to its exact compiler-bound source
   span.

This remains deterministic CPU execution evidence. It is not GPU execution or
performance prediction.

## Remaining boundary

- Multi-root protected capsules still return
  `MultipleKirFunctionBodies`; their V2 roster payload does not yet use this
  V5 singleton-helper envelope.
- Frozen V4 synthetic spans have no function owner and reject synthetics when
  multiple functions are present. A helper closure containing enum-payload or
  runtime-assert synthetic operations therefore remains unavailable until a
  fully function-qualified correspondence version is admitted.
- Semantic Debug Map V1 has statement locations but no MIR terminator
  location. Source Map V2 maps call terminator operations, but the
  Source-to-MIR-to-KIR transformation graph does not claim a call-terminator
  relation.
- The compiler emits exact source/KIR helper mapping. Source-to-ISA closure
  still depends on the separately authenticated finalizer/ISA catalog and the
  remaining T1 optimization-producer acceptance work.
