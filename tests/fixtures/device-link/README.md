# Bidirectional device FFI fixture

`rust-device/src/lib.rs` imports `external_scale_bias_v1` and exports
`rust_accumulate_v1`. `external.amdgpu.ll` defines the former and calls the
latter, so one invocation crosses the Rust/external boundary in both
directions.

The checked-in LLVM IR is source input, not a linked artifact. G3 must assemble
and link it through the supervised LLVM-API worker. G5 and G6 must publish and
authenticate the resulting bundle before the hardware hook can load it. No
fixture evidence grants load or launch authority.
