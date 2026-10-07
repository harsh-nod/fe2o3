use super::*;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

mod diagnostic;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    unrelated_identity: u32,
    route: Option<Gfx942XgmiRouteV1>,
}

enum Fault {
    Error,
    Panic(Box<u64>),
}

#[derive(Default)]
struct Trace {
    calls: Vec<(usize, &'static str)>,
    latches: Vec<(usize, bool, usize)>,
    failure: Option<(usize, Fault)>,
}

struct Scripted {
    endpoint: usize,
    trace: Rc<RefCell<Trace>>,
    poisoned: bool,
    post: bool,
    mismatch: Option<&'static str>,
    retained: Snapshot,
    observed: Snapshot,
}

impl Scripted {
    fn step(&mut self, name: &'static str) -> Result<(), DeviceBindingError> {
        let fault = {
            let mut trace = self.trace.borrow_mut();
            trace.calls.push((self.endpoint, name));
            if trace
                .failure
                .as_ref()
                .is_some_and(|(index, _)| *index == trace.calls.len() - 1)
            {
                trace.failure.take().map(|(_, fault)| fault)
            } else {
                None
            }
        };
        match fault {
            Some(Fault::Error) => Err(DeviceBindingError::Syscall {
                operation: "scripted full-currentness failure",
                source: rustix::io::Errno::IO,
            }),
            Some(Fault::Panic(payload)) => std::panic::panic_any(payload),
            None => Ok(()),
        }
    }

    fn value(
        &mut self,
        before: &'static str,
        after: &'static str,
    ) -> Result<u32, DeviceBindingError> {
        let name = if self.post { after } else { before };
        self.step(name)?;
        Ok(u32::from(self.mismatch == Some(name)))
    }
}

impl Observation for Scripted {
    type Process = u32;
    type Drm = u32;
    type Apertures = u32;
    type Topology = Snapshot;

    fn poisoned(&self) -> bool {
        self.poisoned
    }
    fn set_poisoned(&mut self, value: bool) {
        let mut trace = self.trace.borrow_mut();
        let calls = trace.calls.len();
        trace.latches.push((self.endpoint, value, calls));
        self.poisoned = value;
    }
    fn retained_process(&self) -> u32 {
        7
    }
    fn retained_drm(&self) -> u32 {
        9
    }
    fn retained_uapi(&self) -> KfdUapiVersion {
        KfdUapiVersion::new(1, 18)
    }
    fn retained_apertures(&self) -> &u32 {
        &0
    }
    fn retained_topology(&self) -> &Snapshot {
        &self.retained
    }

