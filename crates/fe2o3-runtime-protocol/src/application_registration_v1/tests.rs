use super::*;
use crate::{
    WorkerV3ApplicationIdentityV1, WorkerV3ApplicationInputOccurrenceV1,
    WorkerV3LoadEnvelopeIdentityV1,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionClientProcessIdentityV1, CompilerExecutionExternalAnchorServiceIdentityV1,
    CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionServiceLaunchManifestV1,
};

fn key(hex: &str) -> [u8; 32] {
    std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
}

fn policy() -> CompilerExecutionIssuerPolicyV1 {
    CompilerExecutionIssuerPolicyV1::new(
        1,
        CompilerExecutionIssuerMeasurementV1::new([1; 32], 123).unwrap(),
        CompilerExecutionIssuerMeasurementV1::new([2; 32], 456).unwrap(),
        key("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"),
        key("3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c"),
    )
    .unwrap()
}

fn fixture() -> WorkerV3ApplicationRegistrationBindingV1 {
    let policy = policy();
    let handoff = CompilerExecutionSupervisorHandoffV1::new(
        CompilerExecutionClientProcessIdentityV1::new(100, 1000, 1001).unwrap(),
        CompilerExecutionServiceLaunchManifestV1::new(
            CompilerExecutionClientProcessIdentityV1::new(200, 1000, 1001).unwrap(),
            CompilerExecutionExternalAnchorServiceIdentityV1::new(6000, 7000).unwrap(),
            &policy,
        ),
    )
    .unwrap();
    let occurrence = occurrence(&[1, 2, 3, 4], 10);
    let expectation = WorkerV3ApplicationHandoffExpectationV1::new(
        WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"opaque envelope").unwrap(),
        &occurrence,
    );
    WorkerV3ApplicationRegistrationBindingV1::new(
        handoff,
        occurrence,
        WorkerV3ApplicationRegistrationDescriptorsV1::new(10, 11, 12, 13).unwrap(),
        expectation,
        WorkerV3ApplicationHandoffChallengeV1::from_bytes([11; 32]).unwrap(),
    )
    .unwrap()
}

