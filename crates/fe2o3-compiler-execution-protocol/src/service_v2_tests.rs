//! Inert packet fixtures only: no protected producer, signing service or GPU credit.
use super::*;
use crate::{
    CompilerExecutionAttestationReceiptV2 as Receipt,
    CompilerExecutionServiceRequestV1 as LegacyRequest,
    CompilerExecutionServiceResponseV1 as LegacyResponse,
};
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::{Digest, Sha256};
#[path = "../tests/support/native_attestation_fixture.rs"]
mod fixture;
#[path = "../tests/support/native_receipt_fixture.rs"]
mod receipt_fixture;
use CompilerExecutionServiceRequestV2 as ServiceRequest;
use CompilerExecutionServiceResponseV2 as ServiceResponse;
use fixture::{header, policy_wire, request_wire, seal};

const LIMIT: usize = 2_000_000;
const WORK: usize = 30_000_000;
const NONCE: [u8; 32] = [0x93; 32];

fn native_carriage(budget: &mut Budget<'_>) -> Carriage {
    let p = policy_wire(2);
    let q = request_wire(2);
    let r = receipt_fixture::receipt_wire(2);
    let wires = p.len() + q.len() + r.len();
    budget.reserve_storage(wires).unwrap();
    let (p, _) = retain(Policy::decode(&p, budget), budget).unwrap();
    let (q, _) = retain(Request::decode(&q, budget), budget).unwrap();
    let (r, _) = retain(Receipt::decode(&r, budget), budget).unwrap();
    let (u, s) = Publication::new([0x81; 32], [0x82; 32], r, budget).unwrap();
    budget.reserve_storage(s.additional_storage()).unwrap();
    let (a, _) = retain(Ack::new(&u, [0x83; 32], budget), budget).unwrap();
    let (c, s) = Carriage::new(p, q, u, a, budget).unwrap();
    budget.reserve_storage(s.additional_storage()).unwrap();
    budget.release_storage(wires).unwrap();
    c
}
fn requests(c: &Carriage) -> [QPayload<'_>; 7] {
    [
        QPayload::Inspect,
        QPayload::Prepare {
            sequence: 1,
            prior_rollback_anchor: [0; 32],
        },
        QPayload::Issue(c.request()),
        QPayload::Publish {
            request: c.request(),
            publication: c.publication(),
        },
        QPayload::Cancel,
        QPayload::Recover(c.request().subject()),
        QPayload::VerifyCurrent {
            carriage: c,
            verification_challenge: NONCE,
        },
    ]
}
fn responses<'a>(c: &'a Carriage, current: &'a Current) -> [PPayload<'a>; 9] {
    [
        PPayload::Ready {
            sequence: 1,
            prior_rollback_anchor: [0; 32],
        },
        PPayload::Prepared(c.request().challenge()),
        PPayload::Issued(c.publication()),
        PPayload::Published {
            acknowledgment: c.acknowledgment(),
            disposition: Disposition::Advanced,
        },
        PPayload::Published {
            acknowledgment: c.acknowledgment(),
            disposition: Disposition::AlreadyAcknowledged,
        },
        PPayload::Cancelled {
            sequence: 1,
            prior_rollback_anchor: [0; 32],
        },
        PPayload::Recovered(c),
        PPayload::ReceiptAbsent {
            sequence: 1,
            prior_rollback_anchor: [0; 32],
        },
        PPayload::VerifiedCurrent(current),
    ]
}

