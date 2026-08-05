use fe2o3_device::KernelMarkerV1;
use fe2o3_host::ValidatedArtifactSelectionV1;

struct Marker;

fn kernel() {}

unsafe impl KernelMarkerV1 for Marker {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "kernel";
    const EXPORT_NAME: &'static str = "kernel.kd";
    const FUNCTION: Self::Function = kernel;
    const REGISTRATION: &'static Self::Registration = &();
}

fn bind(validated: &ValidatedArtifactSelectionV1) {
    let _ = validated.bind_generated_marker::<Marker>();
}

fn main() {}
