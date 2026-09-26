// These fixtures exercise descriptor packaging/accounting, NOT native admission.
// They never fabricate Managed, Admission or Child owners. Successful protected
// Managed cloning and final-pair validation require a real native root launch and
// remain integration coverage gaps; the rootless tests do not establish authority.
fn non_authoritative_transfer_packaging_fixture() -> (TransferFixture, [(u64, u64); 2]) {
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    let policy = make_policy(0, &mut b);
    let (uid, gid) = ids();
    let service = Service::new(uid, gid).unwrap();
    let supervisor = make_supervisor(&policy, service, 0, &mut b);
    let (deployment, charge) = Deployment::new(
        supervisor.deployment(),
        policy.policy(),
        Measurement::new([9; 32], 1).unwrap(),
        &mut b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let endpoint = tempfile::tempfile().unwrap();
    let pidfd = tempfile::tempfile().unwrap();
    let objects = [identity(&endpoint), identity(&pidfd)];
    (
        TransferFixture {
            endpoint: endpoint.into(),
            pidfd: pidfd.into(),
            service,
            deployment: deployment.identity(),
            supervisor: supervisor.deployment().identity(),
            policy: policy.policy().identity(),
        },
        objects,
    )
}

fn assert_transfer_objects_closed(objects: [(u64, u64); 2]) {
    // Object identity, not recycled descriptor numbers, determines cleanup.
    for object in objects {
        assert_eq!(refs(object), 0);
    }
}

#[test]
fn transfer_extraction_exact_work_floor_scratch_and_order() {
    let (transfer, objects) = non_authoritative_transfer_packaging_fixture();
    const EXTRA: usize = 31;
    let floor = EXTRA + TransferFixture::STORAGE;
    let mut w = Work::new(TransferFixture::INTO_DESCRIPTORS_WORK);
    let mut b = Budget::new(&mut w, floor + TransferFixture::INTO_DESCRIPTORS_SCRATCH);
    b.reserve_storage(floor).unwrap();
    let ledger = b.work_ledger_identity_v1();
    assert_eq!(transfer.retained_storage(), TransferFixture::STORAGE);
    assert_eq!(TransferFixture::PAIR_STORAGE, Admission::PAIR_STORAGE);
    assert!(TransferFixture::STORAGE >= size_of::<(TransferFixture, TransferStorage)>());
    assert_eq!(transfer.service(), Service::new(ids().0, ids().1).unwrap());
    let _nominal: (
        TransferDeploymentIdentity,
        TransferSupervisorIdentity,
        TransferPolicyIdentity,
    ) = (
        transfer.deployment_identity(),
        transfer.supervisor_identity(),
        transfer.policy_identity(),
    );
    let (endpoint, pidfd) = transfer.into_ordered_descriptors(&mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), TransferFixture::INTO_DESCRIPTORS_WORK);
    assert_eq!(
        b.peak_storage(),
        floor + TransferFixture::INTO_DESCRIPTORS_SCRATCH
    );
    assert_eq!(b.failed_storage(), None);
    assert_eq!(b.failed_work(), None);
    assert!(b.work_ledger_identity_v1() == ledger);
    b.release_storage(TransferFixture::STORAGE - TransferFixture::PAIR_STORAGE)
        .unwrap();
    assert_eq!(b.storage(), EXTRA + TransferFixture::PAIR_STORAGE);
    let endpoint = File::from(endpoint);
    let pidfd = File::from(pidfd);
    assert_eq!([identity(&endpoint), identity(&pidfd)], objects);
    for fd in [&endpoint, &pidfd] {
        assert!(
            rustix::io::fcntl_getfd(fd)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }
    drop((endpoint, pidfd));
    b.release_storage(TransferFixture::PAIR_STORAGE).unwrap();
    assert_eq!(b.storage(), EXTRA);
    assert_transfer_objects_closed(objects);
}

#[test]
fn transfer_consuming_refusal_closes_both_fds_without_retiring_the_input_charge() {
    for mode in 0..4 {
        let (transfer, objects) = non_authoritative_transfer_packaging_fixture();
        let floor = TransferFixture::STORAGE - usize::from(mode == 0);
        let work = match mode {
            1 => ENTRY_WORK - 1,
            2 => TransferFixture::INTO_DESCRIPTORS_WORK - 1,
            _ => TransferFixture::INTO_DESCRIPTORS_WORK,
        };
        let limit = floor + TransferFixture::INTO_DESCRIPTORS_SCRATCH - usize::from(mode == 3);
        let mut w = Work::new(work);
        let mut b = Budget::new(&mut w, limit);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let error = transfer.into_ordered_descriptors(&mut b).unwrap_err();
        match mode {
            0 => assert!(matches!(error, LaunchError::Resource(Resource::Accounting))),
            1 | 2 => assert!(matches!(error, LaunchError::Resource(Resource::Work(_)))),
            _ => assert!(matches!(error, LaunchError::Resource(Resource::Storage(_)))),
        }
        assert_eq!(
            b.work(),
            match mode {
                1 => 0,
                3 => TransferFixture::INTO_DESCRIPTORS_WORK,
                _ => ENTRY_WORK,
            }
        );
        assert_eq!(b.storage(), floor);
        assert_eq!(b.peak_storage(), floor);
        assert_eq!(
            b.failed_work(),
            match mode {
                1 => Some(ENTRY_WORK),
                2 => Some(TransferFixture::INTO_DESCRIPTORS_WORK),
                _ => None,
            }
        );
        assert_eq!(
            b.failed_storage(),
            (mode == 3).then_some(floor + TransferFixture::INTO_DESCRIPTORS_SCRATCH)
        );
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_transfer_objects_closed(objects);
        b.release_storage(floor).unwrap();
        assert_eq!(b.storage(), 0);
    }
}

#[test]
fn transfer_extraction_preserves_prior_denials_and_peak() {
    let (transfer, objects) = non_authoritative_transfer_packaging_fixture();
    let mut w = Work::new(TransferFixture::INTO_DESCRIPTORS_WORK);
    let mut b = Budget::new(
        &mut w,
        TransferFixture::STORAGE + 2 * TransferFixture::INTO_DESCRIPTORS_SCRATCH,
    );
    b.reserve_storage(TransferFixture::STORAGE).unwrap();
    assert!(
        b.charge_work(TransferFixture::INTO_DESCRIPTORS_WORK + 1)
            .is_err()
    );
    assert!(
        b.reserve_storage(2 * TransferFixture::INTO_DESCRIPTORS_SCRATCH + 1)
            .is_err()
    );
    b.reserve_storage(2 * TransferFixture::INTO_DESCRIPTORS_SCRATCH)
        .unwrap();
    b.release_storage(2 * TransferFixture::INTO_DESCRIPTORS_SCRATCH)
        .unwrap();
    let history = (b.failed_work(), b.failed_storage(), b.peak_storage());
    let pair = transfer.into_ordered_descriptors(&mut b).unwrap();
    assert_eq!(
        (b.failed_work(), b.failed_storage(), b.peak_storage()),
        history
    );
    assert_eq!(b.storage(), TransferFixture::STORAGE);
    drop(pair);
    b.release_storage(TransferFixture::STORAGE).unwrap();
    assert_transfer_objects_closed(objects);
}

#[test]
fn transfer_extraction_checked_storage_and_work_overflow_close_inputs() {
    for storage_overflow in [true, false] {
        let (transfer, objects) = non_authoritative_transfer_packaging_fixture();
        let mut w = Work::new(usize::MAX);
        let mut b = Budget::new(&mut w, usize::MAX);
        let floor = if storage_overflow {
            usize::MAX
        } else {
            TransferFixture::STORAGE
        };
        b.reserve_storage(floor).unwrap();
        if !storage_overflow {
            b.charge_work(usize::MAX).unwrap();
        }
        let error = transfer.into_ordered_descriptors(&mut b).unwrap_err();
        if storage_overflow {
            assert!(matches!(error, LaunchError::Resource(Resource::Storage(_))));
            assert_eq!(b.failed_storage(), Some(usize::MAX));
            assert_eq!(b.work(), TransferFixture::INTO_DESCRIPTORS_WORK);
        } else {
            assert!(matches!(error, LaunchError::Resource(Resource::Work(_))));
            assert_eq!(b.failed_work(), Some(usize::MAX));
            assert_eq!(b.work(), usize::MAX);
        }
        assert_eq!(b.storage(), floor);
        assert_transfer_objects_closed(objects);
    }
}

#[test]
fn transfer_drop_and_caller_unwind_close_only_and_preserve_outer_ledger() {
    let (transfer, objects) = non_authoritative_transfer_packaging_fixture();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(TransferFixture::STORAGE).unwrap();
    drop(transfer);
    assert_transfer_objects_closed(objects);
    assert_eq!(b.storage(), TransferFixture::STORAGE);
    assert_eq!(b.work(), 0);
    b.release_storage(TransferFixture::STORAGE).unwrap();

    let (transfer, objects) = non_authoritative_transfer_packaging_fixture();
    b.reserve_storage(TransferFixture::STORAGE).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let _: LaunchResult<()> = b.with_prepaid_scope(TransferFixture::STORAGE, 0, 0, 64, |b| {
            let pair = transfer.into_ordered_descriptors(b)?;
            // Keep the entire consumed reservation live across this later failure.
            assert!(rustix::io::fcntl_getfd(&pair.0).is_ok());
            panic!("caller after native transfer extraction");
        });
    }));
    assert!(outcome.is_err());
    assert_transfer_objects_closed(objects);
    assert_eq!(b.storage(), TransferFixture::STORAGE);
    assert_eq!(b.work(), TransferFixture::INTO_DESCRIPTORS_WORK);
    assert_eq!(
        b.peak_storage(),
        TransferFixture::STORAGE + 64 + TransferFixture::INTO_DESCRIPTORS_SCRATCH
    );
    assert!(b.work_ledger_identity_v1() == ledger);
}

