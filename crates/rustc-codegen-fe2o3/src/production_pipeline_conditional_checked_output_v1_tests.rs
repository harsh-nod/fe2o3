//! Opt-in genuine compiler/proof callback coverage, never a manufactured receipt.
use super::*;
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

static OBSERVING: AtomicBool = AtomicBool::new(false);
static AGREEMENTS: AtomicUsize = AtomicUsize::new(0);
static REPLAYS: AtomicUsize = AtomicUsize::new(0);
static TARGET: AtomicUsize = AtomicUsize::new(0);
const CHILD: &str = "production_pipeline::conditional_generated_fields_v1::checked_output::tests::genuine_conditional_prefix_child";
const ARGS: &str = "FE2O3_CONDITIONAL_PREFIX_CHILD_ARGS";
const PROFILE: &str = "FE2O3_CONDITIONAL_PREFIX_CHILD_TARGET";
const OUTPUT: &str = "FE2O3_CONDITIONAL_PREFIX_CHILD_OUTPUT";

pub(crate) fn agreement_checked(profile: Profile) {
    if OBSERVING.load(Ordering::SeqCst) {
        let observed = match profile {
            Profile::Gfx942 => 942,
            Profile::Gfx950 => 950,
        };
        assert_eq!(observed, TARGET.load(Ordering::SeqCst));
        AGREEMENTS.fetch_add(1, Ordering::SeqCst);
    }
}

pub(crate) fn replay_completed() {
    if OBSERVING.load(Ordering::SeqCst) {
        REPLAYS.fetch_add(1, Ordering::SeqCst);
    }
}

/// Each input file is a JSON Vec<String> containing the actual pinned rustc
/// argv for a one-root Direct conditional source, including its exact dependency
/// paths. The genuine protected proof runtime must already be available. These
/// tests do not prepare a runtime, invoke Cargo or reconstruct a source owner.
#[test]
#[ignore = "requires genuine FE2O3_CONDITIONAL_PREFIX_GFX942_ARGS and GFX950_ARGS JSON rustc captures plus admitted proof runtime"]
fn genuine_direct_conditional_n_i_both_targets_stop_at_existing_gate() {
    let root =
        std::env::temp_dir().join(format!("fe2o3-conditional-prefix-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    for target in [942, 950] {
        let variable = format!("FE2O3_CONDITIONAL_PREFIX_GFX{target}_ARGS");
        let args = std::env::var_os(&variable).unwrap_or_else(|| panic!("missing {variable}"));
        let output = root.join(format!("gfx{target}.ll"));
        let result = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                CHILD,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(ARGS, args)
            .env(PROFILE, target.to_string())
            .env(OUTPUT, &output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "target {target}:\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            !output.exists(),
            "conditional refusal must emit no native text"
        );
    }
    std::fs::remove_dir(&root).unwrap();
}

#[test]
#[ignore = "child process only; needs exact pinned source arguments and genuine proof execution"]
fn genuine_conditional_prefix_child() {
    let args: Vec<String> =
        serde_json::from_slice(&std::fs::read(std::env::var_os(ARGS).unwrap()).unwrap()).unwrap();
    let target: usize = std::env::var(PROFILE).unwrap().parse().unwrap();
    assert!(matches!(target, 942 | 950));
    TARGET.store(target, Ordering::SeqCst);
    let output = PathBuf::from(std::env::var_os(OUTPUT).unwrap());
    assert!(!output.exists());
    OBSERVING.store(true, Ordering::SeqCst);
    let result =
        crate::run_production_fixed_checked_output_policy6_extraction_driver_v1(&args, &output);
    OBSERVING.store(false, Ordering::SeqCst);
    let error = result.expect_err("conditional N-to-I must not enter ordinary native emission");
    assert!(error.contains("FE2O3-COND-FINALIZER-001"), "{error}");
    assert_eq!(
        AGREEMENTS.load(Ordering::SeqCst),
        1,
        "genuine source/prefix agreement was not exercised"
    );
    assert_eq!(
        REPLAYS.load(Ordering::SeqCst),
        1,
        "proof, exact contract and original-account postchecks must all complete"
    );
    assert!(!output.exists());
}

#[test]
fn conditional_contract_replacement_still_requires_exact_retained_bytes() {
    use super::super::retention::RetainedConditionalContractV1 as Contract;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    // Inert payload storage only. These bytes are not a contract or receipt,
    // and cannot construct the live production request used by the check above.
    for changed in [false, true] {
        let mut work = Work::new(10_000);
        let mut budget = Budget::new(&mut work, 10_000);
        budget.reserve_storage(19).unwrap();
        let account = budget.work_ledger_identity_v1();
        let old = Contract::copy_unreserved(&[3; 32], &mut budget).unwrap();
        let floor = budget.storage();
        let mut bytes = [3; 32];
        if changed {
            bytes[17] ^= 1;
        }
        let new = Contract::copy_unreserved(&bytes, &mut budget).unwrap();
        let result = new.finish_replay_v1(Some(old), 0, floor, &mut budget);
        assert_eq!(result.is_ok(), !changed);
        drop(result);
        assert!(budget.work_ledger_identity_v1() == account);
        assert!(budget.work() > 0);
        let delta = budget.storage() - 19;
        budget.release_storage(delta).unwrap();
        assert_eq!(budget.storage(), 19);
    }
}
