# Complete-body LLVM transport experiment

This is a bounded native transport test, not production kernel authoring, a
protected artifact or whole M0/M2 acceptance. The standalone source is under
`tools/fe2o3-llvm-link-worker/tests/whole-body-transport/`. The production worker
protocol, pipeline, CMake roster and protected finalizer are unchanged.

## Implemented mechanism

A real LLVM AMDGPU kernel Function supplies target and launch metadata. One
side-effecting inline-assembly call contains the complete authored body and
declares all five touched VGPRs as clobbers. LLVM `unreachable` follows the call.
Both naked and ordinary Function shells are tested separately, at O0 and O3.
The existing worker emits an object, invokes LLD, validates its final exports
and metadata, and independently decodes the entire final entry.

The literal expectation, fixed before compilation, is seven instructions:

```asm
v_mov_b32_e32 v34, 1
v_mov_b32_e32 v35, 2
v_mov_b32_e32 v36, 3
v_xor_b32_e32 v32, v34, v35
v_and_b32_e32 v32, v32, v36
v_xor_b32_e32 v33, v35, v32
s_endpgm
```

All four cases preserved exactly 28 bytes:
`8102447e8202467e8302487e2247402a204940262341422a000081bf`.
There were no added entry instructions. Missing workgroup metadata and a wrong
CPU were refused before code generation for each shell.

This answers a narrow architectural question: explicit complete-body assembly
can travel through an LLVM IR Function shell in this tested configuration.
It does not require bypassing LLVM IR, exporting a module-assembly-only symbol,
replacing a dummy kernel or manufacturing a code-object descriptor.

## Actual qualification

Executed on `mi350-2`, using the pinned LLVM 22.0.0git package extracted from
ROCm 7.2.1, gfx942:xnack-, COV6, Wave64 and required workgroup [64,1,1].
No GPU was dispatched.

Repository-copy run `phase11-whole-body-native-r2` was measured at HEAD
`f798cf93d0eff1c7e951488b489e959bae6c2e3e` with a dirty worktree. Its exact
standalone source inputs were:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| WholeBodyTransportCandidate.cpp | 14069 | c39a3043f773dad1d78d3ee8f5b73fad381b3144936628d8de122870b9c5acfa |
| CMakeLists.txt | 1310 | b061d4741597d5015fcb078528f812741c2f98f6702b477df7764c80e6a8291a |
| README.md | 3082 | bb5ecbb43d2bdd7a458912950fa652f78ed13a105e877311ddffa9bb130a9b08 |

Eleven synthetic runner controls passed before the real configure/build/run.
The build took 17,498 ms; each native shell took 37 ms. Both shells passed.
The 220,498-byte receipt has SHA-256
`9edd426280b0120022a854cc8b3353c0236c938f69c939cff81aaa5df4f59bc0`.
Naked and ordinary observation hashes are respectively
`e74f89f9dfe6d28828f1a8d0e105c9596900690b3cafaf6bde755e2aa0c8981b`
and `036317494cc0785c682097057a2ead913f380917ec0d4f416cbd3928b4d91ac6`.

The original outside-repository draft passed separately in run r1 at the same
HEAD before this repository copy was added. Its 216,966-byte receipt SHA-256 is
`8b4e284e860c89a269e3c265639f674e8a21483aab121d662460b814c2643fab`.
That historical receipt is not relabelled as the repository-copy run.

## Descriptor facts and limits

Every case had VGPR capacity and architected boundary 40, enough for v0..v36.
All had zero group/private bytes, but **256 bytes of hidden kernarg storage**.
No explicit parameters does not mean an empty launch ABI.

| Shell / optimization | Entry offset | Descriptor offset | RSRC2 | Code properties | HSACO bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| Naked O0/O3 | 2304 | 2176 | 896 | 30 | 5496 |
| Ordinary O0 | 2304 | 2176 | 912 | 30 | 5496 |
| Ordinary O3 | 2048 | 1856 | 132 | 8 | 5240 |

Ordinary-shell descriptors changed with optimization even though the complete
entry bytes did not. Descriptor stability and ABI consumption need independent
contracts and tests; neither follows from exact instruction preservation.

Inputs, produced executable, selected static libraries, generated build claims,
and ELF-loader resolved files were measured and rechecked. This is not complete
build/runtime-closure attestation or executable authentication. The synthetic
worker request remains synthetic and grants no production authority.

The run retained 109,959,014 new build/output bytes. Combined charged usage was
14,404,996,951 bytes before final metadata, with a 2 MiB publication reservation,
under the unchanged 20 GiB cap. Disk/RAM floors were 40/64 GiB; build jobs two.
Termination-request limits were 60/300/60 seconds for configure/build/native.
These are checkpoint accounting and supervision limits, not OS quotas or a
guaranteed child drain/reap deadline.

## Remaining implementation

The preferred narrow complete-body proposal uses the naked Function shell;
owner agreement and a production typed boundary remain pending. Before useful
assembly kernels can use it, implementation must cover actual argument/system
value consumption, SGPR/VGPR occupancy and special-state contracts, branches and
helpers, memory/synchronization effects, source/canonical/native correspondence,
and unchanged final verification/launch admission.

The fixture initializes its arithmetic inputs but exposes no result besides
termination. It does not establish functional bitselect output, memory safety,
register lifetime, arbitrary whole-body support, source lowering or hardware
behavior. Existing typed ordered regions and ordinary Rust compilation remain
separate implemented routes; this experiment does not replace either.
