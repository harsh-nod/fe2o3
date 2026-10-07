use super::*;
use crate::queue_resources::queue_resource_plan_for_test_v1;
use crate::shared_memory::{
    QueueConstructionFaultV1 as Fault, QueueConstructionMemoryFixtureV1 as Memory,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

impl RingMemoryV1 for Memory {
    fn preflight_cpu(&self, ring: &CpuRingAuthorityV1) -> Result<(), MemorySessionError> {
        match ring {
            CpuRingAuthorityV1::AqlSpecial(t) => self.preflight_cpu(t),
            CpuRingAuthorityV1::ExecutableProbe(t) => self.preflight_cpu(t),
            CpuRingAuthorityV1::UserptrProbe(t) => self.preflight_cpu(t),
        }
    }
    fn preflight_mapped(&self, ring: &MappedRingV1) -> Result<(), MemorySessionError> {
        match ring {
            MappedRingV1::AqlSpecial(t) => self.preflight_mapped(t),
            MappedRingV1::ExecutableProbe(t) => self.preflight_mapped(t),
            MappedRingV1::UserptrProbe(t) => self.preflight_mapped(t),
        }
    }
    fn map(&mut self, ring: CpuRingAuthorityV1) -> Result<MappedRingV1, MemorySessionError> {
        match ring {
            CpuRingAuthorityV1::AqlSpecial(t) => self.map(t).map(MappedRingV1::AqlSpecial),
            CpuRingAuthorityV1::ExecutableProbe(t) => {
                self.map(t).map(MappedRingV1::ExecutableProbe)
            }
            CpuRingAuthorityV1::UserptrProbe(t) => self.map(t).map(MappedRingV1::UserptrProbe),
        }
    }
    fn retain(&mut self, ring: MappedRingV1) -> Result<RingAuthority, MemorySessionError> {
        match ring {
            MappedRingV1::AqlSpecial(t) => self.retain_ring(t).map(RingAuthority::AqlSpecial),
            MappedRingV1::ExecutableProbe(t) => {
                self.retain_ring(t).map(RingAuthority::ExecutableProbe)
            }
            MappedRingV1::UserptrProbe(t) => self.retain_ring(t).map(RingAuthority::UserptrProbe),
        }
    }
}

const BACKINGS: [QueueRingBackingV1; 3] = [
    QueueRingBackingV1::AqlSpecial,
    QueueRingBackingV1::ExecutableProbe,
    QueueRingBackingV1::UserptrProbe,
];

fn ring(memory: &mut Memory, backing: QueueRingBackingV1, bytes: usize) -> RingConstructionV1 {
    RingConstructionV1::Cpu(match backing {
        QueueRingBackingV1::AqlSpecial => CpuRingAuthorityV1::AqlSpecial(memory.allocate(bytes)),
        QueueRingBackingV1::ExecutableProbe => {
            CpuRingAuthorityV1::ExecutableProbe(memory.allocate(bytes))
        }
        QueueRingBackingV1::UserptrProbe => {
            CpuRingAuthorityV1::UserptrProbe(memory.allocate(bytes))
        }
    })
}

fn snapshot(
    ring: &RingConstructionV1,
) -> (
    SharedGttAllocationIdentityV1,
    crate::shared_memory::SharedGttAllocationLayoutV1,
) {
    match ring {
        RingConstructionV1::Cpu(CpuRingAuthorityV1::AqlSpecial(t)) => {
            (t.storage_identity(), t.layout())
        }
        RingConstructionV1::Cpu(CpuRingAuthorityV1::ExecutableProbe(t)) => {
            (t.storage_identity(), t.layout())
        }
        RingConstructionV1::Cpu(CpuRingAuthorityV1::UserptrProbe(t)) => {
            (t.storage_identity(), t.layout())
        }
        RingConstructionV1::Mapped(MappedRingV1::AqlSpecial(t)) => {
            (t.storage_identity(), t.layout())
        }
        RingConstructionV1::Mapped(MappedRingV1::ExecutableProbe(t)) => {
            (t.storage_identity(), t.layout())
        }
        RingConstructionV1::Mapped(MappedRingV1::UserptrProbe(t)) => {
            (t.storage_identity(), t.layout())
        }
        _ => panic!("expected actual construction token"),
    }
}

#[test]
fn ring_handoffs_preserve_each_profile_and_transfer_once() {
    for backing in BACKINGS {
        let mut memory = Memory::new();
        let mut ring = ring(&mut memory, backing, 4096);
        let expected = snapshot(&ring);
        assert!(ring.take_retained().is_err());
        assert_eq!(snapshot(&ring), expected);
        ring.map_in_place(&mut memory).unwrap();
        assert_eq!(snapshot(&ring), expected);
        assert!(ring.map_in_place(&mut memory).is_err());
        let before = memory.calls();
        ring.retain_in_place(&mut memory).unwrap();
        assert_eq!(memory.calls(), before);
        let authority = ring.take_retained().unwrap();
        assert_eq!(authority.backing(), backing);
        assert_eq!(
            authority.facts().logical_bytes(),
            expected.1.requested_bytes()
        );
        assert_eq!(authority.facts().gpu_va_bytes(), expected.1.gpu_va_bytes());
        let retained_identity = match authority {
            RingAuthority::AqlSpecial(a) => a.into_token().storage_identity(),
            RingAuthority::ExecutableProbe(a) => a.into_token().storage_identity(),
            RingAuthority::UserptrProbe(a) => a.into_token().storage_identity(),
        };
        assert_eq!(retained_identity, expected.0);
        assert!(ring.take_retained().is_err());
        assert!(ring.retain_in_place(&mut memory).is_err());
        assert_eq!(memory.calls(), before);
    }
}

#[test]
fn ring_preflight_rejects_foreign_and_closed_sessions_without_moving_tokens() {
    for backing in BACKINGS {
        let mut owner = Memory::new();
        let mut foreign = Memory::new();
        let mut ring = ring(&mut owner, backing, 4096);
        let expected = snapshot(&ring);
        let before = foreign.calls();
        assert!(ring.map_in_place(&mut foreign).is_err());
        assert_eq!(snapshot(&ring), expected);
        assert_eq!(foreign.calls(), before);
        ring.map_in_place(&mut owner).unwrap();
        let before = foreign.calls();
        assert!(ring.retain_in_place(&mut foreign).is_err());
        assert_eq!(snapshot(&ring), expected);
        assert_eq!(foreign.calls(), before);
        owner.close();
        let before = owner.calls();
        assert!(ring.retain_in_place(&mut owner).is_err());
        assert_eq!(snapshot(&ring), expected);
        assert_eq!(owner.calls(), before);

        let mut owner = Memory::new();
        let mut cpu = self::ring(&mut owner, backing, 4096);
        let expected = snapshot(&cpu);
        owner.close();
        let before = owner.calls();
        assert!(cpu.map_in_place(&mut owner).is_err());
        assert_eq!(snapshot(&cpu), expected);
        assert_eq!(owner.calls(), before);
    }
}

#[test]
fn ring_admitted_map_failures_match_exact_r88_custody_and_keep_existing_charges() {
    for backing in BACKINGS {
        for (fault, mapped, panic) in [
            (Fault::MapError, false, false),
            (Fault::MapPanic, false, true),
            (Fault::CurrentnessError(1), false, false),
            (Fault::CurrentnessError(2), false, false),
            (Fault::CurrentnessPanic(1), false, true),
            (Fault::CurrentnessPanic(2), false, true),
            (Fault::ProjectionError, true, false),
            (Fault::ProjectionPanic, true, true),
            (Fault::CommitError, true, false),
            (Fault::CommitPanic, true, true),
        ] {
            let mut memory = Memory::new();
            let _device = memory.charged_device();
            let _charged =
                memory.allocate::<HostVisibleCoherentGttV1>(COMPLETION_SIGNAL_ARENA_BYTES_V1);
            let host = memory.host_usage();
            let device = memory.device_usage();
            let mut ring = ring(&mut memory, backing, 4096);
            let (identity, layout) = snapshot(&ring);
            memory.arm(fault);
            let result = catch_unwind(AssertUnwindSafe(|| ring.map_in_place(&mut memory)));
            assert_eq!(result.is_err(), panic);
            match result {
                Ok(result) => assert!(result.is_err()),
                Err(payload) => Memory::assert_panic(fault, payload.as_ref()),
            }
            assert!(matches!(ring, RingConstructionV1::InSession(id) if id == identity));
            match backing {
                QueueRingBackingV1::AqlSpecial => {
                    memory.assert_terminal::<AqlQueueGttV1>(identity, layout, mapped)
                }
                QueueRingBackingV1::ExecutableProbe => {
                    memory.assert_terminal::<ExecutableAqlQueueProbeGttV1>(identity, layout, mapped)
                }
                QueueRingBackingV1::UserptrProbe => {
                    memory.assert_terminal::<UserptrAqlQueueProbeGttV1>(identity, layout, mapped)
                }
            }
            assert_eq!(memory.host_usage(), host);
            assert_eq!(memory.device_usage(), device);
            let before = memory.calls();
            assert!(ring.map_in_place(&mut memory).is_err());
            assert!(ring.retain_in_place(&mut memory).is_err());
            assert_eq!(memory.calls(), before);
        }
    }
}

fn prefix(
    memory: &mut Memory,
    backing: QueueRingBackingV1,
    sizes: [usize; 4],
    foreign_vm: bool,
) -> QueueResourcePrefixV1 {
    let mut ring = ring(memory, backing, sizes[0]);
    ring.map_in_place(memory).unwrap();
    ring.retain_in_place(memory).unwrap();
    QueueResourcePrefixV1::new(
        ring.take_retained().unwrap(),
        memory.control(sizes[1], foreign_vm),
        memory.eop(sizes[2]),
        memory.context_save(sizes[3]),
    )
}

fn identities(
    prefix: &QueueResourcePrefixV1,
) -> Vec<(
    fe2o3_runtime_model::MemoryMappingKeyV1,
    fe2o3_runtime_model::MemoryPublicationKeyV1,
)> {
    [
        prefix.ring.as_ref().map(RingAuthority::facts),
        prefix.control.as_ref().map(|a| a.facts()),
        prefix.eop.as_ref().map(|a| a.facts()),
        prefix.context_save.as_ref().map(|a| a.facts()),
    ]
    .into_iter()
    .flatten()
    .map(|f| (f.mapping(), f.publication()))
    .collect()
}

fn sizes() -> [usize; 4] {
    let plan = queue_resource_plan_for_test_v1(4096);
    [
        4096,
        CONTROL_BYTES,
        plan.end_of_pipe().mapping_bytes() as usize,
        plan.context_save().mapping_bytes() as usize,
    ]
}

#[test]
fn resource_prefix_success_transfers_exact_roster_once_without_native_calls() {
    for backing in BACKINGS {
        let mut memory = Memory::new();
        let mut prefix = prefix(&mut memory, backing, sizes(), false);
        let before = identities(&prefix);
        let calls = memory.calls();
        let sequence = AtomicU64::new(123);
        prefix
            .build_with(
                memory.device(),
                queue_resource_plan_for_test_v1(4096),
                &sequence,
            )
            .unwrap();
        assert!(identities(&prefix).is_empty());
        assert_eq!(sequence.load(Ordering::Relaxed), 124);
        // Even a corrupted local attempt flag cannot overwrite occupied output.
        prefix.started = false;
        assert!(
            prefix
                .build_with(
                    memory.device(),
                    queue_resource_plan_for_test_v1(4096),
                    &sequence
                )
                .is_err()
        );
        let authority = prefix.take_complete().unwrap();
        validate_resource_authority(&authority).unwrap();
        let after: Vec<_> = [
            authority.ring.facts(),
            authority.control.facts(),
            authority.eop.facts(),
            authority.context_save.facts(),
        ]
        .into_iter()
        .map(|f| (f.mapping(), f.publication()))
        .collect();
        assert_eq!(after, before);
        assert_eq!(authority.view.plan.queue.id.0, 123);
        assert!(prefix.take_complete().is_err());
        assert_eq!(memory.calls(), calls);
    }
}

#[test]
fn resource_prefix_rejections_preserve_all_present_owners_and_identity_order() {
    for fault in 0..11 {
        let mut memory = Memory::new();
        let mut wrong_sizes = sizes();
        if fault < 4 {
            wrong_sizes[fault] += 4096;
        }
        let mut prefix = prefix(
            &mut memory,
            QueueRingBackingV1::AqlSpecial,
            wrong_sizes,
            fault == 4,
        );
        let removed: Option<Box<dyn std::any::Any>> = match fault {
            5 => prefix
                .ring
                .take()
                .map(|v| Box::new(v) as Box<dyn std::any::Any>),
            6 => prefix
                .control
                .take()
                .map(|v| Box::new(v) as Box<dyn std::any::Any>),
            7 => prefix
                .eop
                .take()
                .map(|v| Box::new(v) as Box<dyn std::any::Any>),
            8 => prefix
                .context_save
                .take()
                .map(|v| Box::new(v) as Box<dyn std::any::Any>),
            _ => None,
        };
        let before = identities(&prefix);
        let calls = memory.calls();
        let initial = if fault == 9 { u64::MAX } else { 125 };
        let sequence = AtomicU64::new(initial);
        let plan = queue_resource_plan_for_test_v1(if fault == 10 { 8192 } else { 4096 });
        assert!(
            prefix.build_with(memory.device(), plan, &sequence).is_err(),
            "fault {fault}"
        );
        assert_eq!(identities(&prefix), before);
        assert!(prefix.complete.is_none());
        assert_eq!(sequence.load(Ordering::Relaxed), initial);
        assert!(prefix.build_with(memory.device(), plan, &sequence).is_err());
        assert_eq!(identities(&prefix), before);
        assert_eq!(memory.calls(), calls);
        assert_eq!(removed.is_some(), (5..9).contains(&fault));
    }
}

#[test]
fn borrowed_view_rejects_every_mapping_and_publication_substitution() {
    let mut memory = Memory::new();
    let mut prefix = prefix(&mut memory, QueueRingBackingV1::AqlSpecial, sizes(), false);
    prefix
        .build_with(
            memory.device(),
            queue_resource_plan_for_test_v1(4096),
            &AtomicU64::new(127),
        )
        .unwrap();
    let authority = prefix.complete.as_ref().unwrap();
    let facts = [
        authority.ring.facts(),
        authority.control.facts(),
        authority.eop.facts(),
        authority.context_save.facts(),
    ];
    let calls = memory.calls();
    for ordinal in 0..4 {
        for publication in [false, true] {
            let mut view = authority.view;
            let fields = &mut view.plan.resources;
            let binding = match ordinal {
                0 => &mut fields.ring,
                1 => &mut fields.control,
                2 => &mut fields.eop,
                _ => &mut fields.context_save,
            };
            if publication {
                binding.publication.id.0 += 1;
            } else {
                binding.mapping.id.0 += 1;
            }
            assert!(validate_resource_view(view, facts).is_err());
            validate_resource_authority(authority).unwrap();
        }
    }
    assert_eq!(memory.calls(), calls);
}
