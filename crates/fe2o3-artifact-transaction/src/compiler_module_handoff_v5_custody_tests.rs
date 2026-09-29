//! Actual inert publication custody only, never native observation authority.
use super::*;
use crate::compiler_module_handoff::conditional_v5::tests::fixture::Fixture;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::{Duration, Instant};

const LIMIT: usize = MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5;

fn isolated(name: &str) -> bool {
    const ENV: &str = "FE2O3_TEST_QUOTED_V5_CUSTODY";
    if std::env::var_os(ENV).is_some() {
        return false;
    }
    let name = format!("{module}::{name}", module = module_path!());
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg(name.strip_prefix("fe2o3_artifact_transaction::").unwrap())
        .arg("--nocapture")
        .env(ENV, "1");
    let mut child = crate::with_artifact_process_spawn_v1(|| command.spawn()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "quoted custody test failed: {status}");
            return true;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            child.wait().unwrap();
            panic!("quoted custody test timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn published(f: &Fixture) -> CompilerModuleHandoffReceiptV5 {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    f.reserve(&mut budget);
    f.publish(&mut budget).unwrap()
}

fn assert_lock(f: &Fixture, held: bool) {
    // Independently test a fresh OS open-file description, not only the process table.
    let fd = rustix::fs::open(
        f.path.join(crate::LOCK_FILE),
        OFlags::RDWR | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .unwrap();
    assert_eq!(
        crate::acquire_linux_ofd_exclusive_lock(&fd, true).unwrap(),
        !held
    );
    drop(fd);
    assert_eq!(
        PinnedOutput::open(&f.path)
            .unwrap()
            .try_lock()
            .unwrap()
            .is_none(),
        held
    );
}

#[test]
fn try_recovery_refuses_real_writers_without_mutation_then_recovers_exactly() {
    if isolated("try_recovery_refuses_real_writers_without_mutation_then_recovers_exactly") {
        return;
    }
    let f = Fixture::new();
    let receipt = published(&f);
    let ready = fs::read(f.slot().join(READY_ENTRY)).unwrap();
    let payload = fs::read(f.slot().join(PAYLOAD_ENTRY)).unwrap();
    let output = PinnedOutput::open(&f.path).unwrap();
    let barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(17).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    for kernel_only in [false, true] {
        // Cover the real cooperating writer and, separately, contention that
        // bypasses the process reservation table and reaches the OS OFD lock.
        let writer = if kernel_only {
            let fd = rustix::fs::open(
                f.path.join(crate::LOCK_FILE),
                OFlags::RDWR | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .unwrap();
            assert!(crate::acquire_linux_ofd_exclusive_lock(&fd, true).unwrap());
            (None, Some(fd))
        } else {
            (Some(output.try_lock().unwrap().unwrap()), None)
        };
        let before = budget.work();
        assert!(matches!(
            try_recover_compiler_module_handoff_receipt_v5(
                &f.path,
                &f.producer,
                f.attempt,
                &barrier,
                &mut budget
            ),
            Err(Error::Busy)
        ));
        assert!(budget.work() > before);
        assert_eq!(budget.storage(), 17);
        assert!(ledger == budget.work_ledger_identity_v1());
        assert_eq!(fs::read(f.slot().join(READY_ENTRY)).unwrap(), ready);
        assert_eq!(fs::read(f.slot().join(PAYLOAD_ENTRY)).unwrap(), payload);
        f.ready();
        assert_lock(&f, true);
        drop(writer);
        let before = budget.work();
        assert_eq!(
            try_recover_compiler_module_handoff_receipt_v5(
                &f.path,
                &f.producer,
                f.attempt,
                &barrier,
                &mut budget
            )
            .unwrap(),
            receipt
        );
        assert!(budget.work() > before);
        assert_eq!(budget.storage(), 17);
        assert!(ledger == budget.work_ledger_identity_v1());
        assert_eq!(fs::read(f.slot().join(READY_ENTRY)).unwrap(), ready);
        assert_eq!(fs::read(f.slot().join(PAYLOAD_ENTRY)).unwrap(), payload);
        f.ready();
        assert_lock(&f, false);
    }
}

#[test]
fn quote_uses_receipt_length_and_actual_input_capacities() {
    let f = Fixture::new();
    let receipt = published(&f);
    let quote = quote_compiler_module_handoff_currentness_custody_v5(&f.path, &f.producer, receipt)
        .unwrap();
    assert_eq!(quote.receipt(), receipt);
    assert_eq!(quote.retained_storage(), quote.lease.0 + quote.token.0);
    assert_eq!(
        quote.token.0,
        token_headers::<Handoff>() + quote.dynamic + receipt.length + METADATA
    );
    assert!(quote.retained_storage() < MAX_COMPILER_MODULE_HANDOFF_BYTES_V5);
    let mut producer = f.producer.clone();
    producer.stable_source.reserve(4096);
    producer.crate_name.reserve(1024);
    let larger =
        quote_compiler_module_handoff_currentness_custody_v5(&f.path, &producer, receipt).unwrap();
    let difference = producer.stable_source.capacity() + producer.crate_name.capacity()
        - f.producer.stable_source.capacity()
        - f.producer.crate_name.capacity();
    assert_eq!(
        larger.retained_storage() - quote.retained_storage(),
        difference * 2
    );
    let longer = f.path.join("longer-output");
    let changed =
        quote_compiler_module_handoff_currentness_custody_v5(&longer, &f.producer, receipt)
            .unwrap();
    assert!(changed.retained_storage() > quote.retained_storage());
    let mut bad = receipt;
    bad.length += 1;
    assert!(matches!(
        quote_compiler_module_handoff_currentness_custody_v5(&f.path, &f.producer, bad),
        Err(Error::HandoffIdentityMismatch)
    ));
}

#[test]
fn quoted_custody_retains_real_locks_and_original_ledger() {
    if isolated("quoted_custody_retains_real_locks_and_original_ledger") {
        return;
    }
    fn send_static<T: Send + 'static>() {}
    send_static::<CompilerModuleHandoffCurrentnessLeaseV5>();
    send_static::<CompilerModuleHandoffConsumptionTokenV5>();
    send_static::<CompilerModuleHandoffCurrentnessCustodyQuoteV5>();
    let f = Fixture::new();
    let receipt = published(&f);
    let quote = quote_compiler_module_handoff_currentness_custody_v5(&f.path, &f.producer, receipt)
        .unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(quote.retained_storage()).unwrap();
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
    let (lease, storage) = acquire_compiler_module_handoff_currentness_lease_with_quote_v5(
        &f.path,
        &f.producer,
        &quote,
        &barrier,
        &mut budget,
    )
    .unwrap();
    assert_eq!(storage, quote.lease_storage());
    assert_eq!(lease.storage(), storage);
    assert!(
        dynamic_storage(
            &lease.binding.output,
            &lease.binding.producer,
            &lease.binding.parent,
            &lease.binding.slot_directory
        )
        .unwrap()
            <= quote.dynamic
    );
    assert_eq!(budget.storage(), floor);
    let (token, storage) = lease
        .acquire_current_token_with_quote(&quote, &barrier, &mut budget)
        .unwrap();
    assert_eq!(storage, quote.token_storage());
    assert_eq!(token.storage(), storage);
    assert_eq!(
        token.handoff().canonical_bytes(),
        f.handoff.canonical_bytes()
    );
    assert_eq!(token.handoff().backing_capacity(), receipt.length());
    lease.validate_current_token(&token).unwrap();
    token.revalidate_locked_currentness(&mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(ledger == budget.work_ledger_identity_v1());
    assert!(budget.work() > 0);
    assert_lock(&f, true);
    assert!(matches!(
        lease.acquire_current_token_with_quote(&quote, &barrier, &mut budget),
        Err(Error::Busy)
    ));
    assert_eq!(budget.storage(), floor);
    assert!(matches!(
        crate::try_acquire_artifact_process_spawn_lease_v1(),
        Err(crate::ArtifactProcessSpawnLeaseErrorV1::RetirementInProgress)
    ));
    drop(token);
    let (ordinary, storage) = lease.acquire_current_token(&mut budget).unwrap();
    assert_eq!(
        storage.0,
        token_headers::<Handoff>() + quote.dynamic + payload_storage(ordinary.handoff()).unwrap()
    );
    assert_lock(&f, true);
    drop(ordinary);
    let (token, _) = lease
        .acquire_current_token_with_quote(&quote, &barrier, &mut budget)
        .unwrap();
    // Both real owners transfer without unsafe Send; destruction precedes barrier release.
    std::thread::spawn(move || {
        drop((lease, token));
        drop(barrier);
    })
    .join()
    .unwrap();
    assert_lock(&f, false);
    assert_eq!(budget.storage(), floor);
    drop(crate::try_acquire_artifact_process_spawn_lease_v1().unwrap());
}

#[test]
fn quoted_custody_refuses_changed_shape_receipt_and_short_funding() {
    if isolated("quoted_custody_refuses_changed_shape_receipt_and_short_funding") {
        return;
    }
    let f = Fixture::new();
    let receipt = published(&f);
    let quote = quote_compiler_module_handoff_currentness_custody_v5(&f.path, &f.producer, receipt)
        .unwrap();
    let barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(quote.retained).unwrap();
    let floor = budget.storage();
    let mut producer = f.producer.clone();
    producer.crate_name.reserve(4096);
    assert!(matches!(
        acquire_compiler_module_handoff_currentness_lease_with_quote_v5(
            &f.path,
            &producer,
            &quote,
            &barrier,
            &mut budget
        ),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), floor);
    assert_lock(&f, false);
    let before = budget.work();
    let (lease, _) = acquire_compiler_module_handoff_currentness_lease_with_quote_v5(
        &f.path,
        &f.producer,
        &quote,
        &barrier,
        &mut budget,
    )
    .unwrap();
    let lease_work = budget.work() - before;
    let before = budget.work();
    let (token, _) = lease
        .acquire_current_token_with_quote(&quote, &barrier, &mut budget)
        .unwrap();
    let token_work = budget.work() - before;
    drop(token);
    let mut different = quote;
    different.receipt.transaction_identity =
        CompilerModuleHandoffTransactionIdentityV5::from_bytes([0; 32]);
    assert!(matches!(
        acquire_compiler_module_handoff_currentness_lease_with_quote_v5(
            &f.path,
            &f.producer,
            &different,
            &barrier,
            &mut budget
        ),
        Err(Error::Coordination(
            CompilerModuleHandoffErrorV1::DigestMismatch
        ))
    ));
    assert!(matches!(
        lease.acquire_current_token_with_quote(&different, &barrier, &mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    for token in [false, true] {
        for storage_short in [false, true] {
            let mut work = Work::new(if storage_short {
                usize::MAX
            } else {
                (if token { token_work } else { lease_work }) - 1
            });
            let mut b = Budget::new(&mut work, if storage_short { floor } else { LIMIT });
            b.reserve_storage(floor).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let result = if token {
                lease
                    .acquire_current_token_with_quote(&quote, &barrier, &mut b)
                    .map(|_| ())
            } else {
                acquire_compiler_module_handoff_currentness_lease_with_quote_v5(
                    &f.path,
                    &f.producer,
                    &quote,
                    &barrier,
                    &mut b,
                )
                .map(|_| ())
            };
            if storage_short {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert!(b.failed_storage().is_some());
            } else {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert!(b.failed_work().is_some());
            }
            assert_eq!(b.storage(), floor);
            assert!(ledger == b.work_ledger_identity_v1());
            assert_lock(&f, false);
            assert_eq!(lease.receipt(), receipt);
        }
    }
    let (token, _) = lease
        .acquire_current_token_with_quote(&quote, &barrier, &mut budget)
        .unwrap();
    assert_lock(&f, true);
    drop((lease, token));
    assert_lock(&f, false);
}

#[test]
fn quoted_metadata_capacity_refuses_before_lock_acquisition() {
    if isolated("quoted_metadata_capacity_refuses_before_lock_acquisition") {
        return;
    }
    let f = Fixture::new();
    let receipt = published(&f);
    let mut quote =
        quote_compiler_module_handoff_currentness_custody_v5(&f.path, &f.producer, receipt)
            .unwrap();
    let _barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
    let output = PinnedOutput::open(&f.path).unwrap();
    let lock = output.try_lock().unwrap().unwrap();
    // Private fault injection cannot manufacture a public quote. Busy would prove
    // that the constructor tried to lock before checking retained allocations.
    quote.dynamic = 0;
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let result = entry(&mut budget, 0, |resources| {
        resources.reserve(quote.lease.0)?;
        mint_quoted(&f.path, &f.producer, &quote, resources)
            .map(|_| ())
            .map_err(Error::from)
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 0);
    assert_lock(&f, true);
    drop(lock);
    assert_lock(&f, false);
}

#[test]
fn quoted_custody_scope_exit_and_unwind_drop_under_original_barrier() {
    if isolated("quoted_custody_scope_exit_and_unwind_drop_under_original_barrier") {
        return;
    }
    let f = Fixture::new();
    let receipt = published(&f);
    let quote = quote_compiler_module_handoff_currentness_custody_v5(&f.path, &f.producer, receipt)
        .unwrap();
    let barrier = crate::try_acquire_artifact_lock_retirement_barrier_v1().unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(quote.retained).unwrap();
    let floor = budget.storage();
    let (lease, _) = acquire_compiler_module_handoff_currentness_lease_with_quote_v5(
        &f.path,
        &f.producer,
        &quote,
        &barrier,
        &mut budget,
    )
    .unwrap();
    for unwind in [false, true] {
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.with_prepaid_scope(
                floor,
                8,
                8,
                size_of::<CompilerModuleHandoffConsumptionTokenV5>(),
                |b| {
                    let (token, _) = lease.acquire_current_token_with_quote(&quote, &barrier, b)?;
                    assert_lock(&f, true);
                    if unwind {
                        panic!("quoted token rollback");
                    }
                    b.release_storage(1)?;
                    Ok::<_, Error>(token)
                },
            )
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result,
                Ok(Err(Error::Resource(Resource::Accounting)))
            ));
        }
        assert_eq!(budget.storage(), floor);
        assert_lock(&f, false);
        assert_eq!(lease.receipt(), receipt);
        assert!(matches!(
            crate::try_acquire_artifact_process_spawn_lease_v1(),
            Err(crate::ArtifactProcessSpawnLeaseErrorV1::RetirementInProgress)
        ));
    }
    // A genuine on-disk mismatch fails after token locking without losing the lease.
    let file = fs::OpenOptions::new()
        .write(true)
        .open(f.slot().join(PAYLOAD_ENTRY))
        .unwrap();
    file.write_all_at(&[0xff], 0).unwrap();
    assert!(
        lease
            .acquire_current_token_with_quote(&quote, &barrier, &mut budget)
            .is_err()
    );
    assert_eq!(budget.storage(), floor);
    assert_eq!(lease.receipt(), receipt);
    assert_lock(&f, false);
    drop(lease);
}
