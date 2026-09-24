//! Independent inert wire fixtures. These tests establish no durable/GPU authority.
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V2 as RW,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V2 as QW,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as PW,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_STORAGE_V2 as CCS,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_CONSTRUCT_WORK_V2 as CCW,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_STORAGE_V2 as CDS,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_DECODE_WORK_V2 as CDW,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_STORAGE_V2 as CS,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_WORK_V2 as CW,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_STORAGE_V2 as AS,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_WORK_V2 as AW,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_STORAGE_V2 as UDS,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_DECODE_WORK_V2 as UDW,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_STORAGE_V2 as US,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_WORK_V2 as UW,
    CompilerExecutionAttestationReceiptV1 as LegacyReceipt,
    CompilerExecutionAttestationReceiptV2 as Receipt,
    CompilerExecutionAttestationRequestV2 as Request, CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionReceiptCarriageV1 as LegacyCarriage,
    CompilerExecutionReceiptCarriageV2 as Carriage,
    CompilerExecutionReceiptPublicationAckV1 as LegacyAck,
    CompilerExecutionReceiptPublicationAckV2 as Ack,
    CompilerExecutionReceiptPublicationErrorV1 as Framing,
    CompilerExecutionReceiptPublicationErrorV2 as Error,
    CompilerExecutionReceiptPublicationV1 as LegacyPublication,
    CompilerExecutionReceiptPublicationV2 as Publication,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};
#[path = "support/native_attestation_fixture.rs"]
mod fixture;
#[path = "support/native_receipt_fixture.rs"]
mod receipt_fixture;
use fixture::*;
use receipt_fixture::*;
const LIMIT: usize = 1_000_000;
const JOURNAL: [u8; 32] = [0x81; 32];
const OCCURRENCE: [u8; 32] = [0x82; 32];
const WORKER: [u8; 32] = [0x83; 32];

