extern crate self as core;

use fe2o3_device::{KernelMarkerV1, kernel};

#[macro_export]
macro_rules! stringify {
    ($($tokens:tt)*) => {
        "caller-shadowed"
    };
}

#[kernel]
pub extern "C" fn marker_hygiene(value: u32) -> u32 {
    value + 1
}

fn assert_marker<T: KernelMarkerV1>() {}

fn main() {
    assert_marker::<__fe2o3_kernel_marker_marker_hygiene>();
    assert_eq!(__fe2o3_kernel_name_marker_hygiene, "marker_hygiene");
    assert_eq!(
        <__fe2o3_kernel_marker_marker_hygiene as KernelMarkerV1>::LOGICAL_NAME,
        "marker_hygiene"
    );
    assert_eq!(
        <__fe2o3_kernel_marker_marker_hygiene as KernelMarkerV1>::EXPORT_NAME,
        "marker_hygiene"
    );

    let function: extern "C" fn(u32) -> u32 =
        <__fe2o3_kernel_marker_marker_hygiene as KernelMarkerV1>::FUNCTION;
    assert_eq!(function(41), 42);

    let registration =
        <__fe2o3_kernel_marker_marker_hygiene as KernelMarkerV1>::REGISTRATION;
    assert_eq!(registration.3, "marker_hygiene");
    assert_eq!(registration.4, "marker_hygiene");
    assert_eq!((registration.5)(41), 42);
}
