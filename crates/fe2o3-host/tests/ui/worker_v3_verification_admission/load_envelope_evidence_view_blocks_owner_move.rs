use fe2o3_host::{
    CompilerGeneratedKernelExpectationRosterV1, WorkerV3RosterVerificationRequestV1,
};

fn move_owner_while_view_is_live<R>(owner: WorkerV3RosterVerificationRequestV1<'_, R>)
where
    R: CompilerGeneratedKernelExpectationRosterV1,
{
    let view = owner.load_envelope_evidence_view();
    drop(owner);
    let _ = view.binding();
}

fn main() {}