// Identity-only V3 wire with native identity coordinates and synthetic signed
// anchor observations. This deliberately does NOT authenticate native carriage
// currentness: the missing protected leaf constructor must do that separately.
fn current_wire(c: &Carriage) -> Vec<u8> {
    use fe2o3_external_anchor_protocol::{
        AnchorPositionV1, AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1,
        HashChainHeadV1, PinnedAnchorKeyV1, TransactionDigestV1, UnsignedAnchorObservationV1,
    };
    let signer = SigningKey::from_bytes(&[0x52; 32]);
    let key = PinnedAnchorKeyV1::from_bytes(signer.verifying_key().to_bytes()).unwrap();
    let mut anchors = Vec::new();
    for recover in [false, true] {
        let prepared = AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
            .prepare(TransactionDigestV1::from_bytes([0x91; 32]), &key)
            .unwrap();
        let pending = if recover {
            prepared.begin_recovery(CallerNonceV1::from_bytes([0x92; 32]), &key)
        } else {
            prepared.begin_advance(CallerNonceV1::from_bytes([0x91; 32]), &key)
        }
        .unwrap();
        let observation = UnsignedAnchorObservationV1::from_challenge(
            pending.challenge(),
            AnchorPositionV1::Proposed,
        );
        let signature = signer.sign(&observation.signing_bytes());
        let signed = observation.attach_signature(signature.to_bytes());
        let anchor =
            AnchorTransitionReceiptV1::new(pending.challenge().clone(), &signed, &key).unwrap();
        anchors.extend_from_slice(anchor.canonical_bytes());
    }
    let mut verification = vec![0; crate::COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V3];
    header(&mut verification, b"F2O3CEV3", 3);
    let mut offset = 24;
    for value in [
        c.policy().identity().as_bytes(),
        c.request().subject().identity().sha256(),
        c.identity().as_bytes(),
        &c.acknowledgment().issuer_journal_identity(),
        &c.acknowledgment().worker_ledger_record_identity(),
    ] {
        put(&mut verification, &mut offset, value);
    }
    put(&mut verification, &mut offset, &1u64.to_le_bytes());
    put(&mut verification, &mut offset, &[0; 32]);
    put(
        &mut verification,
        &mut offset,
        &c.acknowledgment().current_rollback_anchor(),
    );
    put(&mut verification, &mut offset, &key.to_bytes());
    put(&mut verification, &mut offset, &anchors);
    put(&mut verification, &mut offset, &[0x94; 32]);
    put(&mut verification, &mut offset, &[0x95; 32]);
    assert_eq!(offset + 32, verification.len());
    seal(
        &mut verification,
        "COMPILER-EXECUTION-CURRENT-RECORD-VERIFICATION",
        3,
    );
    let signer = SigningKey::from_bytes(&[0x51; 32]);
    let mut bytes = vec![0; V];
    header(&mut bytes, b"F2O3CEA3", 3);
    let mut offset = 24;
    put(&mut bytes, &mut offset, &NONCE);
    put(&mut bytes, &mut offset, &verification);
    put(&mut bytes, &mut offset, &signer.verifying_key().to_bytes());
    let message = derive_identity(
        b"FE2O3/COMPILER-EXECUTION-CURRENT-RECORD-ATTESTATION-SIGNATURE/V3\0",
        &bytes[..offset],
    );
    put(&mut bytes, &mut offset, &signer.sign(&message).to_bytes());
    assert_eq!(offset + 32, V);
    seal(
        &mut bytes,
        "COMPILER-EXECUTION-CURRENT-RECORD-ATTESTATION",
        3,
    );
    bytes
}
fn current(c: &Carriage, budget: &mut Budget<'_>) -> Current {
    let bytes = current_wire(c);
    budget.reserve_storage(bytes.len()).unwrap();
    let (v, _) = retain(decode_current(&bytes, budget), budget).unwrap();
    budget.release_storage(bytes.len()).unwrap();
    v
}
fn reseal(bytes: &mut [u8], response: bool) {
    seal(
        bytes,
        if response {
            "COMPILER-EXECUTION-SERVICE-RESPONSE"
        } else {
            "COMPILER-EXECUTION-SERVICE-REQUEST"
        },
        2,
    );
}
fn admitted(bytes: &[u8], response: bool) -> Result<()> {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(bytes.len()).unwrap();
    let result = if response {
        ServiceResponse::decode(bytes, &mut budget).map(|_| ())
    } else {
        ServiceRequest::decode(bytes, &mut budget).map(|_| ())
    };
    assert_eq!(budget.storage(), bytes.len());
    result
}
fn packet_wires() -> Vec<(bool, Vec<u8>)> {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    let c = native_carriage(&mut budget);
    let v = current(&c, &mut budget);
    let mut wires = Vec::new();
    let mut id = None;
    for payload in requests(&c) {
        let (p, _) = retain(
            ServiceRequest::new(c.policy(), payload, &mut budget),
            &mut budget,
        )
        .unwrap();
        id = Some(p.identity());
        wires.push((false, p.canonical_bytes().to_vec()));
        budget.release_storage(p.retained_storage()).unwrap();
    }
    for payload in responses(&c, &v) {
        let (p, _) = retain(
            ServiceResponse::new(id.unwrap(), c.policy(), payload, &mut budget),
            &mut budget,
        )
        .unwrap();
        wires.push((true, p.canonical_bytes().to_vec()));
        budget.release_storage(p.retained_storage()).unwrap();
    }
    wires
}

