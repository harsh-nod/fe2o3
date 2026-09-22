# Inspecting the pinned AMD ISA catalog

The `inspect_amd_isa_catalog_v1` host example makes the existing immutable AMD ISA metadata usable from a terminal. It observes the catalog; it does not assemble a kernel, admit a source marker, choose an encoding, continue compilation, run a simulator, or qualify hardware behavior.

This is an example target in `fe2o3-amdgcn-model`, not a new compiler subcommand or public API. Cargo discovers it without a manifest change. Its output is bounded human-readable text, not a new persisted schema or portable capture format.

## Usage

Use the repository's pinned Rust toolchain and already provisioned offline dependencies:

```sh
cargo run --offline --locked --jobs 2 -p fe2o3-amdgcn-model --example inspect_amd_isa_catalog_v1 -- --help
cargo run --offline --locked --jobs 2 -p fe2o3-amdgcn-model --example inspect_amd_isa_catalog_v1 -- gfx942
cargo run --offline --locked --jobs 2 -p fe2o3-amdgcn-model --example inspect_amd_isa_catalog_v1 -- gfx950 --list
cargo run --offline --locked --jobs 2 -p fe2o3-amdgcn-model --example inspect_amd_isa_catalog_v1 -- gfx942 V_XOR_B32
cargo run --offline --locked --jobs 2 -p fe2o3-amdgcn-model --example inspect_amd_isa_catalog_v1 -- gfx942 BUFFER_INVL2
```

Cargo builds the host example. The inspector itself only uses compiled-in immutable metadata, its arguments, and standard output/error. It does not read an XML/archive or kernel file, invoke a compiler child, evaluate a condition expression, emit machine code, contact a service, or use a GPU.

`TARGET` accepts exactly `gfx942` (CDNA3 metadata) or `gfx950` (CDNA4 metadata). Target aliases, uppercase names, leading/trailing whitespace, and feature suffixes refuse. For example, `gfx942:xnack-`, `MI300X`, and `GFX942` are not catalog target names even if another compiler interface accepts related syntax.

Instruction names also use exact upstream spelling. `BUFFER_INVL2` resolves because the pinned catalog explicitly declares it as an alias of `BUFFER_INV`; the report retains both the queried and canonical names. Lowercase `v_xor_b32`, inferred suffix `V_XOR_B32_e32`, and unknown names refuse instead of guessing.

With no second argument, the report summarizes the target, content identities, inventory counts, condition-definition gaps, and source-marker family coverage. `--list` adds every canonical instruction name exactly once, sorted by name. A name or explicit alias adds that family's declared flags, aliases, encoding alternatives, and operands.

## Five different questions

Every report keeps these claims separate:

| Question | What the inspector establishes |
| --- | --- |
| Catalogued? | The exact canonical name or explicit alias appears in this pinned target's metadata inventory. |
| Encodable? | Not established. An opcode and an encoding definition are metadata, not encoding selection, operand legality, or an encoder. |
| Authorable? | Encoding-level authoring is unavailable in the reviewed overlay. A separately reported family-level source-marker observation is narrower and does not select an encoding. |
| Simulated? | Encoding-level simulation is unavailable in the reviewed overlay. Catalog membership is not simulator support. |
| Qualified? | Proof and hardware qualification are unavailable in the reviewed overlay. These reports are not compiler, proof, load, or hardware receipts. |

The pinned overlay currently identifies six reviewed `gfx942` u32 source-marker families: `V_ADD_U32`, `V_AND_B32`, `V_MOV_B32`, `V_OR_B32`, `V_SUB_U32`, and `V_XOR_B32`. The inspector obtains that observation from `reviewed_source_marker_family()`; it does not infer coverage from an opcode or the presence of an encoding.

`S_MOV_B32` illustrates why the distinction matters: the existing narrow KIR validator recognizes it, but the catalog's source-marker overlay does not claim its source-macro coverage. The same vector names under `gfx950` do not borrow the `gfx942` overlay. An unavailable overlay observation is not a claim that every other compiler subsystem rejects the family.

