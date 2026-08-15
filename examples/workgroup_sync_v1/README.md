# Workgroup synchronization V1

This standalone package defines two separate fixed `64x1x1` gfx942 source,
oracle, and formal profiles.

## LDS publish/read reduction

`lds_publish_read_reduce_i32_v1` is ordinary attributed Rust source. Every
admitted lane publishes one `i32` to its same-index LDS slot. The public
`fe2o3-device` workgroup reduction supplies the uniform publish barrier,
deterministic reduction barriers, and final reuse barrier. Lane zero is the
only global output writer. Admission rejects mathematical sums outside `i32`,
so the device's wrapping tree computes the exact mathematical sum.

The source compiles now, but `workgroup64_lds_i32_base_v1` traps closed. A later
compiler phase must authenticate the exact kernel and supply one aligned,
epoch-branded workgroup allocation to all lanes.

## Scoped atomic add

The second profile admits one coherent global `u32` atomic object, relaxed
ordering, system scope, and exactly 64 eligibility declarations. Eligible
lanes add once; ineligible lanes do not touch the object. Overflow is rejected
by host admission so the final value is an exact mathematical sum.

Its ordinary `#[kernel(typed, ...)]` source is quarantined under
`src/quarantined/`. The correct signature uses `DeviceGlobalMutPtr<u32>` to
state global address space without pretending the concurrently shared object
is a Rust exclusive slice. The current typed kernel ABI accepts scalars and
slices only, so profile registration must add this explicit pointer shape
before the source can be compiled as a kernel.

## Evidence boundary

The checked CPU oracles and deterministic debug/release tests are usable now.
They fail before output mutation and reject missing or divergent barriers,
stale epochs, duplicate or wrong owners, invalid lane counts, incorrect atomic
address space, ordering, scope, target, eligibility, overflow, and substituted
outputs. Verus models initialization, convergence, epoch reuse, ownership,
exact integer sums, and atomic eligibility, with expected-negative mutations.

This phase does **not** provide compiler profile authentication, source-to-IR
or IR-to-machine correspondence, artifact admission, protected loading, or
MI300X execution evidence. Those are later phases. The package uses no COMGR
and no shell linker.

## Validation

```sh
cargo test --locked --manifest-path examples/workgroup_sync_v1/Cargo.toml
cargo test --release --locked --manifest-path examples/workgroup_sync_v1/Cargo.toml
cargo clippy --locked --manifest-path examples/workgroup_sync_v1/Cargo.toml \
  --all-targets -- -D warnings
VERUS=/absolute/path/to/pinned/verus \
  examples/workgroup_sync_v1/run-verus.sh
```
