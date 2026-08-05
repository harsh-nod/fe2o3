extern crate self as core;

#[macro_export]
macro_rules! stringify {
    ($($tokens:tt)*) => {
        "caller-shadowed"
    };
}

#[fe2o3_macros::kernel]
fn marker_hygiene() {}

#[test]
fn kernel_marker_uses_sysroot_stringify() {
    assert_eq!(__fe2o3_kernel_name_marker_hygiene, "marker_hygiene");
}
