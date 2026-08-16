# Exact MoE Top-2 Private Structural Record V2

This increment adds a private, inert whole-module diagnostic record to the
successful `collected-moe-top2-v1` rustc admission. It adds no public verifier
or candidate schema and proves no source-to-Kernel-IR semantic refinement.

## Sealed Live Inputs

The producer is a child module of the MoE rustc admission module. Its only live
entry point accepts an opaque witness whose fields are private. The sealer also
requires an opaque `ValidatedMoeTop2AuthorityV1` by value. That token's field is
private to a separate validation submodule, and only `validate_authority` can
construct it after checking the complete authority predicate. A raw or merely
nonzero `MoeTop2AuthorityV1` cannot satisfy the sealer's type. The parent
admission path obtains the token only after it has authenticated:

- source contents retained in rustc's loaded `SourceFile` and their SHA-256
  identity, without reopening the source path;
- compiler semantics, trusted definitions, and the exact root instance;
- the rustc-derived opaque checked `FnAbi` identity and a bounded structural
  projection of the checked header, result, and eight pair-mode arguments;
- the admitted complete portable-MIR module and its semantic identity; and
- the already validated `MoeTop2KernelIrV1` and `MoeTop2ProfileV1` values.

The `FnAbi` projection is not a complete representation of rustc's `FnAbi`.
The opaque identity commits the checked Rust calling-convention discriminator,
variadic flag, fixed and actual argument counts, unwind flag, ignored-return
mode discriminator, return size and ABI alignment, and, for every argument,
layout size/alignment, pair-mode discriminator, and both components' complete
checked regular-attribute bits, extension, pointee size, and optional pointee
alignment. The projection makes only a bounded structural subset readable in
this diagnostic.

The portable-MIR summary counts functions, roots, helpers, blocks, statements,
terminators, CFG edges, external imports, root arguments and locals,
assignments, calls, indexed places, repeated values, and observed binary
operator kinds over the complete imported module. These are pinned diagnostics,
not routing semantics or refinement evidence.

## Acyclic Session Binding

Construction is deliberately one-way:

1. The rustc admission validates source, session semantics, trusted definitions,
   root instance, `FnAbi`, MIR, KIR, and profile.
2. It constructs and hashes the final source authority without referring to the
   structural record.
3. It seals the authenticated values together with compiler semantics identity,
   trusted definitions identity, root instance identity, and final source
   authority identity.
4. The private producer derives a record that commits those bindings.
5. Receipt consumption validates the authority first, then requires every
   record binding to equal that consumed authority.

The record points to the completed authority; the authority does not point back
to the record. There is therefore no digest cycle, and a detached source,
`FnAbi`, MIR identity, or compiler session cannot be supplied to the producer.

## Canonical Entries

The validated KIR and profile are encoded into 31 ordered aggregate entries.
Those entries collectively serialize every current field of
`MoeTop2KernelIrV1`, `MoeTop2ProfileV1`, and their current nested descriptors,
resources, arrays, and policy records. An aggregate entry may contain several
leaf fields; this is not a claim of one table entry per leaf.

Each entry has a unique name, membership bits, and a canonical value. The KIR,
profile, ABI projection, effects projection, and routing projection identities
are domain-separated hashes over selected entries from that table. The private
classifier rejects exact name, order, removal, duplication, membership, and
value drift before snapshot comparison.

The final record digest frames the rustc-loaded source contents and identity,
the FnAbi identity and structural projection, every whole-module MIR summary
field, the complete aggregate canonical table, and every same-session binding.
A readable snapshot beside the implementation was mechanically captured from a
successful live rustc admission. Focused tests mutate the internal classifier
candidate; no caller-constructible provenance object is exposed.

## Boundary

This record establishes only that one authenticated rustc admission observed
the pinned structural inputs and selected the pinned validated KIR/profile
encoding in the same source-authority session. It does not establish that MIR
values or effects simulate the KIR routing state machine.

The first unproved boundary remains a mechanically checked value- and
effect-preserving simulation from the authenticated portable-MIR CFG to the
exact MoE KIR, including failure paths, loops, indexing, FP32 comparisons,
writes, and ordered routing transitions. Issue #106 remains open.

The record grants no Worker V2, LLVM, ISA, artifact, load, launch, runtime, GPU,
or hardware authority. It proves no IEEE FP32 or OCML semantics,
logical-to-machine addressing, generalized memory safety, or race freedom.
