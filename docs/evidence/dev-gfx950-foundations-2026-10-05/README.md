# Gfx950 Runtime Foundations

Base: `1748550ba74b5bced1c264a97ecd223b62531b9d`.

This increment removes three prerequisites for the MI350 multi-GPU path:
target-explicit topology observations, an inert gfx950 code-object loader profile,
and authenticated target-bound physical-machine analysis. It does **not** admit
gfx950 devices, allocate GPU memory, create queues, execute a GPU kernel, or
establish a gfx950 machine-semantic refinement. No A0-A7 milestone exit,
HIP/HSA parity, performance result, or whole-runtime formal proof is claimed.

## Changes And Boundaries

- `discover_default_topology_for_target(GfxTarget::Gfx950)` reads the complete
  inventory without granting native authority. Default discovery remains gfx942.
  Mixed, unknown, and wrong-target inventories reject; render-node correlation
  and closed property validation remain in force. Both endpoints of a gfx942
  XGMI route must still be gfx942.
- `AdmittedProfile::Gfx950XnackOffCov6` requires exact COV6 flags `0x64f`,
  matching processor and feature metadata, and the existing bounded load shape.
  Its closure identity is separate. The existing gfx942 closure golden digest
  remains `49fe3218f7d4b82a975e137af5287aa1b8a3d31d7a5af85a1331c3fa931d9d64`.
  CPU materialization is not native loading or execution authority.
- The machine analyzer selects the exact LLVM subtarget. Request, effects,
  trace, every decoded instruction, bundle, policy, and receipt retain the
  selected target. Gfx950 domains/tag 2 are distinct; existing gfx942 domains,
  fields, ordering, and tag 1 are retained. Shared process/runtime-file closure
  mechanisms remain host-process identity checks, not device semantic claims.
- All existing gfx942 semantic/refinement entry points explicitly reject gfx950.
  Gfx950 atomic, DS, barrier, and trap instruction families remain unsupported.
  Native analysis establishes payload-bound decoding and effect/CFG records,
  not source-to-machine correctness.
- Existing gfx942 KFD preparation, packet projection, initial bind, auxiliary
  callbacks, and replacement/rebind paths reject foreign target envelopes before
  native preparation or mutation. Synthetic tests check rejection without input
  loss, changed native observations, or callback execution.

Three native agents implemented and reviewed the separate loader, topology/KFD,
and machine-analysis areas. The primary integrated verifier guards and live
qualification. Independent cross-component review found no actionable target
substitution defect; tests and scope limits below remain part of that assessment.

## Live Read-Only Qualification

On `mi350`, the opt-in
`topology::tests::target_selection::live_gfx950_topology_is_observed_without_gfx942_route_authority`
test passed: **1 passed, 0 failed**, test duration 0.01 seconds. All eight gfx950
GPUs correlated with their render nodes and unique IDs. Observed geometry was
1024 SIMDs, 160 KiB LDS, eight XCCs, and wave64. The same test required default
gfx942 discovery and both gfx942 route directions to reject this host.

This test opened no KFD/DRM device, allocated no GPU memory, and submitted no
GPU work. An empty KFD process snapshot was only an observation, not an exclusive
reservation. See [live-topology.json](live-topology.json) and
[host-observations.json](host-observations.json) for actual commands and output.

The stripped GNU test executable was verified on both hosts with SHA256
`0da0db10f083fa17e74acd17c0970c4199d5f30e6cb821021e7aaba00baee159`.
It was built after the discovery changes and before the later KFD envelope
guards; subsequent CPU regression results qualify those guards separately.

## CPU Qualification

- Full KFD library suite: **1968 passed, 0 failed, 1 ignored** in 732.84 seconds.
  The ignored case is the separately run live topology test. All seven new
  dispatch/profile rejection tests also passed in focused runs before the full
  matrix completed. These lifecycle/failure matrices use CPU fixtures.
- Refreshed default-feature kernel-analysis suite: **142 library tests and
  28 physical-machine integration tests passed**; two opt-in LLVM-MC
  compatibility cases were ignored. Optional configured tests do not establish
  their external-artifact cases when their environment is absent.
- Scoped checked-u32 verifier suite: **39 passed**, including self-consistent
  gfx950 add and move-prefix evidence rejected by gfx942 refinement boundaries.
- Loader: **27 tests and one compile-fail doctest passed**, eight
  environment-dependent cases ignored. The real accepted symbolic gfx950 object
  was then supplied explicitly to the optional loader test: **1 passed**.
- The retained real gfx950 request and bundle reopen through a separate explicit
  offline test: **1 passed**. It retains target through every instruction,
  binds exact payload bytes, and rejects gfx942 dataflow/EXEC use. Reopening does
  not reconstruct authenticated process custody from saved records.
- Strict Clippy passes for all four changed libraries and both changed
  machine-analysis integration-test targets. Two broader all-target
  attempts fail in existing test code: unused verifier fixture variants;
  duplicate fixture module inclusion, manual saturating arithmetic, conditional
  style, needless lifetimes and item order in analysis tests. The affected
  statements were not changed. No all-target/workspace lint-clean claim is made.
- Scoped formatting and whitespace checks pass. No new Verus proof, protected
  gfx950 compiler campaign, full workspace test run or GPU benchmark was run.
- Downstream `fe2o3-runtime` and `fe2o3-host` library typechecking passes with
  the locked offline dependency graph and the new target-explicit APIs.

## Native Toolchain

The worker and native machine-effect tests built on MI350 with installed ROCm
7.2.1 and its exactly matching LLVM/LLD development package, extracted into owned
scratch without changing global packages or services. CMake configuration is
retained in [native-configure.json](native-configure.json).

