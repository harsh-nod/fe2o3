use fe2o3_core::{CooperativeLaunchCapability, GpuContext};
use std::sync::Arc;

fn forge(context: Arc<GpuContext>) -> CooperativeLaunchCapability {
    CooperativeLaunchCapability { context }
}

fn main() {}
