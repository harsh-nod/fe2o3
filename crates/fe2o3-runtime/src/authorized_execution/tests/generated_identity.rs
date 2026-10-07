use super::*;
use crate::generated_source::{GeneratedBufferSlotV1, GeneratedHostRosterV1};
use crate::{
    GeneratedGfx942PersistentStorageV1, Gfx942RuntimeBufferAccessV1 as Access,
    RuntimeGfx942GeneratedReservationErrorV1 as Error, RuntimeGfx942GeneratedSourceMutV1,
    RuntimeGfx942GeneratedSourceV1,
};
use fe2o3_kfd::{
    GFX942_MAX_FIXED_DISPATCH_DATA_V1, Gfx942FixedDispatchPacketV1, Gfx942KfdDispatchPointerFixupV1,
};
use std::sync::Arc;

#[derive(Debug, Eq, PartialEq)]
struct RosterSnapshot {
    identity: *const (),
    buffers: [Option<GeneratedBufferSlotV1>; GFX942_MAX_FIXED_DISPATCH_DATA_V1],
    count: usize,
    readback_bytes: u64,
    fixup_count: usize,
    dispatch: [u8; 32],
}

fn roster_snapshot(roster: &GeneratedHostRosterV1) -> RosterSnapshot {
    RosterSnapshot {
        identity: Arc::as_ptr(&roster.source_identity),
        buffers: roster.buffers,
        count: roster.count,
        readback_bytes: roster.readback_bytes,
        fixup_count: roster.fixup_count,
        dispatch: roster.dispatch_contract_sha256,
    }
}

fn copy_roster(roster: &GeneratedHostRosterV1) -> GeneratedHostRosterV1 {
    GeneratedHostRosterV1 {
        source_identity: Arc::clone(&roster.source_identity),
        buffers: roster.buffers,
        count: roster.count,
        readback_bytes: roster.readback_bytes,
        fixup_count: roster.fixup_count,
        dispatch_contract_sha256: roster.dispatch_contract_sha256,
    }
}

#[derive(Debug, Eq, PartialEq)]
struct BufferSnapshot {
    address: *const u8,
    bytes: Vec<u8>,
    access: Option<Access>,
}

#[derive(Debug, Eq, PartialEq)]
struct HostSnapshot {
    identity: *const (),
    executable_address: *const u8,
    executable: Vec<u8>,
    buffers: Vec<BufferSnapshot>,
    fixup_address: *const Gfx942KfdDispatchPointerFixupV1,
    fixups: Vec<Gfx942KfdDispatchPointerFixupV1>,
    object: [u8; 32],
    object_length: u64,
    kernel: String,
    dispatch: [u8; 32],
    descriptor_offset: u64,
    kernarg_alignment: u64,
    timeout: u32,
}

