use super::super::tests::fixture;
use super::*;
use fe2o3_kernel_ir::*;
fn admit(module: &Module) -> (VerifiedCanonicalKernelIrModuleV22, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV22::from_module_ref_with_verification_budget_v22(
            module,
            &mut budget,
        )
        .unwrap();
    (owner, receipt.retained_storage())
}
fn edited() -> Module {
    fixture::module_with_registers(true)
}
#[test]
fn exact_owner_llvm_four_slot_and_actual_step_grammar() {
    for module in [fixture::module(), edited()] {
        let (owner, retained) = admit(&module);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
        let mut budget = Budget::new(&mut work, 16_000_000);
        budget.reserve_storage(retained + 73).unwrap();
        let (emission, charge) =
            lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
                .unwrap();
        budget.reserve_storage(charge.retained_storage()).unwrap();
        let floor = budget.storage();
        let (text, receipt) =
            physical_lds_exchange_native_observation_input_v22(&owner, &emission, &mut budget)
                .unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            receipt.retained_storage(),
            std::mem::size_of::<String>() + 16 * 1024
        );
        assert_eq!(text.capacity(), 16 * 1024);
        assert!(text.starts_with(
            "FE2O3_PHYSICAL_LDS_EXCHANGE_V22_NATIVE_OBSERVATION_INPUT_V1\ncanonical "
        ));
        let digest = Sha256::digest(emission.llvm_ir().as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(text.contains(&format!("llvm {digest} {}\n", emission.llvm_ir().len())));
        assert!(
            text.contains("entry physical_lds_exchange_fixture\nlaunch 128 1 1 1 1 1\nlds_frame 0 512 4 1\nblocks 1\n")
        );
        assert!(text.contains("step 0 0 0 8 0 0 0 0 0 0\n"));
        assert!(text.contains("step 0 3 0 14 0 0 24 0 0 0\n"));
        assert!(text.ends_with("end\n") && text.lines().count() <= 48);
        assert!(emission.llvm_ir().contains("(ptr addrspace(1) %input_data, i64 %input_length, ptr addrspace(1) %output_data, i64 %output_length)"));
        let count = owner.module().functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .len();
        assert!(text.contains(&format!(
            "block 0 4 0 {count} 255 255\nsteps {}\n",
            count - 1
        )));
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        drop(text);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}
#[test]
fn mismatched_owner_or_inert_corrupted_correspondence_refuses() {
    let (owner, retained) = admit(&fixture::module());
    let (other, other_retained) = admit(&edited());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget
        .reserve_storage(retained + other_retained + 79)
        .unwrap();
    let (mut emission, charge) =
        lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap();
    budget.reserve_storage(charge.retained_storage()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        physical_lds_exchange_native_observation_input_v22(&other, &emission, &mut budget),
        Err(Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22::Profile(
            _
        ))
    ));
    // Private unit corruption only; public output exposes no mutable relation.
    emission.operations[13].as_mut().unwrap().results[0] = Some(ValueId(u32::MAX));
    assert!(matches!(
        physical_lds_exchange_native_observation_input_v22(&owner, &emission, &mut budget),
        Err(Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22::Profile(
            _
        ))
    ));
    assert_eq!(budget.storage(), floor);
}
#[test]
fn store_must_use_actual_ready_lds_read_ssa_not_another_well_typed_u32() {
    let mut module = fixture::module();
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    let other = block.operations[7].results[0].id; // The actual launch-index U32.
    let OperationKind::Gfx942PhysicalLdsExchangeStep(store) = &mut block.operations[29].kind else {
        panic!("store")
    };
    store.operands[2] = Some(other);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget.reserve_storage(83).unwrap();
    assert!(matches!(
        VerifiedCanonicalKernelIrModuleV22::from_module_ref_with_verification_budget_v22(
            &module,
            &mut budget
        ),
        Err(CanonicalKernelIrReplayAdmissionErrorV22::Verification(_))
    ));
    assert_eq!(budget.storage(), 83);
}
#[test]
fn exact_one_below_and_denial_history_preserve_owner_emission_floor() {
    let (owner, retained) = admit(&fixture::module());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget.reserve_storage(retained + 97).unwrap();
    let (emission, charge) =
        lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap();
    let floor = retained + 97 + charge.retained_storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    work.charge_work(11).unwrap();
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget.reserve_storage(floor).unwrap();
    drop(
        physical_lds_exchange_native_observation_input_v22(&owner, &emission, &mut budget).unwrap(),
    );
    let needed_work = budget.work();
    let needed_storage = budget.peak_storage();
    for case in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(needed_work - usize::from(case == 1));
        work.charge_work(11).unwrap();
        let mut budget = Budget::new(&mut work, needed_storage - usize::from(case == 2));
        budget.reserve_storage(floor).unwrap();
        let result =
            physical_lds_exchange_native_observation_input_v22(&owner, &emission, &mut budget);
        match (case, result) {
            (0, Ok(_)) => assert_eq!(budget.work(), needed_work),
            (
                1,
                Err(Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22::Resource(Resource::Work(
                    e,
                ))),
            ) => assert_eq!(e.actual(), e.limit() + 1),
            (
                2,
                Err(Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22::Resource(
                    Resource::Storage(e),
                )),
            ) => {
                assert_eq!(e.actual(), e.limit() + 1);
                assert_eq!(budget.failed_storage(), Some(needed_storage));
            }
            value => panic!("{value:?}"),
        }
        assert_eq!(budget.storage(), floor);
    }
}
#[test]
#[ignore = "explicit inert V22 export only; no source or native authority; fresh directory required"]
fn export_inert_lds_exchange_and_register_site_edits_for_native_observer() {
    use std::{fs, io::Write, path::Path};
    let directory =
        std::env::var("FE2O3_PHYSICAL_V22_NATIVE_FIXTURE_DIR").expect("fresh directory");
    assert!(
        directory.len() <= 4000
            && Path::new(&directory).is_absolute()
            && directory.bytes().all(|b| b >= 32 && b != 127)
    );
    fs::create_dir(&directory).expect("directory must be new");
    for (name, module) in [("one", fixture::module()), ("registers", edited())] {
        let (owner, retained) = admit(&module);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
        let mut budget = Budget::new(&mut work, 16_000_000);
        budget.reserve_storage(retained).unwrap();
        let (emission, charge) =
            lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
                .unwrap();
        budget.reserve_storage(charge.retained_storage()).unwrap();
        let (sidecar, charge) =
            physical_lds_exchange_native_observation_input_v22(&owner, &emission, &mut budget)
                .unwrap();
        budget.reserve_storage(charge.retained_storage()).unwrap();
        for (suffix, bytes) in [
            ("ll", emission.llvm_ir().as_bytes()),
            ("expect", sidecar.as_bytes()),
            ("kir22", owner.canonical_bytes()),
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
        "inert V22 same-owner export only; no source/native/hardware/protected execution or milestone closure"
    );
}

#[test]
fn corrupted_private_frame_cannot_change_the_same_owner_reservation() {
    let (owner, retained) = admit(&fixture::module());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget.reserve_storage(retained + 73).unwrap();
    let (mut emission, charge) =
        lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap();
    budget.reserve_storage(charge.retained_storage()).unwrap();
    let floor = budget.storage();
    emission.lds_frame.byte_length = 1024;
    assert!(matches!(
        physical_lds_exchange_native_observation_input_v22(&owner, &emission, &mut budget),
        Err(Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22::Profile(
            _
        ))
    ));
    assert_eq!(budget.storage(), floor);
}
