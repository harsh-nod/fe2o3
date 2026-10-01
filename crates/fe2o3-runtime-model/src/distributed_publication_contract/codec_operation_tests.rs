use super::*;
use DistributedPublicationContractErrorV1 as E;
use alloc::vec::Vec;

fn coordinates(seed: u8) -> UntrustedDistributedOperationCoordinatesV1 {
    let digest = |field: u8| {
        IdentityDigestV1::from_untrusted_bytes(core::array::from_fn(|index| {
            seed.wrapping_mul(19)
                .wrapping_add(field.wrapping_mul(37))
                .wrapping_add((index as u8).wrapping_mul(53))
        }))
    };
    UntrustedDistributedOperationCoordinatesV1 {
        runtime_instance: DistributedRuntimeInstanceIdV1::from_untrusted_digest(digest(1)),
        participant: DistributedParticipantIdV1::from_untrusted_digest(digest(2)),
        participant_incarnation: 0x0102_0304_0506_0708 ^ u64::from(seed),
        coordinator: DistributedParticipantIdV1::from_untrusted_digest(digest(3)),
        coordinator_epoch: 0x1112_1314_1516_1718 ^ u64::from(seed),
        membership: DistributedMembershipIdV1::from_untrusted_digest(digest(4)),
        membership_epoch: 0x2122_2324_2526_2728 ^ u64::from(seed),
        run: DistributedRunIdV1::from_untrusted_digest(digest(5)),
        operation: DistributedOperationIdV1::from_untrusted_digest(digest(6)),
        attempt: 0x3132_3334_3536_3738 ^ u64::from(seed),
        artifact: RuntimeArtifactIdV1::from_untrusted_digest(digest(7)),
        execution_plan: DistributedExecutionPlanIdV1::from_untrusted_digest(digest(8)),
        placement_plan: DistributedPlacementPlanIdV1::from_untrusted_digest(digest(9)),
        target: DistributedTargetDescriptionIdV1::from_untrusted_digest(digest(10)),
        runtime_model: RuntimeModelIdV1::from_untrusted_digest(digest(11)),
    }
}

// Independent layout oracle: no production cursor, field, or operation helper.
fn reference_encode(c: UntrustedDistributedOperationCoordinatesV1) -> Vec<u8> {
    let mut bytes = Vec::from(&b"FE2O3/DISTRIBUTED-OPERATION-DESCRIPTION/V1\0"[..]);
    bytes.extend_from_slice(&[1, 0, 0, 0]);
    bytes.extend_from_slice(c.runtime_instance.digest().as_bytes());
    bytes.extend_from_slice(c.participant.digest().as_bytes());
    bytes.extend_from_slice(&c.participant_incarnation.to_le_bytes());
    bytes.extend_from_slice(c.coordinator.digest().as_bytes());
    bytes.extend_from_slice(&c.coordinator_epoch.to_le_bytes());
    bytes.extend_from_slice(c.membership.digest().as_bytes());
    bytes.extend_from_slice(&c.membership_epoch.to_le_bytes());
    bytes.extend_from_slice(c.run.digest().as_bytes());
    bytes.extend_from_slice(c.operation.digest().as_bytes());
    bytes.extend_from_slice(&c.attempt.to_le_bytes());
    bytes.extend_from_slice(c.artifact.digest().as_bytes());
    bytes.extend_from_slice(c.execution_plan.digest().as_bytes());
    bytes.extend_from_slice(c.placement_plan.digest().as_bytes());
    bytes.extend_from_slice(c.target.digest().as_bytes());
    bytes.extend_from_slice(c.runtime_model.digest().as_bytes());
    bytes
}

fn reference_decode(bytes: &[u8]) -> Result<ModelDistributedOperationBindingV1, E> {
    if bytes.len() != 431 {
        return Err(E::WrongLength);
    }
    if &bytes[..43] != b"FE2O3/DISTRIBUTED-OPERATION-DESCRIPTION/V1\0" {
        return Err(E::WrongDomain);
    }
    if bytes[43..45] != [1, 0] {
        return Err(E::WrongSchema);
    }
    if bytes[45..47] != [0, 0] {
        return Err(E::NonzeroReserved);
    }
    let digest = |start| {
        IdentityDigestV1::from_untrusted_bytes(bytes[start..start + 32].try_into().unwrap())
    };
    let integer = |start| u64::from_le_bytes(bytes[start..start + 8].try_into().unwrap());
    ModelDistributedOperationBindingV1::from_untrusted_coordinates(
        UntrustedDistributedOperationCoordinatesV1 {
            runtime_instance: DistributedRuntimeInstanceIdV1::from_untrusted_digest(digest(47)),
            participant: DistributedParticipantIdV1::from_untrusted_digest(digest(79)),
            participant_incarnation: integer(111),
            coordinator: DistributedParticipantIdV1::from_untrusted_digest(digest(119)),
            coordinator_epoch: integer(151),
            membership: DistributedMembershipIdV1::from_untrusted_digest(digest(159)),
            membership_epoch: integer(191),
            run: DistributedRunIdV1::from_untrusted_digest(digest(199)),
            operation: DistributedOperationIdV1::from_untrusted_digest(digest(231)),
            attempt: integer(263),
            artifact: RuntimeArtifactIdV1::from_untrusted_digest(digest(271)),
            execution_plan: DistributedExecutionPlanIdV1::from_untrusted_digest(digest(303)),
            placement_plan: DistributedPlacementPlanIdV1::from_untrusted_digest(digest(335)),
            target: DistributedTargetDescriptionIdV1::from_untrusted_digest(digest(367)),
            runtime_model: RuntimeModelIdV1::from_untrusted_digest(digest(399)),
        },
    )
}

