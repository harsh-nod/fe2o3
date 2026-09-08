use fe2o3_device::KernelMarkerV1;
use fe2o3_host::__generated::{
    CompilerGeneratedHostArgumentsV2, CompilerGeneratedHostContractV2,
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedKernelExpectationV2,
    CompilerGeneratedKernelProfileV1, GeneratedArgumentLayoutError, GeneratedHostMemoryBindingV2,
    GeneratedKfdWriteSlice,
};
use fe2o3_host::{
    AdmittedGeneratedHostContractV2, GeneratedHostDispatchEvidenceV2,
    GeneratedHostLaunchGeometryV2, GeneratedHostRuntimeCoordinatesV2,
    ReviewedGeneratedHostAsyncBackendV2, ReviewedGeneratedHostAsyncStatusV2,
};

struct Marker;
fn kernel() {}

unsafe impl KernelMarkerV1 for Marker {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "borrowed";
    const EXPORT_NAME: &'static str = "borrowed";
    const FUNCTION: Self::Function = kernel;
    const REGISTRATION: &'static Self::Registration = &();
}

unsafe impl CompilerGeneratedKernelExpectationV1 for Marker {
    const PROFILE: CompilerGeneratedKernelProfileV1 =
        CompilerGeneratedKernelProfileV1::new([1; 32]);
    const KERNEL_BINDING_ID_V1: [u8; 32] = [2; 32];
}

unsafe impl CompilerGeneratedKernelExpectationV2 for Marker {
    fn generated_host_contract_v2()
    -> Result<CompilerGeneratedHostContractV2, GeneratedArgumentLayoutError> {
        unimplemented!()
    }
}

struct Arguments<'allocation> {
    output: GeneratedKfdWriteSlice<'allocation, f32>,
}

unsafe impl<'allocation> CompilerGeneratedHostArgumentsV2<'allocation, Marker>
    for Arguments<'allocation>
{
    fn generated_host_memory_bindings_v2(&self) -> Vec<GeneratedHostMemoryBindingV2> {
        unimplemented!()
    }
}

struct Backend;
struct Context;
struct Stream;

unsafe impl<'allocation>
    ReviewedGeneratedHostAsyncBackendV2<Marker, Context, Stream, Arguments<'allocation>>
    for Backend
{
    type Submission = ();
    type SubmitError = ();

    fn observe_runtime_v2(
        &self,
        _context: &Context,
        _stream: &Stream,
    ) -> GeneratedHostRuntimeCoordinatesV2 {
        unimplemented!()
    }

    fn submit_v2(
        &mut self,
        _context: &mut Context,
        _stream: &Stream,
        _geometry: GeneratedHostLaunchGeometryV2,
        _arguments: &mut Arguments<'allocation>,
    ) -> Result<Self::Submission, Self::SubmitError> {
        unimplemented!()
    }

    fn poll_v2(
        &mut self,
        _context: &mut Context,
        _stream: &Stream,
        _submission: &mut Self::Submission,
    ) -> ReviewedGeneratedHostAsyncStatusV2 {
        unimplemented!()
    }

    fn quiesce_v2(
        &mut self,
        _context: &mut Context,
        _stream: &Stream,
        _submission: &mut Self::Submission,
    ) -> ReviewedGeneratedHostAsyncStatusV2 {
        unimplemented!()
    }
}

fn release_before_completion<'allocation>(
    admission: &AdmittedGeneratedHostContractV2<Marker>,
    backend: &'allocation mut Backend,
    context: &'allocation mut Context,
    stream: &'allocation Stream,
    evidence: GeneratedHostDispatchEvidenceV2,
    output: &'allocation mut [f32],
) {
    let arguments = Arguments {
        output: GeneratedKfdWriteSlice::new(output),
    };
    let pending = admission
        .prepare(
            backend,
            context,
            stream,
            evidence,
            GeneratedHostLaunchGeometryV2::new([1, 1, 1], [1, 1, 1], 0),
            arguments,
        )
        .unwrap()
        .submit()
        .unwrap();

    output[0] = 1.0;
    drop(pending);
}

fn main() {}