    fn ensure_opener_process(&mut self) -> Result<(), DeviceBindingError> {
        self.post = false;
        self.step("opener")
    }
    fn observe_process(&mut self) -> Result<u32, DeviceBindingError> {
        self.value("process-before", "process-after")
            .map(|value| 7 + value)
    }
    fn check_reset(&mut self) -> Result<(), DeviceBindingError> {
        self.step(if self.post {
            "reset-after"
        } else {
            "reset-before"
        })
    }
    fn validate_kfd_before(&mut self) -> Result<(), DeviceBindingError> {
        self.step("kfd-before")
    }
    fn validate_kfd_after(&mut self) -> Result<(), DeviceBindingError> {
        self.post = true;
        self.step("kfd-after")
    }
    fn validate_render(&mut self) -> Result<(), DeviceBindingError> {
        self.step(if self.post {
            "render-after"
        } else {
            "render-before"
        })
    }
    fn observe_uapi(&mut self) -> Result<KfdUapiVersion, DeviceBindingError> {
        self.step("uapi")?;
        Ok(KfdUapiVersion::new(
            1,
            if self.mismatch == Some("uapi") {
                19
            } else {
                18
            },
        ))
    }
    fn observe_drm(&mut self) -> Result<u32, DeviceBindingError> {
        self.value("drm-before", "drm-after").map(|value| 9 + value)
    }
    fn observe_xnack(&mut self) -> Result<i32, DeviceBindingError> {
        self.value("xnack-before", "xnack-after")
            .map(|value| value as i32)
    }
    fn observe_apertures(&mut self) -> Result<u32, DeviceBindingError> {
        self.step("apertures")?;
        Ok(u32::from(self.mismatch == Some("apertures")))
    }
    fn discover<M: Mode>(&mut self) -> Result<(Snapshot, M::Topology), DeviceBindingError> {
        self.step("discover")?;
        let mut timer = M::Timer::<4>::new();
        for phase in 0..4 {
            timer.measure(phase, || ());
        }
        Ok((self.observed.clone(), M::topology(timer)))
    }
    fn validate_route(
        &mut self,
        snapshot: &Snapshot,
        route: Gfx942XgmiRouteV1,
    ) -> Result<(), DeviceBindingError> {
        self.step("route")?;
        let observed = snapshot
            .route
            .ok_or(DeviceBindingError::ObservableCurrentnessChanged(
                "observed XGMI topology route",
            ))?;
        exact_route(observed, route)
    }
    fn vram_lost_counter(drm: u32) -> u32 {
        drm
    }
}

const PRE: [&str; 9] = [
    "opener",
    "process-before",
    "reset-before",
    "kfd-before",
    "render-before",
    "uapi",
    "drm-before",
    "xnack-before",
    "apertures",
];
const POST: [&str; 6] = [
    "kfd-after",
    "render-after",
    "process-after",
    "xnack-after",
    "drm-after",
    "reset-after",
];

fn expected_pair() -> Vec<(usize, &'static str)> {
    PRE.into_iter()
        .map(|step| (0, step))
        .chain(PRE.into_iter().map(|step| (1, step)))
        .chain([(0, "discover"), (0, "route")])
        .chain(POST.into_iter().map(|step| (0, step)))
        .chain(POST.into_iter().map(|step| (1, step)))
        .collect()
}

fn fixtures(route: Gfx942XgmiRouteV1) -> (Scripted, Scripted, Rc<RefCell<Trace>>) {
    let trace = Rc::new(RefCell::new(Trace::default()));
    let snapshot = Snapshot {
        unrelated_identity: 42,
        route: Some(route),
    };
    let build = |endpoint| Scripted {
        endpoint,
        trace: Rc::clone(&trace),
        poisoned: false,
        post: false,
        mismatch: None,
        retained: snapshot.clone(),
        observed: snapshot.clone(),
    };
    (build(0), build(1), trace)
}

fn composed_pair(
    source: &mut Scripted,
    peer: &mut Scripted,
    route: Gfx942XgmiRouteV1,
) -> Result<(), DeviceBindingError> {
    use crate::memory::MemorySessionError;
    use crate::shared_memory::{SharedMemorySessionPhaseV1, test_xgmi_pair_terminal};

    let mut source_phase = SharedMemorySessionPhaseV1::Active;
    let mut peer_phase = SharedMemorySessionPhaseV1::Active;
    let result = test_xgmi_pair_terminal(&mut source_phase, &mut peer_phase, || {
        pair::<Disabled, _>(source, peer, route).map_err(MemorySessionError::Device)
    });
    let expected = if result.is_ok() {
        SharedMemorySessionPhaseV1::Active
    } else {
        SharedMemorySessionPhaseV1::Quarantined
    };
    assert_eq!([source_phase, peer_phase], [expected; 2]);
    assert_eq!([source.poisoned, peer.poisoned], [result.is_err(); 2]);
    match result {
        Ok(()) => Ok(()),
        Err(MemorySessionError::Device(error)) => Err(error),
        Err(error) => panic!("unexpected session result: {error:?}"),
    }
}

#[test]
fn ordinary_full_sequence_and_returned_counter_are_unchanged() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    let (mut source, _, trace) = fixtures(route);
    assert_eq!(single(&mut source).unwrap(), 9);
    let expected: Vec<_> = PRE
        .into_iter()
        .chain(["discover"])
        .chain(POST)
        .map(|step| (0, step))
        .collect();
    assert_eq!(trace.borrow().calls, expected);
    assert!(!source.poisoned);
}

#[test]
fn full_pair_encloses_one_fresh_discovery_for_both_directions() {
    for route in crate::topology::tests::admitted_xgmi_routes() {
        let (mut source, mut peer, trace) = fixtures(route);
        composed_pair(&mut source, &mut peer, route).unwrap();
        assert_eq!(trace.borrow().calls, expected_pair());
        assert!(!source.poisoned && !peer.poisoned);
        composed_pair(&mut source, &mut peer, route).unwrap();
        assert_eq!(trace.borrow().calls, expected_pair().repeat(2));
        source.observed.unrelated_identity += 1;
        assert!(matches!(
            composed_pair(&mut source, &mut peer, route),
            Err(DeviceBindingError::TopologySnapshotChanged)
        ));
        assert_eq!(
            trace
                .borrow()
                .calls
                .iter()
                .filter(|(_, step)| *step == "discover")
                .count(),
            3
        );
        assert!(source.poisoned && peer.poisoned);
    }
}