#[test]
fn transfer_scope_requires_owner_both_contexts_and_full_candidate_pair() {
    let (f, p) = prepared_fixture();
    for pair in [0, TransferFixture::PAIR_STORAGE] {
        let floor = p.retained_storage() + f.context_storage() + pair;
        for mode in 0..5 {
            let paid = floor - usize::from(mode == 1);
            let work = match mode {
                2 => ENTRY_WORK - 1,
                3 => TRANSFER_LOCAL_WORK - 1,
                _ => TRANSFER_LOCAL_WORK,
            };
            let limit = paid + TRANSFER_FRAME - usize::from(mode == 4);
            let mut w = Work::new(work);
            let mut b = Budget::new(&mut w, limit);
            b.reserve_storage(paid).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let mut called = false;
            let result = supervisor_transfer_scope(
                p.retained_storage(),
                &f.supervisor,
                &f.policy,
                pair,
                &mut b,
                |_| {
                    called = true;
                    Ok(())
                },
            );
            assert_eq!(called, mode == 0);
            assert_eq!(result.is_ok(), mode == 0);
            match mode {
                1 => assert!(matches!(
                    result,
                    Err(LaunchError::Resource(Resource::Accounting))
                )),
                2 | 3 => assert!(matches!(
                    result,
                    Err(LaunchError::Resource(Resource::Work(_)))
                )),
                4 => assert!(matches!(
                    result,
                    Err(LaunchError::Resource(Resource::Storage(_)))
                )),
                _ => (),
            }
            assert_eq!(b.storage(), paid);
            assert_eq!(
                b.work(),
                match mode {
                    1 | 3 => ENTRY_WORK,
                    2 => 0,
                    _ => TRANSFER_LOCAL_WORK,
                }
            );
            assert_eq!(
                b.peak_storage(),
                paid + if mode == 0 { TRANSFER_FRAME } else { 0 }
            );
            assert!(b.work_ledger_identity_v1() == ledger);
        }
    }
}

