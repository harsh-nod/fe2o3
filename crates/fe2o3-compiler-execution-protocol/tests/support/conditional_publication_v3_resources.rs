use super::*;

fn resource(error: &Error) -> Option<&Resource> {
    match error {
        Error::Resource(e) | Error::Attestation(Attestation::Resource(e)) => Some(e),
        Error::Attestation(Attestation::Subject(
            fe2o3_artifact_transaction::CompilerExecutionSubjectErrorV3::Resource(e),
        )) => Some(e),
        _ => None,
    }
}

// Isolate fixed operation admission, not fresh ownership or a protected lifecycle.
fn boundaries<F, Op>(cost: usize, scratch: usize, prepare: F)
where
    F: Fn() -> (usize, Op),
    Op: FnOnce(&mut Budget<'_>) -> Result<(), Error>,
{
    for case in 0..5 {
        let (input, operation) = prepare();
        let inherited = if case == 4 { 37 } else { 0 };
        let floor = input + inherited - usize::from(case == 3);
        let maximum = input + inherited + scratch - usize::from(case == 2);
        let mut work = Work::new(9 + cost - usize::from(case == 1));
        let mut b = Budget::new(&mut work, maximum);
        b.charge_work(9).unwrap();
        b.reserve_storage(floor).unwrap();
        if case == 4 {
            assert!(b.reserve_storage(usize::MAX).is_err());
            assert!(b.charge_work(usize::MAX).is_err());
        }
        let denials = (b.failed_work(), b.failed_storage());
        let ledger = b.work_ledger_identity_v1();
        let address = &b as *const _;
        let result = operation(&mut b);
        assert_eq!(&b as *const _, address);
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(b.storage(), floor);
        match case {
            0 | 4 => {
                result.unwrap();
                assert_eq!(b.work(), 9 + cost);
                assert_eq!(b.peak_storage(), floor + scratch);
                assert_eq!((b.failed_work(), b.failed_storage()), denials);
            }
            1 => {
                assert!(matches!(
                    resource(&result.unwrap_err()),
                    Some(Resource::Work(_))
                ));
                assert_eq!(b.failed_work(), Some(9 + cost));
            }
            2 => {
                assert!(matches!(
                    resource(&result.unwrap_err()),
                    Some(Resource::Storage(_))
                ));
                assert_eq!(b.failed_storage(), Some(maximum + 1));
            }
            _ => {
                assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                assert_eq!(b.work(), 17);
            }
        }
    }
}

#[test]
fn conditional_carriage_v3_decoders_exact_short_and_prior_denials() {
    boundaries(UDW, UDS, || {
        (584, |b: &mut Budget<'_>| {
            Publication::decode(&publication_wire(3), b).map(|_| ())
        })
    });
    boundaries(AW, AS, || {
        (288, |b: &mut Budget<'_>| {
            Ack::decode(&ack_wire(3), b).map(|_| ())
        })
    });
    boundaries(CDW, CDS, || {
        (2090, |b: &mut Budget<'_>| {
            Carriage::decode(&carriage_wire(3), b).map(|_| ())
        })
    });
}

#[test]
fn conditional_carriage_v3_constructors_exact_short_and_prior_denials() {
    boundaries(UW, US, || {
        let r = receipt();
        (r.retained_storage(), move |b: &mut Budget<'_>| {
            Publication::new(JOURNAL, OCCURRENCE, r, b).map(|_| ())
        })
    });
    boundaries(AW, AS, || {
        let u = publication();
        (u.retained_storage(), move |b: &mut Budget<'_>| {
            Ack::new(&u, WORKER, b).map(|_| ())
        })
    });
    boundaries(CCW, CCS, || {
        let (p, q, u, a) = (policy(), request(), publication(), ack());
        let inherited = p.retained_storage()
            + q.retained_storage()
            + u.retained_storage()
            + a.retained_storage();
        (inherited, move |b: &mut Budget<'_>| {
            Carriage::new(p, q, u, a, b).map(|_| ())
        })
    });
}