#[test]
fn every_pair_callback_error_and_panic_retains_devices_sessions_and_original_cause() {
    use crate::memory::MemorySessionError;
    use crate::shared_memory::{SharedMemorySessionPhaseV1, test_xgmi_pair_terminal};
    let expected = expected_pair();
    for route in crate::topology::tests::admitted_xgmi_routes() {
        for index in 0..expected.len() {
            for panic in [false, true] {
                let (mut source, mut peer, trace) = fixtures(route);
                let mut source_phase = SharedMemorySessionPhaseV1::Active;
                let mut peer_phase = SharedMemorySessionPhaseV1::Active;
                let payload = Box::new(0xcafe_u64);
                let identity = (&*payload) as *const u64;
                let fault = if panic {
                    Fault::Panic(payload)
                } else {
                    Fault::Error
                };
                trace.borrow_mut().failure = Some((index, fault));
                let result = catch_unwind(AssertUnwindSafe(|| {
                    test_xgmi_pair_terminal(&mut source_phase, &mut peer_phase, || {
                        pair::<Disabled, _>(&mut source, &mut peer, route)
                            .map_err(MemorySessionError::Device)
                    })
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
                assert_eq!(trace.borrow().calls, expected[..=index]);
                assert!(source.poisoned && peer.poisoned);
                assert_eq!(source_phase, SharedMemorySessionPhaseV1::Quarantined);
                assert_eq!(peer_phase, SharedMemorySessionPhaseV1::Quarantined);
                assert!(matches!(
                    composed_pair(&mut source, &mut peer, route),
                    Err(DeviceBindingError::CurrentnessFencePoisoned)
                ));
                assert_eq!(trace.borrow().calls, expected[..=index]);
            }
        }
    }
}

#[test]
fn every_ordinary_full_callback_error_and_panic_short_circuits() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    let expected: Vec<_> = PRE
        .into_iter()
        .chain(["discover"])
        .chain(POST)
        .map(|step| (0, step))
        .collect();
    for index in 0..expected.len() {
        for panic in [false, true] {
            let (mut source, _, trace) = fixtures(route);
            let payload = Box::new(7_u64);
            let identity = (&*payload) as *const u64;
            trace.borrow_mut().failure = Some((
                index,
                if panic {
                    Fault::Panic(payload)
                } else {
                    Fault::Error
                },
            ));
            let result = catch_unwind(AssertUnwindSafe(|| single(&mut source)));
            if panic {
                let payload = result.unwrap_err().downcast::<Box<u64>>().unwrap();
                assert!(std::ptr::eq(&**payload, identity));
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(DeviceBindingError::Syscall {
                        operation: "scripted full-currentness failure",
                        source: rustix::io::Errno::IO,
                    })
                ));
            }
            assert_eq!(trace.borrow().calls, expected[..=index]);
        }
    }
}

#[test]
fn endpoint_identity_changes_fail_at_the_exact_pre_or_post_stage() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
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
        ] {
            let (mut source, mut peer, trace) = fixtures(route);
            if endpoint == 0 {
                source.mismatch = Some(name);
            } else {
                peer.mismatch = Some(name);
            }
            let error = composed_pair(&mut source, &mut peer, route).unwrap_err();
            let expected = match name {
                "process-before" | "process-after" => {
                    matches!(error, DeviceBindingError::ProcessIncarnationChanged)
                }
                "uapi" => matches!(error, DeviceBindingError::UapiChanged),
                "drm-before" | "drm-after" => {
                    matches!(error, DeviceBindingError::ObservableCurrentnessChanged(_))
                }
                "xnack-before" | "xnack-after" => {
                    matches!(error, DeviceBindingError::UnsupportedXnackMode)
                }
                "apertures" => matches!(error, DeviceBindingError::AperturesChanged),
                _ => unreachable!(),
            };
            assert!(expected);
            assert_eq!(trace.borrow().calls.last(), Some(&(endpoint, name)));
            assert!(source.poisoned && peer.poisoned);
        }
    }
}

