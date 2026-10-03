# Closed Rust Fill Wave Qualification

This checkpoint advances the machine-effects component needed for the first
verified multi-GPU Rust-kernel workflow. It does **not** enable application
admission, certify native execution, or establish HIP/HSA parity.

## Implementation

`fe2o3-kernel-analysis::Gfx942FillKernelV1` borrows the original HSACO and selects
its entry using the existing descriptor/symbol inspector. The closed profile
checks the same inspection's initial-register layout, resource words, required
workgroup size and complete 68-byte function extent. Every instruction word is
matched; following section padding is not treated as executable function code.
The input is the genuine `fill_write_only` Rust artifact retained by the
[entry-layout checkpoint](../dev-entry-layout-2026-10-03/README.md), not a new
handwritten assembly or diagnostic scalar helper fixture.

The shared executable body projects all 14 instructions over eight touched
SGPRs, four VGPRs per lane, EXEC, VCC and SCC. It captures the original kernarg
read address and 16 bytes before the overlapping scalar registers are changed.
After the wait, it derives pointer/count from those bytes, filters active lanes,
and records sparse four-byte stores. Both the direct END path and the store/END
path terminate; their modeled instruction counts are 10 and 14 respectively.

The public wrapper rejects invalid local IDs, duplicate active local IDs,
unsupported wave coordinates/workgroup size, overlapping kernarg/output regions,
and overflowing or misaligned spatial descriptions. These wrapper checks are
CPU-tested, not themselves a formally proved memory-admission boundary.

## Proof Scope

The exact production bodies are shared with
`crates/fe2o3-kernel-analysis/verus/gfx942_fill_wave_v1.rs`.
For each initially active lane with local X below 64, its index is
`64 * group_x + local_x`. Its final store exists exactly when that index is below
the count; its address is `pointer + 4 * index`, and its value is `index as u32`.
Inactive vector registers are preserved, VCC excludes inactive lanes, saved EXEC
retains the incoming mask, and both branch paths reach the modeled relative END
offset. The byte-observation fold proves little-endian effects and unchanged
bytes outside the recorded intervals. A separate arithmetic lemma establishes
disjoint four-byte intervals for distinct local IDs in a workgroup.

The spatial theorem assumes a nonwrapping output extent and valid active IDs.
The AMDHSA entry contract, ordinary wave64 execution modes, immutable readable
kernarg snapshot, actual allocation backing and exclusive ownership remain
caller-established premises. ISA interpretation follows the
[AMD CDNA3 ISA reference](https://www.amd.com/content/dam/amd/en/documents/instinct-tech-docs/instruction-set-architectures/amd-instinct-mi300-cdna3-instruction-set-architecture.pdf);
it is not proved by Verus. Modeled END drain is not host-visible completion or an
AQL completion receipt. Partial EXEC does not imply complete output initialization.

The campaign pins the reviewed production/proof wrappers, declarations and
single-arm shared bodies before generating temporary logical mutants. These
hashes prevent source substitution; they are not evidence of semantic correctness.
Each mutant must fail at its declared theorem and exact diagnostic span, with
the expected verifier counts and shared-macro expansion where Verus emits it.
The before-loop invariant diagnostic has no macro expansion and is instead tied
to its exact invariant and enclosing function. Positive whole-crate verification
runs both before and after mutants; tool closure and source continuity are checked.
Only negative runs disable automatic post-failure recommendation diagnostics;
overflow, indexing, type and termination obligations are not disabled.

Reproduce the bounded proof campaign from the repository root:

```sh
python3 -I -B crates/fe2o3-kernel-analysis/verus/gfx942_fill_wave_check.py \
  --verus /home/harsh/.local/opt/verus-0.2026.08.09.92f466f/verus \
  --output /absolute/fresh/output-directory
```

## Qualification Results

Qualified against parent `44c732318117e5bfb471009a973035e9e59d198c` using
`nightly-2026-04-03`, four Cargo jobs, no incremental compilation, test opt-level 1,
debug assertions and overflow checks enabled, and serial Rust tests.

- Default analysis suite: **613 passed, 3 ignored**, including 2 doc tests;
  complete 616-entry roster matched. The ignored cases are two opt-in LLVM-MC
  compatibility checks and a slow 30-repeat worker teardown stress test.
- Authenticated-machine-only library: **57 passed, 2 ignored**. No-default
  library: **8 passed**. The eight new fill tests include 544 individual code-bit
  mutations, 96 resource-bit mutations, symbol extents and 175 wave cases.
- Verus: **30 obligations**, positive before/after; **16 logical mutants**;
  **14 campaign controls**; all 21 stages accepted. Every owned proof process
  group was absent on completion. Prior scalar-proof controls: **17 passed**.
- Strict authenticated-machine library Clippy, formatting, whitespace and
  verifier/host consumer compilation passed. Strict test-target Clippy did not:
  it encountered existing duplicate fixture modules, two existing test-style
  lints, and integration tests requiring default features. The failed receipt
  is retained and excluded from the eleven accepted CPU receipts.
- Source snapshots cover 6398 files; before/after identities match. The audit
  also checked 28 executable identities and confirmed the Clippy failure sites
  are unchanged from the parent.

[Qualification archive](qualification.tar.xz): 193 manifest entries, 1,043,636 bytes,
SHA256 `4c9a504fb5949dd62b59c58440c3b020699143a583716ba0d595c4e359fce833`.
It includes source snapshots, commands, stdout/stderr, owned-process receipts,
staged proof inputs, candidate patch, review notes, audit and sealing scripts.
Raw evidence remains at `/home/harsh/.codex-tmp/fe2o3-fill-wave-20261003`.

## Multi-GPU Critical Path

1. Check the genuine complete source/neutral-KIR fill relation using the retained
   `ProductionSemanticKirOwnerV1`, including stored value and all attributed effects.
2. Independently check optimized target KIR against the same relation. Neutral
   correspondence spans cannot be reused as optimized operation ordinals.
3. Integrate version-aware admission: genuine fill uses KIR V9, while the singleton
   semantic-machine request currently retains V8 through
   `ValidatedCompilerProofInputsV4`. Reuse existing V8/V9/V11 owner logic; do not retag.
4. Compose the exact artifact, physical ABI, actual launch geometry, grid-wide
   coverage, memory ownership, native completion and protected provider lineage.
5. Qualify the first two-device workflow on freshly observed free GPUs: real fill,
   authenticated completion, existing staging/upload and native peer transfer,
   then validated readback. Runtime paths alone are not sufficient admission evidence.

No protected admission gate or deployment policy changes in this checkpoint.
No MI300X job, native GPU execution or performance benchmark was launched.
