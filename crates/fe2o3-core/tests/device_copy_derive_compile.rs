//! Compile-pass coverage against the real public trait and derive.
//!
//! Generic CI checks this target without linking it because linking
//! `fe2o3-core` requires libamdhip64.

extern crate fe2o3_core as actual_fe2o3_core;
extern crate self as fe2o3_core;

use actual_fe2o3_core::DeviceCopy;

// The derive must bind the Cargo dependency itself rather than resolve this
// caller alias when it emits its unsafe implementation and field obligations.

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
struct Named {
    first: u32,
    second: f32,
}

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
struct Tuple(u16, i16, [u8; 4]);

#[derive(Clone, Copy, DeviceCopy)]
#[repr(transparent)]
struct Transparent {
    value: u64,
    marker: [u8; 0],
}

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
struct Unit;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(transparent)]
struct AlignedZst([u64; 0]);

// DeviceCopy is a representation contract. Integer bits can still carry
// application-defined host addresses or resource handles.
#[derive(Clone, Copy, DeviceCopy)]
#[repr(transparent)]
struct IntegerEncodedHandle(u64);

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
struct Nested {
    pair: Named,
    tuples: [Tuple; 2],
}

fn assert_device_copy<T: DeviceCopy>() {}

#[test]
fn accepted_layouts_implement_the_public_trait() {
    assert_device_copy::<Named>();
    assert_device_copy::<Tuple>();
    assert_device_copy::<Transparent>();
    assert_device_copy::<Unit>();
    assert_device_copy::<AlignedZst>();
    assert_device_copy::<IntegerEncodedHandle>();
    assert_device_copy::<Nested>();
}