All reports print `grants_authority: false`. Catalog and overlay hashes identify content, not authenticated source custody, compilation authority, continuation permission, executable artifacts, proof success, simulator correctness, or successful hardware execution.

## Identities and unavailable metadata

The summary prints the catalog format, target profile, architecture, upstream archive hash, member name and hash, compact metadata hash, and independent reviewed-coverage hash. Do not replace these with an array index or treat metadata hashes and reviewed-overlay hashes as interchangeable.

Instruction details retain every encoding alternative, sorted by encoding name, condition name, then opcode. Equal-key alternatives are not merged. Each includes the actual condition declaration count, whether a condition definition is available, whether the named encoding definition is available, and its optional bit count. Operand rows preserve their numeric order and explicitly show input/output/implicit and binary-microcode flags, optional field name, type and data-format names, referenced-definition availability, and width.

Useful examples of upstream gaps are:

```sh
cargo run --offline --locked --jobs 2 -p fe2o3-amdgcn-model --example inspect_amd_isa_catalog_v1 -- gfx942 GLOBAL_LOAD_DWORD
cargo run --offline --locked --jobs 2 -p fe2o3-amdgcn-model --example inspect_amd_isa_catalog_v1 -- gfx942 FLAT_LOAD_DWORD
```

In the pinned metadata, `GLOBAL_LOAD_DWORD` has an `ENC_FLAT_GLBL/default` reference with zero condition declarations, so its condition definition is unavailable even though the encoding definition exists. `FLAT_LOAD_DWORD` has an `ENC_FLAT/default` reference with three declarations, so availability is true but uniqueness is not claimed. The inspector neither invents missing definitions nor evaluates or selects among conditions. The summary counts references with absent or multiple declarations, not unique missing condition names.

Source-declared flags are not a complete effects model. A missing explicit `EXEC` operand is not a guarantee of no `EXEC` dependency. An operand bit width is not a legal register range, allocation proof, lifetime analysis, or resource bound. The compact catalog also does not retain all upstream descriptive prose; such prose can contain restrictions. Consult the pinned specification and actual compiler admission checks before writing a kernel.

The example deliberately does not dump raw compact XML trees or detached dictionary indices. Those trees require their matching metadata dictionary and do not become portable instruction identities by being printed. For catalog provenance, generator policy, and known gaps, see [AMD ISA specification catalog v1](amd-isa-spec-catalog-v1.md).

## Bounds and failures

Arguments must be valid UTF-8 and contain 1 through 128 bytes. At most two user arguments are accepted; parsing examines at most three to reject extras without collecting an unbounded iterator.

The inspector allows at most 4,096 canonical families, 128 alternatives per family, 64 operands per alternative, and 128 aliases for a detailed family. It constructs a report of at most 1 MiB before writing to standard output. Unknown targets/names, unavailable argument forms, bound violations, or output errors return a nonzero exit status. There is no truncation, target fallback, implicit alias synthesis, or partially accepted report. An operating-system output error can still interrupt an actual write; that is not an atomic-file or successful-report guarantee.

Canonical names, aliases, alternatives, and operands are explicitly ordered, with no timestamps or process-specific data. Repeated queries against the same immutable catalog produce identical text. The text remains an example's presentation, not a versioned serialization contract.

## Tests and scope

The example's sibling tests can be run with:

```sh
cargo test --offline --locked --jobs 2 -p fe2o3-amdgcn-model --example inspect_amd_isa_catalog_v1
```

The tests cover exact-target and spelling refusals, explicit aliases, actual catalog/overlay identities and counts, deterministic inventories and details, absent/triplicate conditions, family coverage boundaries, all catalogued detail renderings, and byte/output bounds. Non-UTF-8 argument coverage is Unix-specific. The tests exercise metadata inspection only; passing them does not qualify kernel authoring, compilation, simulation, proof, or hardware behavior.

This is a bounded author-facing inspection increment for #280. It does not close assembly authoring, debugger visualization, or multi-level compilation milestones.
