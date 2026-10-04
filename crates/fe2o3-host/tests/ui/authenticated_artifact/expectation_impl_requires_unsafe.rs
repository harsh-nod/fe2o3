use fe2o3_device::KernelMarkerV1;
use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedKernelProfileV1,
};

fn kernel() {}

struct Marker;

unsafe impl KernelMarkerV1 for Marker {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "logical";
    const EXPORT_NAME: &'static str = "export";
    const FUNCTION: Self::Function = kernel;
    const REGISTRATION: &'static Self::Registration = &();
}

impl CompilerGeneratedKernelExpectationV1 for Marker {
    const PROFILE: CompilerGeneratedKernelProfileV1 =
        CompilerGeneratedKernelProfileV1::new([2; 32]);
    const KERNEL_BINDING_ID_V1: [u8; 32] = [1; 32];
}

fn main() {}
