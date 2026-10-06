use super::*;

#[test]
fn only_dependency_arena_presence_selects_queue_first_close() {
    let mut group = Gfx950EngineeringPeerGroupV1 {
        incarnation: 7,
        contexts: vec![],
        buffers: BTreeMap::new(),
        next_buffer: 1,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
        projection_mlp_scratch: None,
    };
    assert!(!group.has_peer_dependency_arena());
    for kind in [
        BufferKind::PublicVram,
        BufferKind::WaveOutputStateV5,
        BufferKind::WaveMlpStateV1,
        BufferKind::WaveMlpTilesStateV2,
        BufferKind::WaveQkvAttentionOutputTilesStateV6,
        BufferKind::PeerDependencyArena,
        BufferKind::CombinedMlpStateV1,
    ] {
        group.buffers.insert(
            1,
            BufferRecord {
                token: Gfx950EngineeringPeerBufferV1 {
                    group: 7,
                    id: 1,
                    owner: 0,
                    bytes: 4096,
                },
                local_id: 1,
                mapping: PeerMapping::new(vec![]).unwrap(),
                kind,
            },
        );
        assert_eq!(
            group.has_peer_dependency_arena(),
            matches!(
                kind,
                BufferKind::PeerDependencyArena | BufferKind::CombinedMlpStateV1
            )
        );
    }
    group.buffers.clear();
    group.closed = true;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DependencyCloseEvent {
    Check,
    Destroy(usize),
    ReleaseAll,
    Finish(usize),
}

#[derive(Default)]
struct DependencyCloseState {
    events: Vec<DependencyCloseEvent>,
    live_at_event: Vec<Vec<usize>>,
    live: BTreeSet<usize>,
    completed: Vec<usize>,
    dropped_pending: Vec<usize>,
    poisoned: bool,
}

struct MockDependencyDisabled {
    rank: usize,
    completed: bool,
    state: std::rc::Rc<std::cell::RefCell<DependencyCloseState>>,
}

impl Drop for MockDependencyDisabled {
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();
        assert!(state.live.remove(&self.rank));
        if !self.completed {
            state.poisoned = true;
            state.dropped_pending.push(self.rank);
        }
    }
}

struct MockDependencyClose {
    participants: usize,
    fail_at: Option<usize>,
    state: std::rc::Rc<std::cell::RefCell<DependencyCloseState>>,
}

impl MockDependencyClose {
    fn new(participants: usize, fail_at: Option<usize>) -> Self {
        Self {
            participants,
            fail_at,
            state: std::rc::Rc::default(),
        }
    }

    fn step(&mut self, event: DependencyCloseEvent) -> Result<()> {
        let mut state = self.state.borrow_mut();
        let live = state.live.iter().copied().collect();
        state.live_at_event.push(live);
        state.events.push(event);
        if self.fail_at == Some(state.events.len()) {
            return Err("injected dependency close failure".into());
        }
        Ok(())
    }
}

impl PeerDependencyCloseBackend for MockDependencyClose {
    type Disabled = MockDependencyDisabled;

    fn participants(&self) -> usize {
        self.participants
    }

    fn check(&mut self) -> Result<()> {
        self.step(DependencyCloseEvent::Check)
    }

    fn destroy(&mut self, rank: usize) -> Result<Self::Disabled> {
        self.step(DependencyCloseEvent::Destroy(rank))?;
        assert!(self.state.borrow_mut().live.insert(rank));
        Ok(MockDependencyDisabled {
            rank,
            completed: false,
            state: self.state.clone(),
        })
    }

    fn release_all(&mut self) -> Result<()> {
        self.step(DependencyCloseEvent::ReleaseAll)
    }

    fn finish_context(&mut self, rank: usize, mut disabled: Self::Disabled) -> Result<()> {
        assert_eq!(rank, disabled.rank);
        self.step(DependencyCloseEvent::Finish(rank))?;
        disabled.completed = true;
        self.state.borrow_mut().completed.push(rank);
        Ok(())
    }
}

fn dependency_close_events(participants: usize) -> Vec<DependencyCloseEvent> {
    let mut events = vec![DependencyCloseEvent::Check];
    events.extend((0..participants).map(DependencyCloseEvent::Destroy));
    events.push(DependencyCloseEvent::ReleaseAll);
    events.push(DependencyCloseEvent::Check);
    events.extend((0..participants).map(DependencyCloseEvent::Finish));
    events
}

