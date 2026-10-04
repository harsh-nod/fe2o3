# Pinned public AMD ISA catalog v1

This is an immutable **metadata inventory**, not an executable instruction
catalog or complete legality model. It adds no frontend instruction, validator,
simulator dispatch, optimization policy, source-to-ISA correlation, finalizer,
proof, or hardware authority.

The public API is in
[amd_isa_spec_catalog_v1.rs](../crates/fe2o3-amdgcn-model/src/amd_isa_spec_catalog_v1.rs).
Input pins, the separate reviewed overlay, compact metadata, generated file
hashes, and full AMD MIT attribution are in
[tools/amd-isa-catalog-v1](../tools/amd-isa-catalog-v1).

## Inputs and identity

The importer accepts only the pinned local archive:

- URL: [AMD_GPU_MR_ISA_XML_2026_08_06.zip](https://gpuopen.com/download/AMD_GPU_MR_ISA_XML_2026_08_06.zip)
- Size: 2,294,639 bytes
- SHA256: 82404f1126761b7877595b622afa7e1f311f2f41e89a3abe9aaf8ad045c082e2

| Member | Bytes | SHA256 |
| --- | ---: | --- |
| amdgpu_isa_cdna3.xml | 7,986,028 | af24feb95c9230a87f694c814b0035e7448a57f8503b059f0282dba1e5f20d9c |
| amdgpu_isa_cdna4.xml | 8,679,327 | bb2a96f3db5c67d7c9b90e34559045c272f3536acdb03d16bc455f2f5b0a1597 |

Both XML headers explicitly say AMD Public Use, MIT, copyright 2026, schema
1.1.1, and release date 2026-02-20. The archive URL date differs from the document
release date. Schema documentation is referenced at
[GPUOpen-Tools/isa_spec_manager commit 452645535ac05f466b06a13e5eafeb5a86d3ad11](https://github.com/GPUOpen-Tools/isa_spec_manager/blob/452645535ac05f466b06a13e5eafeb5a86d3ad11/documentation/spec_documentation.md).

| Architecture | Instructions | Encoding forms | Operand types | Data formats |
| --- | ---: | ---: | ---: | ---: |
| CDNA 3 | 1,150 | 32 | 35 | 83 |
| CDNA 4 | 1,240 | 33 | 35 | 98 |

Each query carries format version, architecture, optional reviewed target-profile
name, archive/member identities, exact generated metadata SHA256, and a
**separate** SHA256 of the reviewed coverage overlay. Instruction names, aliases,
encoding names, conditions, and opcodes remain explicit. Array ordinals and
metadata byte ranges are not cross-revision instruction IDs. An identity
record is inert metadata, not an authenticated compiler receipt.

The target selector accepts only exact bare gfx942 and gfx950, selecting CDNA 3
and CDNA 4 respectively. This reviewed catalog mapping is not silicon or feature
qualification. Feature strings, suffixes, whitespace, case-folded names, and
other targets are rejected. Architecture-only queries have no target association.

## Offline regeneration and checks

Acquire the archive separately. The generator never downloads anything and
never extracts archive paths. From the compiler root:

    python3 scripts/amd-isa-catalog-v1.py --archive /explicit/path/AMD_GPU_MR_ISA_XML_2026_08_06.zip --check
    python3 scripts/amd-isa-catalog-v1-tests.py --archive /explicit/path/AMD_GPU_MR_ISA_XML_2026_08_06.zip
    cargo test --offline --locked -p fe2o3-amdgcn-model --lib amd_isa_spec_catalog_v1

To regenerate the fixed repository-owned generated files explicitly, replace
--check with --write. Generated metadata and Rust index bytes are deterministic:
no timestamps, network lookups, local paths, or machine names enter output.
The generated-index.json file records every generated file's exact size and
SHA256. The archive itself is not checked in.

The importer verifies archive size/hash, member count/names/kinds/compression
and bounds, then selected member size/hash **before** XML parsing. Reads reject
symlinks, nonregular files, oversize data, and FIFO blocking. XML must be UTF-8,
without entity/doctype declarations, mixed content, unexpected namespaces, or
excessive depth/node/text counts. Typed references, aliases, field ranges,
operand widths, flags, and duplicates are checked. No XML expression is run.
--check writes nothing; --write changes only known generated files and rejects
symlink/nonregular outputs.

## Representation and queries

No new Rust dependency or runtime parser is introduced. Queries borrow static
immutable data and use exact bounded-name lookups. They expose canonical names
and aliases, source-declared instruction flags, every encoding alternative,
condition declaration counts, numeric opcodes, operand directions,
implicit/microcode flags, widths, fields, operand types, and data formats.

The compact JSON retains every non-Description XML element and attribute:
conditions and their expression trees, bitmaps, predefined values, subtype
lists, functional groups, data formats, and aliases. Formatting whitespace is
not retained; the original raw member hash is. Description prose is deliberately
omitted and can contain additional restrictions, so the catalog is **not** a
self-contained operand/register legality specification.

Each tree node has this versioned shape:

    [tag_string_id, flat_attribute_string_ids, text_string_id, children]

The per-member sorted string dictionary accompanies each borrowed tree.
A detached tree without the dictionary and catalog identity is incomplete.
String IDs encode only this exact metadata document.

Example query sequence:

    let catalog = amd_isa_catalog_for_target_v1("gfx942")?;
    let xor = catalog.instruction("V_XOR_B32").expect("pinned entry");
    for alternative in xor.encodings() {
        // Inspect catalog_identity, instruction name, encoding name,
        // condition, declaration count, opcode and typed operands.
        // None of these observations authorizes encoding or execution.
    }

Instruction flags are reproduced as declared, not treated as complete effect
summaries. Absence of an explicit EXEC operand does not establish that EXEC is
unused. Predefined register values do not establish allocation, register
lifetime, legal resource ranges, ABI, ordering, or machine semantics.

## Explicit upstream condition gaps

Both pinned documents contain these anomalies:

- ENC_FLAT_GLBL/default and ENC_FLAT_SCRATCH/default are referenced while the
  forms have empty condition definitions. Counts are exactly 86 references for
  CDNA 3 and 88 for CDNA 4. Queries report declaration count zero and
  condition_definition_available=false.
- ENC_FLAT/default is declared three times with identical literal-one trees.
  All three remain in metadata; queries report count three, not unique selection.

Only these named, count-checked exceptions are accepted for these pinned members.
Changed counts/trees, unknown conditions, and other duplicates are rejected.
Missing conditions are never synthesized; conditions are never evaluated.
Generated metadata records unresolved references and duplicate declarations.

## Separate reviewed source coverage

The overlay names only the six existing gfx942 u32 typed SSA marker families:
move, add, subtract, AND, OR, and XOR. Executable implementation remains in the
existing closed
[gfx942 validator](../crates/fe2o3-kernel-ir/src/gfx942_inline_assembly_v1.rs);
this is not a second executable instruction definition.

Matching a family does **not** select e32/e64, literal, DPP, SDWA, physical
registers, implicit EXEC behavior, or legal resource ranges. Encoding-level
authoring, simulation, proof, hardware qualification, other instructions, and
other targets are unavailable in this overlay. Architecture-only queries do
not acquire source coverage from generation metadata alone.

The generator rejects replacing the overlay with six arbitrary names, including
a memory instruction, or turning its unavailable encoding stages into support
claims. Metadata and overlay hashes are independent: editing the overlay cannot
preserve the complete query identity under an unchanged upstream pin.

## Controls and evidence limits

Controls cover the actual pinned metadata, exact deterministic regeneration,
malformed archive/XML/file inputs, duplicate/unknown references, both upstream
exceptions, complete non-prose tree preservation, target isolation, aliases,
metadata/overlay identities, and unavailable support states.

Independent numeric controls verify CDNA 3 VOP2 XOR opcode 21 and ADD opcode 52
against words 0x2a404722 and 0x68424920, using published bitfields and explicit
register numbers. This is a metadata/bitfield cross-check, not a general
encoder/decoder, artifact admission, GPU execution, or equivalence proof.

This is one foundation for #280. It does not close #280, #281, or #282 and does
not narrow their original initial-profile or milestone requirements.
