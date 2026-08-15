use fe2o3_host::{LoadedWave64CollectivesV1, ReviewedWave64CollectivesRuntimeAdapterV1};

fn require_clone<T: Clone>() {}
fn require_copy<T: Copy>() {}

fn loaded_is_linear<A: ReviewedWave64CollectivesRuntimeAdapterV1>() {
    require_clone::<LoadedWave64CollectivesV1<'static, 'static, 'static, 'static, A>>();
    require_copy::<LoadedWave64CollectivesV1<'static, 'static, 'static, 'static, A>>();
}

fn main() {}
