# Checked U32 Entry-Prefix Relation

`check_captured_checked_u32_prefix_v1` checks one straight-line entry prefix in
the semantic MIR and current V8 KIR retained by the same
`ProductionSemanticKirOwnerV1`. It accepts only that owner's borrowed
`ProductionCheckedU32AddCaptureV1`, not a replacement module, caller-built
correspondence map, sampled register state or serialized proof receipt.

This is an application-admission prerequisite. It neither changes multi-GPU
transport nor grants kernel load/launch authority. The existing conditional
machine MOV/ADD checker remains separate: its induction/block-parameter entry
contract is not interchangeable with this function-entry contract.

## Supported Profile

The captured operation must be in the function entry block. Up to 256 source
statements, including the checked add, 4096 locals and 128 unadjusted direct u32
arguments are accepted. Before the captured `Copy(local) + u32 literal`, the
source may contain u32 constants, copies and Nops. Moves, storage effects,
projections, memory, calls and other arithmetic reject. The KIR entry must have
no block parameters and contain only constants before the captured checked add.

Argument roles, source ABI types, actual KIR parameters and retained lowerer
bindings establish the input basis. Source assignments and emitted KIR values
are folded independently. The source fold handles mutable-local redefinition
and reads a copied value before writing the destination, including self-copy.
KIR definitions have distinct dense slots; duplicate IDs and parameter
collisions reject. Exact statement spans include zero-operation copies/Nops.
The terminal value and overflow IDs, operand, literal and types are checked.

Acceptance establishes a conditional, terminal-only relation: equal source/KIR
input values in the reported basis imply equal captured operands, and hence
equal checked-add value/overflow. It covers the first traversal from function
entry, not a later backedge or repeated visit. Dead intermediate constants need
not match. `evaluate` computes the conditional result; its caller-supplied input
vector is not an authenticated observation of an execution.

The relation borrows the original compiler owner. Temporary storage is bounded
by the profile; retained storage is proportional to argument count. Whole-owner
function/correspondence scans remain linear in owner size, plus bounded argument
matching and ordered-map work. This is not an end-to-end allocation/RSS bound.

## Proof Boundary

Ordinary Rust and Verus include the same executable argument-basis initializer
and origin-fold macros. Initialization accepts exactly positional argument rows
with bounded, distinct source locals and matching dense KIR scratch length. It
clears source scratch, installs paired argument origins and leaves every unmapped
source local uninitialized. This takes O(locals + arguments) time and constant
extra space beyond the caller's scratch; duplicate detection uses that scratch.

The basis denotation composes with an uninitialized KIR suffix and the existing
fold theorem. A successful symbolic fold agrees with an independently defined
concrete u32 fold for every valid common input vector. Equal initialized terminal
origins then imply equal concrete values. The existing shared widened-add body
proves the modulo-2^32 result and carry. Failed initialization or folding may
modify private temporary state; the checker discards it and returns no relation.

The typed source-statement normalizer now also shares its executable body with
Verus. It consumes the actual borrowed semantic MIR types, locals, statement and
retained operation count, without a caller-built normalized view. Its exact
acceptance contract covers Nop/count0, unprojected typed Copy/count0 and bounded
u32 Constant/count1. Direct AST statement evaluation agrees with the accepted
step; that result composes with the origin-step theorem, including self-copy and
uninitialized-read failure. Individual normalization uses bounded indexed reads
and no allocation.

The proof models every variant of the six inspected enums and every inspected
field. Irrelevant payloads are explicitly erased; sequence length/order are
preserved. Source-token guards bind the real schemas, getters, typed-ID macro and
forwarding paths, including outer attributes and declaration nesting. This is a
reviewed structural bridge, not a theorem about Rust parsing, layout or the
entire semantic MIR model.

The proof does **not** verify ABI discovery, KIR normalization, whole-prefix/span
assembly, actual rustc extraction, LLVM/ISA lowering, physical ABI/EXEC entry state, control-flow
continuation, memory effects, completion or protected compiler provenance.
Those obligations cannot be replaced by the source hashes used to bind tests
and solver runs to reviewed files.

The [qualification record](evidence/dev-checked-u32-prefix-2026-10-03/README.md)
contains CPU tests, lifetime compile failures and the source-bound proof campaign.
The tests construct admitted semantic MIR and invoke the real current lowerer;
they are not an ordinary Rust-source extraction witness. The pinned campaign
runs in the authenticated runtime-model CI workflow and the local Verus lane:

```sh
VERUS=/absolute/path/to/pinned/verus \
  bash crates/fe2o3-verifier/verus/run-checked-u32-prefix.sh
```

## Genuine Rust Extraction

The opt-in `FE2O3_EXTRACT_CHECKED_U32_PREFIX_V1=1` wrapper mode now retains this
exact owner inside the active rustc transaction. It selects one reachable helper's
entry-block `Copy(u32 local) + u32 literal` from the actual imported source, then
consumes the owner once through capture-aware SSA/KIR lowering. It does not run
ordinary SSA construction first or reconstruct an owner from a diagnostic.
Missing or ambiguous sites, multiple roots and unsupported prefixes reject.
Other extraction modes are mutually exclusive; ordinary wrapper passthrough,
metadata binding and mandatory overflow checks remain unchanged.

The [real-source qualification](evidence/dev-rustc-checked-u32-prefix-2026-10-03/README.md)
uses the pinned compiler and an ordinary safe Rust helper reachable from a typed
kernel. The first accepted variant retains two source copy statements, including
a reassignment, and resolves to argument 0. A second variant has an empty prefix
and resolves to argument 1. The integration test requires both actual profiles,
distinct source/KIR identities, boundary evaluations and four specific rejections.
No reference-policy metadata or caller-built semantic MIR supplies these cases.

The stderr JSON is a diagnostic observation, not protected compiler-origin or
launch evidence. Its sample input vectors are hypothetical conditional evaluations,
not observed device arguments. The live owner is dropped before releasing its
capture-storage charge. No LLVM, HSACO or simulation artifact is published.
This mode bounds added capture/checker work; it is not a whole-compiler RSS bound.

The ignored compile-only witness runs in `scripts/ci-local.sh rocm-compile` and
can also be selected directly with the pinned nightly's rustc-dev/rust-src installed:

```sh
cargo test --locked -p rustc-codegen-fe2o3 \
  --test production_extraction_driver_v1 \
  checked_u32_prefix_extraction_v1::genuine_checked_u32_prefix_extraction_uses_actual_sources_and_rejects_unsupported_profiles \
  -- --ignored --exact --nocapture --test-threads=1
```

## Next Functional Gate

Argument-basis initialization now has a shared-executable proof, including an
acceptance-completeness contract and an always-reject negative control. The
[basis qualification](evidence/dev-checked-u32-basis-2026-10-03/README.md)
records 18 verified obligations, 16 intended logical mutants and eight runner
controls. Exact source pins bind the actual row schema, wrapper and caller;
they are not a proof of how the adapter discovers those rows or KIR slots.

The [typed-source normalization qualification](evidence/dev-source-normalization-2026-10-03/README.md)
adds exact source-statement normalization and direct-denotation composition.
It does not prove how ABI rows, KIR slots or complete prefix spans are discovered.

Next prove ABI discovery and KIR/whole-prefix assembly. Then compose physical
entry, continuation and memory obligations into protected per-invocation admission.
Only that admission can authorize a general application-kernel multi-GPU witness.
The A3 milestone, issue #182 and broad HIP/HSA parity remain incomplete.
