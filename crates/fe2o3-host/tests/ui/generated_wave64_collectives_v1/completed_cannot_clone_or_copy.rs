use fe2o3_host::{CompletedWave64CollectivesV1, ReviewedWave64CollectivesRuntimeAdapterV1};

fn require_clone<T: Clone>() {}
fn require_copy<T: Copy>() {}

fn completed_is_linear<A: ReviewedWave64CollectivesRuntimeAdapterV1>() {
    require_clone::<CompletedWave64CollectivesV1<A>>();
    require_copy::<CompletedWave64CollectivesV1<A>>();
}

fn main() {}