fn publication_wire(version: u16) -> [u8; 584] {
    let receipt = receipt_wire(version);
    let mut bytes = [0; 584];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CES1"
        } else {
            b"F2O3CES2"
        },
        version,
    );
    bytes[24..56].copy_from_slice(&receipt[64..96]);
    bytes[56..88].copy_from_slice(&JOURNAL);
    bytes[88..120].copy_from_slice(&OCCURRENCE);
    bytes[120..152].copy_from_slice(&receipt[368..]);
    bytes[152..552].copy_from_slice(&receipt);
    seal(
        &mut bytes,
        "COMPILER-EXECUTION-RECEIPT-PUBLICATION",
        version,
    );
    bytes
}
fn ack_wire(version: u16) -> [u8; 288] {
    let publication = publication_wire(version);
    let receipt = receipt_wire(version);
    let mut bytes = [0; 288];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CEA1"
        } else {
            b"F2O3CEA2"
        },
        version,
    );
    bytes[24..152].copy_from_slice(&publication[24..152]);
    bytes[152..184].copy_from_slice(&publication[552..]);
    bytes[184..216].copy_from_slice(&WORKER);
    bytes[216..224].copy_from_slice(&receipt[200..208]);
    bytes[224..256].copy_from_slice(&receipt[240..272]);
    seal(
        &mut bytes,
        "COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK",
        version,
    );
    bytes
}
fn carriage_wire(version: u16) -> [u8; 2090] {
    let mut bytes = [0; 2090];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CRG1"
        } else {
            b"F2O3CRG2"
        },
        version,
    );
    bytes[24..240].copy_from_slice(&policy_wire(version));
    bytes[240..1186].copy_from_slice(&request_wire(version));
    bytes[1186..1770].copy_from_slice(&publication_wire(version));
    bytes[1770..2058].copy_from_slice(&ack_wire(version));
    seal(&mut bytes, "COMPILER-EXECUTION-RECEIPT-CARRIAGE", version);
    bytes
}
fn admitted<T>(bytes: &[u8], work: usize, decode: impl FnOnce(&[u8], &mut Budget<'_>) -> T) -> T {
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(bytes.len()).unwrap();
    let result = decode(bytes, &mut budget);
    assert_eq!(budget.storage(), bytes.len());
    result
}
fn publication(bytes: &[u8]) -> Publication {
    admitted(bytes, UDW, |b, m| Publication::decode(b, m).unwrap().0)
}
fn ack(bytes: &[u8]) -> Ack {
    admitted(bytes, AW, |b, m| Ack::decode(b, m).unwrap().0)
}
fn policy() -> Policy {
    admitted(&policy_wire(2), PW, |b, m| Policy::decode(b, m).unwrap().0)
}
fn request() -> Request {
    admitted(&request_wire(2), QW, |b, m| {
        Request::decode(b, m).unwrap().0
    })
}
fn receipt(bytes: &[u8]) -> Receipt {
    admitted(bytes, RW, |b, m| Receipt::decode(b, m).unwrap().0)
}
fn error(version: u16, kind: usize, bytes: &[u8]) -> String {
    if version == 1 {
        match kind {
            0 => format!("{:?}", LegacyPublication::decode(bytes).unwrap_err()),
            1 => format!("{:?}", LegacyAck::decode(bytes).unwrap_err()),
            _ => format!("{:?}", LegacyCarriage::decode(bytes).unwrap_err()),
        }
    } else {
        let e = admitted(bytes, CDW, |b, m| match kind {
            0 => Publication::decode(b, m).unwrap_err(),
            1 => Ack::decode(b, m).unwrap_err(),
            _ => Carriage::decode(b, m).unwrap_err(),
        });
        format!("{e:?}")
    }
}

#[test]
fn independent_v1_v2_publication_ack_and_carriage_goldens() {
    for (version, ids, hashes) in [
        (
            1,
            [
                "bee1e91e6705de5cd32568931623dfaa4cfba0ae0db62ed15132dfb024affa11",
                "2f3971559a29aea95e0c2f24156755c501a1d667e485931466b8c5409d0f7898",
                "8d55a642fed0c6265bf334b66db2ed7e58ccf863089ecf5e5651ac25b9c06926",
            ],
            [
                "968c153edc7bad0c763750f2626ce97bb5c8b18333a8a494ca2b341ad121674d",
                "6e15cf97054db0e91377bfcdf1545ffbb6add0e965913544f1d5e3e7910c57d0",
                "a882e55533a334f0fc33b6198b5fd925425910aaa1e77cd855d5feb2bc186a89",
            ],
        ),
        (
            2,
            [
                "09de8b9a8f17ba006c3a52b90c88e28381096d84e9f710d46d430d970d3f7f04",
                "0ebbf44beead3cefca6bafded4cd51967122734a08ffab2d9bc16d5391e5310f",
                "343f88200b8c6d422ed1e7507c496ebd5bd33021d5faaea3332dae8caaddb6de",
            ],
            [
                "2fd707f5e1643f48bbcdf160cf78fbfccc9f883e74bbfd44696c1c5a03a2f22f",
                "4e1912bf381c55685e370dacfa4fad64922fb1a8c7d98faef91922da00799fd2",
                "cbcc5cec733fc8f92da9bf85a744f36b92058815a3534fdae8218a3ca0baf3a3",
            ],
        ),
    ] {
        let u = publication_wire(version);
        let a = ack_wire(version);
        let c = carriage_wire(version);
        for ((bytes, id), hash) in [u.as_slice(), a.as_slice(), c.as_slice()]
            .into_iter()
            .zip(ids)
            .zip(hashes)
        {
            assert_eq!(hex(&bytes[bytes.len() - 32..]), id);
            assert_eq!(hex(&Sha256::digest(bytes)), hash);
        }
        if version == 1 {
            let p = LegacyPublication::new(
                JOURNAL,
                OCCURRENCE,
                LegacyReceipt::decode(&receipt_wire(1)).unwrap(),
            )
            .unwrap();
            assert_eq!(p.canonical_bytes(), &u);
            assert_eq!(LegacyPublication::decode(&u).unwrap(), p);
            assert_eq!(LegacyAck::new(&p, WORKER).unwrap().canonical_bytes(), &a);
            assert_eq!(LegacyAck::decode(&a).unwrap().canonical_bytes(), &a);
            assert_eq!(LegacyCarriage::decode(&c).unwrap().canonical_bytes(), &c);
        } else {
            assert_eq!(publication(&u).canonical_bytes(), &u);
            assert_eq!(ack(&a).canonical_bytes(), &a);
            admitted(&c, CDW, |b, m| {
                assert_eq!(Carriage::decode(b, m).unwrap().0.canonical_bytes(), &c)
            });
        }
    }
}

#[test]
fn consuming_pipeline_transfers_only_growth_and_remains_inert() {
    let p = policy();
    let q = request();
    let r = receipt(&receipt_wire(2));
    let inherited = p.retained_storage() + q.retained_storage() + r.retained_storage();
    let mut work = Work::new(UW + AW + CCW + CW);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(inherited).unwrap();
    let (u, s) = Publication::new(JOURNAL, OCCURRENCE, r, &mut budget).unwrap();
    assert_eq!(
        inherited + s.additional_storage(),
        p.retained_storage() + q.retained_storage() + u.retained_storage()
    );
    budget.reserve_storage(s.additional_storage()).unwrap();
    assert!(!u.proves_durable_publication());
    assert!(!u.grants_compiler_authority());
    let (a, s) = Ack::new(&u, WORKER, &mut budget).unwrap();
    assert_eq!(s.additional_storage(), a.retained_storage());
    budget.reserve_storage(s.additional_storage()).unwrap();
    assert!(!a.proves_durable_publication());
    assert!(!a.grants_compiler_authority());
    let floor = budget.storage();
    let (c, s) = Carriage::new(p, q, u, a, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(floor + s.additional_storage(), c.retained_storage());
    budget.reserve_storage(s.additional_storage()).unwrap();
    assert_eq!(c.canonical_bytes(), &carriage_wire(2));
    assert!(
        c.identity()
            .matches_canonical_bytes(c.canonical_bytes(), &mut budget)
            .unwrap()
    );
    assert!(c.requires_protected_policy_verification());
    assert!(!c.grants_compiler_authority());
    assert!(!c.grants_load_authority());
    assert!(!c.grants_launch_authority());
    assert_eq!(
        c.acknowledgment().current_rollback_anchor(),
        c.publication().receipt().next_rollback_anchor()
    );
    assert_eq!(c.request().challenge().prior_rollback_anchor(), [0; 32]);
    assert_eq!(c.policy().identity(), c.publication().policy_identity());
    assert_eq!(budget.work(), UW + AW + CCW + CW);
}

#[test]
fn all_byte_mutations_and_wrong_lengths_reject() {
    for version in [1, 2] {
        for (kind, original) in [
            publication_wire(version).to_vec(),
            ack_wire(version).to_vec(),
            carriage_wire(version).to_vec(),
        ]
        .into_iter()
        .enumerate()
        {
            for i in 0..original.len() {
                let mut bytes = original.clone();
                bytes[i] ^= 0x80;
                let _ = error(version, kind, &bytes);
            }
            for len in [0, original.len() - 1, original.len() + 1, 100_000] {
                let _ = error(version, kind, &vec![0; len]);
            }
        }
    }
}

#[test]
fn outer_and_resealed_nested_family_substitutions_reject() {
    for version in [1, 2] {
        for (kind, bytes) in [
            publication_wire(3 - version).to_vec(),
            ack_wire(3 - version).to_vec(),
            carriage_wire(3 - version).to_vec(),
        ]
        .into_iter()
        .enumerate()
        {
            assert!(error(version, kind, &bytes).contains("InvalidMagic"));
        }
        let mut u = publication_wire(version);
        u[152..552].copy_from_slice(&receipt_wire(3 - version));
        seal(&mut u, "COMPILER-EXECUTION-RECEIPT-PUBLICATION", version);
        assert!(error(version, 0, &u).contains("InvalidMagic(\"receipt\")"));
        for axis in 0..4 {
            let mut c = carriage_wire(version);
            match axis {
                0 => c[24..240].copy_from_slice(&policy_wire(3 - version)),
                1 => c[240..1186].copy_from_slice(&request_wire(3 - version)),
                2 => c[1186..1770].copy_from_slice(&publication_wire(3 - version)),
                _ => c[1770..2058].copy_from_slice(&ack_wire(3 - version)),
            }
            seal(&mut c, "COMPILER-EXECUTION-RECEIPT-CARRIAGE", version);
            assert!(error(version, 2, &c).contains("InvalidMagic"));
        }
    }
}

#[test]
fn publication_nested_signature_and_binding_errors_precede_footer() {
    for version in [1, 2] {
        let mut u = publication_wire(version);
        u[56..120].fill(0);
        u[152 + 304] ^= 1;
        u[552] ^= 1;
        assert!(error(version, 0, &u).contains("SignatureRejected"));
        u[152..552].copy_from_slice(&receipt_wire(version));
        assert!(error(version, 0, &u).contains("ZeroValue(\"issuer journal\")"));
        u[56..88].copy_from_slice(&JOURNAL);
        assert!(error(version, 0, &u).contains("ZeroValue(\"compiler occurrence\")"));
        u[88..120].copy_from_slice(&OCCURRENCE);
        u[24] ^= 1;
        u[120] ^= 1;
        assert!(error(version, 0, &u).contains("PolicyMismatch"));
        u[24] ^= 1;
        assert!(error(version, 0, &u).contains("ReceiptMismatch"));
        u[120] ^= 1;
        assert!(error(version, 0, &u).contains("IdentityMismatch(\"receipt publication\")"));
    }
}

#[test]
fn ack_constructor_decode_asymmetry_is_frozen() {
    for version in [1, 2] {
        let mut bytes = receipt_wire(version);
        bytes[64..96].fill(0);
        rebuild(&mut bytes, version);
        if version == 1 {
            let u =
                LegacyPublication::new(JOURNAL, OCCURRENCE, LegacyReceipt::decode(&bytes).unwrap())
                    .unwrap();
            let a = LegacyAck::new(&u, WORKER).unwrap();
            assert_eq!(
                error(1, 1, a.canonical_bytes()),
                "ZeroValue(\"issuer policy\")"
            );
        } else {
            let r = receipt(&bytes);
            let mut work = Work::new(UW + AW);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(r.retained_storage()).unwrap();
            let (u, s) = Publication::new(JOURNAL, OCCURRENCE, r, &mut budget).unwrap();
            budget.reserve_storage(s.additional_storage()).unwrap();
            let (a, _) = Ack::new(&u, WORKER, &mut budget).unwrap();
            assert!(error(2, 1, a.canonical_bytes()).contains("ZeroValue(\"issuer policy\")"));
        }
    }
}

#[test]
fn carriage_decodes_all_children_before_context_then_checks_receipt_before_ack() {
    for version in [1, 2] {
        let mut c = carriage_wire(version);
        c[24 + 24] ^= 1;
        seal(&mut c[24..240], "COMPILER-EXECUTION-ISSUER-POLICY", version);
        c[1770 + 184..1770 + 216].fill(0);
        c[1770 + 152] ^= 1;
        seal(
            &mut c[1770..2058],
            "COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK",
            version,
        );
        assert!(error(version, 2, &c).contains("ZeroValue(\"Worker ledger record\")"));
        c[1770 + 184..1770 + 216].copy_from_slice(&WORKER);
        seal(
            &mut c[1770..2058],
            "COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK",
            version,
        );
        let e = error(version, 2, &c);
        assert!(e.starts_with("Attestation("));
        assert!(e.contains("PolicyMismatch"));
        c[24..240].copy_from_slice(&policy_wire(version));
        assert!(error(version, 2, &c).contains("PublicationMismatch"));
        c[1770..2058].copy_from_slice(&ack_wire(version));
        c[2058] ^= 1;
        assert!(error(version, 2, &c).contains("IdentityMismatch(\"compiler receipt carriage\")"));
    }
}

#[test]
fn ack_matching_checks_every_publication_axis_but_worker_is_independent() {
    let u = publication(&publication_wire(2));
    for (offset, expected) in [
        (24, "PolicyMismatch"),
        (56, "IssuerJournalMismatch"),
        (88, "OccurrenceMismatch"),
        (120, "ReceiptMismatch"),
        (152, "PublicationMismatch"),
        (216, "SequenceMismatch"),
        (224, "RollbackAnchorMismatch"),
        (184, ""),
    ] {
        let mut bytes = ack_wire(2);
        bytes[offset] ^= 2;
        seal(&mut bytes, "COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK", 2);
        let a = ack(&bytes);
        let mut work = Work::new(2 * AW);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget
            .reserve_storage(u.retained_storage() + a.retained_storage())
            .unwrap();
        let result = a.matches_publication(&u, &mut budget);
        if expected.is_empty() {
            result.unwrap();
            assert!(matches!(
                a.matches_worker_ledger_record(WORKER, &mut budget),
                Err(Error::Framing(Framing::WorkerLedgerMismatch))
            ));
        } else {
            assert!(format!("{:?}", result.unwrap_err()).contains(expected));
        }
    }
}

#[test]
fn exact_and_one_short_decode_limits_restore_the_full_input_floor() {
    for (kind, bytes, total, scratch, last) in [
        (0, publication_wire(2).to_vec(), UDW, UDS, RW),
        (1, ack_wire(2).to_vec(), AW, AS, AW),
        (2, carriage_wire(2).to_vec(), CDW, CDS, AW),
    ] {
        for case in 0..4 {
            let floor = bytes.len();
            let mut work = Work::new(total - usize::from(case == 1));
            let mut budget = Budget::new(&mut work, floor + scratch - usize::from(case == 2));
            let entered = floor - usize::from(case == 3);
            budget.reserve_storage(entered).unwrap();
            let result = match kind {
                0 => Publication::decode(&bytes, &mut budget)
                    .map(|(v, s)| (v.retained_storage(), s.additional_storage())),
                1 => Ack::decode(&bytes, &mut budget)
                    .map(|(v, s)| (v.retained_storage(), s.additional_storage())),
                _ => Carriage::decode(&bytes, &mut budget)
                    .map(|(v, s)| (v.retained_storage(), s.additional_storage())),
            };
            assert_eq!(budget.storage(), entered);
            match case {
                0 => {
                    let (retained, additional) = result.unwrap();
                    assert_eq!(retained, additional);
                    assert_eq!(budget.work(), total);
                    assert_eq!(budget.peak_storage(), floor + scratch);
                }
                1 => {
                    assert!(format!("{:?}", result.unwrap_err()).contains("Work("));
                    assert_eq!(budget.work(), total - last + 8);
                }
                2 => {
                    assert!(format!("{:?}", result.unwrap_err()).contains("Storage("));
                    assert_eq!(budget.failed_storage(), Some(floor + scratch));
                }
                _ => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert_eq!(budget.work(), 8);
                }
            }
        }
    }
}

#[test]
fn exact_and_one_short_consuming_constructor_limits_preserve_reservations() {
    for kind in 0..3 {
        for case in 0..4 {
            let p = policy();
            let q = request();
            let u = publication(&publication_wire(2));
            let a = ack(&ack_wire(2));
            let r = receipt(&receipt_wire(2));
            let (floor, total, scratch, last) = match kind {
                0 => (r.retained_storage(), UW, US, UW),
                1 => (u.retained_storage(), AW, AS, AW),
                _ => (
                    p.retained_storage()
                        + q.retained_storage()
                        + u.retained_storage()
                        + a.retained_storage(),
                    CCW,
                    CCS,
                    AW,
                ),
            };
            let mut work = Work::new(total - usize::from(case == 1));
            let mut budget = Budget::new(&mut work, floor + scratch - usize::from(case == 2));
            let entered = floor - usize::from(case == 3);
            budget.reserve_storage(entered).unwrap();
            let result = match kind {
                0 => Publication::new(JOURNAL, OCCURRENCE, r, &mut budget)
                    .map(|(v, s)| (v.retained_storage(), s.additional_storage())),
                1 => Ack::new(&u, WORKER, &mut budget)
                    .map(|(v, s)| (v.retained_storage(), s.additional_storage())),
                _ => Carriage::new(p, q, u, a, &mut budget)
                    .map(|(v, s)| (v.retained_storage(), s.additional_storage())),
            };
            assert_eq!(budget.storage(), entered);
            match case {
                0 => {
                    let (retained, additional) = result.unwrap();
                    assert_eq!(retained, additional + if kind == 1 { 0 } else { floor });
                    assert_eq!(budget.work(), total);
                    assert_eq!(budget.peak_storage(), floor + scratch);
                }
                1 => {
                    assert!(format!("{:?}", result.unwrap_err()).contains("Work("));
                    assert_eq!(budget.work(), total - last + 8);
                }
                2 => {
                    assert!(format!("{:?}", result.unwrap_err()).contains("Storage("));
                    assert_eq!(budget.failed_storage(), Some(floor + scratch));
                }
                _ => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert_eq!(budget.work(), 8);
                }
            }
        }
    }
}

