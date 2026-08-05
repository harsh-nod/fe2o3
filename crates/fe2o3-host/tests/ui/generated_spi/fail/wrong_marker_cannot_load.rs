use fe2o3_core::GpuContext;
use fe2o3_device::KernelMarkerV1;
use fe2o3_host::{
    GeneratedKernelBindingV1, LoadedKernel, ObservedContext, ValidatedArtifactSelectionV1,
};
use std::sync::Arc;

struct First;
struct Second;

fn marker_function() {}

unsafe impl KernelMarkerV1 for First {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "first";
    const EXPORT_NAME: &'static str = "first.kd";
    const FUNCTION: Self::Function = marker_function;
    const REGISTRATION: &'static Self::Registration = &();
}

unsafe impl KernelMarkerV1 for Second {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "second";
    const EXPORT_NAME: &'static str = "second.kd";
    const FUNCTION: Self::Function = marker_function;
    const REGISTRATION: &'static Self::Registration = &();
}

fn cross(
    binding: GeneratedKernelBindingV1<First>,
    validated: &ValidatedArtifactSelectionV1,
    observed: &ObservedContext,
    context: &Arc<GpuContext>,
) {
    let _: LoadedKernel<Second> =
        unsafe { LoadedKernel::<Second>::load_generated(binding, validated, observed, context) }
            .unwrap();
}

fn main() {}
