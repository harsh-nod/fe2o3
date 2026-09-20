use super::*;
use crate::currentness_diagnostic::Enabled;
use crate::memory::MemorySessionError;
use crate::shared_memory::{SharedMemorySessionPhaseV1 as Phase, test_xgmi_pair_terminal};

fn composed<M: Mode>(
    source: &mut Scripted,
    peer: &mut Scripted,
    route: Gfx942XgmiRouteV1,
    phases: &mut [Phase; 2],
) -> Result<(), MemorySessionError> {
    let [source_phase, peer_phase] = phases;
    test_xgmi_pair_terminal(source_phase, peer_phase, || {
        pair::<M, _>(source, peer, route)
            .map(|_| ())
            .map_err(MemorySessionError::Device)
    })
}

#[test]
fn both_modes_preserve_success_and_latch_order_and_disabled_has_no_clock_reads() {
    for route in crate::topology::tests::admitted_xgmi_routes() {
        let (mut source, mut peer, ordinary) = fixtures(route);
        let before = crate::currentness_diagnostic::tests::clock_reads();
        pair::<Disabled, _>(&mut source, &mut peer, route).unwrap();
        assert_eq!(crate::currentness_diagnostic::tests::clock_reads(), before);
        let (mut source, mut peer, measured) = fixtures(route);
        let detail = pair::<Enabled, _>(&mut source, &mut peer, route).unwrap();
        assert!(detail.is_complete());
        assert_eq!(ordinary.borrow().calls, measured.borrow().calls);
        assert_eq!(measured.borrow().calls, expected_pair());
        let expected = [(0, true, 0), (1, true, 0), (0, false, 32), (1, false, 32)];
        assert_eq!(ordinary.borrow().latches, expected);
        assert_eq!(measured.borrow().latches, expected);
        assert!(!source.poisoned && !peer.poisoned);
    }
}

fn callback_fault<M: Mode>(route: Gfx942XgmiRouteV1, index: usize, panic: bool) {
    let (mut source, mut peer, trace) = fixtures(route);
    let payload = Box::new(0xcafe_u64);
    let identity = &*payload as *const u64;
    trace.borrow_mut().failure = Some((
        index,
        if panic {
            Fault::Panic(payload)
        } else {
            Fault::Error
        },
    ));
    let mut phases = [Phase::Active; 2];
    let result = catch_unwind(AssertUnwindSafe(|| {
        composed::<M>(&mut source, &mut peer, route, &mut phases)
    }));
    if panic {
        let payload = result.unwrap_err().downcast::<Box<u64>>().unwrap();
        assert!(std::ptr::eq(&**payload, identity));
    } else {
        assert!(matches!(
            result.unwrap(),
            Err(MemorySessionError::Device(DeviceBindingError::Syscall {
                operation: "scripted full-currentness failure",
                source: rustix::io::Errno::IO,
            }))
        ));
    }
    assert_eq!(trace.borrow().calls, expected_pair()[..=index]);
    assert_eq!(trace.borrow().latches, [(0, true, 0), (1, true, 0)]);
    assert_eq!(phases, [Phase::Quarantined; 2]);
    assert!(source.poisoned && peer.poisoned);
    assert!(matches!(
        pair::<M, _>(&mut source, &mut peer, route),
        Err(DeviceBindingError::CurrentnessFencePoisoned)
    ));
    assert_eq!(trace.borrow().calls, expected_pair()[..=index]);
}

#[test]
fn both_modes_preserve_every_pair_callback_error_panic_and_terminal_reentry() {
    for route in crate::topology::tests::admitted_xgmi_routes() {
        for index in 0..expected_pair().len() {
            for panic in [false, true] {
                callback_fault::<Disabled>(route, index, panic);
                callback_fault::<Enabled>(route, index, panic);
            }
        }
    }
}

fn mismatch<M: Mode>(
    route: Gfx942XgmiRouteV1,
    endpoint: usize,
    name: &'static str,
) -> (String, Vec<(usize, &'static str)>) {
    let (mut source, mut peer, trace) = fixtures(route);
    let target = if endpoint == 0 {
        &mut source
    } else {
        &mut peer
    };
    match name {
        "retained" => target.retained.unrelated_identity += 1,
        "observed" => source.observed.unrelated_identity += 1,
        "route" => source.observed.route = None,
        "poison" => target.poisoned = true,
        _ => target.mismatch = Some(name),
    }
    let mut phases = [Phase::Active; 2];
    let error = composed::<M>(&mut source, &mut peer, route, &mut phases).unwrap_err();
    assert_eq!(phases, [Phase::Quarantined; 2]);
    assert!(source.poisoned && peer.poisoned);
    assert_eq!(trace.borrow().latches, [(0, true, 0), (1, true, 0)]);
    let calls = trace.borrow().calls.clone();
    (format!("{error:?}"), calls)
}

#[test]
fn both_modes_preserve_identity_snapshot_route_and_prior_poison_rejections() {
    for route in crate::topology::tests::admitted_xgmi_routes() {
        for endpoint in 0..2 {
            for name in [
                "process-before",
                "process-after",
                "uapi",
                "drm-before",
                "drm-after",
                "xnack-before",
                "xnack-after",
                "apertures",
                "retained",
                "observed",
                "route",
                "poison",
            ] {
                assert_eq!(
                    mismatch::<Disabled>(route, endpoint, name),
                    mismatch::<Enabled>(route, endpoint, name)
                );
            }
        }
    }
}

struct PanickingFinish;

thread_local! { static FINISH_PANIC: RefCell<Option<Box<u64>>> = const { RefCell::new(None) }; }

impl Mode for PanickingFinish {
    type Timer<const N: usize> = Disabled;
    type Topology = ();
    type Pair = ();
    fn topology(_: Disabled) {}
    fn pair(_: Disabled, _: ()) {
        std::panic::panic_any(FINISH_PANIC.with_borrow_mut(Option::take).unwrap());
    }
}

#[test]
fn final_diagnostic_panic_cannot_reactivate_devices_or_sessions() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    let (mut source, mut peer, trace) = fixtures(route);
    let mut phases = [Phase::Active; 2];
    let payload = Box::new(0xcafe_u64);
    let identity = &*payload as *const u64;
    FINISH_PANIC.with_borrow_mut(|slot| *slot = Some(payload));
    let panic = catch_unwind(AssertUnwindSafe(|| {
        composed::<PanickingFinish>(&mut source, &mut peer, route, &mut phases)
    }))
    .unwrap_err();
    let payload = panic.downcast::<Box<u64>>().unwrap();
    assert!(std::ptr::eq(&**payload, identity));
    assert_eq!(trace.borrow().calls, expected_pair());
    assert_eq!(trace.borrow().latches, [(0, true, 0), (1, true, 0)]);
    assert!(source.poisoned && peer.poisoned);
    assert_eq!(phases, [Phase::Quarantined; 2]);
}