#[test]
fn identities_and_relationship_checks_use_explicit_metering() {
    let u = publication(&publication_wire(2));
    let a = ack(&ack_wire(2));
    let c = admitted(&carriage_wire(2), CDW, |b, m| {
        Carriage::decode(b, m).unwrap().0
    });
    let floor = c.retained_storage() + u.retained_storage() + a.retained_storage() + 37;
    let mut work = Work::new(2 * UW + 3 * AW + CW);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(budget.reserve_storage(LIMIT + 1).is_err());
    budget.reserve_storage(floor).unwrap();
    assert!(
        u.identity()
            .matches_canonical_bytes(u.canonical_bytes(), &mut budget)
            .unwrap()
    );
    assert!(
        a.identity()
            .matches_canonical_bytes(a.canonical_bytes(), &mut budget)
            .unwrap()
    );
    assert!(
        c.identity()
            .matches_canonical_bytes(c.canonical_bytes(), &mut budget)
            .unwrap()
    );
    u.matches_issued_record(
        c.policy().identity(),
        JOURNAL,
        OCCURRENCE,
        u.receipt().identity(),
        &mut budget,
    )
    .unwrap();
    a.matches_publication(&u, &mut budget).unwrap();
    a.matches_worker_ledger_record(WORKER, &mut budget).unwrap();
    assert_eq!(budget.work(), 2 * UW + 3 * AW + CW);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_storage(), Some(LIMIT + 1));
    assert_eq!(budget.peak_storage(), floor + US.max(AS).max(CS));
    assert_eq!(a.policy_identity(), u.policy_identity());
    assert_eq!(a.issuer_journal_identity(), JOURNAL);
    assert_eq!(a.compiler_occurrence_identity(), OCCURRENCE);
    assert_eq!(a.receipt_identity(), u.receipt_identity());
    assert_eq!(a.publication_identity(), u.identity());
    assert_eq!(a.worker_ledger_record_identity(), WORKER);
    assert_eq!(a.sequence(), 1);
}

