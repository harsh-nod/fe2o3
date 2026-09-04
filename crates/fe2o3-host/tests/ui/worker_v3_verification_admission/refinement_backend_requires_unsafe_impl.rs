use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, WorkerV3ProtectedSemanticMachineRefinementEvidenceV1,
    WorkerV3SemanticMachineRefinementBackendV1, WorkerV3SemanticMachineRefinementRequestV1,
};

struct ExternalBackend;

impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3SemanticMachineRefinementBackendV1<K>
    for ExternalBackend
{
    type Error = ();

    unsafe fn verify_semantic_machine_refinement(
        &mut self,
        _request: &WorkerV3SemanticMachineRefinementRequestV1<'_, '_, K>,
    ) -> Result<WorkerV3ProtectedSemanticMachineRefinementEvidenceV1, Self::Error> {
        unreachable!()
    }
}

fn main() {}
