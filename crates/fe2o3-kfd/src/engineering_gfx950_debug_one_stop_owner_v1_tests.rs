use super::*;
#[test]
fn one_stop_owners_and_all_error_transports_move_one_pointer_sized_box() {
    use core::mem::size_of;
    let pointer = size_of::<Box<OneStopInner>>();
    assert_eq!(size_of::<Gfx950DebugOneStopPreparedV1>(), pointer);
    assert_eq!(size_of::<Gfx950DebugOneStopInFlightV1>(), pointer);
    assert_eq!(size_of::<DebugOneStopTeardownWitnessV1>(), pointer);
    assert!(size_of::<Gfx950DebugOneStopFailureV1>() < 128);
    assert!(size_of::<(DebugOneStopTeardownWitnessV1, E)>() < 128);
    assert!(size_of::<OneStopInner>() < 8192);
    let _: fn(Gfx950DebugOneStopPreparedV1) -> Box<OneStopInner> = |v| v.inner;
    let _: fn(Gfx950DebugOneStopInFlightV1) -> Box<OneStopInner> = |v| v.inner;
    let _: fn(DebugOneStopTeardownWitnessV1) -> Box<OneStopInner> = |v| v.inner;
}
#[test]
fn exact_original_deadline_boundary_never_accepts_late_positive() {
    let end = Instant::now();
    assert!(native::before_deadline(end - Duration::from_nanos(1), end));
    assert!(!native::before_deadline(end, end));
    assert!(!native::before_deadline(end + Duration::from_nanos(1), end));
    assert!(native::check_deadline(None).is_err());
    assert!(native::check_deadline(Some(end)).is_err());
}
#[test]
fn completion_needs_real_initialized_signal_frontier_one_and_zero_exception() {
    let kind = fe2o3_aql::AMD_SIGNAL_KIND_USER_V1;
    assert!(resources::completion((1, 1), (kind, 0), 0).unwrap());
    assert!(!resources::completion((1, 0), (kind, 1), 0).unwrap());
    assert!(!resources::completion((1, 1), (kind, 1), 0).unwrap());
    for counters in [(0, 0), (0, 1), (1, 0), (1, 2), (2, 1), (2, 2)] {
        assert!(resources::completion(counters, (kind, 0), 0).is_err());
    }
    for signal in [(0, 0), (kind, -1), (kind, 2)] {
        assert!(resources::completion((1, 1), signal, 0).is_err());
    }
    for exception in [-1, 1, i64::MAX] {
        assert!(resources::completion((1, 1), (kind, 0), exception).is_err());
    }
}
#[test]
fn checkpoint_is_small_fixed_repr_c_with_distinct_signal_fields() {
    use core::mem::{align_of, offset_of, size_of};
    assert_eq!(align_of::<checkpoint::Checkpoint>(), 8);
    assert_eq!(size_of::<checkpoint::Checkpoint>(), 776);
    assert_eq!(offset_of!(checkpoint::Checkpoint, descriptor), 72);
    assert_eq!(offset_of!(checkpoint::Checkpoint, metadata_root), 216);
    assert_eq!(offset_of!(checkpoint::Checkpoint, packet_bytes), 416);
    assert_eq!(offset_of!(checkpoint::Checkpoint, kernarg_values), 480);
    assert_eq!(offset_of!(checkpoint::Checkpoint, reserved), 744);
    assert_eq!(offset_of!(checkpoint::Checkpoint, magic), 0);
    assert_eq!(offset_of!(checkpoint::Checkpoint, version), 8);
    assert_eq!(
        offset_of!(checkpoint::Checkpoint, signal_value),
        offset_of!(checkpoint::Checkpoint, signal_base) + 8
    );
    assert_eq!(size_of::<[u8; 64]>(), 64);
    assert_eq!(size_of::<[u8; 264]>(), 264);
}
