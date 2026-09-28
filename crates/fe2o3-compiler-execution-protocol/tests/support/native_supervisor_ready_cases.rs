fn ready(deployment: &Deployment, b: &mut Budget<'_>) -> Ready {
    let (ready, delta) = Ready::new(PID, deployment, b).unwrap();
    assert_eq!(delta.additional_storage(), ready.retained_storage());
    b.reserve_storage(delta.additional_storage()).unwrap();
    ready
}

#[test]
fn independent_transcript_and_exact_context_round_trip() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    b.charge_work(7).unwrap();
    b.reserve_storage(19).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let deployment = deployment!(Policy, Deployment, 7, &mut b);
    let ready = ready(&deployment, &mut b);
    let mut bytes = [0; BYTES];
    bytes[..8].copy_from_slice(if VERSION == 2 {
        b"F2O3CSR2"
    } else {
        b"F2O3CSR3"
    });
    bytes[8..10].copy_from_slice(&VERSION.to_le_bytes());
    bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
    bytes[16..20].copy_from_slice(&PID.to_le_bytes());
    bytes[24..56].copy_from_slice(deployment.identity().as_bytes());
    reseal(&mut bytes, VERSION);
    assert_eq!(ready.canonical_bytes(), &bytes);
    assert_eq!(ready.supervisor_pid(), PID);
    assert_eq!(ready.deployment_identity(), deployment.identity());
    assert_eq!(ready.identity().as_bytes(), &bytes[56..]);
    b.reserve_storage(BYTES).unwrap();
    let floor = b.storage();
    let before = b.work();
    let (decoded, delta) = Ready::decode(&bytes, PID, &deployment, &mut b).unwrap();
    assert_eq!(decoded, ready);
    assert_eq!(delta.additional_storage(), ready.retained_storage());
    b.reserve_storage(delta.additional_storage()).unwrap();
    assert!(
        decoded
            .matches_deployment(PID, &deployment, &mut b)
            .unwrap()
    );
    assert!(
        !decoded
            .matches_deployment(PID + 1, &deployment, &mut b)
            .unwrap()
    );
    let charge = decoded.retained_storage();
    drop(decoded);
    b.release_storage(charge).unwrap();
    assert_eq!(b.work(), before + 3 * WORK);
    assert_eq!(b.storage(), floor);
    assert!(b.work_ledger_identity_v1() == ledger);
}

#[test]
fn every_byte_and_wrong_length_reject_without_losing_the_input_floor() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let deployment = deployment!(Policy, Deployment, 7, &mut b);
    let ready = ready(&deployment, &mut b);
    b.reserve_storage(BYTES).unwrap();
    let floor = b.storage();
    for index in 0..BYTES {
        let mut bytes = *ready.canonical_bytes();
        bytes[index] ^= 1;
        assert!(
            Ready::decode(&bytes, PID, &deployment, &mut b).is_err(),
            "byte {index}"
        );
        assert_eq!(b.storage(), floor);
    }
    for len in [0, 1, BYTES - 1, BYTES + 1, 100_000] {
        let bytes = vec![0; len];
        let mut work = Work::new(WORK);
        let floor = deployment.retained_storage();
        let mut b = Budget::new(&mut work, floor + SCRATCH);
        b.reserve_storage(floor).unwrap();
        assert!(matches!(
            Ready::decode(&bytes, PID, &deployment, &mut b),
            Err(Error::Framing(Framing::Length))
        ));
        assert_eq!((b.work(), b.storage()), (WORK, floor));
    }
}

#[test]
fn canonical_resealing_cannot_substitute_the_expected_child_or_deployment() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let deployment = deployment!(Policy, Deployment, 7, &mut b);
    let changed = deployment!(Policy, Deployment, 8, &mut b);
    let other = deployment!(OtherPolicy, OtherDeployment, 7, &mut b);
    let ready = ready(&deployment, &mut b);
    b.reserve_storage(BYTES).unwrap();
    for pid in [0, PID + 1, u32::MAX] {
        assert!(matches!(
            Ready::decode(ready.canonical_bytes(), pid, &deployment, &mut b),
            Err(Error::ContextMismatch)
        ));
    }
    assert!(matches!(
        Ready::decode(ready.canonical_bytes(), PID, &changed, &mut b),
        Err(Error::ContextMismatch)
    ));
    assert!(!ready.matches_deployment(PID, &changed, &mut b).unwrap());
    let mut bytes = *ready.canonical_bytes();
    bytes[16..20].copy_from_slice(&(PID + 1).to_le_bytes());
    reseal(&mut bytes, VERSION);
    assert!(matches!(
        Ready::decode(&bytes, PID, &deployment, &mut b),
        Err(Error::ContextMismatch)
    ));
    // Correctly changing both public context and record is valid structure, never provenance.
    assert!(Ready::decode(&bytes, PID + 1, &deployment, &mut b).is_ok());
    let (foreign, delta) = OtherReady::new(PID, &other, &mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    assert!(matches!(
        Ready::decode(foreign.canonical_bytes(), PID, &deployment, &mut b),
        Err(Error::Framing(Framing::Magic))
    ));
    bytes = *foreign.canonical_bytes();
    bytes[7] = b'0' + VERSION as u8;
    bytes[8..10].copy_from_slice(&VERSION.to_le_bytes());
    reseal(&mut bytes, VERSION);
    assert!(matches!(
        Ready::decode(&bytes, PID, &deployment, &mut b),
        Err(Error::ContextMismatch)
    ));
}