#[test]
fn conditional_carriage_v3_identity_and_relationship_resource_boundaries() {
    boundaries(UW, US, || {
        let identity = publication().identity();
        (584, move |b: &mut Budget<'_>| {
            identity
                .matches_canonical_bytes(&publication_wire(3), b)
                .map(|yes| assert!(yes))
        })
    });
    boundaries(AW, AS, || {
        let identity = ack().identity();
        (288, move |b: &mut Budget<'_>| {
            identity
                .matches_canonical_bytes(&ack_wire(3), b)
                .map(|yes| assert!(yes))
        })
    });
    boundaries(CW, CS, || {
        let identity = carriage(&carriage_wire(3)).unwrap().identity();
        (2090, move |b: &mut Budget<'_>| {
            identity
                .matches_canonical_bytes(&carriage_wire(3), b)
                .map(|yes| assert!(yes))
        })
    });
    boundaries(UW, US, || {
        let u = publication();
        (u.retained_storage(), move |b: &mut Budget<'_>| {
            u.matches_issued_record(
                u.policy_identity(),
                JOURNAL,
                OCCURRENCE,
                u.receipt_identity(),
                b,
            )
        })
    });
    boundaries(AW, AS, || {
        let (u, a) = (publication(), ack());
        (
            u.retained_storage() + a.retained_storage(),
            move |b: &mut Budget<'_>| a.matches_publication(&u, b),
        )
    });
    boundaries(AW, AS, || {
        let a = ack();
        (a.retained_storage(), move |b: &mut Budget<'_>| {
            a.matches_worker_ledger_record(WORKER, b)
        })
    });
}

#[test]
fn conditional_carriage_v3_nested_ceiling_and_late_refusal_keep_original_account() {
    use fe2o3_artifact_transaction::{
        CompilerExecutionSubjectErrorV3 as SubjectError, MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5,
    };
    let bytes = carriage_wire(3);
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5 + 1);
    b.reserve_storage(bytes.len() + 37).unwrap();
    let ledger = b.work_ledger_identity_v1();
    assert!(matches!(
        Carriage::decode(&bytes, &mut b),
        Err(Error::Attestation(Attestation::Subject(
            SubjectError::Resource(Resource::Accounting)
        )))
    ));
    assert_eq!(b.storage(), bytes.len() + 37);
    assert_eq!(
        b.work(),
        CW + COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3
            + COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V3
            + 8
    );
    assert!(b.work_ledger_identity_v1() == ledger);

    for failure in 0..3 {
        let mut bytes = carriage_wire(3);
        match failure {
            0 => bytes[1770 + 184..1770 + 216].fill(0),
            1 => {
                bytes[1770 + 152] ^= 1;
                seal(
                    &mut bytes[1770..2058],
                    "COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK",
                    3,
                );
            }
            _ => bytes[2058] ^= 1,
        }
        let mut work = Work::new(WORK);
        let mut b = Budget::new(&mut work, LIMIT);
        b.reserve_storage(bytes.len() + 71).unwrap();
        assert!(b.reserve_storage(usize::MAX).is_err());
        assert!(b.charge_work(usize::MAX).is_err());
        let floor = b.storage();
        let denials = (b.failed_work(), b.failed_storage());
        let ledger = b.work_ledger_identity_v1();
        let result = Carriage::decode(&bytes, &mut b);
        assert!(match failure {
            0 => matches!(
                result,
                Err(Error::Framing(Framing::ZeroValue("Worker ledger record")))
            ),
            1 => matches!(result, Err(Error::Framing(Framing::PublicationMismatch))),
            _ => matches!(
                result,
                Err(Error::Framing(Framing::IdentityMismatch(
                    "compiler receipt carriage"
                )))
            ),
        });
        let expected = if failure == 0 {
            CDW - COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V3 - AW
        } else {
            CDW
        };
        assert_eq!(b.work(), expected);
        assert_eq!(b.storage(), floor);
        assert_eq!((b.failed_work(), b.failed_storage()), denials);
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn conditional_carriage_v3_consuming_mismatch_keeps_all_inherited_reservations() {
    let (p, q, u) = (policy(), request(), publication());
    let mut bytes = ack_wire(3);
    bytes[152] ^= 1;
    seal(&mut bytes, "COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK", 3);
    let a = run(bytes.len(), |b| Ack::decode(&bytes, b)).unwrap().0;
    let floor = p.retained_storage()
        + q.retained_storage()
        + u.retained_storage()
        + a.retained_storage()
        + 37;
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    assert!(matches!(
        Carriage::new(p, q, u, a, &mut b),
        Err(Error::Framing(Framing::PublicationMismatch))
    ));
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), CCW);
    assert!(b.work_ledger_identity_v1() == ledger);
}
