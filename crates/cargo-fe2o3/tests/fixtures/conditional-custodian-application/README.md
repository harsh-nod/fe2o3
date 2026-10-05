# Genuine Conditional Application Fixture

This standalone package selects a freshly compiled write-only u32 index-fill
kernel. Its host entry consumes Cargo's inherited application-custodian handoff,
requests production compiler-currentness auditing and a retained conditional
proof, and revalidates the resulting remote artifact. It imports no captured
binding, compiler receipt, proof receipt, or application handoff.

Run through the `genuine` campaign in
`scripts/qualify-proof-resource-inspection.sh`, using the real installed compiler
and proof-service entrypoints. Run Cargo from this package's directory: the local
`.cargo/config.toml` supplies static GNU-host linking, and `--manifest-path` alone
does not make Cargo discover that configuration. Production supplies explicit
device and host targets, so the static flags do not affect host proc macros or
the AMDGPU compilation. The device phase selects the package library, whose
AMDGPU branch is `no_std`; the host phase selects the ordinary binary. AMDGPU
does not support a binary crate target.

The zero-argument genuine campaign now passes installed compiler/currentness,
ordinary host binding and retained proof admission. It performs no GPU work.
See `docs/runtime-multi-gpu-critical-path.md` for qualification evidence and the
remaining hardware deployment work.

The optional hardware mode accepts exactly two distinct, nonzero GPU unique IDs
after Cargo's `--`, each written with a `0x` prefix. These are hardware IDs, not
HIP ordinals or render-node indices. Missing hardware never silently selects the
admission-only mode. Hardware qualification requires the installed services and
KFD plus the selected render nodes in the application's private namespace.

The separate `genuine-two-gpu` campaign now provides that namespace transport and
validates the hardware success record. From the repository root, in the real-root
qualification environment with the same pinned inputs as `genuine`, select two
freshly observed free devices and run:

```bash
FE2O3_PROOF_INSTALL_CAMPAIGN=genuine-two-gpu \
FE2O3_GENUINE_GPU_UID0="${GPU_UID0:?}" \
FE2O3_GENUINE_GPU_UID1="${GPU_UID1:?}" \
bash scripts/qualify-proof-resource-inspection.sh
```

The selector observes topology and both directed routes before creating the
private scope. It rechecks selected identities, exact character nodes and Unix
mode permissions after namespace transport and immediately before Cargo.
Application UID1000 is unchanged; only necessary numeric GPU groups are added.
ACL and device-cgroup access are still enforced by actual native opens. Selected
render mounts do not make process-global KFD a two-GPU security boundary.

The harness bounds captured stdout to 1 MiB and does not wait for descendant pipe
EOF after its direct child exits. Successful child status and exactly one matching
typed JSON record are required; missing, duplicate or mismatched records reject.
The ordinary `genuine` campaign remains admission-only. Tests and live MI300X
read-only selection pass, but the full hardware campaign has not yet run.

The hardware path retains one remote artifact across two conditional invocations
of the unchanged device kernel (65 elements, grid 128, workgroup 64). Exact
completion receipts gate charged result extraction. It stages the actual completed
bytes into PUBLIC allocations, copies both peer directions, requires two native
XGMI retirements, and checks every source, destination and guard byte. It then
revalidates the retained artifact, drains, inspects owned shutdown and verifies
result-credit refund before emitting the `fe2o3.genuine-two-gpu.v1` JSON record.
An operation timeout or quarantined shutdown is failure, not successful settlement.

`tests/conditional_native_case.rs` exercises the fixture's actual parser and data
oracles without constructing proof or native authority. These tests and host
typechecking do not qualify GPU execution. The two fills produce identical index
payloads; distinct destination sentinels reject missing copies, but this is not a
data-differentiated routing benchmark or a physical-overlap measurement. Full
HIP/HSA parity and performance remain separate gates.
