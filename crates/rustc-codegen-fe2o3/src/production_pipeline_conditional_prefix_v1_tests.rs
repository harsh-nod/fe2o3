//! Account controls are inert. Source tests require original compiler captures
//! and genuine admitted proof execution; they never manufacture a receipt.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::{Digest, Sha256};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

#[path = "production_pipeline_conditional_final_results_v1.rs"]
mod results;
#[path = "production_pipeline_conditional_final_results_v1_tests.rs"]
mod results_tests;

static OBSERVING: AtomicBool = AtomicBool::new(false);
static AGREEMENTS: AtomicUsize = AtomicUsize::new(0);
static INSTALLED: AtomicUsize = AtomicUsize::new(0);
static REPLAY_COMPLETED: AtomicUsize = AtomicUsize::new(0);
static LATE_MODE: AtomicUsize = AtomicUsize::new(0);
static FAULT_OBSERVED: AtomicBool = AtomicBool::new(false);
static FINAL_IDENTITY: Mutex<Option<results::Identity>> = Mutex::new(None);
static TARGET: AtomicUsize = AtomicUsize::new(0);
const ARGS: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_ARGS";
const ARGS_SHA256: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_ARGS_SHA256";
const PROFILE: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_TARGET";
const MODE: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_MODE";
const RESULT: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_RESULT";

fn identity(output: &Owner) -> results::Identity {
    let identity = output.canonical().identity();
    results::Identity {
        sha256: crate::encode_hex(identity.digest()),
        canonical_length: identity.canonical_length(),
    }
}

pub(super) fn agreement_checked(profile: Profile, output: &Owner) -> Result<(), Error> {
    if OBSERVING.load(Ordering::SeqCst) {
        let target = match profile {
            Profile::Gfx942 => 942,
            Profile::Gfx950 => 950,
        };
        assert_eq!(target, TARGET.load(Ordering::SeqCst));
        AGREEMENTS.fetch_add(1, Ordering::SeqCst);
        *FINAL_IDENTITY.lock().unwrap() = Some(identity(output));
        if LATE_MODE.load(Ordering::SeqCst) == 1 {
            FAULT_OBSERVED.store(true, Ordering::SeqCst);
            return Err(Error::Resource(Resource::Accounting));
        }
    }
    Ok(())
}

pub(super) fn source_replayed() -> Result<(), ProductionPipelineError> {
    if OBSERVING.load(Ordering::SeqCst) {
        REPLAY_COMPLETED.fetch_add(1, Ordering::SeqCst);
        if LATE_MODE.load(Ordering::SeqCst) == 2 {
            FAULT_OBSERVED.store(true, Ordering::SeqCst);
            return Err(resource(Resource::Accounting));
        }
    }
    Ok(())
}

