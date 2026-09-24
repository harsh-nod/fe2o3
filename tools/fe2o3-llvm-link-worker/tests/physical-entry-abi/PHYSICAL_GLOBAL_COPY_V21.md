# V21 physical global-copy native observer

This separate opt-in test target consumes unchanged emitted LLVM and an inert
expectation file through the existing pinned ordinary worker:

```text
physical-global-copy-canonical-candidate ABS_INPUT_LLVM ABS_INPUT_EXPECTATION O0|O3 ABS_FRESH_DIRECTORY
```

It does not run the GPU or authenticate source/canonical custody. Both existing
V20 observer targets and their six-slot ABI/relations remain unchanged.

The actual emitter signature is exactly:

```llvm
(ptr addrspace(1) %input_data, i64 %input_length,
 ptr addrspace(1) %output_data, i64 %output_length)
```

These names are checked against actual emitted argument metadata, not inferred
from Rust parameter spelling. Expected explicit offsets are 0/8/16/24, each
eight bytes; pointer components 0 and 2 are global_buffer, lengths are by_value.
The explicit span is still 32 bytes, so the independently checked 13 hidden
argument rows and total 288-byte COV6 kernarg extent remain exact. No readonly,
noalias, restrict, initialization or host-pointer safety attribute is fabricated.
The live MIR38 shared-slice/owned-slice ABI still requires separate source
qualification and is reported false by this observer.

## Same-owner diagnostic producer

`physical_global_copy_native_observation_input_v21(&actual_owner, &emission,
&mut existing_budget)` records the original canonical content identity, unchanged
LLVM SHA256/length, bounded entry, launch, actual block/Return and actual step
descriptors/native ordinals. It also joins each retained emission correspondence
row back to the actual operation/source-site/results. The original source,
canonical and mandatory-check owners must stay retained externally. A digest or
sidecar is not their replacement.

The returned String and retained-payload receipt use the same cumulative ledger.
All output capacity, hashing/rendering work and fixed scratch are prepaid; caller
storage floors/work/peak/denial history are preserved. No extra executable graph
or source parser is introduced.

Strict grammar, one ASCII space, final LF, no unknown/duplicate/trailing rows:

```text
FE2O3_PHYSICAL_GLOBAL_COPY_V21_NATIVE_OBSERVATION_INPUT_V1
canonical DOMAIN_SEPARATED_CANONICAL_DIGEST CANONICAL_BYTE_LENGTH
llvm LLVM_SHA256 LLVM_BYTE_LENGTH
entry SYMBOL
launch 64 1 1 2 1 1
blocks 1
block 0 4 0 NATIVE_COUNT 255 255
steps STEP_COUNT
step 0 NATIVE_ORDINAL OPCODE D S0 S1 IMM_BYTE0 IMM_BYTE1 IMM_BYTE2 IMM_BYTE3
...
end
```

The sole actual Return contributes the last native end instruction and has no
step row. Parsing is bounded to 16 KiB/48 lines; LLVM to 32 KiB, 32 instructions,
one actual decoded block, HSACO to 64 KiB, report to 512 KiB, process alarm90s.
Decimals and lowercase hex are canonical; symbols use the existing 128-byte
ASCII letter/underscore-first grammar.

## Independent native relation

The new descriptor-to-MC table is separate from the emitter and uses the existing
full-entry relation: fresh payload digest, contiguous encoding coverage, exact
opcode/explicit definitions/implicit uses+definitions/register tuples/operand
order/immediates/memory width+direction/control flags, actual block membership,
empty successor list, exact end, zero compiler prologue and zero compiler tail.
Unrecognized opcode/operand/resource/metadata behavior refuses.

The memory/wait census requires four scalar pair loads at offsets0/8/16/24 and
LGKM0, one global dword load immediately followed by VM0, and exact
compare/save-mask/store/VM0/restore/end suffix. Other arithmetic/move instructions
are matched individually; their count and actual native ordinals are not copied
from the baseline fixture. The external genuine canonical owner is responsible
for its verified SSA/pointer/carry relation. This native observer does not
reconstruct Rust custody or treat claimed diagnostic provenance as authority.

The new load expectation is backed by retained official ROCm LLVM revision
f58b06dce1f9c15707c5f808fd002e18c2accf7e:

- FLATInstructions.td lines34-88,95-171,229-253,2635-2651,2775 define the
  global EXEC dependency, one destination, vaddr pair/off address mode,
  offset/cache operands, opcode0x14 and exact bit fields.
- gfx9_asm_flat.s lines1335-1378 independently cover destination, address,
  scalar origin/off, offset and cache encodings.

See GLOBAL_COPY_UPSTREAM_PINS.json for exact URLs, source paths, lengths and SHA256.
For zero offset/cache/ACC/SCC bits and disabled scalar address, the low word is
0xdc508000 and the high word is (destination<<24)|0x007f0000|address_low.
Those are source-derived expectations, not a claim that this draft was natively
compiled or that every allocation has passed the pinned MC decoder.

The original descriptor checker remains exact: SGPR0:1 kernarg, SGPR2 workgroupX,
VGPR0 workitemX, RSRC2=132, only kernarg pointer enable, zero preload/scratch/LDS/
spills/stack/reserved fields, emitted capacities covering actual physical minima.
No descriptor is rewritten.

## Negative controls and nonclaims

Each instruction is freshly removed in a separate native payload. Further
fresh-byte controls change all four kernarg offsets and scalar origins,
workgroup/lane/scale sources, pointer-high and carry operands, load destination/
address/off origin/offset/cache policy, both VM waits, bound length/EXEC mask,
store load-result register/output address, descriptor entry/resource/extent/
reserved fields, and all four explicit names plus a hidden argument name.
Each payload is rehashed and decoded; no evidence struct is altered. Metadata
string controls require unique exact anchors; ambiguous names refuse rather
than selecting an unrelated occurrence. At most96 native/descriptor mutations
plus5 metadata controls; the report records the actual count.

12 bounded grammar/content negatives and2 actual LLVM target/launch negatives
remain distinct. The canonical store-SSA substitution test rejects a different
well-typed U32 before any owner export. Native checks establish physical operand
preservation, not recovery of SSA identity from machine code.

Reports keep source_authentication, canonical_owner_admission, native functional
execution, hardware execution, protected admission, initialized-input validity,
runtime input/output bounds, real host nonalias, live Rust ABI qualification,
general hazard proof and milestone completion false.

## Root qualification, not yet performed

Four focused model tests and one ignored inert export are authored. The ignored
test `export_inert_global_copy_and_register_site_edits_for_native_observer`
requires `FE2O3_PHYSICAL_V21_NATIVE_FIXTURE_DIR` to name a fresh absolute
directory; it exports copy, copy-v22 and copy-extra-v22 as exact .kir21, unchanged
.ll and .expect. It invokes no worker or device.

Root should qualify those controls, compile all three strict CMake targets with
the reviewed SDK/worker pins, rerun existing V20 regressions, and run all three
inert V21 cases at O0/O3. Then repeat with genuinely retained MIR38 source
emissions only after source/ABI/mandatory checks are separately qualified.
Static disassembly/metadata observation does not discharge runtime slice bounds,
initialization, disjoint allocation or functional/hardware execution obligations.
