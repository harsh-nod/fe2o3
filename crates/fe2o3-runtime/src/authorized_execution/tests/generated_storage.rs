use super::*;
use crate::{
    RuntimeGfx942GeneratedReservationErrorV1 as Error, RuntimeGfx942GeneratedSourceMutV1,
    RuntimeGfx942GeneratedSourceV1,
};

#[test]
fn generated_storage_conversion_preserves_reserved_source_identity() {
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let expected = RuntimeGfx942GeneratedSourceV1::new(&projection, &hsaco, &authority)
        .validate(7)
        .unwrap();
    let mut storage = projection.into_generated_storage_v1();
    let source = RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &hsaco, &authority);
    assert!(source.validate(7).unwrap().matches(&expected));
    assert!(source.matches_roster(&expected));
}

#[test]
fn generated_storage_byte_identical_replacement_is_not_the_reserved_source() {
    let (hsaco, original) = source_projection();
    let (_, replacement) = source_projection();
    let authority = source_authority(&original);
    let expected = RuntimeGfx942GeneratedSourceV1::new(&original, &hsaco, &authority)
        .validate(7)
        .unwrap();
    let original = original.into_generated_storage_v1();
    let mut replacement = replacement.into_generated_storage_v1();
    {
        let source = RuntimeGfx942GeneratedSourceMutV1::new(&mut replacement, &hsaco, &authority);
        let actual = source.validate(7).unwrap();
        assert_eq!(actual.buffers, expected.buffers);
        assert_eq!(
            actual.dispatch_contract_sha256,
            expected.dispatch_contract_sha256
        );
        assert!(!actual.matches(&expected));
        assert!(!source.matches_roster(&expected));
    }
    assert!(original.control_available());
    assert!(replacement.control_available());
}

#[test]
fn generated_storage_control_transfer_is_one_shot_and_preserves_occupied_destination() {
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let expected = RuntimeGfx942GeneratedSourceV1::new(&projection, &hsaco, &authority)
        .validate(7)
        .unwrap();
    let mut storage = projection.into_generated_storage_v1();
    let (_, other) = source_projection_with_geometry(
        fe2o3_aql::AqlDispatchGeometryV1::new([128, 1, 1], [64, 1, 1]).unwrap(),
    );
    let mut other = other.into_generated_storage_v1();
    let mut destination = None;
    assert!(other.transfer_control_into(&mut destination));
    let original_destination = destination.as_ref().unwrap().geometry();
    {
        let mut source = RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &hsaco, &authority);
        assert!(!source.transfer_control_into(&mut destination));
        assert_eq!(
            destination.as_ref().unwrap().geometry(),
            original_destination
        );
        assert!(source.matches_roster(&expected));
        drop(destination.take());
        assert!(source.transfer_control_into(&mut destination));
        assert_ne!(
            destination.as_ref().unwrap().geometry(),
            original_destination
        );
        assert!(!source.transfer_control_into(&mut None));
        assert!(!source.matches_roster(&expected));
        assert!(matches!(
            source.validate(7),
            Err(Error::UnsupportedPreparation)
        ));
    }
    assert!(!storage.control_available());
    assert_eq!(storage.buffers().len(), 3);
    assert!(destination.is_some());
}

#[test]
fn generated_storage_authority_rejections_preserve_control_for_valid_retry() {
    let (hsaco, projection) = source_projection();
    let good = source_authority(&projection);
    let mut storage = projection.into_generated_storage_v1();
    for field in 0..6 {
        let mut authority = TestAuthorityV1 {
            object: good.object,
            length: good.length,
            kernel: good.kernel,
            dispatch: good.dispatch,
            device: good.device,
            current: true,
        };
        match field {
            0 => authority.object[0] ^= 1,
            1 => authority.length += 1,
            2 => authority.kernel = "other",
            3 => authority.dispatch[0] ^= 1,
            4 => authority.device += 1,
            _ => authority.current = false,
        }
        let source = RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &hsaco, &authority);
        if field < 5 {
            assert!(matches!(source.validate(7), Err(Error::AuthorityMismatch)));
        } else {
            assert!(matches!(
                source.validate(7),
                Err(Error::AuthorityNotCurrent)
            ));
        }
        assert!(storage.control_available());
        assert!(
            RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &hsaco, &good)
                .validate(7)
                .is_ok()
        );
    }
}

#[test]
fn generated_storage_artifact_rejections_preserve_control() {
    let (hsaco, projection) = source_projection();
    let authority = source_authority(&projection);
    let mut storage = projection.into_generated_storage_v1();
    for change_length in [false, true] {
        let mut changed = hsaco.clone();
        if change_length {
            changed.push(0);
        } else {
            changed[0] ^= 1;
        }
        let source = RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &changed, &authority);
        assert!(matches!(source.validate(7), Err(Error::ArtifactMismatch)));
        assert!(storage.control_available());
    }
    assert!(
        RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &hsaco, &authority)
            .validate(7)
            .is_ok()
    );
}