#[test]
fn transfer_scope_checked_floor_and_nested_failure_history() {
    let (f, p) = prepared_fixture();
    let floor = p.retained_storage() + f.context_storage() + TransferFixture::PAIR_STORAGE;
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(floor).unwrap();
    let error = supervisor_transfer_scope(
        usize::MAX,
        &f.supervisor,
        &f.policy,
        TransferFixture::PAIR_STORAGE,
        &mut b,
        |_| -> LaunchResult<()> { panic!("overflow entered callback") },
    );
    assert!(matches!(
        error,
        Err(LaunchError::Preparation(Error::Resource(
            Resource::Arithmetic
        )))
    ));
    assert_eq!(b.work(), ENTRY_WORK);
    assert_eq!(b.storage(), floor);
    let error = supervisor_transfer_scope(
        p.retained_storage(),
        &f.supervisor,
        &f.policy,
        TransferFixture::PAIR_STORAGE,
        &mut b,
        |b| -> LaunchResult<()> {
            b.charge_work(LIMIT)?;
            unreachable!()
        },
    );
    assert!(matches!(
        error,
        Err(LaunchError::Resource(Resource::Work(_)))
    ));
    assert_eq!(b.work(), ENTRY_WORK + TRANSFER_LOCAL_WORK);
    assert_eq!(
        b.failed_work(),
        Some(ENTRY_WORK + TRANSFER_LOCAL_WORK + LIMIT)
    );
    assert_eq!(b.storage(), floor);
    assert_eq!(b.peak_storage(), floor + TRANSFER_FRAME);
}

