use fe2o3_device::{Blocked, Index1D, KernelMarkerV1, WriteOnlyDisjointSlice};

#[allow(dead_code)] // Component-only feature cells intentionally use one output family.
type TileOutput = WriteOnlyDisjointSlice<f32, Blocked<Index1D, 16, 4>>;
#[allow(dead_code)] // Component-only feature cells intentionally use one output family.
type RouteOutput = WriteOnlyDisjointSlice<u32, Index1D>;

#[allow(dead_code)] // Component-only feature cells do not expose the fused marker.
type DecodePhysical = fn(
    &[f32],
    &[f32],
    &[u16],
    &[u16],
    &[f32],
    &[f32],
    &[u8],
    &[u8],
    &[f32],
    &[f32],
    TileOutput,
    TileOutput,
    RouteOutput,
);

#[cfg(any(
    feature = "kernel-gpt-oss-decode",
    not(any(
        feature = "kernel-gpt-oss-decode-router-serial",
        feature = "kernel-gpt-oss-decode-held-fragments",
        feature = "kernel-gpt-oss-decode-scalar-attention",
        feature = "kernel-gpt-oss-decode-pipelined-attention",
        feature = "kernel-gpt-oss-decode-interleaved-stores",
        feature = "kernel-gpt-oss-router-component",
        feature = "kernel-gpt-oss-attention-component",
        feature = "kernel-gpt-oss-expert-component",
    ))
))]
#[test]
fn decode_context_and_globals_are_absent_from_the_physical_abi() {
    use fe2o3_gfx950_gpt_oss_decode::kernel::__fe2o3_kernel_marker_gfx950_gpt_oss_120b_decode_megakernel_v1;
    let _: DecodePhysical =
        <__fe2o3_kernel_marker_gfx950_gpt_oss_120b_decode_megakernel_v1 as KernelMarkerV1>::FUNCTION;
}

macro_rules! variant_abi {
    ($test:ident, $feature:literal, $module:ident) => {
        #[cfg(feature = $feature)]
        #[test]
        fn $test() {
            use fe2o3_gfx950_gpt_oss_decode::$module::{
                __fe2o3_kernel_marker_gfx950_gpt_oss_120b_decode_megakernel_v1,
            };
            let _: DecodePhysical =
                <__fe2o3_kernel_marker_gfx950_gpt_oss_120b_decode_megakernel_v1 as KernelMarkerV1>::FUNCTION;
        }
    };
}

variant_abi!(
    router_serial_abi,
    "kernel-gpt-oss-decode-router-serial",
    kernel_router_serial
);
variant_abi!(
    held_fragments_abi,
    "kernel-gpt-oss-decode-held-fragments",
    kernel_held_fragments
);
variant_abi!(
    scalar_attention_abi,
    "kernel-gpt-oss-decode-scalar-attention",
    kernel_scalar_attention
);
variant_abi!(
    pipelined_attention_abi,
    "kernel-gpt-oss-decode-pipelined-attention",
    kernel_pipelined_attention
);
variant_abi!(
    interleaved_stores_abi,
    "kernel-gpt-oss-decode-interleaved-stores",
    kernel_interleaved_stores
);

#[cfg(feature = "kernel-gpt-oss-router-component")]
#[test]
fn router_component_context_and_globals_are_absent_from_the_physical_abi() {
    use fe2o3_gfx950_gpt_oss_decode::kernel_components::__fe2o3_kernel_marker_gfx950_gpt_oss_120b_router_v1;
    type Router = fn(&[f32], &[f32], RouteOutput);
    let _: Router =
        <__fe2o3_kernel_marker_gfx950_gpt_oss_120b_router_v1 as KernelMarkerV1>::FUNCTION;
}

#[cfg(feature = "kernel-gpt-oss-attention-component")]
#[test]
fn attention_component_context_and_globals_are_absent_from_the_physical_abi() {
    use fe2o3_gfx950_gpt_oss_decode::kernel_components::__fe2o3_kernel_marker_gfx950_gpt_oss_120b_attention_v1;
    type Attention = fn(&[u16], &[u16], &[f32], &[f32], TileOutput);
    let _: Attention =
        <__fe2o3_kernel_marker_gfx950_gpt_oss_120b_attention_v1 as KernelMarkerV1>::FUNCTION;
}

#[cfg(feature = "kernel-gpt-oss-expert-component")]
#[test]
fn expert_component_context_and_globals_are_absent_from_the_physical_abi() {
    use fe2o3_gfx950_gpt_oss_decode::kernel_components::__fe2o3_kernel_marker_gfx950_gpt_oss_120b_expert_v1;
    type Expert = fn(&[u8], &[u8], &[f32], &[f32], &[u32], TileOutput);
    let _: Expert =
        <__fe2o3_kernel_marker_gfx950_gpt_oss_120b_expert_v1 as KernelMarkerV1>::FUNCTION;
}