pub(super) fn installed(value: &ConditionalPrefixForFV1) {
    if OBSERVING.load(Ordering::SeqCst) {
        assert_eq!(AGREEMENTS.load(Ordering::SeqCst), 1);
        assert_eq!(REPLAY_COMPLETED.load(Ordering::SeqCst), 1);
        assert!(value.preparation.ranked.has_conditional_roots_v1());
        assert!(
            !value
                .preparation
                .ranked
                .grants_artifact_or_launch_authority()
        );
        assert_eq!(
            *FINAL_IDENTITY.lock().unwrap(),
            Some(identity(value.chain.output()))
        );
        INSTALLED.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn conditional_target_scope_restores_scratch_without_refunding_work() {
    for fail in [false, true] {
        let mut work = Work::new(100);
        let mut target = Budget::new(&mut work, 100);
        target.reserve_storage(17).unwrap();
        target.charge_work(3).unwrap();
        let account = target.work_ledger_identity_v1();
        let result = scoped_target(&mut target, |target| {
            target.charge_work(5)?;
            target.reserve_storage(23)?;
            if fail {
                Err(Error::Resource(Resource::Accounting))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.is_err(), fail);
        assert_eq!(target.storage(), 17);
        assert_eq!(target.work(), 8);
        assert!(target.work_ledger_identity_v1() == account);
    }
}

#[test]
fn conditional_target_scope_restores_on_exhaustion_and_unwind() {
    for mode in ["work", "storage", "panic"] {
        let mut work = Work::new(10);
        let mut target = Budget::new(&mut work, 100);
        target.reserve_storage(17).unwrap();
        let account = target.work_ledger_identity_v1();
        let result = catch_unwind(AssertUnwindSafe(|| {
            scoped_target(&mut target, |target| {
                target.charge_work(7)?;
                target.reserve_storage(23)?;
                match mode {
                    "work" => target.charge_work(4)?,
                    "storage" => target.reserve_storage(61)?,
                    _ => panic!("conditional target cleanup control"),
                }
                Ok(())
            })
        }));
        match result {
            Ok(result) => assert!(result.is_err()),
            Err(_) => assert_eq!(mode, "panic"),
        }
        assert_eq!(target.storage(), 17);
        assert!(target.work() >= 7);
        assert!(target.work_ledger_identity_v1() == account);
    }
}

#[test]
fn conditional_target_scope_refuses_releasing_callers_floor() {
    let mut work = Work::new(100);
    let mut target = Budget::new(&mut work, 100);
    target.reserve_storage(17).unwrap();
    assert!(matches!(
        scoped_target(&mut target, |target| {
            target.release_storage(1)?;
            Ok(())
        }),
        Err(Error::Resource(Resource::Accounting))
    ));
}

#[test]
fn conditional_target_scope_refuses_replacing_original_account() {
    let mut original = Work::new(100);
    let mut replacement = Work::new(100);
    let mut target = Budget::new(&mut original, 100);
    target.reserve_storage(17).unwrap();
    assert!(matches!(
        scoped_target(&mut target, |target| {
            *target = Budget::new(&mut replacement, 100);
            target.reserve_storage(17)?;
            Ok(())
        }),
        Err(Error::Resource(Resource::Accounting))
    ));
}

struct Callbacks {
    calls: usize,
    mode: String,
    result: Option<results::Child>,
}
impl rustc_driver::Callbacks for Callbacks {
    fn after_analysis<'tcx>(
        &mut self,
        _: &rustc_interface::interface::Compiler,
        tcx: rustc_middle::ty::TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        self.calls += 1;
        assert_eq!(self.calls, 1);
        // Collect from the live rustc session through the existing authenticated
        // constructors. No owner, binding, formal or proof receipt is supplied.
        let target = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let expected = match TARGET.load(Ordering::SeqCst) {
            942 => Profile::Gfx942,
            950 => Profile::Gfx950,
            value => panic!("unexpected target {value}"),
        };
        assert_eq!(target.canonical_name(), expected.device_target());
        let producers = crate::collector::capture_context_producers_v1(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        assert!(crate::collector::count_kernels_in_cgus(tcx, partitions.codegen_units) > 0);
        crate::production_pipeline::reject_custom_llvm_configuration(
            crate::has_custom_llvm_configuration(tcx.sess),
        )
        .unwrap();
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
            producers,
        )
        .unwrap();
        let crate_name = tcx.crate_name(rustc_hir::def_id::LOCAL_CRATE);
        let source = tcx
            .sess
            .local_crate_source_file()
            .and_then(|source| source.local_path().map(std::path::PathBuf::from));
        let producer = crate::artifact_transaction::ProducerIdentity::from_codegen(
            crate_name.as_str(),
            source.as_deref(),
        )
        .unwrap();
        let ranked = crate::production_pipeline::ProductionCompilation::from_collected_device_closure_for_extraction(
            tcx, closure, producer, std::env::current_dir().unwrap(),
        ).unwrap().verify_general_kernel_checks().unwrap();
        assert!(ranked.ranked.has_conditional_roots_v1());
        assert_eq!(
            ranked.ranked.materialized().helper_source_policy_v1(),
            ProductionHelperSourcePolicyV1::RawEmpty
        );
        let work_limit =
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap();
        let storage_limit = crate::production_canonical_phase_policy_v1::STORAGE_LIMIT;
        let mut work = Work::new(if self.mode == "work" { 0 } else { work_limit });
        let mut budget = Budget::new(
            &mut work,
            if self.mode == "storage" {
                19
            } else {
                storage_limit
            },
        );
        budget.reserve_storage(19).unwrap();
        let account = budget.work_ledger_identity_v1();
        let address = std::ptr::from_mut(&mut budget);
        OBSERVING.store(true, Ordering::SeqCst);
        let error = if self.mode == "fixed6" {
            // Same consuming preparation as the fixed endpoint, with a caller
            // scope solely so this test can inspect reservation cleanup.
            let result = scoped(19, &mut budget, |budget| {
                ranked.prepare_admitted_policy6_v1(budget)
            });
            result
                .err()
                .expect("conditional fixed6 must retain its original refusal")
        } else {
            let result = ranked.lower_refined_cross_block_forwarding_native_with_budget_v1(
                fe2o3_kernel_analysis::CanonicalKirLoopLimitsV1::default(),
                fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingLimitsV1::default(),
                &mut budget,
            );
            result
                .err()
                .expect("conditional F prefix must not produce native output")
        };
        OBSERVING.store(false, Ordering::SeqCst);
        assert_eq!(std::ptr::from_mut(&mut budget), address);
        assert!(budget.work_ledger_identity_v1() == account);
        assert_eq!(budget.storage(), 19);
        let (terminal, resource_kind) = if matches!(self.mode.as_str(), "f" | "fixed6") {
            assert!(
                matches!(
                    error,
                    ProductionPipelineError::RankedVerification(
                        RankedError::ConditionalFinalizerRequired { .. }
                    )
                ),
                "{error}"
            );
            assert!(budget.work() > 0);
            ("conditional-finalizer-required", None)
        } else if self.mode.starts_with("late-") {
            assert!(FAULT_OBSERVED.load(Ordering::SeqCst));
            (
                if self.mode == "late-agreement" {
                    "injected-after-agreement"
                } else {
                    "injected-after-replay"
                },
                None,
            )
        } else {
            use super::super::CheckedOutputPolicy6StageErrorV1 as StageError;
            use fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12 as CanonicalError;
            let refused = match &error {
                ProductionPipelineError::CheckedOutputPolicy6Stage(StageError::Resource(error))
                | ProductionPipelineError::CheckedOutputPolicy6Stage(StageError::Canonical(
                    CanonicalError::Resource(error),
                )) => error,
                _ => panic!("expected direct early resource refusal: {error}"),
            };
            let kind = match refused {
                Resource::Work(_) => "work",
                Resource::Storage(_) => "storage",
                _ => panic!("unexpected resource failure: {refused}"),
            };
            assert_eq!(kind, self.mode);
            ("resource-refusal", Some(kind.to_owned()))
        };
        let record = results::Child {
            schema: results::CHILD_SCHEMA.into(),
            target: expected.device_target().into(),
            mode: self.mode.clone(),
            callbacks: self.calls as u64,
            agreements: AGREEMENTS.load(Ordering::SeqCst) as u64,
            replay_completed: REPLAY_COMPLETED.load(Ordering::SeqCst) as u64,
            installed: INSTALLED.load(Ordering::SeqCst) as u64,
            actual_f_identity: FINAL_IDENTITY.lock().unwrap().clone(),
            terminal: terminal.into(),
            refusal: error.to_string(),
            resource: resource_kind,
            fault_observed: FAULT_OBSERVED.load(Ordering::SeqCst),
            work: budget.work() as u64,
            floor_before: 19,
            floor_after: budget.storage() as u64,
            account_preserved: budget.work_ledger_identity_v1() == account
                && std::ptr::from_mut(&mut budget) == address,
            source_to_final_output_checked: AGREEMENTS.load(Ordering::SeqCst) == 1,
            qualification_credit: false,
            grants_artifact_or_launch_authority: false,
            native_output_emitted: false,
        };
        record.check(expected.device_target(), &self.mode).unwrap();
        self.result = Some(record);
        rustc_driver::Compilation::Stop
    }
}

#[test]
#[ignore = "child only: genuine captured source and admitted proof runtime"]
fn genuine_conditional_f_prefix_child() {
    use std::{
        io::{Read, Write},
        os::unix::fs::OpenOptionsExt,
    };
    let mut bytes = Vec::new();
    std::fs::File::open(std::env::var_os(ARGS).unwrap())
        .unwrap()
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= 1_048_576);
    assert_eq!(
        crate::encode_hex(&Sha256::digest(&bytes)),
        std::env::var(ARGS_SHA256).unwrap()
    );
    let args: Vec<String> = serde_json::from_slice(&bytes).unwrap();
    let target: usize = std::env::var(PROFILE).unwrap().parse().unwrap();
    assert!(matches!(target, 942 | 950));
    TARGET.store(target, Ordering::SeqCst);
    let mode = std::env::var(MODE).unwrap();
    assert!(
        results::MAIN_MODES.contains(&mode.as_str())
            || results::LATE_MODES.contains(&mode.as_str())
    );
    LATE_MODE.store(
        match mode.as_str() {
            "late-agreement" => 1,
            "late-replay" => 2,
            _ => 0,
        },
        Ordering::SeqCst,
    );
    let mut callbacks = Callbacks {
        calls: 0,
        mode,
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert_eq!(callbacks.calls, 1);
    let bytes = serde_json::to_vec(&callbacks.result.expect("genuine child completion")).unwrap();
    assert!(bytes.len() <= 65_536);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(std::env::var_os(RESULT).unwrap())
        .unwrap()
        .write_all(&bytes)
        .unwrap();
}