- Development package: `rocm-llvm-dev_22.0.0.26084.70201-81~24.04_amd64.deb`.
- Package SHA256: `a5115c98c8eb44c73965578e2ef5c049bcc1601093838377e042ae8073d95ba0`.
- Worker build claim:
  `fe2o3-worker-v1-sha256-05e58c63fd0e472e6167e67dc2a0a89e0b61afb578486a8c620dc305da3df998`.
- Worker executable SHA256:
  `616479dc482f1335ee2a5f2cc382bcc36f412cf8d68967a8954f5f4e98a33e97`.

All four native CTest groups pass: codec, pipeline, machine effects, and
device-library policy (4/4 in 0.71 seconds). These include actual LLVM-generated
gfx950 input and wrong-target/metadata/unsupported-effect controls.

The diagnostic fill object used for parser/analyzer qualification comes from
hand-written LLVM IR emitted with local ROCm 7.2.0, not protected Rust source or
the pinned production compiler. Its SHA256 is
`00fca03895dfc3c82b916e4cea43f620ec85d4c2f06568c987c09fb165db05c4`;
the IR SHA256 is
`1cbc93ef19fd5d11257c553263428c87dfa7219bd9444c525a8753313f343038`.
Neither artifact has been executed on a GPU in this campaign.

For successful authenticated analysis, the same IR was emitted with remote
ROCm 7.2.1 and linked with the existing production worker flags, including
`-Bsymbolic`. Its raw HSACO SHA256 is
`8578bc1ecf8f59283eda7d0ba1395fd204a43477e3ff987e34a1471ae14769df`.
The gfx950 authenticated test passes in **124.60 seconds**, retaining three
blocks and 14 instructions. It includes wrong-target deployment-policy rejection,
no-fork checks and rejection by the gfx942 fill checker. The actual execution's
request, bundle and receipt are retained in the qualification archive, along
with separate raw-file hashes and domain-separated identities.

The same worker also passes the existing gfx942 authenticated exact-fill
regression in **99.11 seconds**, using independently retargeted diagnostic IR
and the same symbolic link contract. It checks the exact 14-instruction gfx942
model, descriptor binding and CPU dispatch/byte behavior for lengths 0, 64, 65
and 4097. This is a regression for the existing model, not protected-source
qualification, GPU execution or a gfx950 semantic theorem.

## Attempts And Limits

- An uncompressed topology-binary upload timed out. A premature attempt to run
  the partial destination failed with `Text file busy`; no test ran. A compressed
  retry completed, matched the binary hash, and produced the passing live result.
- The local development-package download first failed certificate validation.
  The explicit configured CA worked, but transfer speed was too low; that owned
  process was terminated and reaped. The remote matched-package download was
  hash-checked before extraction. No certificate checking was disabled.
- An initial expanded authentication suite had 22 passes, one identity-probe
  setup timeout, and two ignored tests. The failed case
  `exact_atomic_opcode_with_mismatched_memory_classification_fails_closed`
  passed independently in 37.37 seconds. A scoped retry does not turn the
  original whole-suite run into an all-green run.
- The first real gfx950 authenticated run passed identity/policy/no-fork stages
  but rejected the diagnostic default-clang-link object after 112.99 seconds.
  The failure was `ControlHandshake`, with native stderr reporting that dynamic
  declarations disagree with loadable sections. That object lacks `DT_FLAGS`
  with `DF_SYMBOLIC`; the existing worker pipeline requires `-Bsymbolic`.
  Loader envelope acceptance alone is not the stricter analyzer profile. The
  negative result is retained, and no analyzer acceptance check was weakened.
- No global host package, service, GPU ownership, or user workload was changed.
  Native toolchain/build artifacts used task-owned `/dev/shm` directories.
  All native sessions completed and were reaped; the entire remote directory
  was removed and its absence independently checked. See [cleanup.json](cleanup.json).

## Remaining Multi-GPU Gate

1. Add separately checked gfx950 device/model, native queue/CWSR, memory and
   SDMA/XGMI contracts for the observed MI350 driver profile.
2. Establish a distinct gfx950 conditional-fill semantic refinement and runtime
   authority, then run the real protected source-to-application CPU campaign.
3. Deploy owned private services on MI350 and run the positive two-device
   application with fresh occupancy checks and exact result/guard validation.
4. Run second-device rejection and queued-deadline/cancellation controls, retain
   complete owned cleanup evidence, then qualify native in-flight failures and
   matched HIP/HSA performance separately.

The Oct4 gfx942 two-GPU fixture and negative-control CPU qualification remain
valid prior evidence. They are not gfx950 qualification and are not hardware
completion of this path. Current status lives in
[the multi-GPU critical path](../../runtime-multi-gpu-critical-path.md).
The concrete next implementation slices and target-agreement safety gate are in
[the native-admission work order](../../runtime-gfx950-native-admission-work-order.md).

## Retained Evidence

[qualification.tar.gz](qualification.tar.gz) contains the source patch and
hashes, selected qualification commands/logs, compiler versions, diagnostic LLVM
IR/code objects and canonical request/bundle/receipt records. It excludes the
native host worker/test executables, package/toolchain caches and private keys.
The first rejected native object and broad lint failures are retained alongside
successful qualification. Earlier fixture-test attempts described above are
reported from their execution results, not presented as additional archive logs.

Archive SHA256:
`0ef813a60a4dadfdaa4f9359a6d4840fb81a85455cfcc1a06925978496abd8ed`.