#[test]
fn dependency_close_destroys_every_queue_and_holds_every_token_before_release() {
    for participants in [2, 8] {
        let mut backend = MockDependencyClose::new(participants, None);
        run_peer_dependency_close(&mut backend).unwrap();
        let state = backend.state.borrow();
        assert_eq!(state.events, dependency_close_events(participants));
        let all = (0..participants).collect::<Vec<_>>();
        assert_eq!(state.live_at_event[participants + 1], all);
        assert_eq!(state.live_at_event[participants + 2], all);
        for rank in 0..participants {
            assert_eq!(
                state.live_at_event[participants + 3 + rank],
                (rank..participants).collect::<Vec<_>>()
            );
        }
        assert_eq!(state.completed, all);
        assert!(state.live.is_empty());
        assert!(state.dropped_pending.is_empty());
        assert!(!state.poisoned);
    }
}

#[test]
fn dependency_close_every_callback_failure_stops_and_drops_all_uncompleted_tokens() {
    for participants in [2, 8] {
        let events = dependency_close_events(participants);
        for fail_at in 1..=events.len() {
            let mut backend = MockDependencyClose::new(participants, Some(fail_at));
            assert!(run_peer_dependency_close(&mut backend).is_err());
            let successful = &events[..fail_at - 1];
            let destroyed = successful
                .iter()
                .filter_map(|event| match event {
                    DependencyCloseEvent::Destroy(rank) => Some(*rank),
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            let completed = successful
                .iter()
                .filter_map(|event| match event {
                    DependencyCloseEvent::Finish(rank) => Some(*rank),
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            let pending = destroyed
                .difference(&completed)
                .copied()
                .collect::<BTreeSet<_>>();
            let state = backend.state.borrow();
            assert_eq!(state.events, events[..fail_at]);
            assert!(state.live.is_empty());
            assert_eq!(
                state.completed.iter().copied().collect::<BTreeSet<_>>(),
                completed
            );
            assert_eq!(
                state
                    .dropped_pending
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>(),
                pending
            );
            assert_eq!(state.poisoned, !pending.is_empty());
        }
    }
}

#[test]
fn dependency_close_second_destroy_failure_never_unmaps_or_finishes() {
    let mut backend = MockDependencyClose::new(2, Some(3));
    assert!(run_peer_dependency_close(&mut backend).is_err());
    let state = backend.state.borrow();
    assert_eq!(
        state.events,
        [
            DependencyCloseEvent::Check,
            DependencyCloseEvent::Destroy(0),
            DependencyCloseEvent::Destroy(1)
        ]
    );
    assert_eq!(state.dropped_pending, [0]);
    assert!(state.completed.is_empty());
    assert!(state.poisoned);
}

#[test]
fn dependency_close_final_fence_failure_never_releases_control_or_completes_tokens() {
    let mut backend = MockDependencyClose::new(2, Some(5));
    assert!(run_peer_dependency_close(&mut backend).is_err());
    let state = backend.state.borrow();
    assert_eq!(state.events, dependency_close_events(2)[..5]);
    assert_eq!(
        state
            .dropped_pending
            .iter()
            .copied()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([0, 1])
    );
    assert!(state.completed.is_empty());
    assert!(state.poisoned);
}

#[test]
fn dependency_close_rejects_invalid_participant_count_before_any_callback() {
    for participants in [0, 1, 9, usize::MAX] {
        let mut backend = MockDependencyClose::new(participants, None);
        assert!(run_peer_dependency_close(&mut backend).is_err());
        let state = backend.state.borrow();
        assert!(state.events.is_empty());
        assert!(state.live.is_empty());
        assert!(!state.poisoned);
    }
}

#[test]
fn ordinary_close_keeps_old_order_and_dependency_failure_still_reaches_group_finish() {
    let source = include_str!("engineering_gfx950_peer.rs");
    let body = source.split("pub fn close(").nth(1).unwrap();
    let branch = body.find("if self.has_peer_dependency_arena()").unwrap();
    let special = body
        .find("self.close_peer_dependency_contexts(tokens)?;")
        .unwrap();
    let ordinary = body.find("} else {").unwrap();
    let release = body.find("self.release(token)?;").unwrap();
    let close = body.find("context.close_inner()?;").unwrap();
    let closed = body.find("self.closed = true;").unwrap();
    let finish = body.find("self.finish(result)").unwrap();
    assert!(branch < special && special < ordinary && ordinary < release);
    assert!(release < close && close < closed && closed < finish);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FenceEvent {
    Shared,
    Full(usize),
    Idle(usize),
    Queue(usize),
}

struct RecordingContextFence {
    participants: usize,
    events: Vec<FenceEvent>,
    fail_at: Option<usize>,
}

impl RecordingContextFence {
    fn record(&mut self, event: FenceEvent) -> Result<()> {
        self.events.push(event);
        if self.fail_at == Some(self.events.len()) {
            Err("injected context fence failure".into())
        } else {
            Ok(())
        }
    }
}

impl ContextFenceBackend for RecordingContextFence {
    fn participants(&self) -> usize {
        self.participants
    }
    fn shared_full_currentness(&mut self) -> Result<()> {
        self.record(FenceEvent::Shared)
    }
    fn full_currentness(&mut self, rank: usize) -> Result<()> {
        self.record(FenceEvent::Full(rank))
    }
    fn idle(&mut self, rank: usize) -> Result<()> {
        self.record(FenceEvent::Idle(rank))
    }
    fn idle_after_currentness(&mut self, rank: usize) -> Result<()> {
        self.record(FenceEvent::Queue(rank))
    }
}

fn fence_events(participants: usize, shared: bool) -> Vec<FenceEvent> {
    if shared {
        std::iter::once(FenceEvent::Shared)
            .chain((0..participants).map(FenceEvent::Queue))
            .collect()
    } else {
        (0..participants)
            .flat_map(|rank| [FenceEvent::Full(rank), FenceEvent::Idle(rank)])
            .collect()
    }
}

#[test]
fn shared_context_fence_checks_every_queue_after_one_fresh_group_observation() {
    for participants in [2, 8] {
        let mut backend = RecordingContextFence {
            participants,
            events: vec![],
            fail_at: None,
        };
        run_context_fence(&mut backend, true).unwrap();
        assert_eq!(backend.events, fence_events(participants, true));
        run_context_fence(&mut backend, true).unwrap();
        let expected = fence_events(participants, true).repeat(2);
        assert_eq!(backend.events, expected);
        backend.fail_at = Some(expected.len() + 1);
        assert!(run_context_fence(&mut backend, true).is_err());
        assert_eq!(&backend.events[..expected.len()], expected);
        assert_eq!(&backend.events[expected.len()..], [FenceEvent::Shared]);
    }
}

#[test]
fn ordinary_context_fence_keeps_full_and_idle_checks_in_rank_order() {
    for participants in [2, 8] {
        let mut backend = RecordingContextFence {
            participants,
            events: vec![],
            fail_at: None,
        };
        run_context_fence(&mut backend, false).unwrap();
        assert_eq!(backend.events, fence_events(participants, false));
    }
}

#[test]
fn every_context_fence_failure_stops_and_quarantines_the_group() {
    for participants in [2, 8] {
        for shared in [false, true] {
            let expected = fence_events(participants, shared);
            for fail_at in 1..=expected.len() {
                let mut backend = RecordingContextFence {
                    participants,
                    events: vec![],
                    fail_at: Some(fail_at),
                };
                let result = run_context_fence(&mut backend, shared);
                assert!(result.is_err());
                assert_eq!(backend.events, expected[..fail_at]);
                let mut group = Gfx950EngineeringPeerGroupV1 {
                    incarnation: 7,
                    contexts: vec![],
                    buffers: BTreeMap::new(),
                    next_buffer: 1,
                    poisoned: false,
                    closed: false,
                    shared_full_currentness: shared,
                    projection_mlp_scratch: None,
                };
                let token = Gfx950EngineeringPeerBufferV1 {
                    group: 7,
                    id: 1,
                    owner: 0,
                    bytes: 8,
                };
                assert!(group.finish(result).is_err());
                assert!(group.poisoned);
                let quarantine = "peer group is closed or quarantined";
                assert_eq!(group.require_active().unwrap_err(), quarantine);
                assert_eq!(group.allocate(0, &[1], 8).unwrap_err(), quarantine);
                assert_eq!(group.write(token, 0, &[0]).unwrap_err(), quarantine);
                assert_eq!(group.read(token, 0, 1).unwrap_err(), quarantine);
                assert_eq!(group.release(token).unwrap_err(), quarantine);
                assert_eq!(group.close().unwrap_err(), quarantine);
            }
        }
    }
}

#[test]
fn group_record_budget_is_bounded_by_participant_allocation_limits() {
    assert_eq!(group_allocation_limit(2).unwrap(), 4096);
    assert_eq!(group_allocation_limit(8).unwrap(), 16384);
    for world in [0, 1, 3, 9, usize::MAX] {
        assert!(group_allocation_limit(world).is_err());
    }
}
use crate::memory::MemorySessionError;

struct FakeBackend {
    events: Vec<&'static str>,
    check: usize,
    fail_check: Option<usize>,
    map_progress: u32,
    map_error: bool,
    unmap_progress: u32,
    unmap_error: bool,
    free_error: bool,
}

impl Default for FakeBackend {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            check: 0,
            fail_check: None,
            map_progress: 2,
            map_error: false,
            unmap_progress: 2,
            unmap_error: false,
            free_error: false,
        }
    }
}

impl PeerTransactionBackend for FakeBackend {
    fn check(&mut self) -> Result<()> {
        self.events.push("check");
        self.check += 1;
        if self.fail_check == Some(self.check) {
            Err("injected currentness".into())
        } else {
            Ok(())
        }
    }
    fn map(&mut self, peers: &[u32]) -> KernelOutcome<u32> {
        assert_eq!(peers, [3, 7]);
        self.events.push("map");
        KernelOutcome {
            value: self.map_progress,
            result: if self.map_error {
                Err(MemorySessionError::Injected("map"))
            } else {
                Ok(())
            },
        }
    }
    fn unmap(&mut self, peers: &[u32]) -> KernelOutcome<u32> {
        assert_eq!(peers, [3, 7]);
        self.events.push("unmap");
        KernelOutcome {
            value: self.unmap_progress,
            result: if self.unmap_error {
                Err(MemorySessionError::Injected("unmap"))
            } else {
                Ok(())
            },
        }
    }
    fn release_owner(&mut self) -> Result<()> {
        self.events.push("free-owner");
        if self.free_error {
            Err("injected owner release".into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn actual_mapping_transaction_requires_fences_and_unmaps_peers_before_owner_free() {
    let mut backend = FakeBackend::default();
    let mut mapping = PeerMapping::new(vec![3, 7]).unwrap();
    mapping.map(&mut backend).unwrap();
    assert_eq!(mapping.phase, Phase::PeersMapped);
    assert_eq!(mapping.mapped, 2);
    mapping.release(&mut backend).unwrap();
    assert_eq!(mapping.phase, Phase::Released);
    assert_eq!(mapping.unmapped, 2);
    assert_eq!(
        backend.events,
        [
            "check",
            "map",
            "check",
            "check",
            "unmap",
            "check",
            "free-owner"
        ]
    );
    assert!(mapping.release(&mut backend).is_err());
    assert!(mapping.map(&mut backend).is_err());
    assert_eq!(backend.events.len(), 7);
}

#[test]
fn every_map_prefix_errno_and_postcheck_failure_quarantines_without_retry_or_free() {
    for progress in [0, 1, 2, 3, u32::MAX] {
        for error in [false, true] {
            if progress == 2 && !error {
                continue;
            }
            let mut backend = FakeBackend {
                map_progress: progress,
                map_error: error,
                ..FakeBackend::default()
            };
            let mut mapping = PeerMapping::new(vec![3, 7]).unwrap();
            assert!(mapping.map(&mut backend).is_err());
            assert_eq!(mapping.phase, Phase::Quarantined);
            assert_eq!(mapping.mapped, progress);
            assert!(mapping.map(&mut backend).is_err());
            assert!(mapping.release(&mut backend).is_err());
            assert_eq!(backend.events, ["check", "map"]);
        }
    }
    for fail_check in [1, 2] {
        let mut backend = FakeBackend {
            fail_check: Some(fail_check),
            ..FakeBackend::default()
        };
        let mut mapping = PeerMapping::new(vec![3, 7]).unwrap();
        assert!(mapping.map(&mut backend).is_err());
        assert_eq!(mapping.phase, Phase::Quarantined);
        assert!(!backend.events.contains(&"free-owner"));
        assert!(!backend.events.contains(&"unmap"));
    }
}

#[test]
fn every_unmap_prefix_errno_and_postcheck_failure_prevents_owner_free() {
    for progress in [0, 1, 2, 3, u32::MAX] {
        for error in [false, true] {
            if progress == 2 && !error {
                continue;
            }
            let mut backend = FakeBackend {
                unmap_progress: progress,
                unmap_error: error,
                ..FakeBackend::default()
            };
            let mut mapping = PeerMapping::new(vec![3, 7]).unwrap();
            mapping.map(&mut backend).unwrap();
            assert!(mapping.release(&mut backend).is_err());
            assert_eq!(mapping.phase, Phase::Quarantined);
            assert_eq!(mapping.unmapped, progress);
            assert!(!backend.events.contains(&"free-owner"));
            let events = backend.events.len();
            assert!(mapping.release(&mut backend).is_err());
            assert_eq!(backend.events.len(), events);
        }
    }
    for fail_check in [3, 4] {
        let mut backend = FakeBackend {
            fail_check: Some(fail_check),
            ..FakeBackend::default()
        };
        let mut mapping = PeerMapping::new(vec![3, 7]).unwrap();
        mapping.map(&mut backend).unwrap();
        assert!(mapping.release(&mut backend).is_err());
        assert_eq!(mapping.phase, Phase::Quarantined);
        assert!(!backend.events.contains(&"free-owner"));
    }
}

#[test]
fn owner_release_failure_is_terminal_even_after_all_peers_are_unmapped() {
    let mut backend = FakeBackend {
        free_error: true,
        ..FakeBackend::default()
    };
    let mut mapping = PeerMapping::new(vec![3, 7]).unwrap();
    mapping.map(&mut backend).unwrap();
    assert!(mapping.release(&mut backend).is_err());
    assert_eq!(mapping.phase, Phase::Quarantined);
    assert_eq!(mapping.unmapped, 2);
    assert_eq!(backend.events.last(), Some(&"free-owner"));
    assert!(mapping.release(&mut backend).is_err());
    assert_eq!(
        backend
            .events
            .iter()
            .filter(|event| **event == "free-owner")
            .count(),
        1
    );
}

#[test]
fn local_only_buffers_do_not_issue_peer_ioctls_but_keep_all_fences() {
    let mut backend = FakeBackend::default();
    let mut mapping = PeerMapping::new(vec![]).unwrap();
    mapping.map(&mut backend).unwrap();
    mapping.release(&mut backend).unwrap();
    assert_eq!(
        backend.events,
        ["check", "check", "check", "check", "free-owner"]
    );
}

fn route() -> RouteFacts {
    RouteFacts {
        targets: [GfxTarget::Gfx950; 2],
        gpus: [1, 2],
        hives: [7, 7],
        node_from: 3,
        node_to: 4,
        source_node: 3,
        destination_node: 4,
        io: true,
        link_type: 11,
        flags: 1,
        bandwidth: 128,
        directional_links: 1,
    }
}

#[test]
fn route_admission_checks_target_hive_direction_enabled_link_and_uniqueness() {
    validate_route(route()).unwrap();
    let mut failures = Vec::new();
    for target in [GfxTarget::Gfx942, GfxTarget::Gfx950] {
        if target == GfxTarget::Gfx950 {
            continue;
        }
        for side in [0, 1] {
            let mut facts = route();
            facts.targets[side] = target;
            failures.push(facts);
        }
    }
    let mut facts = route();
    facts.gpus[1] = 1;
    failures.push(facts);
    let mut facts = route();
    facts.hives = [0, 0];
    failures.push(facts);
    let mut facts = route();
    facts.hives[1] = 8;
    failures.push(facts);
    let mut facts = route();
    facts.node_from = 4;
    failures.push(facts);
    let mut facts = route();
    facts.node_to = 3;
    failures.push(facts);
    let mut facts = route();
    facts.io = false;
    failures.push(facts);
    let mut facts = route();
    facts.link_type = 2;
    failures.push(facts);
    let mut facts = route();
    facts.flags = 0;
    failures.push(facts);
    for count in [0, 2] {
        let mut facts = route();
        facts.directional_links = count;
        failures.push(facts);
    }
    for facts in failures {
        assert!(validate_route(facts).is_err(), "{facts:?}");
    }
}

#[test]
fn unreported_bandwidth_does_not_override_enabled_route_or_rejection_flags() {
    validate_route(RouteFacts {
        bandwidth: 0,
        ..route()
    })
    .unwrap();
    for flags in [0, 3, 17, 33] {
        let error = validate_route(RouteFacts {
            bandwidth: 0,
            flags,
            ..route()
        })
        .unwrap_err();
        assert!(
            error.contains("link is disabled, noncoherent, peer-disabled, or has unknown flags")
        );
        assert!(error.contains("reported_max_bandwidth 0"));
    }
    let error = validate_route(RouteFacts {
        bandwidth: 0,
        link_type: 2,
        ..route()
    })
    .unwrap_err();
    assert!(error.contains("link type is not XGMI"));
}

#[test]
fn group_and_mapping_rosters_reject_duplicates_and_wrong_cardinality() {
    checked_roster(&[1, 2]).unwrap();
    checked_roster(&[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
    for ids in [vec![], vec![1], vec![1, 1], vec![0, 2], vec![1, 2, 3]] {
        assert!(checked_roster(&ids).is_err());
    }
    for peers in [vec![3, 3], vec![7, 3], vec![1, 2, 3, 4, 5, 6, 7, 8]] {
        assert!(PeerMapping::new(peers).is_err());
    }
}

#[test]
fn peer_bindings_require_read_only_access_to_an_explicit_mapping() {
    require_peer_access(0, 1, 7, &[3, 7], BufferAccessV1::Read).unwrap();
    for access in [BufferAccessV1::Write, BufferAccessV1::ReadWrite] {
        assert!(require_peer_access(0, 1, 7, &[3, 7], access).is_err());
    }
    assert!(require_peer_access(0, 2, 9, &[3, 7], BufferAccessV1::Read).is_err());
    for access in [
        BufferAccessV1::Read,
        BufferAccessV1::Write,
        BufferAccessV1::ReadWrite,
    ] {
        require_peer_access(0, 0, 1, &[], access).unwrap();
    }
}

#[test]
fn noncoherent_disabled_p2p_and_unknown_link_flags_are_not_admitted() {
    for flags in [1, 5, 9, 13] {
        validate_route(RouteFacts { flags, ..route() }).unwrap();
    }
    for flags in [0, 3, 17, 33, 0x8000_0001, u32::MAX] {
        assert!(validate_route(RouteFacts { flags, ..route() }).is_err());
    }
}

#[test]
fn group_tokens_are_incarnation_bound_and_failures_poison_without_native_access() {
    let token = Gfx950EngineeringPeerBufferV1 {
        group: 7,
        id: 3,
        owner: 0,
        bytes: 8,
    };
    let record = BufferRecord {
        token,
        local_id: 1,
        kind: BufferKind::PublicVram,
        mapping: PeerMapping {
            peers: vec![2],
            mapped: 1,
            unmapped: 0,
            phase: Phase::PeersMapped,
        },
    };
    let mut group = Gfx950EngineeringPeerGroupV1 {
        incarnation: 7,
        contexts: vec![],
        buffers: [(3, record)].into(),
        next_buffer: 4,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
        projection_mlp_scratch: None,
    };
    group.validate_token(token).unwrap();
    for changed in [
        Gfx950EngineeringPeerBufferV1 { group: 8, ..token },
        Gfx950EngineeringPeerBufferV1 { owner: 1, ..token },
        Gfx950EngineeringPeerBufferV1 { bytes: 16, ..token },
        Gfx950EngineeringPeerBufferV1 { id: 4, ..token },
    ] {
        assert!(group.validate_token(changed).is_err());
    }
    assert!(
        group
            .finish::<()>(Err("injected native uncertainty".into()))
            .is_err()
    );
    assert!(group.require_active().is_err());
    assert!(group.write(token, 0, &[0]).is_err());
    assert!(group.read(token, 0, 1).is_err());
    assert!(group.release(token).is_err());
    assert!(group.close().is_err());
}
