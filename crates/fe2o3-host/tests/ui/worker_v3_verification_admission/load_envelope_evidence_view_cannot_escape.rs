use fe2o3_host::{
    CompilerGeneratedKernelExpectationRosterV1, WorkerV3RosterLoadEnvelopeEvidenceViewV1,
    WorkerV3RosterVerificationRequestV1,
};

fn escape<R>(
    owner: WorkerV3RosterVerificationRequestV1<'_, R>,
) -> WorkerV3RosterLoadEnvelopeEvidenceViewV1<'static>
where
    R: CompilerGeneratedKernelExpectationRosterV1,
{
    owner.load_envelope_evidence_view()
}

fn main() {}