#[test]
fn resealed_invalid_framing_still_rejects_with_exact_reason() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let deployment = deployment!(Policy, Deployment, 7, &mut b);
    let ready = ready(&deployment, &mut b);
    b.reserve_storage(BYTES).unwrap();
    for (range, expected) in [
        (16..20, Framing::SupervisorPid),
        (24..56, Framing::DeploymentIdentity),
    ] {
        let mut bytes = *ready.canonical_bytes();
        bytes[range].fill(0);
        reseal(&mut bytes, VERSION);
        assert!(
            matches!(Ready::decode(&bytes, PID, &deployment, &mut b), Err(Error::Framing(e)) if e == expected)
        );
    }
    for offset in [10, 11, 20, 21, 22, 23] {
        let mut bytes = *ready.canonical_bytes();
        bytes[offset] = 1;
        reseal(&mut bytes, VERSION);
        assert!(matches!(
            Ready::decode(&bytes, PID, &deployment, &mut b),
            Err(Error::Framing(Framing::Reserved))
        ));
    }
    for version in [0, 1, OTHER_VERSION, u16::MAX] {
        let mut bytes = *ready.canonical_bytes();
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        reseal(&mut bytes, VERSION);
        assert!(matches!(
            Ready::decode(&bytes, PID, &deployment, &mut b),
            Err(Error::Framing(Framing::Version))
        ));
    }
    let mut bytes = *ready.canonical_bytes();
    bytes[12..16].copy_from_slice(&0u32.to_le_bytes());
    reseal(&mut bytes, VERSION);
    assert!(matches!(
        Ready::decode(&bytes, PID, &deployment, &mut b),
        Err(Error::Framing(Framing::Length))
    ));
    assert!(matches!(
        Ready::new(0, &deployment, &mut b),
        Err(Error::Framing(Framing::SupervisorPid))
    ));
}

#[test]
fn all_operations_require_exact_work_scratch_and_complete_borrowed_floors() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let deployment = deployment!(Policy, Deployment, 7, &mut b);
    let ready = ready(&deployment, &mut b);
    for operation in 0..3 {
        let floor = deployment.retained_storage()
            + match operation {
                0 => 0,
                1 => BYTES,
                _ => ready.retained_storage(),
            };
        for mode in 0..5 {
            let mut work = Work::new(match mode {
                1 => 7,
                2 => WORK - 1,
                _ => WORK,
            });
            let mut b = Budget::new(&mut work, floor + SCRATCH - usize::from(mode == 3));
            let prepaid = floor - usize::from(mode == 4);
            b.reserve_storage(prepaid).unwrap();
            let result = match operation {
                0 => Ready::new(PID, &deployment, &mut b).map(|(r, d)| {
                    assert_eq!(r, ready);
                    assert_eq!(d.additional_storage(), r.retained_storage());
                }),
                1 => Ready::decode(ready.canonical_bytes(), PID, &deployment, &mut b)
                    .map(|(r, _)| assert_eq!(r, ready)),
                _ => ready
                    .matches_deployment(PID, &deployment, &mut b)
                    .map(|matched| assert!(matched)),
            };
            match mode {
                0 => {
                    result.unwrap();
                    assert_eq!(b.work(), WORK);
                    assert_eq!(b.peak_storage(), floor + SCRATCH);
                }
                1 | 2 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    assert_eq!(b.work(), if mode == 1 { 0 } else { 8 });
                    assert_eq!(b.failed_work(), Some(if mode == 1 { 8 } else { WORK }));
                    assert_eq!(b.peak_storage(), prepaid);
                }
                3 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                    assert_eq!(b.work(), WORK);
                    assert_eq!(b.failed_storage(), Some(floor + SCRATCH));
                    assert_eq!(b.peak_storage(), prepaid);
                }
                _ => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert_eq!(b.work(), 8);
                    assert_eq!(b.peak_storage(), prepaid);
                }
            }
            assert_eq!(b.storage(), prepaid);
        }
    }
}

#[test]
fn cumulative_denials_survive_semantic_rejection_and_following_success() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, LIMIT);
    let deployment = deployment!(Policy, Deployment, 7, &mut b);
    let ready = ready(&deployment, &mut b);
    b.reserve_storage(BYTES).unwrap();
    let failed = b.work() + LIMIT;
    assert!(b.charge_work(LIMIT).is_err());
    let failed_storage = b.storage() + LIMIT;
    assert!(b.reserve_storage(LIMIT).is_err());
    let floor = b.storage();
    assert!(matches!(
        Ready::decode(ready.canonical_bytes(), PID + 1, &deployment, &mut b),
        Err(Error::ContextMismatch)
    ));
    assert!(ready.matches_deployment(PID, &deployment, &mut b).unwrap());
    assert_eq!(b.storage(), floor);
    assert_eq!(b.failed_work(), Some(failed));
    assert_eq!(b.failed_storage(), Some(failed_storage));
}
