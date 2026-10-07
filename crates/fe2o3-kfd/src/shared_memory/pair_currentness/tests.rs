use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn pair_sessions_reactivate_only_after_complete_success() {
    let mut source = SharedMemorySessionPhaseV1::Active;
    let mut peer = SharedMemorySessionPhaseV1::Active;
    with_terminal_pair(&mut source, &mut peer, || Ok(())).unwrap();
    assert_eq!(source, SharedMemorySessionPhaseV1::Active);
    assert_eq!(peer, SharedMemorySessionPhaseV1::Active);
    let result = with_terminal_pair(&mut source, &mut peer, || {
        Err::<(), _>(MemorySessionError::ProcessChanged)
    });
    assert!(matches!(result, Err(MemorySessionError::ProcessChanged)));
    assert_eq!(source, SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(peer, SharedMemorySessionPhaseV1::Quarantined);
    assert!(matches!(
        with_terminal_pair::<()>(&mut source, &mut peer, || panic!("terminal reentry")),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
}

#[test]
fn pair_session_unwind_preserves_original_payload_and_both_phases() {
    let mut source = SharedMemorySessionPhaseV1::Active;
    let mut peer = SharedMemorySessionPhaseV1::Active;
    let payload = Box::new(19_u64);
    let identity = (&*payload) as *const u64;
    let result = catch_unwind(AssertUnwindSafe(|| {
        with_terminal_pair::<()>(&mut source, &mut peer, || std::panic::panic_any(payload))
    }));
    let payload = result.unwrap_err().downcast::<Box<u64>>().unwrap();
    assert!(std::ptr::eq(&**payload, identity));
    assert_eq!(source, SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(peer, SharedMemorySessionPhaseV1::Quarantined);
}

#[test]
fn inactive_pair_is_not_reactivated_and_has_no_observation_effects() {
    for initial in [
        [
            SharedMemorySessionPhaseV1::Active,
            SharedMemorySessionPhaseV1::Quarantined,
        ],
        [
            SharedMemorySessionPhaseV1::Quarantined,
            SharedMemorySessionPhaseV1::Active,
        ],
        [
            SharedMemorySessionPhaseV1::Quarantined,
            SharedMemorySessionPhaseV1::Quarantined,
        ],
    ] {
        let [mut source, mut peer] = initial;
        assert!(matches!(
            with_terminal_pair::<()>(&mut source, &mut peer, || panic!("inactive observation")),
            Err(MemorySessionError::SharedSessionQuarantined)
        ));
        assert_eq!([source, peer], initial);
    }
}
