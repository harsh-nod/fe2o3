# Runtime model verification

`runtime_lifecycle_v1.rs` is the initial issue #137 Verus specification. It
proves two facts over a bounded sequence model:

1. a retaining dispatch is bound to the exact VM, physical-device identity,
   and device generation carried by its referenced mapping; and
2. releasing a mapping preserves the runtime invariant when no prepared,
   published, or ambiguous dispatch retains that mapping.

The proof contains no `assume`, `admit`, `external_body`, uninterpreted
specification, or external function specification. Run it with the exact Verus
release whose executable, complete release closure, version, proof source, and
negative mutation are pinned under `verus/pins`:

```sh
VERUS=/absolute/path/to/verus \
  crates/fe2o3-runtime-model/verus/verify-verus.sh
```

The command also checks an expected-negative mutation that releases a mapping
while a dispatch is published. The mutation must fail at the claimed
postcondition.

The launcher runs Verus with a cleared environment, bounded timeout, pinned Z3
path, and the repository's authenticated closure checker. A matching transcript
from a substituted executable is not accepted.

This is a proof of the abstract transition relation, not a refinement proof of
the executable Rust implementation in `src/model.rs`. Establishing that
refinement, and connecting quiescence observations to KFD/firmware behavior,
remain explicit later milestones. The proof grants no syscall, device,
firmware, execution, progress, or performance authority.
