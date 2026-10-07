//! Codec fixtures only. None establishes a native compiler or GPU authority.
use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationReceiptV3 as Receipt,
    CompilerExecutionAttestationRequestV3 as Request,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerPolicyV3 as Policy, CompilerExecutionReceiptCarriageV3 as Carriage,
    CompilerExecutionReceiptPublicationAckV3 as Ack,
    CompilerExecutionReceiptPublicationV3 as Publication,
    CompilerExecutionServiceLaunchManifestV3 as Launch,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
#[allow(dead_code)]
#[path = "../../../fe2o3-compiler-execution-protocol/tests/support/native_attestation_fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../../../fe2o3-compiler-execution-protocol/tests/support/native_receipt_fixture.rs"]
mod receipt_fixture;

fn retain<T, E: fmt::Debug>(
    result: std::result::Result<(T, HandoffStorage), E>,
    b: &mut Budget<'_>,
) -> T {
    let (owner, storage) = result.unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    owner
}
fn retained<T>(result: Result<(T, Storage)>, b: &mut Budget<'_>) -> T {
    let (owner, storage) = result.unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    owner
}
fn occurrence(spawn: u8, slots: &[u16]) -> Occurrence {
    Occurrence::new(
        crate::WorkerV3ApplicationIdentityV1::from_test_parts([3; 32], 1024),
        [spawn; 32],
        &slots
            .iter()
            .map(|slot| InputOccurrence::new(*slot, [*slot as u8; 32]).unwrap())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
fn owners(b: &mut Budget<'_>, parent: u32) -> (Handoff, Association, Vec<u8>) {
    let p = fixture::policy_wire(3);
    let r = fixture::request_wire(3);
    let s = receipt_fixture::receipt_wire(3);
    b.reserve_storage(p.len() + r.len() + s.len()).unwrap();
    let policy = retain(Policy::decode(&p, b), b);
    let request = retain(Request::decode(&r, b), b);
    let receipt = retain(Receipt::decode(&s, b), b);
    let publication = retain(Publication::new([0x81; 32], [0x82; 32], receipt, b), b);
    let ack = retain(Ack::new(&publication, [0x83; 32], b), b);
    let carriage = retain(Carriage::new(policy, request, publication, ack, b), b);
    let policy = retain(Policy::decode(&p, b), b);
    let launch = retain(
        Launch::new(
            Client::new(1201, 1001, 1001).unwrap(),
            Service::new(2001, 2001).unwrap(),
            &policy,
            b,
        ),
        b,
    );
    let handoff = retain(
        Handoff::new(Client::new(parent, 1001, 1001).unwrap(), launch, b),
        b,
    );
    let readiness = crate::conditional_worker_readiness_codec::encode(
        [
            b"record",
            b"claim",
            b"outer",
            b"transcript",
            carriage.canonical_bytes(),
        ],
        std::iter::empty::<&[u8]>(),
    )
    .unwrap();
    b.reserve_storage(readiness.len()).unwrap();
    let association = Association::bind(&readiness, &handoff, &carriage, b).unwrap();
    b.reserve_storage(size_of::<Association>()).unwrap();
    (handoff, association, readiness)
}
fn inputs(b: &mut Budget<'_>, readiness: &[u8], spawn: u8) -> Inputs {
    b.reserve_storage(OCCURRENCE_STORAGE).unwrap();
    retained(
        Inputs::new(
            readiness,
            occurrence(spawn, &[1, 2, 3, 4]),
            Descriptors::new(10, 11, 12, 13).unwrap(),
            Challenge::from_bytes([9; 32]).unwrap(),
            b,
        ),
        b,
    )
}
fn binding() -> (Owned, Binding) {
    let mut account = Owned::new(Work::new(100_000_000), 8_000_000);
    let binding = account.with_budget(|b| {
        let (handoff, association, readiness) = owners(b, 1200);
        let inputs = inputs(b, &readiness, 10);
        retained(Binding::new(handoff, association, inputs, b), b)
    });
    (account, binding)
}

#[test]
fn native_root_publication_binds_nonce_registration_gate_and_native_ready() {
    use crate::NativeApplicationRootTransferV1 as Transfer;
    use fe2o3_compiler_execution_protocol::CompilerExecutionServiceReadyV3 as Ready;
    let (mut account, binding) = binding();
    account.with_budget(|b| {
        let bytes = fixture::policy_wire(3);
        b.reserve_storage(bytes.len()).unwrap();
        let policy = retain(Policy::decode(&bytes, b), b);
        let ready = retain(
            Ready::new(
                3001,
                binding.compiler_handoff().launch_manifest(),
                &policy,
                b,
            ),
            b,
        );
        let id = *binding.identity().as_bytes();
        let packet = Transfer::publication_packet(&ready, [1; 32], id, [2; 32], b).unwrap();
        b.reserve_storage(packet.len()).unwrap();
        let (decoded, gate, _) = Transfer::decode_publication(&packet, [1; 32], id, b).unwrap();
        assert_eq!(decoded.canonical_bytes(), ready.canonical_bytes());
        assert_eq!(gate, [2; 32]);
        assert!(Transfer::decode_publication(&packet, [3; 32], id, b).is_err());
        assert!(Transfer::decode_publication(&packet, [1; 32], [3; 32], b).is_err());
        for axis in [0, 8, 40, 104, 223] {
            let mut wrong = packet;
            wrong[axis] ^= 1;
            assert!(
                Transfer::decode_publication(&wrong, [1; 32], id, b).is_err(),
                "axis {axis}"
            );
        }
        let mut wrong = packet;
        wrong[72..104].fill(0);
        assert!(Transfer::decode_publication(&wrong, [1; 32], id, b).is_err());
        let complete = Transfer::publication_completion([1; 32], id, [2; 32], b).unwrap();
        assert_ne!(
            complete,
            Transfer::publication_completion([1; 32], id, [3; 32], b).unwrap()
        );
        assert!(Transfer::publication_completion([0; 32], id, [2; 32], b).is_err());
        assert_ne!(&complete[..8], &packet[..8]);
    });
}

#[test]
fn native_currentness_gate_binds_actual_native_registration_and_launch() {
    use crate::NativeApplicationCurrentnessRootRecordV1 as Gate;
    let (mut account, binding) = binding();
    account.with_budget(|b| {
        let bytes = fixture::policy_wire(3);
        b.reserve_storage(bytes.len()).unwrap();
        let policy = retain(Policy::decode(&bytes, b), b);
        let manifest = binding.compiler_handoff().launch_manifest();
        let (request, storage) = Gate::request(&policy, manifest, &binding, [42; 32], b).unwrap();
        b.reserve_storage(storage.additional_storage()).unwrap();
        assert_eq!(
            request.registration_identity(),
            *binding.identity().as_bytes()
        );
        assert_eq!(
            request.association_identity(),
            *binding.association().identity().as_bytes()
        );
        assert_eq!(
            request.carriage_identity(),
            binding.association().carriage_identity()
        );
        assert!(request.matches_launch(&policy, manifest, b).unwrap());
        assert!(Gate::request(&policy, manifest, &binding, [0; 32], b).is_err());
        let wrong_launch = retain(
            Launch::new(
                Client::new(1202, 1001, 1001).unwrap(),
                Service::new(2001, 2001).unwrap(),
                &policy,
                b,
            ),
            b,
        );
        assert!(Gate::request(&policy, &wrong_launch, &binding, [42; 32], b).is_err());
        assert!(!request.matches_launch(&policy, &wrong_launch, b).unwrap());
        let wire = request.gate_bytes(b).unwrap();
        b.reserve_storage(wire.len()).unwrap();
        assert_eq!(Gate::decode_gate(&wire, b).unwrap().0, request);
        let mut wrong_tail = wire;
        wrong_tail[wrong_tail.len() - 1] = 1;
        assert!(Gate::decode_gate(&wrong_tail, b).is_err());
    });
}
fn decoded<T>(
    bytes: &[u8],
    q: Quote,
    decode: impl FnOnce(&[u8], &mut Budget<'_>) -> Result<(T, Storage)>,
) -> Result<(T, Storage)> {
    let mut work = Work::new(q.work);
    let mut b = Budget::new(&mut work, q.input_floor + q.scratch);
    b.reserve_storage(q.input_floor).unwrap();
    decode(bytes, &mut b)
}

#[test]
fn native_binding_retains_original_owners_and_round_trips_without_authority() {
    let (mut account, binding) = binding();
    assert_eq!(BINDING_BYTES, 904);
    assert_eq!(INPUT_BYTES, 448);
    assert_eq!(
        binding.association().compiler_handoff_identity(),
        *binding.compiler_handoff().identity().as_bytes()
    );
    assert_eq!(
        binding.association().readiness_sha256(),
        binding.inputs().readiness_sha256()
    );
    assert_eq!(
        binding.association().readiness_byte_len(),
        binding.inputs().readiness_byte_len()
    );
    assert_eq!(binding.occurrence().inputs().len(), 4);
    assert_eq!(binding.descriptors().as_array(), [10, 11, 12, 13]);
    assert!(!binding.authenticates_application_occurrence());
    assert!(!binding.grants_load_authority());
    assert!(!binding.grants_launch_authority());
    account.with_budget(|b| {
        let before = b.storage();
        let work = b.work();
        let ledger = b.work_ledger_identity_v1();
        let account = b.storage_account_identity_v1();
        let (copy, charge) = Binding::decode(binding.canonical_bytes(), b).unwrap();
        assert_eq!(copy, binding);
        assert_eq!(charge.additional_storage(), copy.retained_storage());
        assert_eq!(b.storage(), before);
        assert_eq!(b.work(), work + Binding::decoding_quote().work);
        assert!(ledger == b.work_ledger_identity_v1());
        assert_eq!(account, b.storage_account_identity_v1());
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(b.storage(), before + copy.retained_storage());
    });
}

#[test]
fn all_wire_bytes_lengths_and_legacy_cross_families_reject() {
    let (_, binding) = binding();
    for offset in 0..BINDING_BYTES {
        let mut bytes = *binding.canonical_bytes();
        bytes[offset] ^= 1;
        assert!(
            decoded(&bytes, Binding::decoding_quote(), Binding::decode).is_err(),
            "binding byte {offset}"
        );
    }
    for offset in 0..INPUT_BYTES {
        let mut bytes = *binding.inputs().canonical_bytes();
        bytes[offset] ^= 1;
        assert!(
            decoded(&bytes, Inputs::decoding_quote(), Inputs::decode).is_err(),
            "inputs byte {offset}"
        );
    }
    for length in [0, 1, 23, BINDING_BYTES - 1] {
        assert!(
            decoded(
                &binding.canonical_bytes()[..length],
                Binding::decoding_quote(),
                Binding::decode
            )
            .is_err()
        );
    }
    let mut extra = binding.canonical_bytes().to_vec();
    extra.push(0);
    assert!(decoded(&extra, Binding::decoding_quote(), Binding::decode).is_err());
    assert!(
        crate::WorkerV3ApplicationRegistrationBindingV1::decode(binding.canonical_bytes()).is_err()
    );
    assert!(
        crate::WorkerV3ApplicationRegistrationInputsV1::decode(binding.inputs().canonical_bytes())
            .is_err()
    );
    let mut legacy_magic = *binding.canonical_bytes();
    legacy_magic[..8].copy_from_slice(b"F3AREG1\0");
    seal(&mut legacy_magic);
    assert!(decoded(&legacy_magic, Binding::decoding_quote(), Binding::decode).is_err());
}

#[test]
fn resealed_invalid_input_coordinates_and_identity_bounds_reject() {
    let (_, binding) = binding();
    for axis in 0..10 {
        let mut bytes = *binding.inputs().canonical_bytes();
        match axis {
            0 => bytes[10] = 1,
            1 => bytes[16] = 1,
            2 => bytes[HEADER..HEADER + 32].fill(0),
            3 => bytes[HEADER + 32..OCCURRENCE_OFFSET].fill(0),
            4 => bytes[HEADER + 32..OCCURRENCE_OFFSET].copy_from_slice(
                &((MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 as u64) + 1).to_le_bytes(),
            ),
            5 => bytes[DESCRIPTORS_OFFSET..DESCRIPTORS_OFFSET + 4]
                .copy_from_slice(&2i32.to_le_bytes()),
            6 => bytes[DESCRIPTORS_OFFSET..DESCRIPTORS_OFFSET + 4]
                .copy_from_slice(&11i32.to_le_bytes()),
            7 => bytes[DESCRIPTORS_OFFSET..DESCRIPTORS_OFFSET + 4]
                .copy_from_slice(&195i32.to_le_bytes()),
            8 => bytes[CHALLENGE_OFFSET..INPUT_CHECKSUM].fill(0),
            _ => bytes[OCCURRENCE_OFFSET..DESCRIPTORS_OFFSET].fill(0),
        }
        seal(&mut bytes);
        assert!(
            decoded(&bytes, Inputs::decoding_quote(), Inputs::decode).is_err(),
            "axis {axis}"
        );
    }
    assert!(Inputs::construction_quote(0).is_err());
    assert!(Inputs::construction_quote(MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5).is_ok());
    assert!(Inputs::construction_quote(MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 + 1).is_err());
}

#[test]
fn association_handoff_readiness_and_occurrence_are_independent_bound_axes() {
    let (mut account, baseline) = binding();
    account.with_budget(|b| {
        let (handoff, association, readiness) = owners(b, 1202);
        let changed = inputs(b, &readiness, 11);
        let changed = retained(Binding::new(handoff, association, changed, b), b);
        assert_ne!(changed.identity(), baseline.identity());
        let (wrong_handoff, _, _) = owners(b, 1203);
        let (_, association, readiness) = owners(b, 1204);
        let inputs = inputs(b, &readiness, 10);
        let floor = b.storage();
        assert!(matches!(
            Binding::new(wrong_handoff, association, inputs, b),
            Err(Error::AssociationMismatch)
        ));
        assert_eq!(b.storage(), floor);
    });
    let mut bytes = *baseline.canonical_bytes();
    bytes[INPUT_OFFSET + HEADER] ^= 1;
    seal(&mut bytes[INPUT_OFFSET..BINDING_CHECKSUM]);
    seal(&mut bytes);
    assert!(matches!(
        decoded(&bytes, Binding::decoding_quote(), Binding::decode),
        Err(Error::AssociationMismatch)
    ));
}

#[test]
fn fixed_decode_quotes_preserve_original_account_and_exact_denial_history() {
    let (_, binding) = binding();
    let q = Binding::decoding_quote();
    for case in 0..4 {
        let floor = q.input_floor - usize::from(case == 3);
        let mut work = Work::new(q.work - usize::from(case == 1));
        let mut b = Budget::new(&mut work, floor + q.scratch - usize::from(case == 2));
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let account = b.storage_account_identity_v1();
        let result = Binding::decode(binding.canonical_bytes(), &mut b);
        assert_eq!(result.is_ok(), case == 0, "case {case}");
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(b.storage_account_identity_v1(), account);
        if case == 0 {
            assert_eq!(b.work(), q.work);
            assert_eq!(b.peak_storage(), floor + q.scratch);
        }
        if case == 1 {
            assert!(b.failed_work().is_some());
        }
        if case == 2 {
            assert!(b.failed_storage().is_some());
        }
    }
}

#[test]
fn native_session_borrows_original_binding_and_recovers_only_metered_native_owners() {
    use super::{
        NativeApplicationSessionKindV1 as Kind, NativeApplicationSessionMessageV1 as Message,
    };
    let (mut account, binding) = binding();
    account.with_budget(|b| {
        let hello = retained(Message::hello(binding.inputs(), [1; 32], b), b);
        let challenge = retained(Message::challenge(&binding, [1; 32], [2; 32], b), b);
        let transcript = challenge.transcript().unwrap();
        let accept = retained(Message::accept(transcript, b), b);
        let ready = retained(Message::ready(transcript, b), b);
        assert_eq!(challenge.rights(), 1);
        assert_eq!(hello.rights(), 0);
        assert_eq!(ready.rights(), 0);
        assert!(!ready.authenticates_application_occurrence());
        assert!(!ready.grants_load_authority());
        assert!(!ready.grants_launch_authority());
        for message in [&hello, &challenge, &accept, &ready] {
            let q = Message::decoding_quote(message.kind());
            let (roundtrip, _) = decoded(message.canonical_bytes(), q, Message::decode).unwrap();
            assert_eq!(roundtrip, *message);
            assert!(
                crate::WorkerV3ApplicationSessionMessageV1::decode(message.canonical_bytes())
                    .is_err()
            );
        }
        let (actual, _) = challenge.decode_registration(b).unwrap();
        assert_eq!(actual, binding);
        let (actual, _) = hello.decode_inputs(b).unwrap();
        assert_eq!(actual, *binding.inputs());
        assert!(matches!(ready.decode_registration(b), Err(Error::Kind)));
        assert_eq!(ready.kind(), Kind::Ready);
        assert_eq!(ready.transcript(), Some(transcript));
    });
}

#[test]
fn native_session_nonces_and_resealed_payload_substitution_fail_closed() {
    use super::NativeApplicationSessionMessageV1 as Message;
    let (mut account, binding) = binding();
    account.with_budget(|b| {
        for (app, root) in [([0; 32], [2; 32]), ([1; 32], [0; 32]), ([1; 32], [1; 32])] {
            assert!(matches!(
                Message::challenge(&binding, app, root, b),
                Err(Error::Transcript)
            ));
        }
        assert!(matches!(
            Message::hello(binding.inputs(), [0; 32], b),
            Err(Error::Transcript)
        ));
        let message = retained(Message::challenge(&binding, [1; 32], [2; 32], b), b);
        for offset in 0..message.canonical_bytes().len() {
            let mut bytes = message.canonical_bytes().to_vec();
            bytes[offset] ^= 1;
            assert!(
                decoded(
                    &bytes,
                    Message::decoding_quote(message.kind()),
                    Message::decode
                )
                .is_err(),
                "byte {offset}"
            );
        }
        for axis in 0..4 {
            let mut bytes = message.canonical_bytes().to_vec();
            match axis {
                0 => bytes[16..48].fill(0),
                1 => bytes[48..80].fill(1),
                2 => bytes[80] ^= 1,
                _ => bytes[11] = 1,
            }
            seal(&mut bytes);
            assert!(
                decoded(
                    &bytes,
                    Message::decoding_quote(message.kind()),
                    Message::decode
                )
                .is_err(),
                "axis {axis}"
            );
        }
    });
}

#[test]
fn native_custodian_ready_is_distinct_and_binds_original_transcript_and_controller() {
    use super::{
        NativeApplicationSessionKindV1 as Kind, NativeApplicationSessionMessageV1 as Message,
    };
    let (mut account, binding) = binding();
    account.with_budget(|b| {
        let challenge = retained(Message::challenge(&binding, [1; 32], [2; 32], b), b);
        let transcript = challenge.transcript().unwrap();
        let proof = retained(
            NativeApplicationProofSessionV1::new(transcript, [3; 32], [4; 32], (123, 456, 789), b),
            b,
        );
        let ready = retained(Message::custodian_ready(&proof, b), b);
        assert_eq!(ready.kind(), Kind::CustodianReady);
        assert_eq!(ready.rights(), 1);
        assert_eq!(ready.transcript(), Some(transcript));
        assert_eq!(proof.deployment(), [3; 32]);
        assert_eq!(proof.nonce(), [4; 32]);
        assert_eq!(proof.controller(), (123, 456, 789));
        assert!(!proof.authenticates_controller_custody());
        assert!(!proof.grants_load_authority());
        assert!(!proof.grants_launch_authority());
        let (roundtrip, _) = decoded(
            ready.canonical_bytes(),
            Message::decoding_quote(Kind::CustodianReady),
            Message::decode,
        )
        .unwrap();
        assert_eq!(roundtrip, ready);
        let (actual, charge) = ready.decode_proof_session(b).unwrap();
        assert_eq!(actual, proof);
        assert_eq!(charge.additional_storage(), actual.retained_storage());
        let ordinary = retained(Message::ready(transcript, b), b);
        assert_eq!(ordinary.kind(), Kind::Ready);
        assert_eq!(ordinary.rights(), 0);
        assert!(matches!(ordinary.decode_proof_session(b), Err(Error::Kind)));
        assert!(crate::WorkerV3ApplicationProofSessionV1::decode(proof.canonical_bytes()).is_err());
        assert!(
            crate::WorkerV3ApplicationSessionMessageV1::decode(ready.canonical_bytes()).is_err()
        );
        let mut cross_transcript = ready.canonical_bytes().to_vec();
        cross_transcript[80] ^= 1;
        seal(&mut cross_transcript);
        assert!(matches!(
            decoded(
                &cross_transcript,
                Message::decoding_quote(Kind::CustodianReady),
                Message::decode
            ),
            Err(Error::Transcript)
        ));
        let mut unknown = ordinary.canonical_bytes().to_vec();
        unknown[10] = 6;
        seal(&mut unknown);
        assert!(matches!(
            decoded(
                &unknown,
                Message::decoding_quote(Kind::Ready),
                Message::decode
            ),
            Err(Error::Kind)
        ));
    });
}

#[test]
fn native_proof_session_rejects_repeated_nonces_root_credentials_and_resealed_zero_fields() {
    use super::NativeApplicationSessionMessageV1 as Message;
    let (mut account, binding) = binding();
    account.with_budget(|b| {
        let challenge = retained(Message::challenge(&binding, [1; 32], [2; 32], b), b);
        let transcript = challenge.transcript().unwrap();
        for (deployment, nonce, controller) in [
            ([0; 32], [4; 32], (123, 456, 789)),
            ([3; 32], [0; 32], (123, 456, 789)),
            ([3; 32], [1; 32], (123, 456, 789)),
            ([3; 32], [2; 32], (123, 456, 789)),
            ([3; 32], [4; 32], (0, 456, 789)),
            ([3; 32], [4; 32], (u32::MAX, 456, 789)),
            ([3; 32], [4; 32], (123, 0, 789)),
            ([3; 32], [4; 32], (123, u32::MAX, 789)),
            ([3; 32], [4; 32], (123, 456, 0)),
            ([3; 32], [4; 32], (123, 456, u32::MAX)),
        ] {
            assert!(
                NativeApplicationProofSessionV1::new(transcript, deployment, nonce, controller, b)
                    .is_err()
            );
        }
        let proof = retained(
            NativeApplicationProofSessionV1::new(transcript, [3; 32], [4; 32], (123, 456, 789), b),
            b,
        );
        let q = NativeApplicationProofSessionV1::decoding_quote();
        for offset in 0..NATIVE_APPLICATION_PROOF_SESSION_BYTES_V1 {
            let mut bytes = *proof.canonical_bytes();
            bytes[offset] ^= 1;
            assert!(
                decoded(&bytes, q, NativeApplicationProofSessionV1::decode).is_err(),
                "byte {offset}"
            );
        }
        for axis in 0..10 {
            let mut bytes = *proof.canonical_bytes();
            match axis {
                0 => bytes[10] = 1,
                1 => bytes[16] = 1,
                2 => bytes[196] = 1,
                3 => bytes[24..56].fill(0),
                4 => bytes[56..88].fill(1),
                5 => bytes[88..120].fill(0),
                6 => bytes[120..152].fill(0),
                7 => bytes[152..184].fill(1),
                8 => bytes[188..192].fill(0),
                _ => bytes[192..196].fill(255),
            }
            proof_session::seal_proof(&mut bytes);
            assert!(
                decoded(&bytes, q, NativeApplicationProofSessionV1::decode).is_err(),
                "axis {axis}"
            );
        }
    });
}

#[test]
fn session_and_proof_decoders_preserve_exact_work_scratch_and_input_floor_denials() {
    use super::{
        NativeApplicationSessionKindV1 as Kind, NativeApplicationSessionMessageV1 as Message,
    };
    let (mut account, binding) = binding();
    let (proof, ready) = account.with_budget(|b| {
        let challenge = retained(Message::challenge(&binding, [1; 32], [2; 32], b), b);
        let proof = retained(
            NativeApplicationProofSessionV1::new(
                challenge.transcript().unwrap(),
                [3; 32],
                [4; 32],
                (123, 456, 789),
                b,
            ),
            b,
        );
        let ready = retained(Message::custodian_ready(&proof, b), b);
        (proof, ready)
    });
    for use_message in [false, true] {
        let q = if use_message {
            Message::decoding_quote(Kind::CustodianReady)
        } else {
            NativeApplicationProofSessionV1::decoding_quote()
        };
        for case in 0..4 {
            let floor = q.input_floor - usize::from(case == 3);
            let mut work = Work::new(q.work - usize::from(case == 1));
            let mut b = Budget::new(&mut work, floor + q.scratch - usize::from(case == 2));
            b.reserve_storage(floor).unwrap();
            let account = b.storage_account_identity_v1();
            let ledger = b.work_ledger_identity_v1();
            let ok = if use_message {
                Message::decode(ready.canonical_bytes(), &mut b).is_ok()
            } else {
                NativeApplicationProofSessionV1::decode(proof.canonical_bytes(), &mut b).is_ok()
            };
            assert_eq!(ok, case == 0, "message={use_message},case={case}");
            assert_eq!(b.storage(), floor);
            assert_eq!(b.storage_account_identity_v1(), account);
            assert!(b.work_ledger_identity_v1() == ledger);
            if case == 0 {
                assert_eq!(b.work(), q.work);
                assert_eq!(b.peak_storage(), floor + q.scratch);
            }
            if case == 1 {
                assert!(b.failed_work().is_some());
            }
            if case == 2 {
                assert!(b.failed_storage().is_some());
            }
        }
    }
}
