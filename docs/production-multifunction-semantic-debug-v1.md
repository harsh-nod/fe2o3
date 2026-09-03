# Production multi-function semantic debug V1

Status: implemented additive slice for GitHub issue #215 T1.

Production semantic debugging no longer treats a semantic function index as a
canonical KIR function ordinal. For singleton and multi-root kernel closures,
including one exact semantic helper shared by several roots, the compiler
carries an exact function roster through Source Map V2, the pre-finalization
semantic map, protected lineage, independent finalizer replay, and debugger
queries.

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

For multiple roots, the existing `MultiRootProofRosterTranscriptV2`
correspondence envelope carries one existing `F2MRCOP2` payload per root. The
shared `MultiRootCorrespondencePayloadV2` decoder gives that payload a bounded
typed contract. It retains function-qualified blocks, statements, terminators,
synthetic spans, and parameter bindings. Compiler preparation replays all of
those records against the live correspondence owner. Finalizer admission
independently resolves every function symbol to one absolute KIR ordinal,
verifies semantic root identities, checks complete contiguous operation
coverage, and rejects reordered roots, substituted identities, entry-function
reuse, overlapping spans, and ambiguous helper reuse. One helper may appear in
several root payloads only when its semantic identity, KIR symbol, physical
ordinal, body, parameter bindings, statement spans, and role are byte-for-byte
the same. Transformation Map V2 uses
`multi_root_mir_kir_correspondence_roster_v2`; V1-V5 bytes are unchanged.

## Root-instance custody

`ProductionSemanticDebugInstanceCustodyV1` is an additive finalizer-owned
sidecar. It binds exact Source Map V2, finalized Semantic Debug Map V1,
semantic MIR, canonical KIR V7 projection, and correspondence bytes. Its
function and statement occurrence records retain the correspondence owner
without changing or duplicating the physical Source, MIR, KIR, or ISA graph.

The canonical binary decoder returns
`InertProductionSemanticDebugInstanceCustodyV1`. It is byte- and count-bounded,
checks the exact remaining encoded size before allocation, rejects unknown
roles and nonzero reserved bytes, and revalidates every claimed record identity
and graph reference. The inert type exposes no occurrence records or query
methods. Promotion requires complete equality with a fresh finalizer replay, so
self-consistent owner/function claims and matching content hashes cannot create
admitted custody. Every admitted owner has exactly one distinct kernel entry. A
physical function may be associated with several owners only when every
occurrence names the same semantic `InternalHelper`; kernel-entry reuse remains
rejected.

Forward queries return every owner of a semantic helper. Reverse queries return
every owner of a physical KIR function or KIR statement node. Both are ordered
by canonical owner-qualified coordinates. Artifact-only and direct caller map
admission report `CorrespondenceUnavailable`; legacy V4 production
correspondence reports `LegacyCorrespondenceV4`. The sidecar is association
evidence only and grants no execution, load, attach, or debugger-control
authority.

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

The ignored ordinary two-kernel production extraction test compiles in normal
CI. Running it requires the pinned protected Verus runtime and AMD rust-src
installation; an environment without those inputs cannot establish the
ordinary-source production exit criterion.

## Remaining boundary

- Shared-helper custody does not synthesize one physical KIR or ISA node per
  root. Consumers join the root-qualified sidecar to the one authenticated
  physical node. Context-specialized helper clones require distinct exact
  physical functions and are not inferred from this association.
- Multi-root `F2MRCOP2` synthetic spans are function-qualified and admitted.
  Legacy singleton V4 synthetic spans remain unqualified; this change does not
  reinterpret or upgrade those frozen bytes.
- Semantic Debug Map V1 has statement locations but no MIR terminator
  location. Source Map V2 maps call terminator operations, but the
  Source-to-MIR-to-KIR transformation graph does not claim a call-terminator
  relation.
- The compiler emits exact source/KIR helper mapping. Source-to-ISA closure
  still depends on the separately authenticated finalizer/ISA catalog and the
  remaining T1 optimization-producer acceptance work.
