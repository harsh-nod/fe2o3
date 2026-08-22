use fe2o3_device::DeviceGlobalMutPtr;
use fe2o3_device::atomic::{
    AtomicI32, AtomicI64, AtomicU32, AtomicU64, CORE_ATOMIC_DEFAULT_SCOPE, CoreAtomicDefaultScope,
    GFX942_CORE_ATOMIC_WIDTHS, Ordering, gfx942_supports_core_atomic_width,
};
use std::mem::{align_of, size_of};

const ATOMIC_SOURCE: &str = include_str!("../src/atomic.rs");

#[test]
fn exposes_the_bounded_standard_rust_atomic_surface() {
    let unsigned32 = AtomicU32::new(1);
    assert_eq!(unsigned32.fetch_add(2, Ordering::Relaxed), 1);
    assert_eq!(unsigned32.load(Ordering::Acquire), 3);

    let signed32 = AtomicI32::new(-1);
    assert_eq!(signed32.fetch_max(4, Ordering::AcqRel), -1);
    assert_eq!(signed32.load(Ordering::SeqCst), 4);

    let unsigned64 = AtomicU64::new(9);
    assert_eq!(unsigned64.swap(11, Ordering::Release), 9);

    let signed64 = AtomicI64::new(-3);
    assert_eq!(
        signed64.compare_exchange(-3, 5, Ordering::AcqRel, Ordering::Acquire),
        Ok(-3)
    );
}

#[test]
fn gfx942_atomic_contract_is_explicit_and_bounded() {
    assert_eq!(CORE_ATOMIC_DEFAULT_SCOPE, CoreAtomicDefaultScope::System);
    assert_eq!(GFX942_CORE_ATOMIC_WIDTHS, &[32, 64]);
    assert!(gfx942_supports_core_atomic_width(32));
    assert!(gfx942_supports_core_atomic_width(64));
    for unsupported in [0, 1, 8, 16, 128] {
        assert!(!gfx942_supports_core_atomic_width(unsupported));
    }
}

#[test]
fn global_atomic_views_are_typed_lifetime_bound_and_layout_compatible() {
    let _: for<'a> fn(&'a DeviceGlobalMutPtr<u32>) -> &'a AtomicU32 =
        DeviceGlobalMutPtr::<u32>::as_atomic;
    let _: for<'a> fn(&'a DeviceGlobalMutPtr<i32>) -> &'a AtomicI32 =
        DeviceGlobalMutPtr::<i32>::as_atomic;
    let _: for<'a> fn(&'a DeviceGlobalMutPtr<u64>) -> &'a AtomicU64 =
        DeviceGlobalMutPtr::<u64>::as_atomic;
    let _: for<'a> fn(&'a DeviceGlobalMutPtr<i64>) -> &'a AtomicI64 =
        DeviceGlobalMutPtr::<i64>::as_atomic;

    assert_eq!(size_of::<AtomicU32>(), size_of::<u32>());
    assert_eq!(align_of::<AtomicU32>(), align_of::<u32>());
    assert_eq!(size_of::<AtomicI32>(), size_of::<i32>());
    assert_eq!(align_of::<AtomicI32>(), align_of::<i32>());
    assert_eq!(size_of::<AtomicU64>(), size_of::<u64>());
    assert_eq!(align_of::<AtomicU64>(), align_of::<u64>());
    assert_eq!(size_of::<AtomicI64>(), size_of::<i64>());
    assert_eq!(align_of::<AtomicI64>(), align_of::<i64>());
}

#[test]
fn global_atomic_view_diagnostic_items_are_exact_and_distinct() {
    let markers = [
        "fe2o3_device_global_mut_ptr_u32_as_atomic_v1",
        "fe2o3_device_global_mut_ptr_i32_as_atomic_v1",
        "fe2o3_device_global_mut_ptr_u64_as_atomic_v1",
        "fe2o3_device_global_mut_ptr_i64_as_atomic_v1",
    ];
    for marker in markers {
        assert_eq!(
            ATOMIC_SOURCE.matches(marker).count(),
            1,
            "diagnostic item {marker} must identify exactly one conversion",
        );
    }
}
