use super::*;

const PUBLICATION_DOMAIN: &str = "COMPILER-EXECUTION-RECEIPT-PUBLICATION";
const ACK_DOMAIN: &str = "COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK";
const CARRIAGE_DOMAIN: &str = "COMPILER-EXECUTION-RECEIPT-CARRIAGE";

#[test]
fn conditional_carriage_v3_rejects_outer_and_nested_old_families_both_directions() {
    for old in [1, 2] {
        for (kind, bytes) in [
            publication_wire(old).to_vec(),
            ack_wire(old).to_vec(),
            carriage_wire(old).to_vec(),
        ]
        .iter()
        .enumerate()
        {
            assert!(decode(kind, bytes).is_err());
        }
        let mut publication = publication_wire(3);
        publication[152..552].copy_from_slice(&receipt_wire(old));
        seal(&mut publication, PUBLICATION_DOMAIN, 3);
        assert!(matches!(
            decode(0, &publication),
            Err(Error::Attestation(_))
        ));
        for axis in 0..4 {
            let mut bytes = carriage_wire(3);
            replace_child(&mut bytes, axis, old);
            seal(&mut bytes, CARRIAGE_DOMAIN, 3);
            assert!(carriage(&bytes).is_err());
            let mut bytes = carriage_wire(old);
            replace_child(&mut bytes, axis, 3);
            seal(&mut bytes, CARRIAGE_DOMAIN, old);
            reject_old(old, 2, &bytes);
        }
        let mut bytes = publication_wire(old);
        bytes[152..552].copy_from_slice(&receipt_wire(3));
        seal(&mut bytes, PUBLICATION_DOMAIN, old);
        reject_old(old, 0, &bytes);
        for (kind, bytes) in [
            publication_wire(3).to_vec(),
            ack_wire(3).to_vec(),
            carriage_wire(3).to_vec(),
        ]
        .iter()
        .enumerate()
        {
            reject_old(old, kind, bytes);
        }
    }
}
fn replace_child(bytes: &mut [u8], axis: usize, version: u16) {
    match axis {
        0 => bytes[24..240].copy_from_slice(&policy_wire(version)),
        1 => bytes[240..1186].copy_from_slice(&request_wire(version)),
        2 => bytes[1186..1770].copy_from_slice(&publication_wire(version)),
        _ => bytes[1770..2058].copy_from_slice(&ack_wire(version)),
    }
}
fn reject_old(version: u16, kind: usize, bytes: &[u8]) {
    if version == 1 {
        assert!(match kind {
            0 => CompilerExecutionReceiptPublicationV1::decode(bytes).is_err(),
            1 => CompilerExecutionReceiptPublicationAckV1::decode(bytes).is_err(),
            _ => CompilerExecutionReceiptCarriageV1::decode(bytes).is_err(),
        });
    } else {
        run(bytes.len(), |b| {
            assert!(match kind {
                0 => CompilerExecutionReceiptPublicationV2::decode(bytes, b).is_err(),
                1 => CompilerExecutionReceiptPublicationAckV2::decode(bytes, b).is_err(),
                _ => CompilerExecutionReceiptCarriageV2::decode(bytes, b).is_err(),
            });
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn conditional_carriage_v3_every_byte_lengths_and_identity_domains_reject() {
    for (kind, original, domain) in [
        (0, publication_wire(3).to_vec(), PUBLICATION_DOMAIN),
        (1, ack_wire(3).to_vec(), ACK_DOMAIN),
        (2, carriage_wire(3).to_vec(), CARRIAGE_DOMAIN),
    ] {
        for offset in 0..original.len() {
            let mut bytes = original.clone();
            bytes[offset] ^= 0x80;
            assert!(decode(kind, &bytes).is_err(), "frame {kind} byte {offset}");
        }
        for length in [0, original.len() - 1, original.len() + 1] {
            assert!(decode(kind, &vec![0; length]).is_err());
        }
        for old in [1, 2] {
            let mut bytes = original.clone();
            seal(&mut bytes, domain, old);
            assert!(matches!(
                decode(kind, &bytes),
                Err(Error::Framing(Framing::IdentityMismatch(_)))
            ));
        }
    }
}

#[test]
fn conditional_carriage_v3_publication_nested_failure_order_is_preserved() {
    let mut bytes = publication_wire(3);
    bytes[56..120].fill(0);
    bytes[152 + 304] ^= 1;
    bytes[552] ^= 1;
    assert!(matches!(
        decode(0, &bytes),
        Err(Error::Attestation(Attestation::Framing(
            AttestationFraming::SignatureRejected
        )))
    ));
    bytes[152..552].copy_from_slice(&receipt_wire(3));
    assert!(matches!(
        decode(0, &bytes),
        Err(Error::Framing(Framing::ZeroValue("issuer journal")))
    ));
    bytes[56..88].copy_from_slice(&JOURNAL);
    assert!(matches!(
        decode(0, &bytes),
        Err(Error::Framing(Framing::ZeroValue("compiler occurrence")))
    ));
    bytes[88..120].copy_from_slice(&OCCURRENCE);
    bytes[24] ^= 1;
    bytes[120] ^= 1;
    assert!(matches!(
        decode(0, &bytes),
        Err(Error::Framing(Framing::PolicyMismatch))
    ));
    bytes[24] ^= 1;
    assert!(matches!(
        decode(0, &bytes),
        Err(Error::Framing(Framing::ReceiptMismatch))
    ));
    bytes[120] ^= 1;
    assert!(matches!(
        decode(0, &bytes),
        Err(Error::Framing(Framing::IdentityMismatch(
            "receipt publication"
        )))
    ));
}

#[test]
fn conditional_carriage_v3_all_ack_joins_and_external_worker_identity() {
    for offset in [24, 56, 88, 120, 152, 216, 224, 184] {
        let mut bytes = carriage_wire(3);
        bytes[1770 + offset] ^= 2;
        seal(&mut bytes[1770..2058], ACK_DOMAIN, 3);
        seal(&mut bytes, CARRIAGE_DOMAIN, 3);
        let result = carriage(&bytes);
        if offset == 184 {
            // Worker identity is a claim, not authenticated by the internal joins.
            let owner = result.unwrap();
            run(owner.retained_storage(), |b| {
                assert!(matches!(
                    owner
                        .acknowledgment()
                        .matches_worker_ledger_record(WORKER, b),
                    Err(Error::Framing(Framing::WorkerLedgerMismatch))
                ));
                Ok(())
            })
            .unwrap();
            assert!(owner.requires_protected_policy_verification());
        } else {
            let error = result.unwrap_err();
            assert!(match offset {
                24 => matches!(error, Error::Framing(Framing::PolicyMismatch)),
                56 => matches!(error, Error::Framing(Framing::IssuerJournalMismatch)),
                88 => matches!(error, Error::Framing(Framing::OccurrenceMismatch)),
                120 => matches!(error, Error::Framing(Framing::ReceiptMismatch)),
                152 => matches!(error, Error::Framing(Framing::PublicationMismatch)),
                216 => matches!(error, Error::Framing(Framing::SequenceMismatch)),
                _ => matches!(error, Error::Framing(Framing::RollbackAnchorMismatch)),
            });
        }
    }
}

#[test]
fn conditional_carriage_v3_all_children_precede_receipt_then_ack_then_footer() {
    let mut bytes = carriage_wire(3);
    bytes[24 + 24] ^= 1;
    seal(&mut bytes[24..240], "COMPILER-EXECUTION-ISSUER-POLICY", 3);
    bytes[1770 + 184..1770 + 216].fill(0);
    bytes[1770 + 152] ^= 1;
    seal(&mut bytes[1770..2058], ACK_DOMAIN, 3);
    assert!(matches!(
        carriage(&bytes),
        Err(Error::Framing(Framing::ZeroValue("Worker ledger record")))
    ));
    bytes[1770 + 184..1770 + 216].copy_from_slice(&WORKER);
    seal(&mut bytes[1770..2058], ACK_DOMAIN, 3);
    assert!(matches!(
        carriage(&bytes),
        Err(Error::Attestation(Attestation::Framing(
            AttestationFraming::PolicyMismatch
        )))
    ));
    bytes[24..240].copy_from_slice(&policy_wire(3));
    assert!(matches!(
        carriage(&bytes),
        Err(Error::Framing(Framing::PublicationMismatch))
    ));
    bytes[1770..2058].copy_from_slice(&ack_wire(3));
    bytes[2058] ^= 1;
    assert!(matches!(
        carriage(&bytes),
        Err(Error::Framing(Framing::IdentityMismatch(
            "compiler receipt carriage"
        )))
    ));
}

#[test]
fn conditional_carriage_v3_coherent_request_and_publication_changes_cannot_reuse_ack() {
    for subject_change in [false, true] {
        let mut bytes = carriage_wire(3);
        if subject_change {
            bytes[240 + 224 + 378] ^= 1;
            seal(
                &mut bytes[240 + 224..240 + 914],
                "INERT-COMPILER-EXECUTION-SUBJECT",
                3,
            );
            let identity: [u8; 32] = bytes[240 + 882..240 + 914].try_into().unwrap();
            bytes[240 + 80..240 + 112].copy_from_slice(&identity);
        } else {
            bytes[240 + 24 + 96] ^= 1;
        }
        seal(&mut bytes[264..464], "COMPILER-EXECUTION-CHALLENGE", 3);
        seal(&mut bytes[240..1186], "COMPILER-EXECUTION-REQUEST", 3);
        seal(&mut bytes, CARRIAGE_DOMAIN, 3);
        let error = carriage(&bytes).unwrap_err();
        assert!(if subject_change {
            matches!(
                error,
                Error::Attestation(Attestation::Framing(AttestationFraming::SubjectMismatch))
            )
        } else {
            matches!(
                error,
                Error::Attestation(Attestation::Framing(AttestationFraming::ChallengeMismatch))
            )
        });
    }
    for offset in [56, 88] {
        let mut bytes = carriage_wire(3);
        bytes[1186 + offset] ^= 1;
        seal(&mut bytes[1186..1770], PUBLICATION_DOMAIN, 3);
        seal(&mut bytes, CARRIAGE_DOMAIN, 3);
        let error = carriage(&bytes).unwrap_err();
        assert!(if offset == 56 {
            matches!(error, Error::Framing(Framing::IssuerJournalMismatch))
        } else {
            matches!(error, Error::Framing(Framing::OccurrenceMismatch))
        });
    }
}

#[test]
fn conditional_carriage_v3_second_position_uses_prior_for_signature_and_next_for_ack() {
    use ed25519_dalek::SigningKey;
    let p = policy();
    let prior = receipt().next_rollback_anchor();
    let mut bytes = request_wire(3);
    bytes[120..152].fill(0x72);
    bytes[152..160].copy_from_slice(&2u64.to_le_bytes());
    bytes[160..192].copy_from_slice(&prior);
    seal(&mut bytes[24..224], "COMPILER-EXECUTION-CHALLENGE", 3);
    seal(&mut bytes, "COMPILER-EXECUTION-REQUEST", 3);
    let q = run(bytes.len(), |b| Ok(Request::decode(&bytes, b)?))
        .unwrap()
        .0;
    let key = SigningKey::from_bytes(&[0x51; 32]);
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(p.retained_storage() + q.retained_storage() + size_of::<SigningKey>())
        .unwrap();
    let (r, s) = Receipt::issue(&p, &q, &key, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let next = r.next_rollback_anchor();
    assert_ne!(prior, next);
    let (u, s) = Publication::new(JOURNAL, OCCURRENCE, r, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (a, s) = Ack::new(&u, WORKER, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    assert_eq!(a.sequence(), 2);
    assert_eq!(a.current_rollback_anchor(), next);
    let (c, s) = Carriage::new(p, q, u, a, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    assert_eq!(c.request().challenge().prior_rollback_anchor(), prior);
    assert_eq!(carriage(c.canonical_bytes()).unwrap(), c);
}

#[test]
fn conditional_carriage_v3_coherent_alternate_policy_is_content_not_provisioning() {
    use ed25519_dalek::SigningKey;
    let accepted = policy().identity();
    let key = SigningKey::from_bytes(&[0x54; 32]);
    let mut pbytes = policy_wire(3);
    pbytes[24] ^= 1;
    pbytes[112..144].copy_from_slice(&key.verifying_key().to_bytes());
    seal(&mut pbytes, "COMPILER-EXECUTION-ISSUER-POLICY", 3);
    let mut qbytes = request_wire(3);
    qbytes[48..80].copy_from_slice(&pbytes[184..216]);
    seal(&mut qbytes[24..224], "COMPILER-EXECUTION-CHALLENGE", 3);
    seal(&mut qbytes, "COMPILER-EXECUTION-REQUEST", 3);
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(pbytes.len() + qbytes.len() + size_of::<SigningKey>())
        .unwrap();
    let (p, s) = Policy::decode(&pbytes, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (q, s) = Request::decode(&qbytes, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (r, s) = Receipt::issue(&p, &q, &key, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (u, s) = Publication::new(JOURNAL, OCCURRENCE, r, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (a, s) = Ack::new(&u, WORKER, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    let (c, s) = Carriage::new(p, q, u, a, &mut b).unwrap();
    b.reserve_storage(s.additional_storage()).unwrap();
    assert_ne!(c.policy().identity(), accepted);
    assert!(c.requires_protected_policy_verification());
    assert!(!c.grants_compiler_authority());
    assert!(!c.grants_load_authority());
    assert!(!c.grants_launch_authority());
    assert_eq!(carriage(c.canonical_bytes()).unwrap(), c);
    assert!(matches!(
        c.publication().matches_issued_record(
            accepted,
            JOURNAL,
            OCCURRENCE,
            c.publication().receipt_identity(),
            &mut b
        ),
        Err(Error::Framing(Framing::PolicyMismatch))
    ));
}
