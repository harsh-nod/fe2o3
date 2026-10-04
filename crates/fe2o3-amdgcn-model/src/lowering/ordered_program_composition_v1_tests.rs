use super::*;
use fe2o3_kernel_ir as ordered_composition_fixture_ir;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1, Gfx942OrderedProgramV1, Gfx942ProgramDestinationV1,
    Gfx942ProgramInstructionV1, Gfx942ProgramRoleV1, VerifiedCanonicalKernelIrModuleV17,
};
#[path = "../../../fe2o3-kernel-ir/tests/fixtures/ordered_composition_v1.rs"]
mod fixture;

fn composition(module: &Module) -> VerifiedOrderedProgramCompositionV1 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let (canonical, canonical_receipt) =
        VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
            module,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(canonical_receipt.retained_storage())
        .unwrap();
    VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(canonical, &mut budget)
        .unwrap()
        .0
}
fn emit(
    owner: &VerifiedOrderedProgramCompositionV1,
) -> OrderedProgramCompositionCanonicalEmissionV1 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    lower_ordered_program_composition_to_gfx942_xnack_minus_llvm_ir_v1(owner, &mut budget)
        .unwrap()
        .0
}
#[test]
fn singleton_text_is_byte_identical_to_existing_v17_renderer() {
    for steps in [1, 3, 8, 16] {
        let owner = composition(&fixture::module(1, &[], &[], steps));
        let old =
            lower_canonical_v17_compiler_module_to_gfx942_xnack_minus_llvm_ir(owner.canonical())
                .unwrap();
        let new = emit(&owner);
        assert_eq!(new.llvm_ir(), old);
        assert_eq!(new.canonical_identity(), owner.canonical().identity());
        assert_eq!(new.region_definitions().count(), 1);
        assert_eq!(new.helper_calls().count(), 0);
        assert!(new.llvm_ir().contains("store i32"));
        assert!(
            new.llvm_ir()
                .contains(fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1)
        );
    }
}
#[test]
fn multiple_regions_and_shared_helpers_keep_complete_module_and_calls() {
    for (direct, helpers, calls) in [
        (8, vec![], vec![]),
        (0, vec![1], vec![0, 0]),
        (1, vec![1, 2], vec![0, 1, 0]),
        (1, vec![0], vec![0, 0]),
    ] {
        let owner = composition(&fixture::module(direct, &helpers, &calls, 16));
        let emitted = emit(&owner);
        assert_eq!(
            emitted.region_definitions().count(),
            owner.definitions().len()
        );
        assert_eq!(emitted.helper_calls().count(), calls.len());
        assert_eq!(
            emitted.llvm_ir().matches("asm sideeffect").count(),
            owner.definitions().len()
        );
        assert_eq!(
            emitted
                .llvm_ir()
                .matches("define internal i32 @helper_")
                .count(),
            helpers.len()
        );
        assert_eq!(
            emitted.llvm_ir().matches(" = call i32 @helper_").count(),
            calls.len()
        );
        assert!(
            emitted
                .llvm_ir()
                .contains("define amdgpu_kernel void @kernel(")
        );
        assert!(emitted.llvm_ir().contains("store i32"));
        for row in emitted.region_definitions() {
            let definition = &owner.definitions()[row.key().ordinal() as usize];
            assert_eq!(row.site(), definition.site());
            let op = &owner.canonical().module().functions[row.site().function_ordinal() as usize]
                .body
                .as_ref()
                .unwrap()
                .blocks[row.site().block_ordinal() as usize]
                .operations[row.site().operation_ordinal() as usize];
            let OperationKind::Gfx942OrderedProgram(program) = &op.kind else {
                panic!()
            };
            assert_eq!(row.program(), program.program());
            assert_eq!(row.registers(), program.registers());
            assert_eq!(row.inputs(), program.inputs());
            assert_eq!(row.result(), op.results[0].id);
        }
        for row in emitted.helper_calls() {
            let call = &owner.calls()[row.key().ordinal() as usize];
            assert_eq!(row.site(), call.site());
            assert_eq!(row.callee(), call.callee());
            assert_eq!(
                row.callee_function_ordinal(),
                owner.helpers()[call.callee().ordinal() as usize].function_ordinal()
            );
        }
    }
}
#[test]
fn old_singleton_and_generic_paths_still_refuse_composition() {
    for module in [
        fixture::module(2, &[], &[], 3),
        fixture::module(0, &[1], &[0], 3),
    ] {
        let owner = composition(&module);
        assert!(
            lower_canonical_v17_compiler_module_to_gfx942_xnack_minus_llvm_ir(owner.canonical())
                .is_err()
        );
        assert!(
            lower_compiler_module_to_gfx942_xnack_minus_llvm_ir(owner.canonical().module())
                .is_err()
        );
        assert!(lower_compiler_module_to_llvm_ir(owner.canonical().module()).is_err());
        assert!(emit(&owner).llvm_ir().contains("asm sideeffect"));
    }
}
#[test]
fn all_six_opcodes_and_self_moves_use_existing_renderer_and_constraints() {
    let owner = composition(&fixture::module(1, &[1], &[0], 16));
    let emitted = emit(&owner);
    for mnemonic in [
        "v_mov_b32_e32",
        "v_add_u32_e32",
        "v_sub_u32_e32",
        "v_and_b32_e32",
        "v_or_b32_e32",
        "v_xor_b32_e32",
    ] {
        assert!(emitted.llvm_ir().contains(mnemonic));
    }
    assert!(
        emitted
            .llvm_ir()
            .contains("=&{v33},{v34},{v35},{v36},~{v32}")
    );
    assert!(emitted.llvm_ir().contains("=&{v0},{v62},{v17},{v1},~{v63}"));
    let mut module = fixture::module(1, &[], &[], 1);
    let op = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
    let OperationKind::Gfx942OrderedProgram(old) = op.kind else {
        panic!()
    };
    let mut words = [0; 16];
    words[0] = Gfx942ProgramInstructionV1::Move {
        destination: Gfx942ProgramDestinationV1::Output,
        source: Gfx942ProgramRoleV1::Input0,
    }
    .descriptor();
    words[1] = Gfx942ProgramInstructionV1::Move {
        destination: Gfx942ProgramDestinationV1::Output,
        source: Gfx942ProgramRoleV1::Output,
    }
    .descriptor();
    op.kind = OperationKind::Gfx942OrderedProgram(
        Gfx942OrderedProgramV1::new(
            old.source(),
            old.registers(),
            *old.inputs(),
            Gfx942U32ProgramV1::from_descriptors(2, words).unwrap(),
        )
        .unwrap(),
    );
    assert!(
        emit(&composition(&module))
            .llvm_ir()
            .contains("v_mov_b32_e32 $0, $1\\0A\\09v_mov_b32_e32 $0, $0")
    );
}
#[test]
fn exact_owner_context_rejects_foreign_module_and_target() {
    let owner = composition(&fixture::module(1, &[], &[], 3));
    let copy = owner.canonical().module().clone();
    assert!(
        context::validate_owner_context(&copy, LoweringTarget::Gfx942XnackMinusV1, &owner).is_err()
    );
    assert!(
        context::validate_owner_context(
            owner.canonical().module(),
            LoweringTarget::Gfx942StrictFloatV1,
            &owner
        )
        .is_err()
    );
    assert!(
        context::validate_owner_context(
            owner.canonical().module(),
            LoweringTarget::Gfx942XnackMinusV1,
            &owner
        )
        .is_ok()
    );
}
#[test]
fn exact_operation_pointer_is_required_even_for_equal_content() {
    let owner = composition(&fixture::module(1, &[], &[], 3));
    let module = owner.canonical().module();
    let function = &module.functions[0];
    let lowerer = FunctionLowerer::new(
        module,
        &module.kernels[0],
        function,
        WorkgroupSize::new(64, 1, 1),
        Some(WaveWidth::Wave64),
        LoweringTarget::Gfx942XnackMinusV1,
        SemanticAnchorEmissionV1::Disabled,
    )
    .unwrap();
    let operation = function.body.as_ref().unwrap().blocks[0].operations[0].clone();
    let location =
        LoweringLocation::operation(module, &module.kernels[0], function, BlockId(40), 0);
    let error = lowerer
        .validate_ordered_composition_v1(&operation, &location, &owner)
        .unwrap_err();
    assert!(
        error.diagnostics()[0]
            .message
            .contains("not the actual retained operation")
    );
}
#[test]
fn exact_and_one_short_emission_limits_preserve_floor_and_owner() {
    let owner = composition(&fixture::module(1, &[1], &[0, 0], 8));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(41).unwrap();
    budget.charge_work(13).unwrap();
    let (output, receipt) =
        lower_ordered_program_composition_to_gfx942_xnack_minus_llvm_ir_v1(&owner, &mut budget)
            .unwrap();
    let used = budget.work();
    let peak = budget.peak_storage();
    assert_eq!(budget.storage(), 41);
    assert_eq!(
        receipt.retained_storage(),
        size_of::<OrderedProgramCompositionCanonicalEmissionV1>() + MAX_COMPILER_MODULE_TEXT_BYTES
    );
    for (work_cap, storage_cap, success) in [
        (used, peak, true),
        (used - 1, peak, false),
        (used, peak - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_cap);
        let mut budget = Budget::new(&mut work, storage_cap);
        budget.reserve_storage(41).unwrap();
        budget.charge_work(13).unwrap();
        let result =
            lower_ordered_program_composition_to_gfx942_xnack_minus_llvm_ir_v1(&owner, &mut budget);
        assert_eq!(result.is_ok(), success);
        assert_eq!(budget.storage(), 41);
        if success {
            assert_eq!(result.unwrap().0, output);
        }
    }
    assert_eq!(emit(&owner), output);
}
#[test]
fn full_lowering_diagnostics_are_retained_on_error_with_resource_floor() {
    let mut module = fixture::module(1, &[], &[], 3);
    module.kernels[0].id = KernelId::new("unsafe symbol");
    let owner = composition(&module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(53).unwrap();
    let error =
        lower_ordered_program_composition_to_gfx942_xnack_minus_llvm_ir_v1(&owner, &mut budget)
            .unwrap_err();
    let E::Lowering(error) = error else { panic!() };
    assert!(error.contains(LoweringDiagnosticCode::UnsafeSymbolName));
    assert_eq!(budget.storage(), 53);
    assert!(budget.work() > 0);
}