#[test]
fn every_operation_roundtrips_with_explicit_ownership_and_redecode() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    let c = native_carriage(&mut budget);
    let current = current(&c, &mut budget);
    let floor = budget.storage();
    let mut id = None;
    for payload in requests(&c) {
        let (packet, charge) = ServiceRequest::new(c.policy(), payload, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(charge.additional_storage(), packet.retained_storage());
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(packet.canonical_bytes().len(), packet.kind().packet_bytes());
        assert!(!packet.grants_authority());
        assert_eq!(packet.policy_identity(), c.policy().identity());
        id = Some(packet.identity());
        let (decoded, charge) =
            ServiceRequest::decode(packet.canonical_bytes(), &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(packet, decoded);
        assert_ne!(
            packet.canonical_bytes().as_ptr(),
            decoded.canonical_bytes().as_ptr()
        );
        match packet.kind() {
            QKind::Issue | QKind::Publish => {
                let (q, charge) = packet.decode_request(&mut budget).unwrap();
                budget.reserve_storage(charge.additional_storage()).unwrap();
                assert_eq!(&q, c.request());
                assert_ne!(
                    q.canonical_bytes().as_ptr(),
                    c.request().canonical_bytes().as_ptr()
                );
                budget.release_storage(q.retained_storage()).unwrap();
                if packet.kind() == QKind::Publish {
                    let (u, charge) = packet.decode_publication(&mut budget).unwrap();
                    budget.reserve_storage(charge.additional_storage()).unwrap();
                    assert_eq!(&u, c.publication());
                    budget.release_storage(u.retained_storage()).unwrap();
                }
            }
            QKind::Recover => {
                let (s, charge) = packet.decode_subject(&mut budget).unwrap();
                budget.reserve_storage(charge.additional_storage()).unwrap();
                assert_eq!(&s, c.request().subject());
                budget.release_storage(charge.additional_storage()).unwrap();
            }
            QKind::VerifyCurrent => {
                let (recovered, charge) = packet.decode_carriage(&mut budget).unwrap();
                budget.reserve_storage(charge.additional_storage()).unwrap();
                assert_eq!(recovered, c);
                assert_eq!(packet.verification_challenge(), Some(NONCE));
                budget
                    .release_storage(recovered.retained_storage())
                    .unwrap();
            }
            _ => {
                assert!(matches!(
                    packet.decode_carriage(&mut budget),
                    Err(Error::Payload)
                ));
            }
        }
        assert_eq!(packet.canonical_bytes(), decoded.canonical_bytes());
        budget
            .release_storage(packet.retained_storage() + decoded.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), floor);
    }
    for payload in responses(&c, &current) {
        let (packet, charge) =
            ServiceResponse::new(id.unwrap(), c.policy(), payload, &mut budget).unwrap();
        assert_eq!(charge.additional_storage(), packet.retained_storage());
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let (decoded, charge) =
            ServiceResponse::decode(packet.canonical_bytes(), &mut budget).unwrap();
        budget.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(packet, decoded);
        assert_eq!(packet.request_identity(), id.unwrap());
        assert_eq!(packet.policy_identity(), c.policy().identity());
        assert!(!packet.grants_authority());
        match packet.kind() {
            PKind::Prepared => {
                let (h, s) = packet.decode_challenge(&mut budget).unwrap();
                budget.reserve_storage(s.additional_storage()).unwrap();
                assert_eq!(&h, c.request().challenge());
                budget.release_storage(h.retained_storage()).unwrap();
            }
            PKind::Issued => {
                let (u, s) = packet.decode_publication(&mut budget).unwrap();
                budget.reserve_storage(s.additional_storage()).unwrap();
                assert_eq!(&u, c.publication());
                budget.release_storage(u.retained_storage()).unwrap();
            }
            PKind::Published => {
                let (a, s) = packet.decode_acknowledgment(&mut budget).unwrap();
                budget.reserve_storage(s.additional_storage()).unwrap();
                assert_eq!(&a, c.acknowledgment());
                assert!(packet.disposition().is_some());
                budget.release_storage(a.retained_storage()).unwrap();
            }
            PKind::Recovered => {
                let (r, s) = packet.decode_carriage(&mut budget).unwrap();
                budget.reserve_storage(s.additional_storage()).unwrap();
                assert_eq!(r, c);
                budget.release_storage(r.retained_storage()).unwrap();
            }
            PKind::VerifiedCurrent => {
                let (v, s) = packet
                    .decode_current_record_attestation(&mut budget)
                    .unwrap();
                budget.reserve_storage(s.additional_storage()).unwrap();
                assert_eq!(v, current);
                assert!(!v.grants_authority());
                budget.release_storage(s.additional_storage()).unwrap();
            }
            _ => {
                assert!(matches!(
                    packet.decode_carriage(&mut budget),
                    Err(Error::Payload)
                ));
            }
        }
        budget
            .release_storage(packet.retained_storage() + decoded.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn all_single_byte_mutations_truncations_and_extensions_are_rejected() {
    for (response, bytes) in packet_wires() {
        for i in 0..bytes.len() {
            let mut mutated = bytes.clone();
            mutated[i] ^= 1;
            assert!(admitted(&mutated, response).is_err(), "byte {i}");
        }
        for end in 0..bytes.len() {
            assert!(admitted(&bytes[..end], response).is_err());
        }
        let mut extended = bytes;
        extended.push(0);
        assert!(admitted(&extended, response).is_err());
    }
}

#[test]
fn resealed_bad_tags_reserved_lengths_positions_and_families_are_rejected() {
    for (response, bytes) in packet_wires() {
        assert!(if response {
            LegacyResponse::decode(&bytes).is_err()
        } else {
            LegacyRequest::decode(&bytes).is_err()
        });
        for offset in [0, 7, 8, 9, 10, 11, 12, 19, 20, 23] {
            let mut bad = bytes.clone();
            bad[offset] ^= 0x80;
            reseal(&mut bad, response);
            assert!(admitted(&bad, response).is_err(), "offset {offset}");
        }
        let mut bad = bytes.clone();
        bad[7] = b'1';
        bad[8..10].copy_from_slice(&1u16.to_le_bytes());
        // A valid V1 outer footer does not make V2 retry the legacy family.
        seal(
            &mut bad,
            if response {
                "COMPILER-EXECUTION-SERVICE-RESPONSE"
            } else {
                "COMPILER-EXECUTION-SERVICE-REQUEST"
            },
            1,
        );
        assert!(matches!(admitted(&bad, response), Err(Error::Header)));
        let policy_offset = if response { 56 } else { 24 };
        let mut bad = bytes.clone();
        bad[policy_offset..policy_offset + 32].fill(0);
        reseal(&mut bad, response);
        assert!(admitted(&bad, response).is_err());
        let mut bad = bytes.clone();
        bad[policy_offset + 32] ^= 1;
        reseal(&mut bad, response);
        assert!(admitted(&bad, response).is_err());
        let mut bad = bytes.clone();
        bad[policy_offset + 40] ^= 1;
        reseal(&mut bad, response);
        assert!(admitted(&bad, response).is_err());
    }
}

#[test]
fn resealed_nested_legacy_leaves_never_retry_v1() {
    for (response, bytes) in packet_wires() {
        let tag = u16::from_le_bytes(bytes[10..12].try_into().unwrap());
        let start = if response {
            RESPONSE_PREFIX
        } else {
            REQUEST_PREFIX
        };
        let leaf_ranges: Vec<(usize, usize)> = match (response, tag) {
            (false, 3) => vec![(start, Q)],
            (false, 4) => vec![(start, Q), (start + Q, U)],
            (false, 6) => vec![(start, SUBJECT_BYTES)],
            (false, 7) => vec![(start, C)],
            (true, 2) => vec![(start, H)],
            (true, 3) => vec![(start, U)],
            (true, 4) => vec![(start + 8, A)],
            (true, 6) => vec![(start, C)],
            _ => vec![],
        };
        for (offset, len) in leaf_ranges {
            let mut bad = bytes.clone();
            bad[offset + 7] = b'1';
            bad[offset + 8..offset + 10].copy_from_slice(&1u16.to_le_bytes());
            // The outer identity is genuine; the explicit nested family gate
            // must reject before the nested footer could be admitted.
            assert!(len >= 24);
            reseal(&mut bad, response);
            assert!(admitted(&bad, response).is_err());
        }
    }
    // A fully valid legacy request (not just a tampered native leaf).
    let mut bytes = packet_wires()
        .into_iter()
        .find(|(r, b)| !r && b[10] == 3)
        .unwrap()
        .1;
    bytes[REQUEST_PREFIX..REQUEST_PREFIX + Q].copy_from_slice(&request_wire(1));
    reseal(&mut bytes, false);
    assert!(matches!(admitted(&bytes, false), Err(Error::Attestation)));
}

#[test]
fn fully_resealed_request_publication_nonce_and_policy_mismatches_reject() {
    let original = packet_wires()
        .into_iter()
        .find(|(r, b)| !r && b[10] == 4)
        .unwrap()
        .1;
    for axis in [24, 24 + 96, 24 + 128, 224 + 378] {
        let mut bytes = original.clone();
        bytes[REQUEST_PREFIX + axis] ^= 1;
        if axis >= 224 {
            seal(
                &mut bytes[REQUEST_PREFIX + 224..REQUEST_PREFIX + 914],
                "INERT-COMPILER-EXECUTION-SUBJECT",
                2,
            );
            let identity: [u8; 32] = bytes[REQUEST_PREFIX + 882..REQUEST_PREFIX + 914]
                .try_into()
                .unwrap();
            bytes[REQUEST_PREFIX + 80..REQUEST_PREFIX + 112].copy_from_slice(&identity);
        }
        seal(
            &mut bytes[REQUEST_PREFIX + 24..REQUEST_PREFIX + 224],
            "COMPILER-EXECUTION-CHALLENGE",
            2,
        );
        seal(
            &mut bytes[REQUEST_PREFIX..REQUEST_PREFIX + Q],
            "COMPILER-EXECUTION-REQUEST",
            2,
        );
        reseal(&mut bytes, false);
        assert!(admitted(&bytes, false).is_err());
    }
    let mut bytes = packet_wires()
        .into_iter()
        .find(|(r, b)| !r && b[10] == 7)
        .unwrap()
        .1;
    bytes[REQUEST_PREFIX + C..REQUEST_PREFIX + C + 32].fill(0);
    reseal(&mut bytes, false);
    assert!(admitted(&bytes, false).is_err());
    let bytes = packet_wires()
        .into_iter()
        .find(|(r, b)| *r && b[10] == 4)
        .unwrap()
        .1;
    for (offset, value) in [
        (RESPONSE_PREFIX, 0),
        (RESPONSE_PREFIX, 3),
        (RESPONSE_PREFIX + 1, 1),
        (RESPONSE_PREFIX + 7, 1),
    ] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        reseal(&mut bad, true);
        assert!(admitted(&bad, true).is_err());
    }
}

fn measure(
    bytes: &[u8],
    response: bool,
    work_limit: usize,
    storage: usize,
    floor: usize,
) -> (Result<()>, usize, usize, Option<usize>) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage);
    budget.reserve_storage(floor).unwrap();
    let result = if response {
        ServiceResponse::decode(bytes, &mut budget).map(|_| ())
    } else {
        ServiceRequest::decode(bytes, &mut budget).map(|_| ())
    };
    assert_eq!(budget.storage(), floor);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    )
}
#[test]
fn exact_and_one_short_decode_work_storage_and_input_floor() {
    for (response, bytes) in packet_wires() {
        let prefix = bytes.len() + 37;
        let (ok, used, peak, _) = measure(&bytes, response, WORK, LIMIT, prefix);
        ok.unwrap();
        assert!(
            used <= if response {
                MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_WORK_V2
            } else {
                MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_WORK_V2
            }
        );
        assert!(
            peak - prefix
                <= if response {
                    MAX_COMPILER_EXECUTION_SERVICE_RESPONSE_DECODE_STORAGE_V2
                } else {
                    MAX_COMPILER_EXECUTION_SERVICE_REQUEST_DECODE_STORAGE_V2
                }
        );
        let (ok, actual, actual_peak, _) = measure(&bytes, response, used, peak, prefix);
        ok.unwrap();
        assert_eq!((actual, actual_peak), (used, peak));
        assert!(matches!(
            measure(&bytes, response, used - 1, peak, prefix).0,
            Err(Error::Resource(Resource::Work(_)))
        ));
        let (err, _, _, failed) = measure(&bytes, response, used, peak - 1, prefix);
        assert!(matches!(err, Err(Error::Resource(Resource::Storage(_)))));
        assert_eq!(failed, Some(peak));
        assert!(matches!(
            measure(&bytes, response, used, LIMIT, bytes.len() - 1).0,
            Err(Error::Resource(Resource::Accounting))
        ));
    }
}

