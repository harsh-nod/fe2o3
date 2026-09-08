use fe2o3_device::KernelMarkerV1;
use fe2o3_flash_attention_general_v1::kernel::__fe2o3_kernel_marker_flash_attention_general_v1;

type PhysicalKernel = fn(
    &[u16],
    &[u16],
    &[f32],
    &[f32],
    &mut [f32],
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    u32,
    f32,
);

#[test]
fn logical_context_and_global_roles_lower_to_the_physical_abi() {
    let function: PhysicalKernel =
        <__fe2o3_kernel_marker_flash_attention_general_v1 as KernelMarkerV1>::FUNCTION;
    let _: PhysicalKernel = function;
}

#[test]
fn host_invocation_fails_before_output_mutation() {
    let function: PhysicalKernel =
        <__fe2o3_kernel_marker_flash_attention_general_v1 as KernelMarkerV1>::FUNCTION;
    let query = [0_u16; 16];
    let key = [0_u16; 16];
    let value = [0.0_f32; 16];
    let mask = [0.0_f32; 16];
    let sentinel = f32::from_bits(0x7f7f_ffff);
    let mut output = [sentinel; 16];
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        function(
            &query,
            &key,
            &value,
            &mask,
            &mut output,
            1,
            1,
            16,
            1,
            16,
            16,
            1,
            16,
            16,
            256,
            1,
            16,
            1,
            1,
            1,
            1.0,
        );
    }));
    assert!(failure.is_err());
    assert!(
        output
            .iter()
            .all(|value| value.to_bits() == sentinel.to_bits())
    );
}

#[test]
fn attributed_body_has_no_ambient_capability_or_raw_memory_route() {
    let source = include_str!("../src/kernel.rs");
    for required in [
        "context: KernelContext<'_>",
        "q: Global<'_, u16, ReadOnly>",
        "k_transposed: Global<'_, u16, ReadOnly>",
        "v: Global<'_, f32, ReadOnly>",
        "additive_mask: Global<'_, f32, ReadOnly>",
        "output: Global<'_, f32, ExclusiveReadWrite>",
        "context.numerical_policy::<StrictIeee>()",
        "context.with_workgroup",
        "phase.publish_lds",
        "phase.finish_reusable_phase",
        "bf16_a_global_row_major",
        "bf16_b_global_row_major",
    ] {
        assert!(source.contains(required), "missing {required:?}");
    }
    for forbidden in [
        "::current(",
        "q: &[",
        "k_transposed: &[",
        "v: &[",
        "mask: &[",
    ] {
        assert!(!source.contains(forbidden), "found {forbidden:?}");
    }
}
