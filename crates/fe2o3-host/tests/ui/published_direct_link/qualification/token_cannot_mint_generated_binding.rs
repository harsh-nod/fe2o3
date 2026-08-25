use fe2o3_device::KernelMarkerV1;
use fe2o3_host::{GeneratedKernelBindingV1, ValidatedPublishedDirectLinkSelectionV1};

struct Marker;

fn marker_function() {}

unsafe impl KernelMarkerV1 for Marker {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "marker";
    const EXPORT_NAME: &'static str = "marker.kd";
    const FUNCTION: Self::Function = marker_function;
    const REGISTRATION: &'static Self::Registration = &();
}

fn forge(
    token: &ValidatedPublishedDirectLinkSelectionV1,
) -> GeneratedKernelBindingV1<Marker> {
    unsafe { token.selection().bind_generated_marker::<Marker>() }.unwrap()
}

fn main() {}