#[test]
fn identity_and_match_operations_reject_one_short_work_storage_and_input_floors() {
    let u = publication(&publication_wire(2));
    let a = ack(&ack_wire(2));
    let c = admitted(&carriage_wire(2), CDW, |b, m| {
        Carriage::decode(b, m).unwrap().0
    });
    for (kind, floor, total, scratch) in [
        (0, 584, UW, US),
        (1, 288, AW, AS),
        (2, 2090, CW, CS),
        (3, u.retained_storage(), UW, US),
        (4, a.retained_storage() + u.retained_storage(), AW, AS),
        (5, a.retained_storage(), AW, AS),
    ] {
        for case in 0..4 {
            let mut work = Work::new(total - usize::from(case == 1));
            let mut budget = Budget::new(&mut work, floor + scratch - usize::from(case == 2));
            let entered = floor - usize::from(case == 3);
            budget.reserve_storage(entered).unwrap();
            let result = match kind {
                0 => u
                    .identity()
                    .matches_canonical_bytes(&publication_wire(2), &mut budget),
                1 => a
                    .identity()
                    .matches_canonical_bytes(&ack_wire(2), &mut budget),
                2 => c
                    .identity()
                    .matches_canonical_bytes(&carriage_wire(2), &mut budget),
                3 => u
                    .matches_issued_record(
                        u.policy_identity(),
                        JOURNAL,
                        OCCURRENCE,
                        u.receipt_identity(),
                        &mut budget,
                    )
                    .map(|()| true),
                4 => a.matches_publication(&u, &mut budget).map(|()| true),
                _ => a
                    .matches_worker_ledger_record(WORKER, &mut budget)
                    .map(|()| true),
            };
            assert_eq!(budget.storage(), entered);
            match case {
                0 => {
                    assert!(result.unwrap());
                    assert_eq!(budget.work(), total);
                    assert_eq!(budget.peak_storage(), floor + scratch);
                }
                1 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    assert_eq!(budget.work(), 8);
                }
                2 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                    assert_eq!(budget.work(), total);
                    assert_eq!(budget.failed_storage(), Some(floor + scratch));
                }
                _ => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert_eq!(budget.work(), 8);
                }
            }
        }
    }
}

