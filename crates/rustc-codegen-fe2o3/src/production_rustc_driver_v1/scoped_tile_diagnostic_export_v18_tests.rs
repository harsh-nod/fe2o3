use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::{
    fs,
    panic::{AssertUnwindSafe, catch_unwind},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT_TEST: AtomicU64 = AtomicU64::new(0);
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-v18-export-test-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn output(&self) -> PathBuf {
        self.0.join("result.kir")
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn observation() -> ExportObservation {
    ExportObservation {
        source: [1; 32],
        pending: [2; 32],
        scalar: [3; 32],
        schedule: [4; 32],
        raw_sha256: [5; 32],
        bytes: 8,
    }
}
fn terminal() -> Result<(), Box<PipelineError>> {
    Err(Box::new(PipelineError::ScopedTileObservationIncomplete))
}

#[test]
fn exact_one_completed_candidate_and_only_expected_terminal_refusal_are_accepted() {
    let report = require_completed_observation(1, Some(observation()), None, terminal()).unwrap();
    assert_eq!(report.scalar, [3; 32]);
    for calls in [0, 2] {
        assert!(
            require_completed_observation(calls, Some(observation()), None, terminal()).is_err()
        );
    }
    assert!(require_completed_observation(1, None, None, terminal()).is_err());
    assert!(
        require_completed_observation(
            1,
            Some(observation()),
            Some("write failed".into()),
            terminal()
        )
        .is_err()
    );
    assert!(require_completed_observation(1, Some(observation()), None, Ok(())).is_err());
    assert!(
        require_completed_observation(
            1,
            Some(observation()),
            None,
            Err(Box::new(PipelineError::ExtractionCannotPublish))
        )
        .is_err()
    );
}

#[test]
fn zero_callback_failures_preserve_source_and_resource_causes() {
    for error in [
        PipelineError::EmptyCollectedDeviceClosure,
        resource(Resource::Arithmetic),
    ] {
        let cause = error.to_string();
        let actual =
            require_completed_observation(0, None, None, Err(Box::new(error))).unwrap_err();
        assert_eq!(
            actual,
            format!("diagnostic V18 source observation failed (0 candidate callback(s)): {cause}")
        );
    }
}

#[test]
fn causal_diagnostics_never_turn_nonterminal_failures_into_completed_exports() {
    for calls in [0, 1, 2] {
        for staged in [false, true] {
            let error = PipelineError::ExtractionCannotPublish;
            let cause = error.to_string();
            assert_eq!(
                require_completed_observation(
                    calls,
                    staged.then(observation),
                    None,
                    Err(Box::new(error)),
                )
                .unwrap_err(),
                format!(
                    "diagnostic V18 source observation failed ({calls} candidate callback(s)): {cause}"
                )
            );
            assert!(
                require_completed_observation(calls, staged.then(observation), None, Ok(()))
                    .is_err()
            );
            assert_eq!(
                require_completed_observation(
                    calls,
                    staged.then(observation),
                    Some("original staging failure".into()),
                    Err(Box::new(PipelineError::ExtractionCannotPublish)),
                )
                .unwrap_err(),
                "original staging failure"
            );
        }
    }
}

#[test]
fn canonical_overflow_mode_is_checked_before_creating_staging_or_output() {
    let root = Root::new();
    assert!(
        run_diagnostic_scoped_tile_kir_extraction_driver_v18(
            &["rustc".into()],
            &root.output(),
            Order::Blocked
        )
        .unwrap_err()
        .contains("overflow-checks")
    );
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
}

#[test]
fn private_stage_promotes_exact_bytes_only_to_a_fresh_destination() {
    let root = Root::new();
    let output = root.output();
    let mut stage = StagedOutput::new(&output).unwrap();
    let directory = stage.test_paths().0.to_owned();
    assert_eq!(
        fs::metadata(&directory).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let bytes = b"exact diagnostic bytes";
    stage.write(bytes).unwrap();
    assert!(!output.exists());
    assert!(stage.write(bytes).is_err());
    assert_eq!(
        fs::metadata(stage.test_paths().1)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    stage
        .promote(bytes.len() as u64, &Sha256::digest(bytes).into())
        .unwrap();
    assert_eq!(fs::read(&output).unwrap(), bytes);
    assert!(
        stage
            .promote(bytes.len() as u64, &Sha256::digest(bytes).into())
            .is_err()
    );
    drop(stage);
    assert!(!directory.exists());
    assert_eq!(fs::read(&output).unwrap(), bytes);
    assert!(StagedOutput::new(&output).is_err());
}

#[test]
fn actual_stage_scan_prepayment_preserves_original_work_floor_and_denials() {
    let bytes = b"nonzero exact scan";
    let scratch = std::mem::size_of::<ExportObservation>() + std::mem::size_of::<Sha256>();
    for (work_limit, storage_limit, accepted) in [
        (bytes.len() * 2, 97 + scratch, true),
        (bytes.len() * 2 - 1, 97 + scratch, false),
        (bytes.len() * 2, 97 + scratch - 1, false),
    ] {
        let root = Root::new();
        let mut stage = StagedOutput::new(&root.output()).unwrap();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(97).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = stage_bytes(bytes, &mut budget, &mut stage);
        assert_eq!(result.is_ok(), accepted);
        assert_eq!(budget.storage(), 97);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(!root.output().exists());
        if accepted {
            assert_eq!(
                result.unwrap().unwrap(),
                <[u8; 32]>::from(Sha256::digest(bytes))
            );
            assert_eq!(budget.work(), bytes.len() * 2);
        } else {
            assert!(budget.failed_work().is_some() || budget.failed_storage().is_some());
            assert!(!stage.test_paths().1.exists());
        }
    }
}

#[test]
fn malformed_or_changed_staged_bytes_are_never_promoted() {
    for tamper in [false, true] {
        let root = Root::new();
        let mut stage = StagedOutput::new(&root.output()).unwrap();
        assert!(stage.write(&[]).is_err());
        stage.write(b"payload").unwrap();
        if tamper {
            fs::write(stage.test_paths().1, b"changed").unwrap();
        }
        let digest = if tamper {
            Sha256::digest(b"payload").into()
        } else {
            [0; 32]
        };
        assert!(stage.promote(7, &digest).is_err());
        assert!(!root.output().exists());
    }
}

#[test]
fn existing_final_files_and_links_are_preserved_without_clobber() {
    let root = Root::new();
    let output = root.output();
    let victim = root.0.join("victim");
    fs::write(&victim, b"preserved").unwrap();
    symlink(&victim, &output).unwrap();
    assert!(StagedOutput::new(&output).is_err());
    fs::remove_file(&output).unwrap();
    let mut stage = StagedOutput::new(&output).unwrap();
    stage.write(b"payload").unwrap();
    fs::write(&output, b"other owner").unwrap();
    assert!(
        stage
            .promote(7, &Sha256::digest(b"payload").into())
            .is_err()
    );
    drop(stage);
    assert_eq!(fs::read(&victim).unwrap(), b"preserved");
    assert_eq!(fs::read(&output).unwrap(), b"other owner");
}

#[test]
fn callback_error_and_unwind_remove_only_owned_private_stage() {
    let root = Root::new();
    let output = root.output();
    let directory = {
        let mut stage = StagedOutput::new(&output).unwrap();
        stage.write(b"payload").unwrap();
        let directory = stage.test_paths().0.to_owned();
        assert!(
            require_completed_observation(
                1,
                Some(observation()),
                None,
                Err(Box::new(PipelineError::ExtractionCannotPublish))
            )
            .is_err()
        );
        directory
    };
    assert!(!directory.exists() && !output.exists());
    let mut stage = StagedOutput::new(&output).unwrap();
    stage.write(b"payload").unwrap();
    let directory = stage.test_paths().0.to_owned();
    assert!(
        catch_unwind(AssertUnwindSafe(move || {
            let _stage = stage;
            panic!("injected source observation panic");
        }))
        .is_err()
    );
    assert!(!directory.exists() && !output.exists());
}

#[test]
fn replaced_staged_entry_is_neither_promoted_nor_deleted_by_cleanup() {
    let root = Root::new();
    let mut stage = StagedOutput::new(&root.output()).unwrap();
    stage.write(b"payload").unwrap();
    let path = stage.test_paths().1.to_owned();
    let original = path.with_file_name("original");
    fs::rename(&path, &original).unwrap();
    fs::write(&path, b"foreign").unwrap();
    assert!(
        stage
            .promote(7, &Sha256::digest(b"payload").into())
            .is_err()
    );
    drop(stage);
    assert_eq!(fs::read(&path).unwrap(), b"foreign");
    assert_eq!(fs::read(&original).unwrap(), b"payload");
    assert!(!root.output().exists());
}