fn host_snapshot(data: &crate::persistent_projection::PersistentDispatchDataV1) -> HostSnapshot {
    HostSnapshot {
        identity: Arc::as_ptr(data.source_identity()),
        executable_address: data.executable_image().as_ptr(),
        executable: data.executable_image().to_vec(),
        buffers: data
            .buffers()
            .iter()
            .enumerate()
            .map(|(index, buffer)| BufferSnapshot {
                address: buffer.bytes().as_ptr(),
                bytes: buffer.bytes().to_vec(),
                access: data.buffer_access(index),
            })
            .collect(),
        fixup_address: data.pointer_fixups().as_ptr(),
        fixups: data.pointer_fixups().to_vec(),
        object: data.identity().object_sha256(),
        object_length: data.finalized_hsaco_length(),
        kernel: data.kernel_name().to_owned(),
        dispatch: data.dispatch_contract_sha256(),
        descriptor_offset: data.descriptor_offset(),
        kernarg_alignment: data.kernarg_alignment(),
        timeout: data.timeout_milliseconds(),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ControlSnapshot {
    program: usize,
    geometry: fe2o3_aql::AqlDispatchGeometryV1,
    ordering: fe2o3_aql::AqlDispatchOrderingV1,
    dynamic_group_segment_bytes: u32,
    buffer_count: usize,
}

fn control_snapshot(packet: &Gfx942FixedDispatchPacketV1) -> ControlSnapshot {
    ControlSnapshot {
        program: packet.program_index(),
        geometry: packet.geometry(),
        ordering: packet.ordering(),
        dynamic_group_segment_bytes: packet.dynamic_group_segment_bytes(),
        buffer_count: packet.buffer_count(),
    }
}

struct Transferred {
    hsaco: Vec<u8>,
    authority: TestAuthorityV1,
    storage: GeneratedGfx942PersistentStorageV1,
    roster: GeneratedHostRosterV1,
    host: HostSnapshot,
    control: ControlSnapshot,
    destination: Option<Gfx942FixedDispatchPacketV1>,
}

impl Transferred {
    fn new() -> Self {
        let (hsaco, projection) = source_projection();
        let authority = source_authority(&projection);
        let roster = RuntimeGfx942GeneratedSourceV1::new(&projection, &hsaco, &authority)
            .validate(7)
            .unwrap();
        let host = host_snapshot(projection.data());
        let control = control_snapshot(projection.packet());
        let mut storage = projection.into_generated_storage_v1();
        let mut destination = None;
        {
            let mut source =
                RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &hsaco, &authority);
            assert_eq!(
                roster_snapshot(&source.validate(7).unwrap()),
                roster_snapshot(&roster)
            );
            assert!(source.transfer_control_into(&mut destination));
        }
        let result = Self {
            hsaco,
            authority,
            storage,
            roster,
            host,
            control,
            destination,
        };
        result.assert_retained();
        result
    }

    fn assert_retained(&self) {
        assert!(!self.storage.control_available());
        assert_eq!(host_snapshot(self.storage.data()), self.host);
        assert_eq!(
            control_snapshot(self.destination.as_ref().unwrap()),
            self.control
        );
    }

    fn validate_original(&self) -> GeneratedHostRosterV1 {
        let actual = RuntimeGfx942GeneratedSourceV1::from_generated_storage(
            &self.storage,
            &self.hsaco,
            &self.authority,
        )
        .validate(7)
        .unwrap();
        assert!(Arc::ptr_eq(
            &actual.source_identity,
            &self.roster.source_identity
        ));
        assert_eq!(roster_snapshot(&actual), roster_snapshot(&self.roster));
        assert!(actual.matches(&self.roster));
        assert!(self.roster.matches(&actual));
        self.assert_retained();
        actual
    }
}

#[derive(Debug)]
enum Coordinate {
    SourceIdentity,
    Ordinal(usize),
    Bytes(usize),
    Access(usize, Access),
    Missing(usize),
    Trailing(usize),
    Count,
    Readback,
    Fixups,
    Dispatch,
}

#[test]
fn generated_descriptor_matches_bind_every_coordinate_bidirectionally() {
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let expected = RuntimeGfx942GeneratedSourceV1::new(&projection, &hsaco, &authority)
        .validate(7)
        .unwrap();
    assert_eq!(expected.count, 3);
    assert_eq!(expected.readback_bytes, 60);
    assert_eq!(expected.fixup_count, 1);
    assert_eq!(expected.buffers.len(), 16);
    for index in 0..3 {
        assert_eq!(
            expected.buffers[index],
            Some(GeneratedBufferSlotV1 {
                ordinal: index,
                bytes: 16 + index as u64 * 4,
                access: Access::ReadWrite,
            })
        );
    }
    assert!(expected.buffers[3..].iter().all(Option::is_none));
    assert_eq!(
        expected.dispatch_contract_sha256,
        projection.dispatch_contract_sha256()
    );
    assert!(Arc::ptr_eq(
        &expected.source_identity,
        projection.data().source_identity()
    ));
    let original = roster_snapshot(&expected);
    let original_host = host_snapshot(projection.data());
    let mut storage = projection.into_generated_storage_v1();
    let source = RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &hsaco, &authority);
    let same = source.validate(7).unwrap();
    assert_eq!(roster_snapshot(&same), original);
    assert!(expected.matches(&same));
    assert!(same.matches(&expected));
    assert!(source.matches_roster(&expected));
    let mut cases = vec![
        Coordinate::SourceIdentity,
        Coordinate::Count,
        Coordinate::Readback,
        Coordinate::Fixups,
        Coordinate::Dispatch,
    ];
    for index in 0..3 {
        cases.extend([
            Coordinate::Ordinal(index),
            Coordinate::Bytes(index),
            Coordinate::Access(index, Access::ReadOnly),
            Coordinate::Access(index, Access::WriteOnly),
            Coordinate::Missing(index),
        ]);
    }
    cases.extend((3..16).map(Coordinate::Trailing));
    assert_eq!(cases.len(), 33);
    for case in cases {
        let mut changed = copy_roster(&expected);
        match case {
            Coordinate::SourceIdentity => changed.source_identity = Arc::new(()),
            Coordinate::Ordinal(index) => changed.buffers[index].as_mut().unwrap().ordinal += 1,
            Coordinate::Bytes(index) => changed.buffers[index].as_mut().unwrap().bytes += 1,
            Coordinate::Access(index, access) => {
                changed.buffers[index].as_mut().unwrap().access = access
            }
            Coordinate::Missing(index) => changed.buffers[index] = None,
            Coordinate::Trailing(index) => {
                changed.buffers[index] = Some(GeneratedBufferSlotV1 {
                    ordinal: index,
                    bytes: 4,
                    access: Access::ReadOnly,
                })
            }
            Coordinate::Count => changed.count += 1,
            Coordinate::Readback => changed.readback_bytes += 1,
            Coordinate::Fixups => changed.fixup_count += 1,
            Coordinate::Dispatch => changed.dispatch_contract_sha256[0] ^= 1,
        }
        let substituted = roster_snapshot(&changed);
        assert_ne!(substituted, original, "nontrivial substitution: {case:?}");
        assert!(
            !expected.matches(&changed),
            "forward descriptor match: {case:?}"
        );
        assert!(
            !changed.matches(&expected),
            "reverse descriptor match: {case:?}"
        );
        assert!(
            !source.matches_roster(&changed),
            "source descriptor match: {case:?}"
        );
        assert_eq!(roster_snapshot(&expected), original);
        assert_eq!(roster_snapshot(&changed), substituted);
        assert!(source.matches_roster(&expected));
        assert_eq!(roster_snapshot(&source.validate(7).unwrap()), original);
    }
    assert!(storage.control_available());
    assert_eq!(host_snapshot(storage.data()), original_host);
}