#[test]
fn carriage_preserves_nested_subject_ceiling_and_cumulative_refusal() {
    use fe2o3_artifact_transaction::{
        CompilerExecutionSubjectErrorV2 as SE, MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4,
    };
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V2 as Q_OUTER,
        CompilerExecutionAttestationErrorV2 as AE,
    };
    let bytes = carriage_wire(2);
    let mut work = Work::new(CDW);
    let mut budget = Budget::new(&mut work, MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4 + 1);
    budget.reserve_storage(bytes.len() + 37).unwrap();
    assert!(matches!(
        Carriage::decode(&bytes, &mut budget),
        Err(Error::Attestation(AE::Subject(SE::Resource(
            Resource::Accounting
        ))))
    ));
    assert_eq!(budget.storage(), bytes.len() + 37);
    assert_eq!(budget.work(), CW + PW + Q_OUTER + 8);

    let mut work = Work::new(CDW + CW - 1);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(budget.reserve_storage(LIMIT + 1).is_err());
    budget.reserve_storage(bytes.len() + 37).unwrap();
    let (c, s) = Carriage::decode(&bytes, &mut budget).unwrap();
    budget.reserve_storage(s.additional_storage()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        c.identity().matches_canonical_bytes(&bytes, &mut budget),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.work(), CDW + 8);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_storage(), Some(LIMIT + 1));
}

