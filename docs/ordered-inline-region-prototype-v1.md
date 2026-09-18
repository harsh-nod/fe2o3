# Ordered inline-unit worker prototype

This is a bounded native-test mechanism experiment, not a Rust-source feature or
production exact-region admission. The test constructs one closed LLVM fixture
and calls the existing worker's normal LLVM/object/LLD/post-link pipeline
in-process. It then decodes the actual resulting HSACO with the existing gfx942
physical-machine analyzer. It never invokes a GPU or protected finalizer.

## Reproduce

The inspected MI350-2 prerequisites are the retained ROCm 7.2.1 LLVM/LLD package
(`22.0.0git`, not the worker README's example version), its existing package
build-ID file, retained zstd development headers, and the system zstd1.4.8 shared
library. CMake, GNU g++ 11.4 and Unix Makefiles are used. No package installation,
Cargo invocation, provider libraries, or ambient `/opt/rocm` discovery is needed.

From the compiler checkout on MI350-2:

```sh
FE2O3_TASK=/home/harmenon/fe2o3-authoring-280-282.FEW3gj
FE2O3_PREP=/home/harmenon/ferric-asrock-42/evidence/native-draft-v10-prep
node scripts/assembly-region-worker-prototype.mjs \
  --compiler-repo "$FE2O3_TASK/fe2o3" \
  --llvm-root "$FE2O3_PREP/rocm-7.2.1-extracted/opt/rocm-7.2.1/lib/llvm" \
  --llvm-build-id-file "$FE2O3_PREP/official-llvm-build-id.txt" \
  --zstd-include-dir "$FE2O3_PREP/zstd-dev-extracted/usr/include" \
  --zstd-library /usr/lib/x86_64-linux-gnu/libzstd.so.1 \
  --cmake /usr/local/bin/cmake \
  --cxx /usr/bin/g++ \
  --output "$FE2O3_TASK/assembly-region-worker-prototype-observation-r2"
node --test scripts/assembly-region-worker-prototype.test.mjs
```

Use a new output directory for every attempt. Existing directories are refused;
the script neither cleans nor reuses another build. The fixed asserted package
identity is:

`rocm7.2.1-packages-sha256:eb02c62693d6697017195f0abf5ebcf7e58f60e4d2acf8356de2e944bceec540`

The runner uses Release with two jobs, explicitly disables both OCML providers,
requires and monitors a 40 GiB disk reserve, caps configure/build/test processes at
120/900/120 seconds, and bounds stdout/stderr. The native test additionally has a
90-second alarm, 1 MiB per-HSACO cap, 512-instruction cap and 64 KiB JSON
report cap. These are engineering safeguards, not compiler proof or RSS budgets.

## Observed contract

The first complete runner observation passed at
`assembly-region-worker-prototype-observation-r1` on MI350-2, with compiler HEAD
`8e6316368ce4ca67a7c1377ad2778c397249dbbb` and an explicitly dirty worktree. Exact
worker/test/runner source hashes and executable hashes are retained in its receipt.

| Optimization | Result used | Static kernel instructions | HSACO bytes | Boundary moves |
| --- | --- | ---: | ---: | ---: |
| O0 | yes | 20 | 5752 | 3 |
| O0 | no | 18 | 5752 | 3 |
| O3 | yes | 16 | 5240 | 3 |
| O3 | no | 14 | 5240 | 3 |

Every case retained the same contiguous two-instruction unit:

```text
v_xor_b32_e32 v32, v34, v35   bytes 2247402a
v_add_u32_e32 v33, v32, v36   bytes 20494268
```

The fixture declares fixed v34–v36 live-ins, early-clobber output v33, and scratch
clobber v32. Independent expected opcode/width/register/byte checks validate the
actual decoded pair. Both instructions read EXEC, have no implicit definitions,
and have no memory/control/trap flags. EXEC is not written. Boundary moves and
surrounding compiler-owned memory operations are reported separately. The
unchanged unit does not imply whole-kernel byte/order stability, a general
`sideeffect` scheduling barrier, or physical-register lifetime/ownership proof.

Actual post-link diagnostics reported gfx942:xnack-, code-object V6, wave64,
required workgroup [64,1,1], maximum workgroup 64, kernarg size 280/alignment 8, and
zero group/private segment bytes. The existing worker validates actual symbols
and launch metadata; its returned diagnostic strings are not reclassified as
symbols or a new authority source.

## Negative controls and evidence limits

- 12 closed-test-contract negatives reject changed profile, ownership declarations,
  operation selection or state effects before fixture emission. In particular,
  missing-clobber rejection is a test guard, **not** a claim that LLVM detects an
  incomplete clobber contract.
- 8 corrupted decoded-observation controls exercise the independent checker.
- 2 changed real HSACO payloads are decoded again: a changed physical source
  register and reversed instruction bytes must fail the expected-unit match.
- A real e64/e32 pair is emitted and observed, then rejected by the e32 checker.
- A module-assembly-only body is rejected by the existing compiler-module export
  gate. No dummy LLVM definition or alternative finalizer route is introduced.
- 10 Node tests cover synthetic report shapes and explicitly mocked child-command
  failures. These are control tests, not synthetic compilation receipts.

The runner retains raw process logs, the native observation, source/tool/binary
hashes and a receipt. The standalone worker is compiled and measured; the test
invokes the same pipeline in-process rather than executing that worker via its
protected transport. Per-case HSACO hashes and decoded unit bytes are reported,
but the complete per-case HSACO payloads are not separately retained.

The original observation JSON is 13,940 bytes with SHA256
`16fbc20b344f37da9ef8687a45d0f17254dcb74b449b8b11acdb7db8ca5ee087`.
Its receipt SHA256 is
`7c656c2d4d1348aa9344474255efac59a64a37ec3bd2182ed34eb8f2376a7768`.
These identify that engineering observation, not a release pin or qualified
runtime closure. The existing CMake build claim includes the provider-directory
path even when providers are disabled, so independent fresh directories can
produce different asserted worker claims; each receipt records its own claim.

## Still missing for production source authoring

The current source markers and canonical assembly operation represent individual
SSA instructions, not this ordered unit. Shipping a source-authored exact region
still requires coordinated ordinary-Rust/MIR region identity, one canonical
region representation, physical/implicit-state and encoding contracts, optimizer
boundary rules, CPU semantics, source-to-canonical correspondence, and exact
compiler-handoff/final-byte validation through the normal owners. Whole-body
module assembly additionally lacks the existing worker's LLVM-function export
and launch-contract ownership. This experiment changes none of those gates.

Only gfx942:xnack-/wave64 was exercised. Gfx950, helpers/control flow within the
unit, arbitrary assembly, memory instructions, implicit-state writes, GPU
performance and hardware execution remain outside this qualification.
