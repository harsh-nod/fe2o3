use fe2o3_host::WorkerV3RosterVerificationDecisionV1;
use fe2o3_hsaco_finalize::RevalidatedProtectedWorkerV3FinalizerDerivationV1;

fn escape(
    decision: &WorkerV3RosterVerificationDecisionV1,
) -> &'static RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
    decision.finalizer_derivation()
}

fn main() {}