#[test]
fn second_position_carriage_uses_prior_for_verification_and_next_for_ack() {
    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_ATTESTATION_RECEIPT_ISSUE_WORK_V2 as IW;
    let p = policy();
    let prior = receipt(&receipt_wire(2)).next_rollback_anchor();
    let mut bytes = request_wire(2);
    bytes[24 + 96..24 + 128].fill(0x72);
    bytes[24 + 128..24 + 136].copy_from_slice(&2u64.to_le_bytes());
    bytes[24 + 136..24 + 168].copy_from_slice(&prior);
    seal(&mut bytes[24..224], "COMPILER-EXECUTION-CHALLENGE", 2);
    seal(&mut bytes, "COMPILER-EXECUTION-REQUEST", 2);
    let q = admitted(&bytes, QW, |b, m| Request::decode(b, m).unwrap().0);
    let key = SigningKey::from_bytes(&[0x51; 32]);
    let mut work = Work::new(IW + UW + AW + CCW);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(
            p.retained_storage() + q.retained_storage() + std::mem::size_of::<SigningKey>(),
        )
        .unwrap();
    let (r, s) = Receipt::issue(&p, &q, &key, &mut budget).unwrap();
    budget.reserve_storage(s.additional_storage()).unwrap();
    let next = r.next_rollback_anchor();
    assert_ne!(prior, next);
    let (u, s) = Publication::new(JOURNAL, OCCURRENCE, r, &mut budget).unwrap();
    budget.reserve_storage(s.additional_storage()).unwrap();
    let (a, s) = Ack::new(&u, WORKER, &mut budget).unwrap();
    budget.reserve_storage(s.additional_storage()).unwrap();
    assert_eq!(a.sequence(), 2);
    assert_eq!(a.current_rollback_anchor(), next);
    let (c, s) = Carriage::new(p, q, u, a, &mut budget).unwrap();
    budget.reserve_storage(s.additional_storage()).unwrap();
    assert_eq!(c.request().challenge().prior_rollback_anchor(), prior);
    admitted(c.canonical_bytes(), CDW, |b, m| {
        assert_eq!(Carriage::decode(b, m).unwrap().0, c)
    });
    assert_eq!(budget.work(), IW + UW + AW + CCW);
}

