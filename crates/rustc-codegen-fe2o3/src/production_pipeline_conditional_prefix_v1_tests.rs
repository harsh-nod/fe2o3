//! Account controls are inert. Source tests require original compiler captures
//! and genuine admitted proof execution; they never manufacture a receipt.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static OBSERVING: AtomicBool = AtomicBool::new(false);
static AGREEMENTS: AtomicUsize = AtomicUsize::new(0);
static INSTALLED: AtomicUsize = AtomicUsize::new(0);
static TARGET: AtomicUsize = AtomicUsize::new(0);
const CHILD: &str = "production_pipeline::checked_output_policy6_v1::conditional_prefix_v1::tests::genuine_conditional_f_prefix_child";
const ARGS: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_ARGS";
const PROFILE: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_TARGET";
const MODE: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_MODE";

pub(super) fn agreement_checked(profile: Profile) {
    if OBSERVING.load(Ordering::SeqCst) {
        let target = match profile {
            Profile::Gfx942 => 942,
            Profile::Gfx950 => 950,
        };
        assert_eq!(target, TARGET.load(Ordering::SeqCst));
        AGREEMENTS.fetch_add(1, Ordering::SeqCst);
    }
}

pub(super) fn installed(value: &ConditionalPrefixForFV1) {
    if OBSERVING.load(Ordering::SeqCst) {
        assert_eq!(AGREEMENTS.load(Ordering::SeqCst), 1);
        assert!(value.preparation.ranked.has_conditional_roots_v1());
        assert!(
            !value
                .preparation
                .ranked
                .grants_artifact_or_launch_authority()
        );
        assert!(!value.tail.grants_authority());
        assert_eq!(
            value.tail.input_identity(),
            value.preparation.checked.owner().canonical().identity()
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

/// Same captured Direct one-root source as the existing fixed policy6 tests.
/// Parent and children perform no Cargo, runtime setup or native publication.
#[test]
#[ignore = "requires genuine FE2O3_CONDITIONAL_PREFIX_GFX942_ARGS and GFX950_ARGS captures plus admitted proof runtime"]
fn genuine_direct_conditional_f_prefix_both_targets_and_fixed6_separation() {
    for target in [942, 950] {
        let variable = format!("FE2O3_CONDITIONAL_PREFIX_GFX{target}_ARGS");
        let args = std::env::var_os(&variable).unwrap_or_else(|| panic!("missing {variable}"));
        for mode in ["f", "fixed6", "work", "storage"] {
            let result = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    CHILD,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(ARGS, &args)
                .env(PROFILE, target.to_string())
                .env(MODE, mode)
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "target {target}, mode {mode}:\n{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            let stdout = String::from_utf8(result.stdout).unwrap();
            assert!(stdout.contains(&format!("test {CHILD} ... ok")));
            assert!(stdout.contains("1 passed; 0 failed; 0 ignored;"));
        }
    }
}

struct Callbacks {
    calls: usize,
    mode: String,
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
        let expected = match target.profile() {
            Profile::Gfx942 => 942,
            Profile::Gfx950 => 950,
        };
        assert_eq!(expected, TARGET.load(Ordering::SeqCst));
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
        let expected = usize::from(self.mode == "f");
        assert_eq!(AGREEMENTS.load(Ordering::SeqCst), expected);
        assert_eq!(INSTALLED.load(Ordering::SeqCst), expected);
        if matches!(self.mode.as_str(), "f" | "fixed6") {
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
        } else {
            assert!(
                !matches!(
                    error,
                    ProductionPipelineError::RankedVerification(
                        RankedError::ConditionalFinalizerRequired { .. }
                    )
                ),
                "{error}"
            );
        }
        rustc_driver::Compilation::Stop
    }
}

#[test]
#[ignore = "child only: genuine captured source and admitted proof runtime"]
fn genuine_conditional_f_prefix_child() {
    let args: Vec<String> =
        serde_json::from_slice(&std::fs::read(std::env::var_os(ARGS).unwrap()).unwrap()).unwrap();
    let target: usize = std::env::var(PROFILE).unwrap().parse().unwrap();
    assert!(matches!(target, 942 | 950));
    TARGET.store(target, Ordering::SeqCst);
    let mode = std::env::var(MODE).unwrap();
    assert!(matches!(mode.as_str(), "f" | "fixed6" | "work" | "storage"));
    let mut callbacks = Callbacks { calls: 0, mode };
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert_eq!(callbacks.calls, 1);
}