#[test]
fn operation_wire_nonuniform_coordinates_match_independent_layout() {
    for seed in 0..64 {
        let c = coordinates(seed);
        let binding = ModelDistributedOperationBindingV1 { coordinates: c };
        let expected = reference_encode(c);
        let actual = binding.canonical_description();
        assert_eq!(actual.as_slice(), expected);
        assert_eq!(reference_decode(&actual), Ok(binding));
        assert_eq!(
            ModelDistributedOperationBindingV1::decode_untrusted_description(&actual),
            Ok(binding)
        );
    }
}

#[test]
fn operation_wire_every_truncation_and_extension_keeps_length_precedence() {
    let mut bytes = reference_encode(coordinates(17));
    bytes[..47].fill(0xff);
    for length in 0..431 {
        assert_eq!(
            ModelDistributedOperationBindingV1::decode_untrusted_description(&bytes[..length]),
            reference_decode(&bytes[..length])
        );
        assert_eq!(reference_decode(&bytes[..length]), Err(E::WrongLength));
    }
    for _ in 0..32 {
        bytes.push(0xff);
        assert_eq!(
            ModelDistributedOperationBindingV1::decode_untrusted_description(&bytes),
            Err(E::WrongLength)
        );
    }
}

#[test]
fn operation_wire_every_position_and_byte_value_matches_independent_decode() {
    let mut bytes = reference_encode(coordinates(29));
    for index in 0..bytes.len() {
        let original = bytes[index];
        for value in 0..=u8::MAX {
            bytes[index] = value;
            let actual = ModelDistributedOperationBindingV1::decode_untrusted_description(&bytes);
            assert_eq!(
                actual,
                reference_decode(&bytes),
                "index={index} value={value}"
            );
            if let Ok(binding) = actual {
                assert_eq!(binding.canonical_description().as_slice(), bytes);
            }
        }
        bytes[index] = original;
    }
}

#[test]
fn operation_wire_invalid_coordinates_and_header_errors_keep_precedence() {
    let original = reference_encode(coordinates(41));
    let fields = [
        (47, 32),
        (79, 32),
        (111, 8),
        (119, 32),
        (151, 8),
        (159, 32),
        (191, 8),
        (199, 32),
        (231, 32),
        (263, 8),
        (271, 32),
        (303, 32),
        (335, 32),
        (367, 32),
        (399, 32),
    ];
    for (start, length) in fields {
        let mut bytes = original.clone();
        bytes[start..start + length].fill(0);
        let expected = Err(if length == 32 {
            E::ZeroIdentity
        } else {
            E::ZeroEpochOrAttempt
        });
        assert_eq!(reference_decode(&bytes), expected);
        assert_eq!(
            ModelDistributedOperationBindingV1::decode_untrusted_description(&bytes),
            expected
        );
        bytes[111..119].fill(0);
        bytes[399..431].fill(0);
        assert_eq!(
            ModelDistributedOperationBindingV1::decode_untrusted_description(&bytes),
            Err(E::ZeroIdentity)
        );
        bytes[46] = 1;
        assert_eq!(
            ModelDistributedOperationBindingV1::decode_untrusted_description(&bytes),
            Err(E::NonzeroReserved)
        );
        bytes[43] = 2;
        assert_eq!(
            ModelDistributedOperationBindingV1::decode_untrusted_description(&bytes),
            Err(E::WrongSchema)
        );
        bytes[0] ^= 1;
        assert_eq!(
            ModelDistributedOperationBindingV1::decode_untrusted_description(&bytes),
            Err(E::WrongDomain)
        );
    }
    let mut c = coordinates(0);
    c.participant_incarnation = 0;
    c.runtime_model =
        RuntimeModelIdV1::from_untrusted_digest(IdentityDigestV1::from_untrusted_bytes([0; 32]));
    let malformed = ModelDistributedOperationBindingV1 { coordinates: c };
    assert_eq!(
        malformed.canonical_description().as_slice(),
        reference_encode(c)
    );
    assert_eq!(
        ModelDistributedOperationBindingV1::decode_untrusted_description(
            &malformed.canonical_description()
        ),
        ModelDistributedOperationBindingV1::from_untrusted_coordinates(c)
    );
}

#[test]
fn operation_digest_helpers_preserve_failed_write_frames_and_failed_cursors() {
    let digest = coordinates(3).runtime_instance.digest();
    let payload = digest.as_bytes();
    for length in 0..32 {
        let mut bytes = [0xa5; 34];
        let mut offset = 1;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            codec_operation::write_digest(&mut bytes[..length + 1], &mut offset, digest);
        }));
        assert!(result.is_err());
        assert_eq!(offset, 1);
        assert_eq!(bytes, [0xa5; 34]);
    }
    for start in [33, usize::MAX - 31, usize::MAX] {
        let mut bytes = [0xa5; 34];
        let mut offset = start;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            codec_operation::write_digest(&mut bytes, &mut offset, digest);
        }));
        assert!(result.is_err());
        assert_eq!(offset, start);
        assert_eq!(bytes, [0xa5; 34]);
    }
    for start in [0, 1, 31, 32, 33, usize::MAX - 31, usize::MAX] {
        let mut offset = start;
        let actual = codec_operation::read_digest(payload, &mut offset);
        if start == 0 {
            assert_eq!(actual, Ok(digest));
            assert_eq!(offset, 32);
        } else {
            assert_eq!(actual, Err(E::WrongLength));
            assert_eq!(offset, start);
        }
    }
}