#[test]
fn resealed_ack_zero_fields_freeze_exact_first_error_for_both_families() {
    let axes = [
        (24, 32, "issuer policy"),
        (56, 32, "issuer journal"),
        (88, 32, "compiler occurrence"),
        (120, 32, "receipt"),
        (152, 32, "receipt publication"),
        (184, 32, "Worker ledger record"),
        (216, 8, "receipt sequence"),
        (224, 32, "current rollback anchor"),
    ];
    for version in [1, 2] {
        for i in 0..axes.len() {
            for paired in [false, true] {
                let mut bytes = ack_wire(version);
                let (offset, len, label) = axes[i];
                bytes[offset..offset + len].fill(0);
                if paired && i + 1 < axes.len() {
                    let (next, len, _) = axes[i + 1];
                    bytes[next..next + len].fill(0);
                }
                seal(
                    &mut bytes,
                    "COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK",
                    version,
                );
                let expected = format!("ZeroValue({label:?})");
                assert_eq!(
                    error(version, 1, &bytes),
                    if version == 1 {
                        expected
                    } else {
                        format!("Framing({expected})")
                    }
                );
            }
        }
    }
}

#[test]
fn genuine_alternate_requests_fail_the_complete_carriage_join() {
    for version in [1, 2] {
        for subject_change in [false, true] {
            let mut bytes = carriage_wire(version);
            if subject_change {
                bytes[240 + 224 + 378] ^= 1;
                seal(
                    &mut bytes[240 + 224..240 + 914],
                    "INERT-COMPILER-EXECUTION-SUBJECT",
                    version,
                );
                let identity: [u8; 32] = bytes[240 + 882..240 + 914].try_into().unwrap();
                bytes[240 + 80..240 + 112].copy_from_slice(&identity);
            } else {
                bytes[240 + 24 + 96] ^= 1;
            }
            seal(
                &mut bytes[240 + 24..240 + 224],
                "COMPILER-EXECUTION-CHALLENGE",
                version,
            );
            seal(&mut bytes[240..1186], "COMPILER-EXECUTION-REQUEST", version);
            seal(&mut bytes, "COMPILER-EXECUTION-RECEIPT-CARRIAGE", version);
            let expected = if subject_change {
                "SubjectMismatch"
            } else {
                "ChallengeMismatch"
            };
            assert_eq!(
                error(version, 2, &bytes),
                if version == 1 {
                    format!("Attestation({expected})")
                } else {
                    format!("Attestation(Framing({expected}))")
                }
            );
        }
    }
}

