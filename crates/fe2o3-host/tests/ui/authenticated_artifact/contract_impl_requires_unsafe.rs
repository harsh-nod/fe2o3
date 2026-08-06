use fe2o3_device::KernelMarkerV1;
use fe2o3_host::{CompilerGeneratedKernelContractV1, CompilerGeneratedKernelProfileV1};

struct Marker;

fn marker_function() {}

unsafe impl KernelMarkerV1 for Marker {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "kernel";
    const EXPORT_NAME: &'static str = "kernel.kd";
    const FUNCTION: Self::Function = marker_function;
    const REGISTRATION: &'static Self::Registration = &();
}

impl CompilerGeneratedKernelContractV1 for Marker {
    const PROFILE: CompilerGeneratedKernelProfileV1 =
        CompilerGeneratedKernelProfileV1::TypedVecAddF32V1;

    fn artifact_container_bytes() -> &'static [u8] {
        &[]
    }
}

fn main() {}
