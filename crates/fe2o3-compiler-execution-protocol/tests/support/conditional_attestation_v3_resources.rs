use super::*;

fn boundaries<F, Op>(cost: usize, scratch: usize, prepare: F)
where
    F: Fn() -> (usize, Op),
    Op: FnOnce(&mut Budget<'_>) -> Result<(), Error>,
{
    for case in 0..5 {
        let (input, operation) = prepare();
        if input == 0 && case == 3 {
            continue;
        }
        let inherited = if case == 4 { 19 } else { 0 };
        let floor = input + inherited - usize::from(case == 3);
        let maximum = input + inherited + scratch - usize::from(case == 2);
        let mut work = Work::new(9 + cost - usize::from(case == 1));
        let mut b = Budget::new(&mut work, maximum);
        b.charge_work(9).unwrap();
        b.reserve_storage(floor).unwrap();
        if case == 4 {
            assert!(b.reserve_storage(maximum + 1).is_err());
        }
        let denial = b.failed_storage();
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
                assert_eq!(b.failed_storage(), denial);
            }
            1 => assert!(matches!(
                result,
                Err(Error::Resource(Resource::Work(_)))
                    | Err(Error::Subject(
                        fe2o3_artifact_transaction::CompilerExecutionSubjectErrorV3::Resource(
                            Resource::Work(_)
                        )
                    ))
            )),
            2 => {
                assert!(matches!(
                    result,
                    Err(Error::Resource(Resource::Storage(_)))
                        | Err(Error::Subject(
                            fe2o3_artifact_transaction::CompilerExecutionSubjectErrorV3::Resource(
                                Resource::Storage(_)
                            )
                        ))
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
fn conditional_protocol_v3_all_decoders_exact_short_and_inherited_floors() {
    boundaries(
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3,
        COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3,
        || {
            (216, |b: &mut Budget<'_>| {
                Policy::decode(&policy_wire(3), b).map(|_| ())
            })
        },
    );
    boundaries(
        COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V3,
        COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V3,
        || {
            (200, |b: &mut Budget<'_>| {
                Challenge::decode(&challenge_wire(3), b).map(|_| ())
            })
        },
    );
    boundaries(
        COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V3,
        COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V3,
        || {
            (946, |b: &mut Budget<'_>| {
                Request::decode(&request_wire(3), b).map(|_| ())
            })
        },
    );
    boundaries(
        COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V3,
        COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V3,
        || {
            (400, |b: &mut Budget<'_>| {
                Receipt::decode(&receipt_wire(3), b).map(|_| ())
            })
        },
    );
}

#[test]
fn conditional_protocol_v3_constructors_exact_short_and_inherited_floors() {
    boundaries(
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3,
        COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3,
        || {
            let p = policy();
            let inputs = (
                p.generation(),
                p.executable(),
                p.runtime(),
                *p.verifying_key(),
                *p.external_anchor_verifying_key(),
            );
            drop(p);
            (0, move |b: &mut Budget<'_>| {
                Policy::new(inputs.0, inputs.1, inputs.2, inputs.3, inputs.4, b).map(|_| ())
            })
        },
    );
    boundaries(
        COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V3,
        COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V3,
        || {
            let p = policy();
            let (s, paid) = run(690, |b| Ok(Subject::decode(&subject_wire(3), b)?)).unwrap();
            (
                p.retained_storage() + paid.retained_storage(),
                move |b: &mut Budget<'_>| {
                    Challenge::new(&p, &s, [0x71; 32], 1, [0; 32], b).map(|_| ())
                },
            )
        },
    );
    boundaries(
        COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V3,
        COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V3,
        || {
            let (c, charge) = run(200, |b| Challenge::decode(&challenge_wire(3), b)).unwrap();
            let (s, paid) = run(690, |b| Ok(Subject::decode(&subject_wire(3), b)?)).unwrap();
            (
                charge.additional_storage() + paid.retained_storage(),
                move |b: &mut Budget<'_>| Request::new(c, s, b).map(|_| ()),
            )
        },
    );
    boundaries(
        COMPILER_EXECUTION_ATTESTATION_RECEIPT_ISSUE_WORK_V3,
        COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V3,
        || {
            let p = policy();
            let q = request(&request_wire(3)).unwrap();
            let key = SigningKey::from_bytes(&[0x51; 32]);
            (
                p.retained_storage() + q.retained_storage() + size_of::<SigningKey>(),
                move |b: &mut Budget<'_>| Receipt::issue(&p, &q, &key, b).map(|_| ()),
            )
        },
    );
    boundaries(
        COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V3,
        COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V3,
        || {
            let p = policy();
            let q = request(&request_wire(3)).unwrap();
            let r = receipt(&receipt_wire(3)).unwrap();
            (
                p.retained_storage() + q.retained_storage() + r.retained_storage(),
                move |b: &mut Budget<'_>| r.verify(&p, &q, [0; 32], b).map(|_| ()),
            )
        },
    );
}

#[test]
fn conditional_protocol_v3_identity_queries_exact_short_and_inherited_floors() {
    boundaries(
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3,
        COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3,
        || {
            let id = policy().identity();
            (216, move |b: &mut Budget<'_>| {
                id.matches_canonical_bytes(&policy_wire(3), b)
                    .map(|yes| assert!(yes))
            })
        },
    );
    boundaries(
        COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V3,
        COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V3,
        || {
            let q = request(&request_wire(3)).unwrap();
            let id = q.challenge().identity();
            (200, move |b: &mut Budget<'_>| {
                id.matches_canonical_bytes(&challenge_wire(3), b)
                    .map(|yes| assert!(yes))
            })
        },
    );
    boundaries(
        COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V3,
        COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V3,
        || {
            let id = request(&request_wire(3)).unwrap().identity();
            (946, move |b: &mut Budget<'_>| {
                id.matches_canonical_bytes(&request_wire(3), b)
                    .map(|yes| assert!(yes))
            })
        },
    );
    boundaries(
        COMPILER_EXECUTION_ATTESTATION_RECEIPT_IDENTITY_WORK_V3,
        COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V3,
        || {
            let id = receipt(&receipt_wire(3)).unwrap().identity();
            (400, move |b: &mut Budget<'_>| {
                id.matches_canonical_bytes(&receipt_wire(3), b)
                    .map(|yes| assert!(yes))
            })
        },
    );
    boundaries(
        COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V3,
        COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V3,
        || {
            let binding = request(&request_wire(3)).unwrap().challenge().subject();
            let (s, paid) = run(690, |b| Ok(Subject::decode(&subject_wire(3), b)?)).unwrap();
            (paid.retained_storage(), move |b: &mut Budget<'_>| {
                binding.matches_subject(&s, b).map(|yes| assert!(yes))
            })
        },
    );
}

#[test]
fn conditional_protocol_v3_malformed_nested_decode_keeps_prior_denials_and_floor() {
    let mut bytes = request_wire(3);
    bytes[224] ^= 1;
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(bytes.len() + 71).unwrap();
    assert!(b.reserve_storage(LIMIT).is_err());
    assert!(b.charge_work(WORK + 1).is_err());
    let floor = b.storage();
    let denials = (b.failed_work(), b.failed_storage());
    let ledger = b.work_ledger_identity_v1();
    assert!(matches!(
        Request::decode(&bytes, &mut b),
        Err(Error::Subject(_))
    ));
    assert_eq!(b.storage(), floor);
    assert_eq!((b.failed_work(), b.failed_storage()), denials);
    assert_eq!(
        b.work(),
        COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V3
    );
    assert!(b.work_ledger_identity_v1() == ledger);
}
