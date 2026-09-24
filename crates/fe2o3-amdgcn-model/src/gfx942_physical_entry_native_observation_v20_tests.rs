use super::*;
use crate::gfx942_physical_entry_fixture_v20_tests as fixture;
use fe2o3_kernel_ir::*;
fn admit(module: &Module) -> (VerifiedCanonicalKernelIrModuleV20, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV20::from_module_ref_with_verification_budget_v20(
            module,
            &mut budget,
        )
        .unwrap();
    (owner, receipt.retained_storage())
}
#[test]
fn diagnostic_grammar_is_exact_same_owner_llvm_and_actual_cfg() {
    for select in [false, true] {
        let (owner, retained) = admit(&fixture::module(select));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
        let mut budget = Budget::new(&mut work, 16_000_000);
        budget.reserve_storage(retained).unwrap();
        let (emission, charge) =
            lower_canonical_v20_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
                .unwrap();
        budget.reserve_storage(charge.retained_storage()).unwrap();
        let floor = budget.storage();
        let (text, receipt) =
            physical_entry_native_observation_input_v20(&owner, &emission, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            receipt.retained_storage(),
            std::mem::size_of::<String>() + 16 * 1024
        );
        assert_eq!(text.capacity(), 16 * 1024);
        assert!(
            text.starts_with("FE2O3_PHYSICAL_ENTRY_V20_NATIVE_OBSERVATION_INPUT_V1\ncanonical ")
        );
        assert!(text.contains(&format!(
            "llvm {} {}\n",
            Sha256::digest(emission.llvm_ir().as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            emission.llvm_ir().len()
        )));
        assert!(text.contains("entry physical_fixture\nlaunch 64 1 1 2 1 1\n"));
        assert!(text.ends_with("end\n"));
        assert!(text.lines().count() <= 80);
        if select {
            assert!(text.contains("blocks 4\nblock 0 1 0 13 1 2\nblock 1 2 13 2 3 255\nblock 2 3 15 1 3 255\nblock 3 4 16 9 255 255\nsteps 22\n"));
        } else {
            assert!(text.contains("blocks 1\nblock 0 4 0 21 255 255\nsteps 20\n"));
        }
        assert!(text.contains("step 0 0 0 8 0 0 0 0 0 0\n"));
    }
}
#[test]
fn a_different_verified_subject_cannot_supply_the_emission_correspondence() {
    let (copy, copy_retained) = admit(&fixture::module(false));
    let (select, select_retained) = admit(&fixture::module(true));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget
        .reserve_storage(copy_retained + select_retained)
        .unwrap();
    let (emission, charge) =
        lower_canonical_v20_compiler_module_to_gfx942_xnack_minus_llvm_ir(&copy, &mut budget)
            .unwrap();
    budget.reserve_storage(charge.retained_storage()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        physical_entry_native_observation_input_v20(&select, &emission, &mut budget),
        Err(Gfx942PhysicalEntryCanonicalEmissionErrorV20::Profile(_))
    ));
    assert_eq!(budget.storage(), floor);
}
#[test]
fn diagnostic_exact_and_one_short_resources_preserve_retained_owner_and_emission() {
    let (owner, retained) = admit(&fixture::module(true));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget.reserve_storage(retained + 73).unwrap();
    let (emission, charge) =
        lower_canonical_v20_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap();
    let floor = retained + 73 + charge.retained_storage();
    let run = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        work.charge_work(11).unwrap();
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let success =
            physical_entry_native_observation_input_v20(&owner, &emission, &mut budget).is_ok();
        assert_eq!(budget.storage(), floor);
        (success, budget.work(), budget.peak_storage())
    };
    let (ok, work, storage) = run(16_000_000, 16_000_000);
    assert!(ok);
    assert!(run(work, storage).0);
    assert!(!run(work - 1, storage).0);
    assert!(!run(work, storage - 1).0);
}
#[test]
#[ignore = "explicit inert fixture export; no native worker or GPU; fresh absolute output directory required"]
fn export_inert_verified_copy_diamond_and_register_edit_for_native_observer() {
    use std::{fs, io::Write, path::Path};
    let directory =
        std::env::var("FE2O3_PHYSICAL_V20_NATIVE_FIXTURE_DIR").expect("fresh directory");
    assert!(
        directory.len() <= 4000
            && Path::new(&directory).is_absolute()
            && directory.bytes().all(|b| b >= 32 && b != 127)
    );
    fs::create_dir(&directory).expect("directory must be new");
    for (name, select, edited) in [
        ("copy", false, false),
        ("select", true, false),
        ("copy-s63", false, true),
    ] {
        let mut module = fixture::module(select);
        if edited {
            for op in &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations {
                if let OperationKind::Gfx942PhysicalEntryStep(step) = &mut op.kind
                    && step.instruction.opcode == Gfx942PhysicalEntryOpcodeV20::LoadKernargDword
                    && step.instruction.immediate == 24
                {
                    step.instruction.destination = 63;
                }
            }
        }
        let (owner, retained) = admit(&module);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
        let mut budget = Budget::new(&mut work, 16_000_000);
        budget.reserve_storage(retained).unwrap();
        let (emission, charge) =
            lower_canonical_v20_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
                .unwrap();
        budget.reserve_storage(charge.retained_storage()).unwrap();
        let (sidecar, sidecar_charge) =
            physical_entry_native_observation_input_v20(&owner, &emission, &mut budget).unwrap();
        budget
            .reserve_storage(sidecar_charge.retained_storage())
            .unwrap();
        for (suffix, bytes) in [
            ("ll", emission.llvm_ir().as_bytes()),
            ("expect", sidecar.as_bytes()),
            ("kir20", owner.canonical_bytes()),
        ] {
            let path = Path::new(&directory).join(format!("{name}.{suffix}"));
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            file.write_all(bytes).unwrap();
            file.sync_all().unwrap();
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
    }
    eprintln!(
        "inert V20 canonical/text export only; no source authentication, native execution or milestone closure"
    );
}
