#![cfg_attr(target_arch = "amdgpu", no_std)]

use fe2o3_device::capability_memory::{DisjointWrite, Global, ReadOnly};
use fe2o3_device::{Index1D, KernelContext, kernel};

include!("vecadd_body.rs");

macro_rules! production_f32_add {
    ($lhs:expr, $rhs:expr) => {{ $lhs + $rhs }};
}

fn vecadd_cpu_reference(point: usize, a: &[f32], b: &[f32], output: &mut f32) {
    if point < a.len() && point < b.len() {
        *output = a[point] + b[point];
    }
}

#[kernel(
    typed,
    reference = vecadd_cpu_reference,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn vecadd(
    context: KernelContext<'_>,
    a: Global<'_, f32, ReadOnly>,
    b: Global<'_, f32, ReadOnly>,
    mut c: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    let index = context.invocation().index_1d();
    let _stored = vecadd_kernel_body!(@capability index, production_f32_add, a, b, c);
}

#[cfg(not(target_arch = "amdgpu"))]
mod host_app;
#[cfg(not(target_arch = "amdgpu"))]
mod simulator;

#[cfg(not(target_arch = "amdgpu"))]
pub use host_app::{
    ProtectedVecaddPrerequisite, VecaddKfdPrepareError, VecaddPrepareError, admit_protected_vecadd,
    prepare_protected_vecadd, prepare_protected_vecadd_kfd, vecadd_launch_geometry,
};
#[cfg(not(target_arch = "amdgpu"))]
pub use simulator::{simulate_bundle_v8, vecadd_cpu_oracle};

#[cfg(test)]
mod tests {
    const KERNEL_SOURCE: &str = include_str!("lib.rs");
    const HOST_SOURCE: &str = include_str!("host_app.rs");
    const SHARED_BODY: &str = include_str!("vecadd_body.rs");

    #[test]
    fn cpu_reference_checks_each_input_extent_before_writing() {
        let a: [f32; 65] = std::array::from_fn(|i| i as f32 + 1.0);
        let b: [f32; 65] = std::array::from_fn(|i| i as f32 * 2.0);
        let untouched = f32::from_bits(0x7fc0_0123);
        for a_len in [0, 1, 63, 64, 65] {
            for b_len in [0, 1, 63, 64, 65] {
                for point in (0..=65).chain(std::iter::once(usize::MAX)) {
                    let mut output = untouched;
                    super::vecadd_cpu_reference(point, &a[..a_len], &b[..b_len], &mut output);
                    let expected = if point < a_len && point < b_len {
                        a[point] + b[point]
                    } else {
                        untouched
                    };
                    assert_eq!(
                        output.to_bits(),
                        expected.to_bits(),
                        "point={point}, a_len={a_len}, b_len={b_len}",
                    );
                }
            }
        }
    }

    #[test]
    fn library_kernel_uses_one_hierarchy_root_and_typed_global_capabilities() {
        let production_source = KERNEL_SOURCE
            .split_once("#[cfg(test)]")
            .map_or(KERNEL_SOURCE, |(source, _)| source);
        for required in [
            "reference = vecadd_cpu_reference",
            "launch(required = [64, 1, 1], max = [64, 1, 1])",
            "context: KernelContext<'_>",
            "a: Global<'_, f32, ReadOnly>",
            "b: Global<'_, f32, ReadOnly>",
            "c: Global<'_, f32, DisjointWrite<Index1D>>",
            "context.invocation().index_1d()",
            "vecadd_kernel_body!(@capability index, production_f32_add, a, b, c)",
        ] {
            assert!(production_source.contains(required), "missing `{required}`");
        }
        for forbidden in [
            "namespace =",
            "FE2O3_CODEGEN_PIPELINE",
            "load_module_from_file",
            "launch!",
            "thread::index_1d()",
            "DisjointSlice<f32>",
            "scale: f32",
        ] {
            assert!(
                !production_source.contains(forbidden),
                "retained `{forbidden}`"
            );
        }
    }

    #[test]
    fn generated_host_surface_has_three_physical_arguments_and_no_context_argument() {
        for required in [
            "AdmittedGeneratedHostContractV2",
            "GeneratedHostDispatchEvidenceV2",
            "PreparedGeneratedHostInvocationV2",
            "GeneratedKfdReadSlice::new(a)",
            "GeneratedKfdReadSlice::new(b)",
            "GeneratedKfdWriteSlice::new(c)",
            "Arguments::new",
        ] {
            assert!(HOST_SOURCE.contains(required), "missing `{required}`");
        }
        assert!(!HOST_SOURCE.contains("ProductionGeneratedHostFactsV2::from_authenticated"));
        assert!(!HOST_SOURCE.contains("context: GeneratedKfd"));
    }

    #[test]
    fn shared_body_retains_the_capability_memory_shape() {
        let capability = SHARED_BODY.find("@capability").unwrap();
        let compatibility = capability
            + SHARED_BODY[capability..]
                .find("$($thread_arg:expr)")
                .unwrap();
        let body = &SHARED_BODY[capability..compatibility];
        for required in [
            "if i < $output.len()",
            "$a.load(i)",
            "$b.load(i)",
            "$output.store($index.into_disjoint(), $add!(left, right))",
        ] {
            assert!(body.contains(required), "missing `{required}`");
        }
        assert!(
            body.find("if i < $output.len()").unwrap()
                < body
                    .find("$output.store($index.into_disjoint(), $add!(left, right))")
                    .unwrap()
        );
    }
}