#[test]
fn constructors_check_prepaid_borrowed_owners_and_only_return_packet_storage() {
    let mut fixture_work = Work::new(WORK);
    let mut fixture_budget = Budget::new(&mut fixture_work, LIMIT);
    let c = native_carriage(&mut fixture_budget);
    let v = current(&c, &mut fixture_budget);
    for payload in requests(&c) {
        let floor = c.policy().retained_storage() + payload.input_storage();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(floor - 1).unwrap();
        assert!(matches!(
            ServiceRequest::new(c.policy(), payload, &mut budget),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.work(), resources::ENTRY_WORK);
        assert_eq!(budget.storage(), floor - 1);
    }
    for payload in responses(&c, &v) {
        let floor = c.policy().retained_storage() + payload.input_storage();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(floor - 1).unwrap();
        assert!(matches!(
            ServiceResponse::new(
                CompilerExecutionServiceRequestIdentityV2([0x91; 32]),
                c.policy(),
                payload,
                &mut budget
            ),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.work(), resources::ENTRY_WORK);
        assert_eq!(budget.storage(), floor - 1);
    }
}

#[test]
fn cumulative_work_denials_and_original_ledger_are_not_reset() {
    let bytes = packet_wires()
        .into_iter()
        .find(|(r, b)| !r && b[10] == 4)
        .unwrap()
        .1;
    let (_, used, _, _) = measure(&bytes, false, WORK, LIMIT, bytes.len());
    let mut work = Work::new(used + QW - 1);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(budget.reserve_storage(LIMIT + 1).is_err());
    budget.reserve_storage(bytes.len() + 37).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let (packet, charge) = ServiceRequest::decode(&bytes, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        packet.decode_request(&mut budget),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        ServiceRequest::decode(&bytes, &mut budget),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_storage(), Some(LIMIT + 1));
    assert!(budget.work_ledger_identity_v1() == identity);
}

#[test]
fn exact_and_short_constructor_quotas_cover_publish_verification() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    let c = native_carriage(&mut budget);
    let v = current(&c, &mut budget);
    for case in 0..3 {
        for payload in requests(&c) {
            let publish = matches!(payload, QPayload::Publish { .. });
            let total = if publish {
                COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_WORK_V2
            } else {
                QW
            };
            let scratch = if publish {
                COMPILER_EXECUTION_SERVICE_PUBLISH_REQUEST_STORAGE_V2
            } else {
                QS
            };
            let floor = c.policy().retained_storage() + payload.input_storage();
            let mut work = Work::new(total - usize::from(case == 1));
            let mut b = Budget::new(&mut work, floor + scratch - usize::from(case == 2));
            b.reserve_storage(floor).unwrap();
            let result = ServiceRequest::new(c.policy(), payload, &mut b);
            assert_eq!(b.storage(), floor);
            match case {
                0 => {
                    let (owner, charge) = result.unwrap();
                    assert_eq!(charge.additional_storage(), owner.retained_storage());
                    assert_eq!(b.work(), total);
                    assert_eq!(b.peak_storage(), floor + scratch);
                }
                1 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
                _ => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            }
        }
        for payload in responses(&c, &v) {
            let floor = c.policy().retained_storage() + payload.input_storage();
            let mut work = Work::new(PW - usize::from(case == 1));
            let mut b = Budget::new(&mut work, floor + PS - usize::from(case == 2));
            b.reserve_storage(floor).unwrap();
            let result = ServiceResponse::new(
                CompilerExecutionServiceRequestIdentityV2([0x91; 32]),
                c.policy(),
                payload,
                &mut b,
            );
            assert_eq!(b.storage(), floor);
            match case {
                0 => {
                    let (owner, charge) = result.unwrap();
                    assert_eq!(charge.additional_storage(), owner.retained_storage());
                    assert_eq!(b.work(), PW);
                    assert_eq!(b.peak_storage(), floor + PS);
                }
                1 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
                _ => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            }
        }
    }
}

#[test]
fn packet_decode_is_not_policy_pinned_authentication() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, LIMIT);
    let c = native_carriage(&mut budget);
    let other = SigningKey::from_bytes(&[0x53; 32]);
    let mut bytes = receipt_fixture::receipt_wire(2);
    bytes[272..304].copy_from_slice(&other.verifying_key().to_bytes());
    receipt_fixture::sign(&mut bytes, 2, &other);
    budget.reserve_storage(bytes.len()).unwrap();
    let (r, _) = retain(Receipt::decode(&bytes, &mut budget), &mut budget).unwrap();
    let (u, charge) = Publication::new([0x81; 32], [0x82; 32], r, &mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    // Framing and self-signature alone do not bind the claimed policy's key.
    let mut wire = packet_wires()
        .into_iter()
        .find(|(r, b)| !r && b[10] == 4)
        .unwrap()
        .1;
    wire[REQUEST_PREFIX + Q..REQUEST_PREFIX + Q + U].copy_from_slice(u.canonical_bytes());
    reseal(&mut wire, false);
    admitted(&wire, false).unwrap();
    let floor =
        c.policy().retained_storage() + c.request().retained_storage() + u.retained_storage();
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        ServiceRequest::new(
            c.policy(),
            QPayload::Publish {
                request: c.request(),
                publication: &u
            },
            &mut budget
        ),
        Err(Error::Attestation)
    ));
}

