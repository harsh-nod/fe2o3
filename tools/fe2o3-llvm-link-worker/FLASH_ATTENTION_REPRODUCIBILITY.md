# Exact FlashAttention V1 reproducibility

The canonical `gfx942:xnack-` COV6 FlashAttention V1 kernel-symbol identity is
`d2aa57c0f468f574f44a9fea06bbb8e98aa9b60bb2d9303cc4d8b6caf0cfca54`.
It was reproduced independently with upstream LLVM and in-process LLD:

- LLVM and LLD version: `22.1.8`
- LLVM build identity:
  `upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1`
- LLVM source revision: `ca7933e47d3a3451d81e72ac174dcb5aa28b59d1`
- OCML closure: the four digest-pinned files in `WorkerBuildConfig.h`
- C++ compiler: GCC 13.3.0
- build type: `Release`

The accepted worker fails before target-machine construction unless its
compile-time LLVM build identity is exactly the identity above. It does not
accept ROCm LLVM as an alternate exact compiler identity.

## Independent reproduction

Two previously absent worker build directories produced byte-identical worker
binaries:

- worker SHA-256:
  `874a8adda1051a785bf1cbfb86a6bbe09bd632365958d86467c6749b3d6c2e0a`
- asserted worker build identity:
  `fe2o3-worker-v1-sha256-1e15051fe0b8900351b7fde6e69702d6e6e4bc0362a2c02459c17f4ef4e2b693`

Each worker was exercised from its own previously absent Cargo target and
evidence directory. The handoffs, textual IR, raw linked HSACO, stage
transcripts, and extracted kernel bodies were byte-identical across the two
runs. The transcript SHA-256 was
`c12cd50e2e880419ecb5781a29523a740d788696ec723212d4464f94dfba5f2d`.

## Stage identities

ROCm 7.2.4 LLVM is retained only as historical forensic drift evidence. Its
column predates the current source-authority regeneration, so it is not a
direct stage-by-stage comparison with the current canonical column.

| Stage | Upstream LLVM 22.1.8 (canonical) | ROCm 7.2.4 LLVM (rejected drift) |
| --- | --- | --- |
| Compiler handoff | `c4008265f589aa4a7f7f99a4f949f42040286aec954c231c3a6e8f34663b6ec2` | `d826a2c9c960292a8833f23106ac6b99b382abfcc2715648c676454d4ecbd1b2` |
| Input textual IR | `25cc163bc1ee4d5dfbe90b535a2a9913de148f9496762b147ca95e6dda09aa33` | `cdbcfb2e9ab688ddc3275e632a88f05d510e3a799799fc8952e6180e074de09b` |
| Linked bitcode | `f6cfd3083e2e7f539edbffdc4696c16a5af8bc513d5872f0ab2a9b7ee36e8d50` | `4107b280599ce9658a5614dccaa022f95d961eb96529bb7eb9c70648ffd4fa2a` |
| Optimized bitcode | `7fcae92f41d0edb84da73ef65b4d2f148550f6be915411be81347106a75a65bd` | `f685f158120b3b37cc0732704c086a83bdc43d2dd1d3fad0a1c5c65e1ef7d6a1` |
| Relocatable object | `359d06a95a0483b4363140c8494f54f66acf0c58d6a0fd67b4f432eca0b3dc94` | `0b931914e3f3b9db16e36b0b4979bb6b97ae0fc9d769a9011f2e6ee0edcb9701` |
| Raw linked HSACO | `2ca9d787a2bb016da8f01a895b363fdea7eeab032c45ad7ab844e6317923b16c` | `b6232bdc1f43ae1d5896ab78b914ff4917cd573d2365e9b658b5fd6254b21d98` |
| Kernel symbol | `d2aa57c0f468f574f44a9fea06bbb8e98aa9b60bb2d9303cc4d8b6caf0cfca54` (2540 bytes) | `60e09278e2901a1867a5a187614a4d33f12a45a733e266bf35b2693b85975d65` (2544 bytes) |

The historical comparison first diverged at linked bitcode. Its machine
disassembly then differed in branch distances and VGPR allocation, not merely
in ELF layout or symbol extraction. A worker built against ROCm 7.2.4 reports:

```text
exact FlashAttention V1 published machine identity requires LLVM build identity 'upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1', worker measured '7.2.4'
```

## Reproduction outline

Repeat this configuration with two previously absent absolute worker build
directories:

```bash
cmake -S tools/fe2o3-llvm-link-worker -B "$BUILD_DIR" \
  -DLLVM_DIR="$UPSTREAM_LLVM_BUILD/lib/cmake/llvm" \
  -DLLD_DIR="$UPSTREAM_LLVM_BUILD/lib/cmake/lld" \
  -DFE2O3_PINNED_LLVM_VERSION=22.1.8 \
  -DFE2O3_LLVM_BUILD_ID_FILE="$UPSTREAM_LLVM_BUILD_ID_FILE" \
  -DFE2O3_EXPECTED_LLVM_BUILD_ID=upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1 \
  -DBUILD_TESTING=ON -DCMAKE_BUILD_TYPE=Release
cmake --build "$BUILD_DIR" --parallel
```

For each worker, use a different previously absent Cargo target and evidence
directory:

```bash
CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
FE2O3_FLASH_ATTENTION_HANDOFF_OUTPUT="$EVIDENCE/flash.handoff" \
FE2O3_FLASH_ATTENTION_MODULE_OUTPUT="$EVIDENCE/flash.ll" \
cargo test --locked -p rustc-codegen-fe2o3 \
  --test flash_attention_v1 \
  exact_phase_a_source_authenticates_complete_flash_attention_profile \
  -- --exact

FE2O3_FLASH_ATTENTION_V1_WORKER="$BUILD_DIR/fe2o3-llvm-link-worker" \
FE2O3_FLASH_ATTENTION_V1_WORKER_BUILD_ID="$(cat "$BUILD_DIR/fe2o3-worker-build-id.txt")" \
FE2O3_FLASH_ATTENTION_V1_LLVM_BUILD_ID=upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1 \
FE2O3_FLASH_ATTENTION_V1_HANDOFF="$EVIDENCE/flash.handoff" \
FE2O3_FLASH_ATTENTION_V1_RAW_OUTPUT="$EVIDENCE/flash.raw.hsaco" \
FE2O3_FLASH_ATTENTION_V1_TRANSCRIPT_OUTPUT="$EVIDENCE/flash.transcript" \
cargo test --locked -p fe2o3-hsaco-finalize \
  --test flash_attention_v1_direct_llvm_worker \
  real_worker_produces_reproducible_opaque_flash_attention_v1_receipt \
  -- --ignored --exact
```

Extract kernel bytes using the ELF section offset and symbol range reported by
`llvm-readelf -SW` and `llvm-readelf -sW`. Do not run `llvm-objcopy` with the
input file as its implicit output; that rewrites the container and invalidates
the whole-HSACO identity.

These identities establish repeatable bytes for one closed profile and one
pinned upstream toolchain. They do not prove source-to-LLVM or LLVM-to-ISA
refinement, functional FlashAttention semantics, generalized memory safety,
race freedom, GPU execution, numerical correctness, performance, load
authority, or launch authority.
