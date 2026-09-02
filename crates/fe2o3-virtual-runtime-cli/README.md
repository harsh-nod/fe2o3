# fe2o3 virtual runtime CLI

This Linux-only integration command reuses `fe2o3-kir-sim-cli` secure admission
and executes an admitted request through virtual allocation, exact host copy,
queue, serial dependency, semantic dispatch, completion, and copied-result
stages:

```text
fe2o3-virtual-runtime --bundle kernel.fe2sim --request request.json --repeat 2
fe2o3-virtual-runtime --kir-v7 kernel.kir --request request.json \
  --target amdgpu64-target-neutral --repeat 2
```

`--repeat` is bounded to 256 and dispatch N depends on dispatch N-1. Success is
stable `fe2o3-virtual-runtime-result-v1` JSON on stdout. Admission, misuse, and
semantic faults are stable `fe2o3-virtual-runtime-error-v1` JSON on stderr.
`--fault early-release` attempts to release a dispatch-retained allocation and
must fail with the canonical model's typed `resource_in_use` result.

The command never compiles source, loads an artifact, dispatches a GPU, falls
back to hardware, or predicts performance. Bundle/KIR admission remains the
existing verified simulator trust boundary.
