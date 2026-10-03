# Genuine Rust Entry Capture and Register Layout

Development checkpoint on baseline
`62846ad4d7e78db663d1c22f3dd252af7c658893`. This advances the real output-kernel
path needed for authenticated multi-GPU compute. It does not complete A3, provide
a whole-kernel theorem, or open protected Worker application admission.

## Actual Compiler Output

The pinned Rust extractor emitted the genuine `fill_write_only` fixture through
normal semantic MIR, KIR V9 with one GuardedStore, composed formal/ranked memory
and gfx942 LLVM lowering. This is not the checked-add diagnostic selector.
ROCm 7.2.0 clang22 then emitted the actual HSACO. The source kernel writes
`index as u32` to its bounded write-only output slice.

The function symbol is **68 bytes, 14 instructions**, ending in `S_ENDPGM`.
Following NOP padding is outside the symbol. The sequence contains the kernarg
pointer/count load, 64-bit workgroup/local index construction, wait, unsigned
bounds comparison, EXEC masking, conditional branch, scaled output address and
one four-byte store. The descriptor requests kernarg `s[0:1]`, workgroup X `s2`,
and workitem X `v0`.

Checked-in fixture identities:

- LLVM: 3300 bytes, SHA-256
  `1e15622c5ced04d0d1c431bb9e243bfe373c25f5caf98d4cc41060baa248d380`.
- HSACO: 4896 bytes, SHA-256
  `f31451aa1e21d20120fa7d7d6428c3c5efcfda9493b13e0dbf9a7047a30244c5`.

A second compilation of the checked-in LLVM produces identical HSACO bytes.
Plain LLVM extraction precedes canonical descriptor embedding: these files are
diagnostic evidence, not a protected compiler handoff or launch token.

## Implementation

`InspectedKernelBindings::gfx942_initial_register_layout_v1` derives register
locations from the same inspection's selected descriptor and processor. It uses
fixed arrays and constant-bounded work, with no heap allocation or new parser.
Private fields expose named user-register pairs, independent workgroup inputs,
unpacked workitem IDs and initialized-prefix lengths.

The scratch-free profile rejects private inputs/storage, preload, wave32,
undefined workitem mode, nonexact user count and insufficient encoded capacity.
A surplus USER_SGPR_COUNT may be legal AMDHSA; rejecting it here does not change
general descriptor inspection. Setup order follows the
[LLVM initial-state ABI](https://llvm.org/docs/AMDGPUUsage.html#initial-kernel-execution-state).
The result establishes locations only, not register values, pointer validity,
launch geometry, active lanes, compiler refinement or execution authority.

## Qualification

The archive contains exact bounded commands, controlled environment, output
hashes, source inventories, compiler/tool identities, generated LLVM/HSACO,
candidate patch, read-only agent reviews and replay audit.

- **5 baseline discovery receipts**, bound to the unchanged 6387-file source.
- **46 final-source receipts**, including 33 source-control commands, bound to
  the 6392-file candidate. `final2-*` is the accepted final ledger. Earlier
  `final-*` captures precede two review-driven test assertions; `final-consumers`
  exited zero but its controller correctly rejected changed source.
- HSACO: **70 tests passed, 1 ignored**, with all 71 test identities matched;
  the same result without default features. The ignored test requires an
  environment-supplied external vecadd artifact; the new real fixture runs normally.
- Strict all-target HSACO Clippy, formatting, whitespace and downstream checks
  for verifier, host, finalizer and loader pass.
- Eight new tests cover **768 supported enable combinations**, **512 user-count
  combinations** (496 rejections), public getters, private/preload/mode rejection,
  exact 8/9-register capacity boundary, shifted input placement, wrong processor,
  invalid kernel index and the genuine function boundary.

General descriptor parsing and all runtime/KFD/accounting implementation remain
unchanged. All 76 selected existing proof/declaration/shared-body files are
unchanged. These are finite CPU tests, not a new formal ABI or execution proof;
no solver ran and no new machine-level correctness claim is made.

No GPU host was used. The owned local extraction target was removed after all
emission processes terminated; LLVM, HSACO and command evidence are retained.

## Next Acceptance

The [critical path](../../runtime-multi-gpu-critical-path.md) now targets a
same-owner whole-entry source/KIR checker and the exact 14-instruction machine
relation. Existing write-only ranked projection uses `Access`, not `ValueAccess`;
current generic validation excludes complete indexed-address/operational
equivalence. Bind the actual source call spans, predicate, output address, stored
value and exhaustive reachable effects, then compose real launch/output coverage.

On the machine side, prove the captured load base before register overwrite,
shift/OR index relation, EXEC mask, address arithmetic, store and both terminating
paths. Full output initialization, memory visibility/completion and protected
evidence production remain open. No admitted multi-GPU compute pipeline or
HIP/HSA performance parity is claimed here.
