use super::*;
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
    let mut facts = route();
    facts.bandwidth = 0;
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
