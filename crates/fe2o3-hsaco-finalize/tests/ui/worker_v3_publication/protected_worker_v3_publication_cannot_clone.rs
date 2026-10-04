use fe2o3_hsaco_finalize::{
    PreparedProtectedWorkerV3CompactFinalizerReplayV2,
    PreparedProtectedWorkerV3HsacoPublicationV1,
};

fn require_clone<T: Clone>() {}

fn main() {
    require_clone::<PreparedProtectedWorkerV3CompactFinalizerReplayV2>();
    require_clone::<PreparedProtectedWorkerV3HsacoPublicationV1>();
}