#[test]
fn nested_subject_storage_ceiling_still_applies_and_errors_are_fixed_size() {
    let bytes = packet_wires()
        .into_iter()
        .find(|(r, b)| !r && b[10] == 3)
        .unwrap()
        .1;
    let limit = fe2o3_artifact_transaction::MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4;
    assert!(matches!(
        measure(&bytes, false, WORK, limit + 1, bytes.len()).0,
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(size_of::<Error>() <= 128);
    assert!(Error::CurrentRecord.to_string().len() < 80);
}

#[test]
fn v1_control_wire_and_hash_preimages_are_unchanged() {
    let p = crate::CompilerExecutionIssuerPolicyV1::decode(&policy_wire(1)).unwrap();
    for (packet, kind, request_id, response_id) in [
        (
            LegacyRequest::inspect(&p),
            1u16,
            "536963bbe1001020c1f7d6a2db9820e414991d07f4af932f2fa8fbf95b359c30",
            "60f7f6cc2fda66f93f240689503121d14f5b477bc7af9128bdbada0186eacdad",
        ),
        (
            LegacyRequest::cancel(&p),
            5,
            "ffd4a0cefed89b85b5284f3cfc6e7a830ab0ff784477cb714d31fe6104f1cb36",
            "8580b5e1eda20c786b184198e5e0165c406f156046cc240f640eec0c24324186",
        ),
    ] {
        let mut expected = vec![0; 128];
        header(&mut expected, b"F2O3CSQ1", 1);
        expected[10..12].copy_from_slice(&kind.to_le_bytes());
        expected[24..56].copy_from_slice(p.identity().as_bytes());
        seal(&mut expected, "COMPILER-EXECUTION-SERVICE-REQUEST", 1);
        assert_eq!(packet.canonical_bytes(), expected);
        assert_eq!(fixture::hex(packet.identity().as_bytes()), request_id);
        assert_eq!(LegacyRequest::decode(&expected).unwrap(), packet);
        assert!(matches!(admitted(&expected, false), Err(Error::Header)));
        let response = LegacyResponse::ready(packet.identity(), &p, 1, [0; 32]).unwrap();
        let mut expected = vec![0; 160];
        header(&mut expected, b"F2O3CSP1", 1);
        expected[10..12].copy_from_slice(&1u16.to_le_bytes());
        expected[24..56].copy_from_slice(packet.identity().as_bytes());
        expected[56..88].copy_from_slice(p.identity().as_bytes());
        expected[88..96].copy_from_slice(&1u64.to_le_bytes());
        seal(&mut expected, "COMPILER-EXECUTION-SERVICE-RESPONSE", 1);
        assert_eq!(response.canonical_bytes(), expected);
        assert_eq!(fixture::hex(response.identity().as_bytes()), response_id);
        assert_eq!(LegacyResponse::decode(&expected).unwrap(), response);
        assert!(matches!(admitted(&expected, true), Err(Error::Header)));
    }
}

#[test]
fn fresh_native_families_have_distinct_identity_domains() {
    for (response, bytes) in packet_wires() {
        let domain = if response {
            RESPONSE_DOMAIN
        } else {
            REQUEST_DOMAIN
        };
        let digest = derive_identity(domain, &bytes[..bytes.len() - 32]);
        assert_eq!(digest, bytes[bytes.len() - 32..]);
        let legacy = if response {
            b"FE2O3/COMPILER-EXECUTION-SERVICE-RESPONSE/V1\0".as_slice()
        } else {
            b"FE2O3/COMPILER-EXECUTION-SERVICE-REQUEST/V1\0".as_slice()
        };
        assert_ne!(digest, derive_identity(legacy, &bytes[..bytes.len() - 32]));
        assert_ne!(digest, <[u8; 32]>::from(Sha256::digest(&bytes)));
    }
}
