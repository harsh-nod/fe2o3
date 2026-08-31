use fe2o3_amd_target::AmdTargetId;
use fe2o3_host::AuthenticatedWorkerV3ProgramSetV1;

fn forge(target: AmdTargetId) -> AuthenticatedWorkerV3ProgramSetV1 {
    AuthenticatedWorkerV3ProgramSetV1 {
        rosters: Vec::new(),
        target,
        marker_bindings: Vec::new(),
    }
}

fn main() {}
