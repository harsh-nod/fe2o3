//! Inert transport tests: no protected execution or GPU authority is inferred.
#![cfg(test)]
use super::super::tests::fixture::{Fixture, outer_variant};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::os::unix::fs::PermissionsExt;

const LIMIT: usize = MAX_COMPILER_MODULE_HANDOFF_STORAGE_V5;
const BODY: &[u8] = b"opaque\0receipt\xff";

fn setup(budget: &mut Budget<'_>) -> (Fixture, CompilerModuleHandoffReceiptV5, Subject) {
    setup_profile(false, budget)
}
fn setup_profile(
    gfx950: bool,
    budget: &mut Budget<'_>,
) -> (Fixture, CompilerModuleHandoffReceiptV5, Subject) {
    let mut f = Fixture::new();
    if gfx950 {
        f.handoff = outer_variant(true, 7);
    }
    f.reserve(budget);
    let receipt = f.publish(budget).unwrap();
    let (subject, storage) = Subject::from_publication(receipt, &f.handoff, budget).unwrap();
    assert_eq!(storage.retained_storage(), SUBJECT_STORAGE);
    budget
        .reserve_storage(storage.retained_storage() + BODY.len())
        .unwrap();
    (f, receipt, subject)
}

fn token(
    lease: &CompilerModuleHandoffCurrentnessLeaseV5,
    budget: &mut Budget<'_>,
) -> CompilerModuleHandoffConsumptionTokenV5 {
    let (token, storage) = lease.acquire_current_token(budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    token
}

fn publish_body(
    f: &Fixture,
    subject: &Subject,
    budget: &mut Budget<'_>,
) -> Result<CompilerExecutionReceiptTransportReceiptV3> {
    publish_compiler_execution_receipt_transport_v3(&f.path, &f.producer, subject, BODY, budget)
}

fn recover(
    f: &Fixture,
    subject: &Subject,
    budget: &mut Budget<'_>,
) -> Result<(
    RecoveredCompilerExecutionReceiptTransportV3,
    CompilerExecutionReceiptTransportStorageV3,
)> {
    recover_compiler_execution_receipt_transport_v3(&f.path, &f.producer, subject, budget)
}

fn check_owner(
    owner: RecoveredCompilerExecutionReceiptTransportV3,
    storage: CompilerExecutionReceiptTransportStorageV3,
    receipt: CompilerExecutionReceiptTransportReceiptV3,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(
        storage.retained_storage(),
        owner.wire.capacity()
            + size_of::<RecoveredCompilerExecutionReceiptTransportV3>()
            + size_of::<CompilerExecutionReceiptTransportStorageV3>()
    );
    assert_eq!(owner.exact_bytes(), BODY);
    assert_eq!(
        owner.exact_bytes().as_ptr(),
        owner.wire[BODY_START..].as_ptr()
    );
    assert_eq!(owner.receipt(), receipt);
    assert!(!owner.grants_compiler_authority());
    assert!(!owner.grants_publication_authority());
    assert!(!owner.grants_load_authority());
    assert!(!owner.grants_launch_authority());
    drop(owner);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn conditional_receipt_v3_ready_locked_consumed_and_restart_roundtrip() {
    for gfx950 in [false, true] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (f, handoff, subject) = setup_profile(gfx950, &mut budget);
        assert!(matches!(
            recover(&f, &subject, &mut budget),
            Err(Error::NotPublished)
        ));
        let floor = budget.storage();
        let receipt = publish_body(&f, &subject, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(receipt.subject(), subject.identity());
        assert_eq!(receipt.length(), BODY.len());
        assert!(!receipt.grants_compiler_authority());
        assert!(!receipt.grants_publication_authority());
        assert!(!receipt.grants_load_authority());
        assert!(!receipt.grants_launch_authority());
        assert_eq!(publish_body(&f, &subject, &mut budget).unwrap(), receipt);
        let (owner, storage) = recover(&f, &subject, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        check_owner(owner, storage, receipt, &mut budget);
        let lease = f.lease(handoff, &mut budget);
        let other = f.lease(handoff, &mut budget);
        let current = token(&lease, &mut budget);
        assert!(matches!(
            recover_compiler_execution_receipt_transport_with_currentness_v3(
                &other,
                &current,
                &subject,
                &mut budget
            ),
            Err(Error::Handoff(
                CompilerModuleHandoffErrorV5::MismatchedCurrentnessToken
            ))
        ));
        let floor = budget.storage();
        let (owner, storage) = recover_compiler_execution_receipt_transport_with_currentness_v3(
            &lease,
            &current,
            &subject,
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        check_owner(owner, storage, receipt, &mut budget);
        assert!(matches!(
            lease.acquire_current_token(&mut budget),
            Err(CompilerModuleHandoffErrorV5::Busy)
        ));
        consume_compiler_module_handoff_with_currentness_v5(&lease, current, &mut budget).unwrap();
        assert!(!f.slot().join(PAYLOAD_ENTRY).exists());
        assert!(!f.slot().join(READY_ENTRY).exists());
        assert!(f.slot().join(CONSUMED_ENTRY).exists());
        assert!(f.slot().join(ENTRY).exists());
        drop(other);
        drop(lease);
        let (owner, storage) = recover(&f, &subject, &mut budget).unwrap();
        check_owner(owner, storage, receipt, &mut budget);
        assert!(publish_body(&f, &subject, &mut budget).is_err());
    }
}

fn changed_subject(subject: &Subject, offset: usize, budget: &mut Budget<'_>) -> (Subject, usize) {
    let mut bytes = *subject.canonical_bytes();
    bytes[offset] ^= 1;
    if (152..344).contains(&offset) {
        let pin = |i| bytes[i..i + 32].try_into().unwrap();
        let closure = fe2o3_build_authority::CompilerClosureV2::new(
            pin(152),
            pin(184),
            pin(216),
            pin(248),
            pin(280),
            pin(312),
        )
        .unwrap();
        bytes[346..378].copy_from_slice(&closure.identity_sha256());
    }
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V3\0");
    hash.update(658_u64.to_le_bytes());
    hash.update(&bytes[..658]);
    bytes[658..].copy_from_slice(&hash.finalize());
    budget.reserve_storage(bytes.len()).unwrap();
    let (changed, storage) = Subject::decode(&bytes, budget).unwrap();
    budget.release_storage(bytes.len()).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    (changed, storage.retained_storage())
}

#[test]
fn conditional_receipt_v3_checks_complete_subject_before_and_after_payload_deletion() {
    for gfx950 in [false, true] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (f, handoff, subject) = setup_profile(gfx950, &mut budget);
        let receipt = publish_body(&f, &subject, &mut budget).unwrap();
        // Invocation, every closure digest, and each inner binding digest/length.
        // The attempt, slot, transaction and outer binding remain unchanged.
        let offsets = [
            120, 152, 184, 216, 248, 280, 312, 378, 410, 418, 450, 458, 490, 498, 530, 538, 570,
            578, 610,
        ];
        let lease = f.lease(handoff, &mut budget);
        for consumed in [false, true] {
            if consumed {
                let current = token(&lease, &mut budget);
                consume_compiler_module_handoff_with_currentness_v5(&lease, current, &mut budget)
                    .unwrap();
                assert!(!f.slot().join(PAYLOAD_ENTRY).exists());
            }
            for offset in offsets {
                let (changed, storage) = changed_subject(&subject, offset, &mut budget);
                assert!(
                    matches!(
                        recover(&f, &changed, &mut budget),
                        Err(Error::SubjectBindingMismatch)
                    ),
                    "offset {offset}, consumed {consumed}"
                );
                if !consumed {
                    assert!(matches!(
                        publish_body(&f, &changed, &mut budget),
                        Err(Error::SubjectBindingMismatch)
                    ));
                    let current = token(&lease, &mut budget);
                    assert!(matches!(
                        recover_compiler_execution_receipt_transport_with_currentness_v3(
                            &lease,
                            &current,
                            &changed,
                            &mut budget
                        ),
                        Err(Error::SubjectBindingMismatch)
                    ));
                    let storage = current.storage().retained_storage();
                    drop(current);
                    budget.release_storage(storage).unwrap();
                }
                drop(changed);
                budget.release_storage(storage).unwrap();
                assert_eq!(
                    recover(&f, &subject, &mut budget).unwrap().0.receipt(),
                    receipt
                );
            }
        }
    }
}

#[test]
fn conditional_receipt_v3_codec_rejects_every_mutation_truncation_and_noncanonical_header() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (_, _, subject) = setup(&mut budget);
    let mut r = Resources::Metered(&mut budget);
    let wire = encode(&subject, BODY, &mut r).unwrap();
    assert_eq!(wire.len(), OVERHEAD + BODY.len());
    assert_eq!(
        inspect(&wire, &subject, &mut r).unwrap().length(),
        BODY.len()
    );
    for i in 0..wire.len() {
        let mut changed = wire.clone();
        changed[i] ^= 1;
        assert!(inspect(&changed, &subject, &mut r).is_err(), "offset {i}");
        assert!(inspect(&wire[..i], &subject, &mut r).is_err(), "length {i}");
    }
    for i in [0, 8, 10, 12, 20, SUBJECT_END] {
        let mut changed = wire.clone();
        changed[i] ^= 1;
        let end = changed.len() - 32;
        let digest = identity(&changed[..end]);
        changed[end..].copy_from_slice(&digest);
        assert!(matches!(
            inspect(&changed, &subject, &mut r),
            Err(Error::InvalidTransport("header"))
        ));
    }
    let mut trailing = wire;
    trailing.push(0);
    assert!(inspect(&trailing, &subject, &mut r).is_err());
}

#[test]
fn conditional_receipt_v3_consumed_restart_rejects_resealed_stored_subject() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (f, handoff, subject) = setup(&mut budget);
    let receipt = publish_body(&f, &subject, &mut budget).unwrap();
    let lease = f.lease(handoff, &mut budget);
    let current = token(&lease, &mut budget);
    consume_compiler_module_handoff_with_currentness_v5(&lease, current, &mut budget).unwrap();
    let path = f.slot().join(ENTRY);
    let original = fs::read(&path).unwrap();
    let (changed, storage) = changed_subject(&subject, 120, &mut budget);
    let mut wire = original.clone();
    wire[SUBJECT_START..SUBJECT_END].copy_from_slice(changed.canonical_bytes());
    let end = wire.len() - 32;
    let digest = identity(&wire[..end]);
    wire[end..].copy_from_slice(&digest);
    fs::write(&path, wire).unwrap();
    assert!(matches!(
        recover(&f, &subject, &mut budget),
        Err(Error::SubjectBindingMismatch)
    ));
    fs::write(&path, original).unwrap();
    assert_eq!(
        recover(&f, &subject, &mut budget).unwrap().0.receipt(),
        receipt
    );
    drop(changed);
    budget.release_storage(storage).unwrap();
}

#[test]
fn conditional_receipt_v3_limits_conflicts_and_private_file_checks() {
    for length in [1, MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V3] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (f, _, subject) = setup(&mut budget);
        let body = vec![41; length];
        budget.reserve_storage(body.capacity()).unwrap();
        let receipt = publish_compiler_execution_receipt_transport_v3(
            &f.path,
            &f.producer,
            &subject,
            &body,
            &mut budget,
        )
        .unwrap();
        let (owner, storage) = recover(&f, &subject, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert_eq!(owner.exact_bytes(), body);
        assert_eq!(receipt.length(), length);
        assert_eq!(owner.wire.len(), OVERHEAD + length);
    }
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (f, _, subject) = setup(&mut budget);
    for body in [
        vec![],
        vec![0; MAX_COMPILER_EXECUTION_RECEIPT_TRANSPORT_BYTES_V3 + 1],
    ] {
        assert!(matches!(
            publish_compiler_execution_receipt_transport_v3(
                &f.path,
                &f.producer,
                &subject,
                &body,
                &mut budget
            ),
            Err(Error::InvalidReceiptSize { .. })
        ));
        assert!(!f.slot().join(ENTRY).exists());
    }
    publish_body(&f, &subject, &mut budget).unwrap();
    assert!(matches!(
        publish_compiler_execution_receipt_transport_v3(
            &f.path,
            &f.producer,
            &subject,
            b"different",
            &mut budget
        ),
        Err(Error::ConflictingPublication)
    ));
    let path = f.slot().join(ENTRY);
    let original = fs::read(&path).unwrap();
    for mode in 0..5 {
        match mode {
            0 => fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap(),
            1 => fs::hard_link(&path, f.path.join("extra-link")).unwrap(),
            2 => {
                fs::remove_file(&path).unwrap();
                std::os::unix::fs::symlink("missing", &path).unwrap();
            }
            3 => fs::write(&path, []).unwrap(),
            _ => fs::write(
                &path,
                vec![0; MAX_COMPILER_EXECUTION_RECEIPT_ENVELOPE_BYTES_V3 + 1],
            )
            .unwrap(),
        }
        let result = recover(&f, &subject, &mut budget);
        if mode < 3 {
            assert!(matches!(result, Err(Error::SubjectBindingMismatch)));
        } else {
            assert!(matches!(result, Err(Error::InvalidTransportSize { .. })));
        }
        fs::remove_file(&path).unwrap();
        fs::write(&path, &original).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(recover(&f, &subject, &mut budget).is_ok());
    }
}

struct FailAt(FaultPoint);
impl HandoffHooks for FailAt {
    fn hit(&mut self, point: FaultPoint) -> std::io::Result<()> {
        if point == self.0 {
            Err(std::io::Error::other("injected receipt fault"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn conditional_receipt_v3_faults_preserve_commit_and_allow_recovery_or_retry() {
    for point in [
        FaultPoint::PayloadCreated,
        FaultPoint::PayloadWritten,
        FaultPoint::PayloadSynced,
        FaultPoint::RecordRenamed,
        FaultPoint::PublishedSynced,
    ] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (f, _, subject) = setup(&mut budget);
        let floor = budget.storage();
        assert!(
            publish(
                &f.path,
                &f.producer,
                &subject,
                BODY,
                &mut budget,
                &mut FailAt(point)
            )
            .is_err()
        );
        assert_eq!(budget.storage(), floor);
        let committed = matches!(
            point,
            FaultPoint::RecordRenamed | FaultPoint::PublishedSynced
        );
        assert_eq!(f.slot().join(ENTRY).exists(), committed);
        assert_eq!(recover(&f, &subject, &mut budget).is_ok(), committed);
        let receipt = publish_body(&f, &subject, &mut budget).unwrap();
        assert_eq!(
            recover(&f, &subject, &mut budget).unwrap().0.receipt(),
            receipt
        );
        assert_eq!(fs::read_dir(f.slot()).unwrap().count(), 3);
    }
}

#[test]
fn conditional_receipt_v3_underpaid_input_fails_before_filesystem_or_encoding() {
    let mut work = Work::new(usize::MAX);
    let mut setup_budget = Budget::new(&mut work, LIMIT);
    let (f, _, subject) = setup(&mut setup_budget);
    for paid in [0, SUBJECT_STORAGE - 1, SUBJECT_STORAGE + BODY.len() - 1] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(paid).unwrap();
        assert!(matches!(
            publish_body(&f, &subject, &mut budget),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.storage(), paid);
        assert_eq!(budget.work(), 8);
        assert!(!f.slot().join(ENTRY).exists());
    }
}

#[test]
fn conditional_receipt_v3_underpaid_locked_recovery_keeps_ready_token() {
    for gfx950 in [false, true] {
        let mut setup_work = Work::new(usize::MAX);
        let mut setup_budget = Budget::new(&mut setup_work, LIMIT);
        let (f, handoff, subject) = setup_profile(gfx950, &mut setup_budget);
        let receipt = publish_body(&f, &subject, &mut setup_budget).unwrap();
        let lease = f.lease(handoff, &mut setup_budget);
        let current = token(&lease, &mut setup_budget);
        let required = current.storage.0 + lease.storage().0 + SUBJECT_STORAGE;
        let sidecar = std::fs::read(f.slot().join(ENTRY)).unwrap();
        // Isolated admission probes, not a production account transfer.
        for paid in [0, required - 1] {
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(paid).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            assert!(matches!(
                recover_compiler_execution_receipt_transport_with_currentness_v3(
                    &lease,
                    &current,
                    &subject,
                    &mut budget
                ),
                Err(Error::Resource(Resource::Accounting))
            ));
            assert_eq!(budget.work(), 8);
            assert_eq!(budget.storage(), paid);
            assert_eq!(budget.peak_storage(), paid);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(matches!(
                lease.acquire_current_token(&mut setup_budget),
                Err(CompilerModuleHandoffErrorV5::Busy)
            ));
            assert!(f.slot().join(READY_ENTRY).exists());
            assert!(f.slot().join(PAYLOAD_ENTRY).exists());
            assert!(!f.slot().join(CONSUMED_ENTRY).exists());
            assert_eq!(std::fs::read(f.slot().join(ENTRY)).unwrap(), sidecar);
        }
        let (owner, storage) = recover_compiler_execution_receipt_transport_with_currentness_v3(
            &lease,
            &current,
            &subject,
            &mut setup_budget,
        )
        .unwrap();
        check_owner(owner, storage, receipt, &mut setup_budget);
    }
}

#[test]
fn conditional_receipt_v3_exact_and_one_short_pipeline_budgets() {
    fn run(stage: u8, work_limit: usize, storage_limit: usize) -> (Result<()>, usize, usize) {
        let f = Fixture::new();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let floor = f.reserve(&mut budget);
        budget.charge_work(11).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(budget.charge_work(usize::MAX).is_err());
        let result = (|| -> Result<()> {
            let handoff = f.publish(&mut budget)?;
            let (subject, storage) = Subject::from_publication(handoff, &f.handoff, &mut budget)?;
            budget.reserve_storage(storage.retained_storage() + BODY.len())?;
            let retained = budget.storage();
            let receipt = publish_body(&f, &subject, &mut budget)?;
            assert_eq!(budget.storage(), retained);
            if stage == 0 {
                return Ok(());
            }
            assert_eq!(publish_body(&f, &subject, &mut budget)?, receipt);
            if stage == 1 {
                return Ok(());
            }
            let (owner, storage) = recover(&f, &subject, &mut budget)?;
            check_owner(owner, storage, receipt, &mut budget);
            if stage == 2 {
                return Ok(());
            }
            let (lease, storage) = acquire_compiler_module_handoff_currentness_lease_v5(
                &f.path,
                &f.producer,
                handoff,
                &mut budget,
            )?;
            budget.reserve_storage(storage.retained_storage())?;
            let (current, storage) = lease.acquire_current_token(&mut budget)?;
            budget.reserve_storage(storage.retained_storage())?;
            let retained = budget.storage();
            let (owner, storage) =
                recover_compiler_execution_receipt_transport_with_currentness_v3(
                    &lease,
                    &current,
                    &subject,
                    &mut budget,
                )?;
            assert_eq!(budget.storage(), retained);
            check_owner(owner, storage, receipt, &mut budget);
            if stage == 3 {
                return Ok(());
            }
            consume_compiler_module_handoff_with_currentness_v5(&lease, current, &mut budget)?;
            let (owner, storage) = recover(&f, &subject, &mut budget)?;
            check_owner(owner, storage, receipt, &mut budget);
            Ok(())
        })();
        assert!(budget.storage() >= floor);
        if stage == 0 && matches!(&result, Err(Error::Resource(_))) {
            assert!(
                !f.slot().join(ENTRY).exists(),
                "resource refusal after commit"
            );
        }
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
        assert!(budget.work_ledger_identity_v1() == ledger);
        let measured = (result, budget.work(), budget.peak_storage());
        drop(budget);
        assert_eq!(work.failed_work(), Some(usize::MAX));
        measured
    }
    for stage in 0..5 {
        let (result, work, peak) = run(stage, usize::MAX, LIMIT);
        result.unwrap();
        let (result, exact_work, exact_peak) = run(stage, work, peak);
        result.unwrap();
        assert_eq!((exact_work, exact_peak), (work, peak));
        let (short, accepted, _) = run(stage, work - 1, peak);
        assert!(matches!(short, Err(Error::Resource(Resource::Work(_)))));
        if stage == 0 {
            assert_eq!(accepted, work - (3 * (OVERHEAD + BODY.len()) + 65));
        }
        assert!(matches!(
            run(stage, work, peak - 1).0,
            Err(Error::Resource(Resource::Storage(_)))
        ));
    }
}

#[test]
fn conditional_receipt_v3_storage_receipt_includes_spare_capacity() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (_, _, subject) = setup(&mut budget);
    let (owner, storage) = entry(&mut budget, SUBJECT_STORAGE, |r| {
        let wire = encode(&subject, BODY, r)?;
        let mut spare = r.buffer(wire.len() + 137)?;
        spare.extend_from_slice(&wire);
        assert!(spare.capacity() > spare.len());
        recovered(spare, &subject, r)
    })
    .unwrap();
    let receipt = owner.receipt();
    check_owner(owner, storage, receipt, &mut budget);
}

#[test]
fn conditional_receipt_v3_postcommit_record_replacement_is_detected_without_rollback() {
    struct ReplaceReady(PathBuf);
    impl HandoffHooks for ReplaceReady {
        fn hit(&mut self, point: FaultPoint) -> std::io::Result<()> {
            if point == FaultPoint::RecordRenamed {
                let ready = self.0.join(READY_ENTRY);
                let saved = self.0.join("saved-record");
                fs::rename(&ready, &saved)?;
                fs::copy(&saved, &ready)?;
                fs::remove_file(&saved)?;
            }
            Ok(())
        }
    }
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (f, _, subject) = setup(&mut budget);
    let floor = budget.storage();
    let result = publish(
        &f.path,
        &f.producer,
        &subject,
        BODY,
        &mut budget,
        &mut ReplaceReady(f.slot()),
    );
    assert!(matches!(result, Err(Error::Handoff(_))));
    assert_eq!(budget.storage(), floor);
    assert!(f.slot().join(ENTRY).exists());
    assert_eq!(
        recover(&f, &subject, &mut budget).unwrap().0.exact_bytes(),
        BODY
    );
}

#[test]
fn conditional_receipt_v3_independent_layout_and_domains() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (_, _, subject) = setup(&mut budget);
    let mut r = Resources::Metered(&mut budget);
    let wire = encode(&subject, BODY, &mut r).unwrap();
    let mut expected = vec![0; 754 + BODY.len()];
    expected[..8].copy_from_slice(b"F2O3CRT3");
    expected[8..10].copy_from_slice(&3u16.to_le_bytes());
    expected[12..20].copy_from_slice(&(wire.len() as u64).to_le_bytes());
    expected[24..714].copy_from_slice(subject.canonical_bytes());
    expected[714..722].copy_from_slice(&(BODY.len() as u64).to_le_bytes());
    expected[722..wire.len() - 32].copy_from_slice(BODY);
    let end = expected.len() - 32;
    let mut hash = Sha256::new();
    hash.update(b"fe2o3.compiler-execution-receipt-transport.identity.v3\0");
    hash.update((end as u64).to_le_bytes());
    hash.update(&expected[..end]);
    expected[end..].copy_from_slice(&hash.finalize());
    assert_eq!(wire, expected);
    for (magic, version, domain) in [
        (
            *b"F2O3CRT2",
            2u16,
            &b"fe2o3.compiler-execution-receipt-transport.identity.v2\0"[..],
        ),
        (
            *b"F2O3CRT1",
            1u16,
            &b"fe2o3.compiler-execution-receipt-transport.identity.v1\0"[..],
        ),
    ] {
        let mut changed = wire.clone();
        changed[..8].copy_from_slice(&magic);
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        let mut hash = Sha256::new();
        hash.update(domain);
        hash.update((end as u64).to_le_bytes());
        hash.update(&changed[..end]);
        changed[end..].copy_from_slice(&hash.finalize());
        assert!(matches!(
            inspect(&changed, &subject, &mut r),
            Err(Error::InvalidTransport("header"))
        ));
        changed[..8].copy_from_slice(b"F2O3CRT3");
        changed[8..10].copy_from_slice(&3u16.to_le_bytes());
        assert!(inspect(&changed, &subject, &mut r).is_err());
    }
}

#[test]
fn conditional_receipt_v3_recovery_phase_does_not_reopen_publication() {
    for (phase, consumed) in
        (0..4).flat_map(|phase| [false, true].map(|consumed| (phase, consumed)))
    {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (f, handoff, subject) = setup(&mut budget);
        let receipt = publish_body(&f, &subject, &mut budget).unwrap();
        if consumed {
            let lease = f.lease(handoff, &mut budget);
            let current = token(&lease, &mut budget);
            consume_compiler_module_handoff_with_currentness_v5(&lease, current, &mut budget)
                .unwrap();
            assert!(!f.slot().join(PAYLOAD_ENTRY).exists());
        }
        {
            let output = PinnedOutput::open_existing(&f.path).unwrap();
            let _lock = output.lock().unwrap();
            let mut registry = read_attempt_registry(&output).unwrap();
            registry
                .claim_backend(&f.producer.stable_source, f.attempt)
                .unwrap();
            if phase == 1 || phase == 2 {
                registry
                    .record_legacy_backend_receipt(&f.producer.stable_source, f.attempt)
                    .unwrap();
            }
            if phase == 2 {
                registry
                    .mark_completed(&f.producer.stable_source, f.attempt)
                    .unwrap();
            }
            if phase == 3 {
                registry
                    .mark_failed(&f.producer.stable_source, f.attempt)
                    .unwrap();
            }
            crate::commit_attempt_registry_direct(&output, &registry).unwrap();
        }
        assert!(publish_body(&f, &subject, &mut budget).is_err());
        let recovered = recover(&f, &subject, &mut budget);
        if phase == 1 || phase == 2 {
            assert_eq!(recovered.unwrap().0.receipt(), receipt);
        } else {
            assert!(recovered.is_err());
        }
    }
}

#[test]
fn conditional_receipt_v3_rejects_wrong_family_orphan_and_ambiguous_slot() {
    for mode in 0..5 {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (f, _, subject) = setup(&mut budget);
        publish_body(&f, &subject, &mut budget).unwrap();
        match mode {
            0 => fs::rename(
                f.slot().join(ENTRY),
                f.slot().join("compiler-execution-receipt-v1"),
            )
            .unwrap(),
            1 => {
                fs::remove_file(f.slot().join(READY_ENTRY)).unwrap();
                fs::remove_file(f.slot().join(PAYLOAD_ENTRY)).unwrap();
            }
            2 => {
                fs::copy(f.slot().join(READY_ENTRY), f.slot().join(CONSUMED_ENTRY)).unwrap();
            }
            3 => fs::rename(
                f.slot().join(ENTRY),
                f.slot().join("compiler-execution-receipt-v2"),
            )
            .unwrap(),
            _ => {
                crate::begin_build_attempt(
                    &f.path,
                    &f.producer,
                    crate::BuildInvocation::from_bytes([8; 32]),
                    BuildSession::from_bytes([9; 16]),
                )
                .unwrap();
            }
        }
        assert!(recover(&f, &subject, &mut budget).is_err());
        assert!(publish_body(&f, &subject, &mut budget).is_err());
    }
}

#[test]
fn conditional_receipt_v3_payload_custody_spans_commit() {
    struct ChangePayload {
        slot: PathBuf,
        at: FaultPoint,
        replace: bool,
    }
    impl HandoffHooks for ChangePayload {
        fn hit(&mut self, point: FaultPoint) -> std::io::Result<()> {
            if point == self.at {
                let path = self.slot.join(PAYLOAD_ENTRY);
                if self.replace {
                    let replacement = self.slot.join("replacement-module");
                    fs::copy(&path, &replacement)?;
                    fs::rename(replacement, path)?;
                } else {
                    let mut bytes = fs::read(&path)?;
                    bytes[0] ^= 1;
                    fs::write(path, bytes)?;
                }
            }
            Ok(())
        }
    }
    for (at, replace) in [FaultPoint::PayloadSynced, FaultPoint::RecordRenamed]
        .into_iter()
        .flat_map(|at| [false, true].map(|replace| (at, replace)))
    {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (f, _, subject) = setup(&mut budget);
        let floor = budget.storage();
        let mut hook = ChangePayload {
            slot: f.slot(),
            at,
            replace,
        };
        let result = publish(&f.path, &f.producer, &subject, BODY, &mut budget, &mut hook);
        assert!(
            matches!(&result, Err(Error::Handoff(_))),
            "{at:?}/{replace}: {result:?}"
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            f.slot().join(ENTRY).exists(),
            at == FaultPoint::RecordRenamed
        );
        assert!(recover(&f, &subject, &mut budget).is_err());
    }
}

#[test]
fn conditional_receipt_v3_isolated_operation_budgets_restore_exact_floor() {
    fn run(stage: u8, work_limit: usize, storage_limit: usize) -> (Result<()>, usize, usize) {
        let mut setup_work = Work::new(usize::MAX);
        let mut setup_budget = Budget::new(&mut setup_work, LIMIT);
        let (f, handoff, subject) = setup(&mut setup_budget);
        if stage != 0 {
            publish_body(&f, &subject, &mut setup_budget).unwrap();
        }
        let lease = if stage >= 3 {
            Some(f.lease(handoff, &mut setup_budget))
        } else {
            None
        };
        let current = if stage == 3 {
            Some(token(lease.as_ref().unwrap(), &mut setup_budget))
        } else {
            None
        };
        if stage == 4 {
            let lease = lease.as_ref().unwrap();
            let current = token(lease, &mut setup_budget);
            consume_compiler_module_handoff_with_currentness_v5(lease, current, &mut setup_budget)
                .unwrap();
        }
        // Component-only accounting probe, not transfer of fresh production custody.
        // Isolate this call's peak; prepay all retained fixture inputs anew.
        let floor = setup_budget.storage();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = if stage < 2 {
            publish_body(&f, &subject, &mut budget).map(|_| ())
        } else {
            let result = if stage == 3 {
                recover_compiler_execution_receipt_transport_with_currentness_v3(
                    lease.as_ref().unwrap(),
                    current.as_ref().unwrap(),
                    &subject,
                    &mut budget,
                )
            } else {
                recover(&f, &subject, &mut budget)
            };
            result.map(|(owner, _storage)| drop(owner))
        };
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        if stage == 0 && matches!(&result, Err(Error::Resource(_))) {
            assert!(!f.slot().join(ENTRY).exists());
        }
        (result, budget.work(), budget.peak_storage())
    }
    for stage in 0..5 {
        let (result, work, peak) = run(stage, usize::MAX, LIMIT);
        result.unwrap();
        let (result, exact_work, exact_peak) = run(stage, work, peak);
        result.unwrap();
        assert_eq!((exact_work, exact_peak), (work, peak));
        assert!(matches!(
            run(stage, work - 1, peak).0,
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert!(matches!(
            run(stage, work, peak - 1).0,
            Err(Error::Resource(Resource::Storage(_)))
        ));
    }
}

#[test]
fn conditional_receipt_v3_final_ready_recovery_checks_content_not_only_metadata() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (f, handoff, subject) = setup(&mut budget);
    publish_body(&f, &subject, &mut budget).unwrap();
    let lease = f.lease(handoff, &mut budget);
    let _lock = lease.binding.output.lock().unwrap();
    let path = f.slot().join(PAYLOAD_ENTRY);
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 1;
    fs::write(&path, bytes).unwrap();
    let ready = f.slot().join(READY_ENTRY);
    let mut record = HandoffRecord::<Schema>::decode(&fs::read(&ready).unwrap()).unwrap();
    // Model matching metadata without relying on filesystem timestamp granularity.
    record.file = FileIdentity::from_stat(&fstat(&fs::File::open(&path).unwrap()).unwrap());
    fs::write(ready, record.encode()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        shared::validate(
            &lease.binding.output,
            &f.producer,
            &subject,
            &lease.binding.slot_directory,
            true,
            false,
            &mut Resources::Metered(&mut budget)
        ),
        Err(SharedFailure::Handoff(HandoffEngineError::Common(
            CompilerModuleHandoffErrorV1::DigestMismatch
        )))
    ));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn conditional_receipt_v3_locked_rejects_foreign_occurrence_without_releasing_token() {
    for gfx950 in [false, true] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (f, receipt, subject) = setup_profile(gfx950, &mut budget);
        publish_body(&f, &subject, &mut budget).unwrap();
        let lease = f.lease(receipt, &mut budget);
        let current = token(&lease, &mut budget);
        for changed_transaction in [false, true] {
            let attempt = receipt.attempt();
            let (attempt, transaction) = if changed_transaction {
                let mut digest = *receipt.transaction_identity().as_bytes();
                digest[0] ^= 1;
                (
                    attempt,
                    CompilerModuleHandoffTransactionIdentityV5::from_bytes(digest),
                )
            } else {
                (
                    BuildAttempt::new(
                        attempt.generation() + 1,
                        attempt.session(),
                        attempt.invocation(),
                    )
                    .unwrap(),
                    receipt.transaction_identity(),
                )
            };
            let (changed, paid) = Subject::from_replay_evidence(
                attempt,
                receipt.slot(),
                transaction,
                &f.handoff,
                &mut budget,
            )
            .unwrap();
            budget.reserve_storage(paid.retained_storage()).unwrap();
            let floor = budget.storage();
            let before = budget.work();
            let ledger = budget.work_ledger_identity_v1();
            assert!(matches!(
                recover_compiler_execution_receipt_transport_with_currentness_v3(
                    &lease,
                    &current,
                    &changed,
                    &mut budget
                ),
                Err(Error::SubjectBindingMismatch)
            ));
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > before);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(matches!(
                lease.acquire_current_token(&mut budget),
                Err(CompilerModuleHandoffErrorV5::Busy)
            ));
            assert!(f.slot().join(READY_ENTRY).exists());
            assert!(!f.slot().join(CONSUMED_ENTRY).exists());
            drop(changed);
            budget.release_storage(paid.retained_storage()).unwrap();
        }
        let (owner, paid) = recover_compiler_execution_receipt_transport_with_currentness_v3(
            &lease,
            &current,
            &subject,
            &mut budget,
        )
        .unwrap();
        let recovered = owner.receipt();
        check_owner(owner, paid, recovered, &mut budget);
    }
}

#[test]
fn conditional_receipt_v3_unwind_preserves_floor_denials_and_committed_state() {
    struct PanicAt(FaultPoint);
    impl HandoffHooks for PanicAt {
        fn hit(&mut self, point: FaultPoint) -> std::io::Result<()> {
            assert_ne!(point, self.0, "injected sidecar unwind");
            Ok(())
        }
    }
    for point in [FaultPoint::PayloadSynced, FaultPoint::RecordRenamed] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (f, _, subject) = setup(&mut budget);
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(budget.charge_work(usize::MAX).is_err());
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            publish(
                &f.path,
                &f.producer,
                &subject,
                BODY,
                &mut budget,
                &mut PanicAt(point),
            )
        }));
        assert!(result.is_err());
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
        let committed = point == FaultPoint::RecordRenamed;
        assert_eq!(f.slot().join(ENTRY).exists(), committed);
        assert_eq!(recover(&f, &subject, &mut budget).is_ok(), committed);
        let receipt = publish_body(&f, &subject, &mut budget).unwrap();
        let (owner, paid) = recover(&f, &subject, &mut budget).unwrap();
        check_owner(owner, paid, receipt, &mut budget);
        assert_eq!(budget.storage(), floor);
        drop(budget);
        assert_eq!(work.failed_work(), Some(usize::MAX));
    }
}