#[test]
fn generated_identity_survives_transfer_and_rejects_identical_replacement() {
    let original = Transferred::new();
    let replacement = Transferred::new();
    assert_eq!(replacement.hsaco, original.hsaco);
    let actual = original.validate_original();
    let other = replacement.validate_original();
    assert!(!Arc::ptr_eq(
        &actual.source_identity,
        &other.source_identity
    ));
    let mut other_fields = roster_snapshot(&other);
    other_fields.identity = Arc::as_ptr(&actual.source_identity);
    assert_eq!(other_fields, roster_snapshot(&actual));
    assert!(
        !actual.matches(&other),
        "transferred replacement matched original"
    );
    assert!(
        !other.matches(&actual),
        "transferred original matched replacement"
    );
    for (left, right) in original
        .storage
        .buffers()
        .iter()
        .zip(replacement.storage.buffers())
    {
        assert_eq!(left.bytes(), right.bytes());
        assert_ne!(left.bytes().as_ptr(), right.bytes().as_ptr());
    }
    original.assert_retained();
    replacement.assert_retained();
}

#[test]
fn generated_transferred_source_rejects_later_artifact_substitution() {
    for change_length in [false, true] {
        let fixture = Transferred::new();
        fixture.validate_original();
        let mut changed = fixture.hsaco.clone();
        if change_length {
            changed.push(0);
        } else {
            changed[0] ^= 1;
        }
        let source = RuntimeGfx942GeneratedSourceV1::from_generated_storage(
            &fixture.storage,
            &changed,
            &fixture.authority,
        );
        assert!(
            matches!(source.validate(7), Err(Error::ArtifactMismatch)),
            "post-transfer artifact substitution: length={change_length}"
        );
        fixture.assert_retained();
        fixture.validate_original();
    }
}

#[test]
fn generated_transferred_source_rechecks_authority_and_currentness() {
    for field in 0..6 {
        let mut fixture = Transferred::new();
        fixture.validate_original();
        let original = TestAuthorityV1 {
            object: fixture.authority.object,
            length: fixture.authority.length,
            kernel: fixture.authority.kernel,
            dispatch: fixture.authority.dispatch,
            device: fixture.authority.device,
            current: fixture.authority.current.clone(),
            checks: fixture.authority.checks.clone(),
            fault: fixture.authority.fault.clone(),
        };
        match field {
            0 => fixture.authority.object[0] ^= 1,
            1 => fixture.authority.length += 1,
            2 => fixture.authority.kernel = "other",
            3 => fixture.authority.dispatch[0] ^= 1,
            4 => fixture.authority.device += 1,
            _ => fixture.authority.current.set(false),
        }
        {
            let source = RuntimeGfx942GeneratedSourceV1::from_generated_storage(
                &fixture.storage,
                &fixture.hsaco,
                &fixture.authority,
            );
            if field < 5 {
                assert!(
                    matches!(source.validate(7), Err(Error::AuthorityMismatch)),
                    "post-transfer authority substitution: field={field}"
                );
            } else {
                assert!(
                    matches!(source.validate(7), Err(Error::AuthorityNotCurrent)),
                    "post-transfer stale authority validation"
                );
                assert!(
                    matches!(source.revalidate(), Err(Error::AuthorityNotCurrent)),
                    "post-transfer stale authority revalidation"
                );
            }
        }
        fixture.assert_retained();
        fixture.authority = original;
        fixture.validate_original();
    }
}
