# Differential GPU conformance

Run the CPU/toolchain-safe lane from the repository root:

```sh
scripts/differential/run.sh
```

This builds the fixed C++ CPU oracle, exercises host alias rejection, and
records an explicit hardware skip. Opt in to the real HIP and fe2o3 GPU paths
with an exact target:

```sh
HSA_ENABLE_DXG_DETECTION=1 \
FE2O3_ALLOW_GPU_SMOKE=1 \
FE2O3_TARGET=gfx1151 \
scripts/differential/run.sh
```

This is a legacy opt-in differential smoke target, not a production-support
claim for `gfx1151` or arbitrary `FE2O3_TARGET` values. Production-directed GPU
profiles are separately bounded to named exact `gfx942:xnack-` configurations
and their code-object, wave, and workgroup requirements.

`HSA_ENABLE_DXG_DETECTION` is needed only for WSL `/dev/dxg`. Native Linux uses
`/dev/kfd`. `HIPCXX` or `CXX` may name one compiler executable. Per-command
timeouts and the canonical artifact can be configured with:

- `FE2O3_DIFFERENTIAL_TIMEOUT_SECONDS`
- `FE2O3_DIFFERENTIAL_ARTIFACT`
- `FE2O3_DIFFERENTIAL_ARTIFACT_MAX_BYTES`
- `FE2O3_EXPECT_COMMIT`

Prepare a credential-free, commit-pinned remote command for an Instinct host:

```sh
scripts/differential/run.sh prepare-remote \
  --host mi300x --target gfx942 --checkout /path/to/fe2o3
```

The artifact is canonical JSON with bounded command evidence, exact case
seeds and hashes, target/device/toolchain identities, and explicit PASS, FAIL,
or SKIP phases. It is evidence only and grants neither proof nor launch
authority.
