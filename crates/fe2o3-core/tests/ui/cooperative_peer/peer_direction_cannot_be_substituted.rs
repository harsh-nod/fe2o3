use fe2o3_core::{GpuContext, PeerAccessCapability};
use std::sync::Arc;

fn cross(capability: PeerAccessCapability, other_destination: &Arc<GpuContext>) {
    let _ = capability.enable(other_destination);
}

fn main() {}
