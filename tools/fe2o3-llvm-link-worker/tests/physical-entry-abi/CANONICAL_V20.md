# V20 canonical physical-entry native observation

This is a separate opt-in **test observer**, not a production artifact importer,
GPU runner, source authenticator, or protected finalizer. Existing literal
`physical-entry-abi-candidate copy|select O0|O3 DIRECTORY` behavior remains.

The additional executable accepts:

```text
physical-entry-canonical-candidate ABS_INPUT_LLVM ABS_INPUT_EXPECTATION O0|O3 ABS_FRESH_DIRECTORY
```

Input LLVM is read unchanged, content-joined against the expectation, and passed
to the existing ordinary pinned worker. No source renaming, instruction rewrite,
post-link patch, alternative assembler, or execution occurs in this observer.
The existing worker request/effect identities are visibly synthetic test fields.

The expectation producer is
`physical_entry_native_observation_input_v20(&actual_owner, &emission, &mut budget)`
in `fe2o3-amdgcn-model`. It joins the actual V20 canonical identity, hashes the
unchanged emission, and records actual CFG/step correspondence. This diagnostic
text has no source/canonical loader. A genuine-source qualifier must keep its
real source and mandatory-check owners alive through generation of both files,
and the external runner must join the canonical/content records separately.
The native observer deliberately reports source authentication, canonical owner
admission, protected admission, native functional execution and milestone
completion as false.

## Closed text grammar

All lines have one ASCII space between fields and one LF terminator; final LF
is mandatory, no trailing records or unknown fields. At most 16 KiB / 80 lines.
Decimal values use no sign, leading zero or alternate radix. Digests are
lowercase hex64 and entries use the same bounded 128-byte identifier profile.

```text
FE2O3_PHYSICAL_ENTRY_V20_NATIVE_OBSERVATION_INPUT_V1
canonical DIGEST CANONICAL_BYTE_LENGTH
llvm DIGEST LLVM_BYTE_LENGTH
entry SYMBOL
launch 64 1 1 2 1 1
blocks COUNT
block ID ENCODING FIRST_NATIVE NATIVE_COUNT SUCCESSOR0_OR255 SUCCESSOR1_OR255
...
steps COUNT
step BLOCK_ID NATIVE_ORDINAL DESCRIPTOR_BYTE0 BYTE1 BYTE2 BYTE3 BYTE4 BYTE5 BYTE6 BYTE7
...
end
```

Block encodings are the already allocated closed V20 contract values. The one
block or four-block selector diamond has actual sorted CFG successors; no
additional branch-target list or executable plan is present. Step rows exclude
the canonical control terminators. The checker independently maps the closed
primitive descriptors to expected MC opcode/register/immediate/effect rows;
branch destinations are derived from the actual CFG block records.

## Checks and limits

The shared relation binds fresh MC instruction bytes to every byte of the
complete selected entry, with no uncovered prefix/suffix, exact opcode,
instruction width, explicit and implicit definitions/uses, physical operand
order, immediate values, memory access/width, branch targets, instruction
membership and CFG successor/fallthrough roster. Exact raw wait/end/store
encoding and branch-family checks remain. This is not a universal machine
encoder or hazard proof.

Existing exact six explicit and thirteen hidden arguments, 288-byte kernarg,
Wave64, COV6, launch metadata, no-spill/no-LDS/no-stack conditions, entry SGPR/
VGPR live-ins, zero preload, reserved descriptor fields and emitted register
capacity checks remain. New minima are derived only from the expected physical
rows; legacy defaults remain 20 SGPR / 9 VGPR. Unsupported descriptor allocation
or decoder behavior refuses rather than broadening the contract. Existing
synthetic machine-effect ceilings (16 addresses, 8 reads, 4 writes, 2 returns,
0 calls) are unchanged; not every bounded author program is claimed qualified
by this observer.

Every actual instruction gets an independent changed-byte removal/refusal.
Additional controls mutate stored data/bounds operands, actual branch
destinations, descriptor live-ins/extent/resources/reserved fields and metadata.
Every changed native payload is freshly hashed and decoded. Eleven inert
expectation grammar/content/descriptor controls and two actual-LLVM target/
launch input controls run separately. Maximum 82 native/metadata controls,
64 observed instructions, 64 KiB HSACO, 512 KiB report, and a 90-second process
alarm. All outputs use the existing fresh exclusive file writer.

## Parent qualification sequence

No commands were run by the draft author. Parent should:

1. Run the three new model tests, the existing emitter/core tests, and strict
   lints. The ignored
   `export_inert_verified_copy_diamond_and_register_edit_for_native_observer`
   test requires `FE2O3_PHYSICAL_V20_NATIVE_FIXTURE_DIR` to name a fresh absolute
   directory. It exports `copy`, `select`, and `copy-s63` as exact owner KIR20,
   unchanged emitted LLVM, and inert expectations; it invokes no worker/GPU.
2. Build both CMake observer targets with the same reviewed SDK/worker identities.
   Re-run all four old literal copy/select O0/O3 cases to protect defaults.
3. Run each new exported LLVM/expectation pair at O0 and O3 in separate fresh
   output directories, inspect all report booleans, mutations, emitted metadata
   and complete-entry relation. Register edits are not claimed qualified until
   these actual reports pass.
4. Repeat with genuine source-owned normal compiler emissions/expectations from
   the source lane. Keep source→canonical→mandatory checks→normal handoff
   evidence separate from this observation's synthetic worker fields. No literal
   fixture export closes source ownership, M2, M3 or any remaining milestone.
