# Genuine Rust Output-Entry Fixture

`kernel.ll` was emitted from the repository's actual `fill_write_only` kernel in
`rustc-codegen-fe2o3/tests/fixtures/production-extraction-device/src/lib.rs`, using
the `write-only-output` feature and pinned `nightly-2026-04-03` extractor on
baseline `62846ad4d7e78db663d1c22f3dd252af7c658893`.

This is the normal `FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1` path, not the diagnostic
checked-add-prefix selector. It reaches KIR V9, the composed formal/ranked memory
stage and target lowering. The emitted LLVM precedes canonical descriptor
embedding and does not grant artifact or launch authority. It is not a protected
Worker handoff or a complete source-to-machine proof.

Exact extraction/build commands and raw receipts are in
`docs/evidence/dev-entry-layout-2026-10-03/qualification.tar.xz`.

The HSACO was generated with the installed ROCm LLVM compiler:

```sh
LC_ALL=C SOURCE_DATE_EPOCH=0 /opt/rocm/llvm/bin/clang \
  --target=amdgcn-amd-amdhsa -mcpu=gfx942:xnack- -nogpulib -O2 \
  kernel.ll -o kernel.hsaco
```

SHA-256:

- `kernel.ll`: `1e15622c5ced04d0d1c431bb9e243bfe373c25f5caf98d4cc41060baa248d380`
- `kernel.hsaco`: `f31451aa1e21d20120fa7d7d6428c3c5efcfda9493b13e0dbf9a7047a30244c5`

The function symbol covers 68 bytes and 14 instructions, ending in `S_ENDPGM`.
Following NOP padding belongs to `.text`, not this function. The descriptor
requests only kernarg `s[0:1]`, workgroup X `s2`, and workitem X `v0`.

Tests use this artifact to verify descriptor/entry inspection and initial input
locations. They do not certify the body, memory visibility, or complete output
initialization. Subsequent machine refinement must check the actual load,
shift/OR, bounds/EXEC, address, store and termination semantics.
