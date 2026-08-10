use fe2o3_device::atomic::{
    AtomicI32, AtomicI64, AtomicU32, AtomicU64, CORE_ATOMIC_DEFAULT_SCOPE, CoreAtomicDefaultScope,
    GFX942_CORE_ATOMIC_WIDTHS, Ordering, gfx942_supports_core_atomic_width,
};

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