#[test]
fn transfer_context_nested_exact_and_one_short_preserve_original_ledger() {
    // Actual same-owner preparation revalidation, not a protected Managed owner.
    let (f, p) = prepared_fixture();
    let floor = p.retained_storage() + f.context_storage() + TransferFixture::PAIR_STORAGE;
    let quota = p.revalidation_quota().unwrap();
    let work = TRANSFER_LOCAL_WORK + quota.work();
    let run = |work_limit, storage_limit| {
        let mut w = Work::new(work_limit);
        let mut b = Budget::new(&mut w, storage_limit);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = supervisor_transfer_scope(
            p.retained_storage(),
            &f.supervisor,
            &f.policy,
            TransferFixture::PAIR_STORAGE,
            &mut b,
            |b| Ok(p.revalidate_inner::<false>(&f.supervisor, &f.policy, b)?),
        );
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        (
            result,
            b.work(),
            b.peak_storage(),
            b.failed_work(),
            b.failed_storage(),
        )
    };
    let (result, accepted, peak, failed_work, failed_storage) =
        run(work, floor + TRANSFER_FRAME + quota.scratch());
    result.unwrap();
    assert_eq!(accepted, work);
    assert_eq!((failed_work, failed_storage), (None, None));
    assert!(peak > floor + TRANSFER_FRAME);
    let exact = run(work, peak);
    exact.0.unwrap();
    assert_eq!(exact.1, work);
    assert_eq!(exact.2, peak);
    let short_work = run(work - 1, peak);
    assert!(short_work.0.is_err());
    assert_eq!(short_work.3, Some(work));
    let short_storage = run(work, peak - 1);
    assert!(short_storage.0.is_err());
    assert_eq!(short_storage.4, Some(peak));
}

#[test]
fn transfer_required_actual_context_rejects_every_policy_and_supervisor_axis() {
    // Exercise the actual preparation check called by Managed continuity inside
    // the production transfer frame. No child/admission/Managed fixture is made.
    let (f, p) = prepared_fixture();
    let mut w = Work::new(LIMIT);
    let mut b = Budget::new(&mut w, LIMIT);
    b.reserve_storage(p.retained_storage() + f.context_storage() + TransferFixture::PAIR_STORAGE)
        .unwrap();
    for axis in 1..=5 {
        let wrong = make_policy(axis, &mut b);
        let before = b.storage();
        let error = supervisor_transfer_scope(
            p.retained_storage(),
            &f.supervisor,
            &wrong,
            TransferFixture::PAIR_STORAGE,
            &mut b,
            |b| Ok(p.revalidate_inner::<false>(&f.supervisor, &wrong, b)?),
        );
        assert!(matches!(
            error,
            Err(LaunchError::Preparation(Error::Invalid(
                Failure::ContextMismatch
            )))
        ));
        assert_eq!(b.storage(), before);
        let charge = wrong.retained_storage();
        drop(wrong);
        b.release_storage(charge).unwrap();
    }
    for axis in 1..=8 {
        let wrong = make_supervisor(&f.policy, p.deployment.deployment().service(), axis, &mut b);
        let before = b.storage();
        let error = supervisor_transfer_scope(
            p.retained_storage(),
            &wrong,
            &f.policy,
            TransferFixture::PAIR_STORAGE,
            &mut b,
            |b| Ok(p.revalidate_inner::<false>(&wrong, &f.policy, b)?),
        );
        assert!(matches!(
            error,
            Err(LaunchError::Preparation(Error::Invalid(
                Failure::ContextMismatch
            )))
        ));
        assert_eq!(b.storage(), before);
        let charge = wrong.retained_storage();
        drop(wrong);
        b.release_storage(charge).unwrap();
    }
}

#[test]
fn transfer_quota_counts_both_continuity_passes_and_live_output_overlap() {
    let continuity = LaunchQuota {
        work: 123,
        scratch: Admission::IO_STORAGE + 456,
    };
    let validate = supervisor_transfer_quota_for(continuity, false).unwrap();
    let clone = supervisor_transfer_quota_for(continuity, true).unwrap();
    assert_eq!(
        validate.work(),
        TRANSFER_LOCAL_WORK + 123 + Admission::VALIDATE_TRANSFER_WORK
    );
    assert_eq!(validate.scratch(), TRANSFER_FRAME + continuity.scratch());
    assert_eq!(
        clone.work(),
        2 * TRANSFER_LOCAL_WORK
            + 2 * 123
            + Admission::CLONE_TRANSFER_WORK
            + Admission::VALIDATE_TRANSFER_WORK
    );
    assert_eq!(
        clone.scratch(),
        TRANSFER_FRAME + TransferFixture::STORAGE + validate.scratch()
    );
    for clone_pair in [false, true] {
        assert!(
            supervisor_transfer_quota_for(
                LaunchQuota {
                    work: usize::MAX,
                    scratch: 0
                },
                clone_pair
            )
            .is_err()
        );
        assert!(
            supervisor_transfer_quota_for(
                LaunchQuota {
                    work: 0,
                    scratch: usize::MAX
                },
                clone_pair
            )
            .is_err()
        );
    }
    assert!(
        supervisor_transfer_quota_for(
            LaunchQuota {
                work: (usize::MAX
                    - Admission::CLONE_TRANSFER_WORK
                    - Admission::VALIDATE_TRANSFER_WORK)
                    / 2,
                scratch: 0,
            },
            true
        )
        .is_err()
    );
}