#[test]
fn both_exact_retained_snapshots_are_required_even_with_unchanged_route() {
    for route in crate::topology::tests::admitted_xgmi_routes() {
        for endpoints in [1, 2, 3] {
            let (mut source, mut peer, trace) = fixtures(route);
            if endpoints & 1 != 0 {
                source.retained.unrelated_identity += 1;
            }
            if endpoints & 2 != 0 {
                peer.retained.unrelated_identity += 1;
            }
            assert!(matches!(
                composed_pair(&mut source, &mut peer, route),
                Err(DeviceBindingError::TopologySnapshotChanged)
            ));
            assert_eq!(trace.borrow().calls.last(), Some(&(0, "route")));
            assert!(
                !trace
                    .borrow()
                    .calls
                    .iter()
                    .any(|(_, step)| *step == "kfd-after")
            );
            assert!(source.poisoned && peer.poisoned);
        }
    }
}

#[test]
fn missing_reversed_or_changed_fresh_route_precedes_base_snapshot_mismatch() {
    let routes = crate::topology::tests::admitted_xgmi_routes();
    let changed = crate::topology::tests::admitted_xgmi_routes_with_bandwidth(32000);
    for direction in 0..2 {
        assert_eq!(
            routes[direction].source_gpu_id(),
            changed[direction].source_gpu_id()
        );
        assert_eq!(
            routes[direction].destination_gpu_id(),
            changed[direction].destination_gpu_id()
        );
        assert_eq!(
            routes[direction].topology_generation(),
            changed[direction].topology_generation()
        );
        assert_ne!(routes[direction], changed[direction]);
        for observed in [None, Some(routes[1 - direction]), Some(changed[direction])] {
            let (mut source, mut peer, trace) = fixtures(routes[direction]);
            source.observed.route = observed;
            assert!(matches!(
                composed_pair(&mut source, &mut peer, routes[direction]),
                Err(DeviceBindingError::ObservableCurrentnessChanged(_))
            ));
            assert_eq!(trace.borrow().calls.last(), Some(&(0, "route")));
            assert!(source.poisoned && peer.poisoned);
        }
    }
}

#[test]
fn prior_poison_never_reobserves_or_revives_either_endpoint() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    for endpoints in [1, 2, 3] {
        let (mut source, mut peer, trace) = fixtures(route);
        source.poisoned = endpoints & 1 != 0;
        peer.poisoned = endpoints & 2 != 0;
        assert!(matches!(
            composed_pair(&mut source, &mut peer, route),
            Err(DeviceBindingError::CurrentnessFencePoisoned)
        ));
        assert!(source.poisoned && peer.poisoned);
        assert!(trace.borrow().calls.is_empty());
    }
}

#[test]
fn opener_rejection_precedes_access_to_that_endpoints_reset_fifo() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    for endpoint in 0..2 {
        let (mut source, mut peer, trace) = fixtures(route);
        let index = expected_pair()
            .iter()
            .position(|entry| *entry == (endpoint, "opener"))
            .unwrap();
        trace.borrow_mut().failure = Some((index, Fault::Error));
        assert!(composed_pair(&mut source, &mut peer, route).is_err());
        assert!(!trace.borrow().calls.contains(&(endpoint, "reset-before")));
    }
}

#[test]
fn full_pair_wiring_keeps_pure_binding_and_batch_scoped_validation_separate() {
    let source = include_str!("../../shared_memory.rs");
    let full = source
        .split("pub(crate) fn validate_gfx942_xgmi_route_with_peer(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn validate_gfx942_xgmi_publication_with_peer(")
        .next()
        .unwrap();
    assert!(
        full.find("self.validate_gfx942_xgmi_pair_binding(peer, route)?")
            .unwrap()
            < full.find("pair_currentness::with_terminal_pair").unwrap()
    );
    assert_eq!(full.matches("check_xgmi_pair_currentness").count(), 1);
    assert!(!full.contains("check_currentness()"));
    assert!(!full.contains("check_xgmi_route_currentness"));
    let batch = source
        .split("pub(crate) fn validate_gfx942_xgmi_publication_with_peer(")
        .nth(1)
        .unwrap()
        .split("fn validate_gfx942_xgmi_pair_binding(")
        .next()
        .unwrap();
    assert_eq!(
        batch.matches("check_xgmi_publication_currentness").count(),
        2
    );
    assert!(!batch.contains("check_xgmi_pair_currentness"));
}
