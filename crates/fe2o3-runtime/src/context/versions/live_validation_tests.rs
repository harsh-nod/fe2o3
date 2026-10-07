use super::*;

fn fixture() -> (ContextVersionsV1, RuntimeAllocationIdV1, AllocationRecordV1) {
    let id = RuntimeAllocationIdV1 {
        context_generation: 11,
        local: 1,
    };
    let device = RuntimeDeviceIdV1 {
        context_generation: 11,
        local: 7,
    };
    let mut versions = ContextVersionsV1::new(11, 4, 4, 4).unwrap();
    let mut enrolled = [None];
    versions
        .enroll_roster(&[enrollment(id, device, 64)], &mut enrolled)
        .unwrap();
    let reference = enrolled[0].unwrap();
    versions.commit_live(reference).unwrap();
    let record = AllocationRecordV1 {
        backend_allocation: 23,
        device,
        kind: RuntimeMemoryKindV1::HostVisible,
        byte_len: 64,
        journal: Some(reference),
    };
    (versions, id, record)
}

#[test]
fn live_validation_rejects_every_nonlive_phase() {
    for phase in [
        None,
        Some(AllocationPhaseV1::Provisional),
        Some(AllocationPhaseV1::Disposed),
    ] {
        let (mut versions, id, record) = fixture();
        let reference = record.journal.unwrap();
        assert_eq!(versions.validate_live(id, &record), Ok(reference));
        versions.phases[reference.slot] = phase;
        assert_eq!(
            versions.validate_live(id, &record),
            Err(ContextVersionJournalErrorV1::InvalidState)
        );
        assert_eq!(versions.phases[reference.slot], phase);
    }
    let (mut versions, id, record) = fixture();
    versions.phases.clear();
    assert_eq!(
        versions.validate_live(id, &record),
        Err(ContextVersionJournalErrorV1::InvalidState)
    );
}

#[test]
fn live_validation_binds_full_identity_device_and_extent() {
    for fault in 0..7 {
        let (versions, mut id, mut record) = fixture();
        match fault {
            0 => id.context_generation += 1,
            1 => id.local += 1,
            2 => record.device.context_generation += 1,
            3 => record.device.local += 1,
            4 => record.byte_len += 1,
            5 => record.journal.as_mut().unwrap().key.context_generation += 1,
            6 => record.journal.as_mut().unwrap().key.local += 1,
            _ => unreachable!(),
        }
        assert_eq!(
            versions.validate_live(id, &record),
            Err(ContextVersionJournalErrorV1::InvalidAllocationReference),
            "fault={fault}"
        );
    }
}

#[test]
fn live_validation_preserves_lookup_before_phase_error() {
    let (mut versions, id, mut record) = fixture();
    let reference = record.journal.unwrap();
    versions.phases[reference.slot] = Some(AllocationPhaseV1::Disposed);
    record.journal.as_mut().unwrap().key.local += 1;
    assert_eq!(
        versions.validate_live(id, &record),
        Err(ContextVersionJournalErrorV1::InvalidAllocationReference)
    );
    record.journal = None;
    assert_eq!(
        versions.validate_live(id, &record),
        Err(ContextVersionJournalErrorV1::InvalidState)
    );
}