fn occurrence(slots: &[u16], spawn: u8) -> WorkerV3ApplicationOccurrenceV1 {
    WorkerV3ApplicationOccurrenceV1::new(
        WorkerV3ApplicationIdentityV1::from_test_parts([3; 32], 1024),
        [spawn; 32],
        &slots
            .iter()
            .map(|&slot| WorkerV3ApplicationInputOccurrenceV1::new(slot, [slot as u8; 32]).unwrap())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn ready_fixture() -> WorkerV3ApplicationSupervisorReadyV1 {
    let binding = fixture();
    WorkerV3ApplicationSupervisorReadyV1::new(
        &binding,
        fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV1::new(
            300,
            binding.compiler_handoff().launch_manifest(),
            &policy(),
        )
        .unwrap(),
    )
    .unwrap()
}

fn reseal_ready(bytes: &mut [u8]) {
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/WORKER-V3/APPLICATION-SUPERVISOR-READY/V1\0");
    hash.update(176u64.to_le_bytes());
    hash.update(&bytes[..176]);
    bytes[176..].copy_from_slice(&hash.finalize());
}

#[test]
fn application_readiness_round_trip_and_distinct_compiler_profile() {
    use fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV1;
    let ready = ready_fixture();
    let bytes = ready.canonical_bytes();
    assert_eq!(bytes.len(), 208);
    assert_eq!(
        &bytes[..24],
        b"F3ARDY1\0\x01\0\0\0\xd0\0\0\0\0\0\0\0\0\0\0\0"
    );
    assert_eq!(&bytes[24..56], fixture().identity().as_bytes());
    assert_eq!(
        &bytes[56..176],
        ready.compiler_readiness().canonical_bytes()
    );
    assert_eq!(
        WorkerV3ApplicationSupervisorReadyV1::decode(bytes).unwrap(),
        ready
    );
    assert!(ready.matches_binding(&fixture(), &policy()));
    assert!(CompilerExecutionServiceReadyV1::decode(bytes).is_err());
    assert!(
        WorkerV3ApplicationSupervisorReadyV1::decode(ready.compiler_readiness().canonical_bytes())
            .is_err()
    );
}

#[test]
fn application_readiness_rejects_every_byte_corruption_and_noncanonical_frame() {
    let ready = ready_fixture();
    for offset in 0..208 {
        let mut bytes = *ready.canonical_bytes();
        bytes[offset] ^= 1;
        assert!(
            WorkerV3ApplicationSupervisorReadyV1::decode(&bytes).is_err(),
            "offset {offset}"
        );
    }
    for length in 0..210 {
        if length == 208 {
            continue;
        }
        let mut bytes = ready.canonical_bytes().to_vec();
        bytes.resize(length, 0);
        assert!(WorkerV3ApplicationSupervisorReadyV1::decode(&bytes).is_err());
    }
    for offset in [0, 8, 10, 11, 12, 16, 23, 56, 64, 66, 68, 76, 175] {
        let mut bytes = *ready.canonical_bytes();
        bytes[offset] ^= 1;
        reseal_ready(&mut bytes);
        assert!(
            WorkerV3ApplicationSupervisorReadyV1::decode(&bytes).is_err(),
            "resealed offset {offset}"
        );
    }
    let mut bytes = *ready.canonical_bytes();
    bytes[24..56].fill(0);
    reseal_ready(&mut bytes);
    assert!(WorkerV3ApplicationSupervisorReadyV1::decode(&bytes).is_err());
}

#[test]
fn application_readiness_requires_exact_registration_launch_and_policy() {
    use fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV1;
    let binding = fixture();
    let ready = ready_fixture();
    let mut bytes = *ready.canonical_bytes();
    bytes[24] ^= 1;
    reseal_ready(&mut bytes);
    assert!(
        !WorkerV3ApplicationSupervisorReadyV1::decode(&bytes)
            .unwrap()
            .matches_binding(&binding, &policy())
    );
    let mut other = binding.canonical_bytes().to_vec();
    // A different canonical challenge changes only the full application registration.
    let changed = WorkerV3ApplicationRegistrationBindingV1::new(
        binding.compiler_handoff().clone(),
        binding.occurrence().clone(),
        binding.descriptors(),
        binding.expectation(),
        WorkerV3ApplicationHandoffChallengeV1::from_bytes([99; 32]).unwrap(),
    )
    .unwrap();
    other.copy_from_slice(changed.canonical_bytes());
    assert!(!ready.matches_binding(
        &WorkerV3ApplicationRegistrationBindingV1::decode(&other).unwrap(),
        &policy()
    ));
    for policy_version in [1, 2] {
        let policy = CompilerExecutionIssuerPolicyV1::new(
            policy_version,
            policy().executable(),
            policy().runtime(),
            *policy().verifying_key(),
            *policy().external_anchor_verifying_key(),
        )
        .unwrap();
        let launch = CompilerExecutionServiceLaunchManifestV1::new(
            CompilerExecutionClientProcessIdentityV1::new(
                if policy_version == 1 { 201 } else { 200 },
                1000,
                1001,
            )
            .unwrap(),
            binding
                .compiler_handoff()
                .launch_manifest()
                .external_anchor_service(),
            &policy,
        );
        let compiler = CompilerExecutionServiceReadyV1::new(300, &launch, &policy).unwrap();
        assert!(WorkerV3ApplicationSupervisorReadyV1::new(&binding, compiler.clone()).is_err());
        let mut bytes = *ready.canonical_bytes();
        bytes[56..176].copy_from_slice(compiler.canonical_bytes());
        reseal_ready(&mut bytes);
        assert!(
            !WorkerV3ApplicationSupervisorReadyV1::decode(&bytes)
                .unwrap()
                .matches_binding(&binding, &policy)
        );
    }
    // PID is inert in this codec; authenticating the actual issuer remains the supervisor's job.
    let different_pid = CompilerExecutionServiceReadyV1::new(
        301,
        binding.compiler_handoff().launch_manifest(),
        &policy(),
    )
    .unwrap();
    assert!(
        WorkerV3ApplicationSupervisorReadyV1::new(&binding, different_pid)
            .unwrap()
            .matches_binding(&binding, &policy())
    );
}

// Independent resealing permits semantic mutations to pass the outer integrity gate.
fn reseal(bytes: &mut [u8]) {
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/WORKER-V3/APPLICATION-REGISTRATION/V1\0");
    hash.update(808u64.to_le_bytes());
    hash.update(&bytes[..808]);
    bytes[808..].copy_from_slice(&hash.finalize());
}

#[test]
fn fixed_layout_round_trip_retains_full_original_binding() {
    let value = fixture();
    let bytes = value.canonical_bytes();
    assert_eq!(bytes.len(), 840);
    assert_eq!(
        &bytes[..24],
        b"F3AREG1\0\x01\0\0\0\x48\x03\0\0\0\0\0\0\0\0\0\0"
    );
    assert_eq!(&bytes[24..208], value.compiler_handoff().canonical_bytes());
    assert_eq!(
        &bytes[208..512],
        value.occurrence().encode_canonical().unwrap()
    );
    assert_eq!(
        &bytes[512..528],
        &[10, 0, 0, 0, 11, 0, 0, 0, 12, 0, 0, 0, 13, 0, 0, 0]
    );
    assert_eq!(
        &bytes[528..724],
        value.expectation().encode_canonical().unwrap()
    );
    assert_eq!(
        &bytes[724..808],
        value.challenge().encode_canonical().unwrap()
    );
    assert_eq!(&bytes[808..], value.identity().as_bytes());
    assert_eq!(
        WorkerV3ApplicationRegistrationBindingV1::decode(bytes).unwrap(),
        value
    );
    assert_eq!(value.descriptors().as_array(), [10, 11, 12, 13]);
    assert_eq!(value.compiler_handoff().submitter().pid(), 100);
    assert_eq!(
        value.compiler_handoff().launch_manifest().client().pid(),
        200
    );
}

#[test]
fn session_round_trip_has_exact_phase_lengths_rights_and_transcript() {
    let binding = fixture();
    let inputs =
        WorkerV3ApplicationRegistrationInputsV1::decode(&binding.canonical_bytes()[208..808])
            .unwrap();
    assert_eq!(inputs.canonical_bytes().len(), 600);
    assert!(inputs.matches_binding(&binding));
    let hello = WorkerV3ApplicationSessionMessageV1::hello(inputs.clone(), [31; 32]).unwrap();
    let challenge =
        WorkerV3ApplicationSessionMessageV1::challenge(binding.clone(), [31; 32], [32; 32])
            .unwrap();
    let transcript = challenge.transcript().unwrap();
    assert_eq!(transcript.app_nonce(), [31; 32]);
    assert_eq!(transcript.root_nonce(), [32; 32]);
    assert_eq!(transcript.binding(), *binding.identity().as_bytes());
    assert_eq!(hello.inputs(), Some(&inputs));
    assert_eq!(hello.transcript(), None);
    assert_eq!(challenge.registration(), Some(&binding));
    for (message, kind, length, rights) in [
        (hello, WorkerV3ApplicationSessionKindV1::Hello, 712, 0),
        (
            challenge,
            WorkerV3ApplicationSessionKindV1::Challenge,
            952,
            1,
        ),
        (
            WorkerV3ApplicationSessionMessageV1::accept(transcript),
            WorkerV3ApplicationSessionKindV1::Accept,
            112,
            0,
        ),
        (
            WorkerV3ApplicationSessionMessageV1::ready(transcript),
            WorkerV3ApplicationSessionKindV1::Ready,
            112,
            0,
        ),
    ] {
        assert_eq!(message.kind(), kind);
        assert_eq!(message.canonical_bytes().len(), length);
        assert_eq!(message.rights(), rights);
        assert_eq!(
            WorkerV3ApplicationSessionMessageV1::decode(message.canonical_bytes()).unwrap(),
            message
        );
        for length in 0..message.canonical_bytes().len() {
            assert!(
                WorkerV3ApplicationSessionMessageV1::decode(&message.canonical_bytes()[..length])
                    .is_err()
            );
        }
        for offset in [0, 7, 8, 9, 10, 11, 12, 15] {
            let mut changed = message.canonical_bytes().to_vec();
            changed[offset] = 255;
            assert!(WorkerV3ApplicationSessionMessageV1::decode(&changed).is_err());
        }
        let mut trailing = message.canonical_bytes().to_vec();
        trailing.push(0);
        assert!(WorkerV3ApplicationSessionMessageV1::decode(&trailing).is_err());
    }
}

#[test]
fn session_rejects_invalid_transcripts_and_challenge_body_substitution() {
    let binding = fixture();
    let inputs =
        WorkerV3ApplicationRegistrationInputsV1::decode(&binding.canonical_bytes()[208..808])
            .unwrap();
    let hello = WorkerV3ApplicationSessionMessageV1::hello(inputs.clone(), [31; 32]).unwrap();
    assert!(WorkerV3ApplicationSessionMessageV1::hello(inputs, [0; 32]).is_err());
    for offset in [48, 80] {
        let mut bytes = hello.canonical_bytes().to_vec();
        bytes[offset] = 1;
        assert!(WorkerV3ApplicationSessionMessageV1::decode(&bytes).is_err());
    }
    let challenge =
        WorkerV3ApplicationSessionMessageV1::challenge(binding.clone(), [31; 32], [32; 32])
            .unwrap();
    for message in [
        hello,
        challenge.clone(),
        WorkerV3ApplicationSessionMessageV1::accept(challenge.transcript().unwrap()),
        WorkerV3ApplicationSessionMessageV1::ready(challenge.transcript().unwrap()),
    ] {
        for offset in if message.kind() == WorkerV3ApplicationSessionKindV1::Hello {
            &[16][..]
        } else {
            &[16, 48, 80][..]
        } {
            let mut bytes = message.canonical_bytes().to_vec();
            bytes[*offset..*offset + 32].fill(0);
            assert!(WorkerV3ApplicationSessionMessageV1::decode(&bytes).is_err());
        }
        let mut bytes = message.canonical_bytes().to_vec();
        bytes[48..80].fill(31);
        assert!(WorkerV3ApplicationSessionMessageV1::decode(&bytes).is_err());
    }
    let changed = WorkerV3ApplicationRegistrationBindingV1::new(
        binding.handoff.clone(),
        binding.occurrence.clone(),
        binding.descriptors,
        binding.expectation,
        WorkerV3ApplicationHandoffChallengeV1::from_bytes([19; 32]).unwrap(),
    )
    .unwrap();
    let mut bytes = challenge.canonical_bytes().to_vec();
    bytes[112..].copy_from_slice(changed.canonical_bytes());
    assert!(WorkerV3ApplicationSessionMessageV1::decode(&bytes).is_err());
    bytes[80..112].copy_from_slice(changed.identity().as_bytes());
    assert_ne!(
        WorkerV3ApplicationSessionMessageV1::decode(&bytes)
            .unwrap()
            .transcript(),
        challenge.transcript()
    );
}

#[test]
fn local_inputs_match_every_application_dimension_but_not_compiler_handoff() {
    let original = fixture();
    let inputs =
        WorkerV3ApplicationRegistrationInputsV1::decode(&original.canonical_bytes()[208..808])
            .unwrap();
    for mutation in 0..5 {
        let mut value = original.clone();
        match mutation {
            0 => {
                value.handoff = CompilerExecutionSupervisorHandoffV1::new(
                    CompilerExecutionClientProcessIdentityV1::new(101, 1000, 1001).unwrap(),
                    value.handoff.launch_manifest().clone(),
                )
                .unwrap()
            }
            1 => {
                value.occurrence = occurrence(&[1, 2, 3, 4], 12);
                value.expectation = WorkerV3ApplicationHandoffExpectationV1::new(
                    value.expectation.envelope(),
                    &value.occurrence,
                );
            }
            2 => {
                value.descriptors =
                    WorkerV3ApplicationRegistrationDescriptorsV1::new(10, 11, 12, 14).unwrap()
            }
            3 => {
                value.expectation = WorkerV3ApplicationHandoffExpectationV1::new(
                    WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"different").unwrap(),
                    &value.occurrence,
                )
            }
            4 => {
                value.challenge =
                    WorkerV3ApplicationHandoffChallengeV1::from_bytes([99; 32]).unwrap()
            }
            _ => unreachable!(),
        }
        let changed = WorkerV3ApplicationRegistrationBindingV1::new(
            value.handoff,
            value.occurrence,
            value.descriptors,
            value.expectation,
            value.challenge,
        )
        .unwrap();
        assert_ne!(changed.identity(), original.identity());
        assert_eq!(inputs.matches_binding(&changed), mutation == 0);
    }
    for size in [0, 599, 601, 840, 4096] {
        assert!(WorkerV3ApplicationRegistrationInputsV1::decode(&vec![0; size]).is_err());
    }
}

#[test]
fn every_byte_mutation_and_nonexact_length_rejects() {
    let value = fixture();
    for index in 0..840 {
        let mut bytes = *value.canonical_bytes();
        bytes[index] ^= 1;
        assert!(
            WorkerV3ApplicationRegistrationBindingV1::decode(&bytes).is_err(),
            "byte {index}"
        );
    }
    for length in [0, 23, 184, 839, 841, 4096] {
        assert_eq!(
            WorkerV3ApplicationRegistrationBindingV1::decode(&vec![0; length]),
            Err(WorkerV3ApplicationRegistrationErrorV1::Length)
        );
    }
}

#[test]
fn exact_four_slot_profile_and_consistent_expectation_are_required() {
    let value = fixture();
    for slots in [
        &[1, 2, 3][..],
        &[1, 2, 3, 4, 5],
        &[1, 2, 3, 5],
        &[2, 3, 4, 5],
    ] {
        let occurrence = occurrence(slots, 10);
        let expectation = WorkerV3ApplicationHandoffExpectationV1::new(
            value.expectation().envelope(),
            &occurrence,
        );
        assert_eq!(
            WorkerV3ApplicationRegistrationBindingV1::new(
                value.handoff.clone(),
                occurrence,
                value.descriptors,
                expectation,
                value.challenge,
            ),
            Err(WorkerV3ApplicationRegistrationErrorV1::InputProfile)
        );
    }
    assert_eq!(
        WorkerV3ApplicationRegistrationBindingV1::new(
            value.handoff.clone(),
            occurrence(&[1, 2, 3, 4], 12),
            value.descriptors,
            value.expectation,
            value.challenge,
        ),
        Err(WorkerV3ApplicationRegistrationErrorV1::ExpectationMismatch)
    );
}

#[test]
fn resealed_bad_coordinates_headers_and_nested_records_reject() {
    let value = fixture();
    for slot in 0..4 {
        for invalid in [0, 1, 2, -1, i32::MIN, 195] {
            let mut bytes = *value.canonical_bytes();
            bytes[512 + slot * 4..516 + slot * 4].copy_from_slice(&invalid.to_le_bytes());
            reseal(&mut bytes);
            assert_eq!(
                WorkerV3ApplicationRegistrationBindingV1::decode(&bytes),
                Err(WorkerV3ApplicationRegistrationErrorV1::Descriptors)
            );
        }
        for other in 0..4 {
            if slot == other {
                continue;
            }
            let mut bytes = *value.canonical_bytes();
            bytes[512 + slot * 4..516 + slot * 4]
                .copy_from_slice(&(10 + i32::try_from(other).unwrap()).to_le_bytes());
            reseal(&mut bytes);
            assert_eq!(
                WorkerV3ApplicationRegistrationBindingV1::decode(&bytes),
                Err(WorkerV3ApplicationRegistrationErrorV1::Descriptors)
            );
        }
    }
    assert!(WorkerV3ApplicationRegistrationDescriptorsV1::new(3, 4, 5, i32::MAX).is_ok());
    for offset in [10, 11, 16, 23] {
        let mut bytes = *value.canonical_bytes();
        bytes[offset] = 1;
        reseal(&mut bytes);
        assert_eq!(
            WorkerV3ApplicationRegistrationBindingV1::decode(&bytes),
            Err(WorkerV3ApplicationRegistrationErrorV1::Reserved)
        );
    }
    for offset in [24, 208, 528, 724] {
        let mut bytes = *value.canonical_bytes();
        bytes[offset] ^= 1;
        reseal(&mut bytes);
        assert!(matches!(
            WorkerV3ApplicationRegistrationBindingV1::decode(&bytes),
            Err(WorkerV3ApplicationRegistrationErrorV1::CompilerHandoff(_)
                | WorkerV3ApplicationRegistrationErrorV1::ApplicationHandoff(_))
        ));
    }
}

#[test]
fn valid_nested_substitution_cannot_break_cross_binding() {
    let value = fixture();
    let mut bytes = *value.canonical_bytes();
    bytes[208..512].copy_from_slice(&occurrence(&[1, 2, 3, 4], 12).encode_canonical().unwrap());
    reseal(&mut bytes);
    assert_eq!(
        WorkerV3ApplicationRegistrationBindingV1::decode(&bytes),
        Err(WorkerV3ApplicationRegistrationErrorV1::ExpectationMismatch)
    );
    let mut bytes = *value.canonical_bytes();
    bytes[208..512].copy_from_slice(&occurrence(&[1, 2, 3, 5], 10).encode_canonical().unwrap());
    reseal(&mut bytes);
    assert_eq!(
        WorkerV3ApplicationRegistrationBindingV1::decode(&bytes),
        Err(WorkerV3ApplicationRegistrationErrorV1::InputProfile)
    );
}

#[test]
fn every_valid_binding_dimension_changes_identity_without_becoming_authority() {
    let original = fixture();
    for mutation in 0..7 {
        let mut value = original.clone();
        match mutation {
            0 => {
                value.handoff = CompilerExecutionSupervisorHandoffV1::new(
                    CompilerExecutionClientProcessIdentityV1::new(101, 1000, 1001).unwrap(),
                    value.handoff.launch_manifest().clone(),
                )
                .unwrap()
            }
            1 => value.occurrence = occurrence(&[1, 2, 3, 4], 12),
            2 => {
                value.descriptors =
                    WorkerV3ApplicationRegistrationDescriptorsV1::new(10, 11, 12, 14).unwrap()
            }
            3 => {
                value.expectation = WorkerV3ApplicationHandoffExpectationV1::new(
                    WorkerV3LoadEnvelopeIdentityV1::from_exact_bytes(b"other envelope").unwrap(),
                    &value.occurrence,
                )
            }
            4 => {
                value.challenge =
                    WorkerV3ApplicationHandoffChallengeV1::from_bytes([12; 32]).unwrap()
            }
            5 | 6 => {
                let mut inputs = value.occurrence.inputs().to_vec();
                if mutation == 5 {
                    inputs[3] = WorkerV3ApplicationInputOccurrenceV1::new(4, [99; 32]).unwrap();
                }
                value.occurrence = WorkerV3ApplicationOccurrenceV1::new(
                    if mutation == 5 {
                        value.occurrence.application()
                    } else {
                        WorkerV3ApplicationIdentityV1::from_test_parts([4; 32], 2048)
                    },
                    value.occurrence.spawn_identity(),
                    &inputs,
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        value.expectation = WorkerV3ApplicationHandoffExpectationV1::new(
            value.expectation.envelope(),
            &value.occurrence,
        );
        let changed = WorkerV3ApplicationRegistrationBindingV1::new(
            value.handoff,
            value.occurrence,
            value.descriptors,
            value.expectation,
            value.challenge,
        )
        .unwrap();
        assert_ne!(
            changed.identity(),
            original.identity(),
            "dimension {mutation}"
        );
        assert_eq!(
            WorkerV3ApplicationRegistrationBindingV1::decode(changed.canonical_bytes()).unwrap(),
            changed
        );
        assert!(!changed.occurrence().authenticates_application_occurrence());
        assert!(!changed.expectation().grants_launch_authority());
    }
}