#[test]
fn public_matchers_return_admitted_negative_results() {
    let u = publication(&publication_wire(2));
    let a = ack(&ack_wire(2));
    let c = admitted(&carriage_wire(2), CDW, |b, m| {
        Carriage::decode(b, m).unwrap().0
    });
    for (kind, original, work_limit) in [
        (0, publication_wire(2).to_vec(), UW),
        (1, ack_wire(2).to_vec(), AW),
        (2, carriage_wire(2).to_vec(), CW),
    ] {
        for mode in 0..4 {
            let mut bytes = original.clone();
            let last = bytes.len() - 1;
            match mode {
                0 => bytes[24] ^= 1,
                1 => bytes[last] ^= 1,
                2 => {
                    bytes.pop();
                }
                _ => bytes.push(0),
            }
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(bytes.len()).unwrap();
            let matches = match kind {
                0 => u.identity().matches_canonical_bytes(&bytes, &mut budget),
                1 => a.identity().matches_canonical_bytes(&bytes, &mut budget),
                _ => c.identity().matches_canonical_bytes(&bytes, &mut budget),
            };
            assert!(!matches.unwrap());
            assert_eq!(budget.work(), work_limit);
            assert_eq!(budget.storage(), bytes.len());
        }
    }
    let mut pbytes = policy_wire(2);
    pbytes[24] ^= 1;
    seal(&mut pbytes, "COMPILER-EXECUTION-ISSUER-POLICY", 2);
    let other_policy = admitted(&pbytes, PW, |b, m| {
        Policy::decode(b, m).unwrap().0.identity()
    });
    let mut rbytes = receipt_wire(2);
    rbytes[24] ^= 1;
    rebuild(&mut rbytes, 2);
    let other_receipt = receipt(&rbytes).identity();
    let mut work = Work::new(4 * UW);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(u.retained_storage()).unwrap();
    for (axis, expected) in [
        "PolicyMismatch",
        "IssuerJournalMismatch",
        "OccurrenceMismatch",
        "ReceiptMismatch",
    ]
    .into_iter()
    .enumerate()
    {
        let error = u
            .matches_issued_record(
                if axis == 0 {
                    other_policy
                } else {
                    u.policy_identity()
                },
                if axis == 1 { [0x91; 32] } else { JOURNAL },
                if axis == 2 { [0x92; 32] } else { OCCURRENCE },
                if axis == 3 {
                    other_receipt
                } else {
                    u.receipt_identity()
                },
                &mut budget,
            )
            .unwrap_err();
        assert_eq!(format!("{error:?}"), format!("Framing({expected})"));
        assert_eq!(budget.work(), (axis + 1) * UW);
        assert_eq!(budget.storage(), u.retained_storage());
    }
}
