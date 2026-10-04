//! Inert verified-graph/text tests. They do not replace independent worker/decode qualification.
use super::*;
use crate::gfx942_physical_entry_fixture_v20_tests as fixture;
use fe2o3_kernel_ir::*;
fn owner(module: &Module) -> (VerifiedCanonicalKernelIrModuleV20, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV20::from_module_ref_with_verification_budget_v20(
            module,
            &mut budget,
        )
        .unwrap();
    (owner, receipt.retained_storage())
}
fn emit(select: bool) -> Gfx942PhysicalEntryCanonicalEmissionV20 {
    let (owner, retained) = owner(&fixture::module(select));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(retained).unwrap();
    lower_canonical_v20_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
        .unwrap()
        .0
}
#[test]
fn copy_literal_full_entry_contains_only_authored_instruction_rows() {
    let output = emit(false);
    let rows = output
        .assembly_template()
        .lines()
        .filter(|line| !line.ends_with(':'))
        .collect::<Vec<_>>();
    let expected = [
        "s_load_dwordx2 s[8:9], s[0:1], 0",
        "s_load_dwordx2 s[10:11], s[0:1], 8",
        "s_load_dword s12, s[0:1], 16",
        "s_load_dword s13, s[0:1], 20",
        "s_load_dword s14, s[0:1], 24",
        "s_load_dword s15, s[0:1], 28",
        "s_waitcnt lgkmcnt(0)",
        "s_lshl_b32 s16, s2, 6",
        "v_add_u32_e32 v2, s16, v0",
        "v_mov_b32_e32 v3, 0",
        "v_mov_b32_e32 v4, s9",
        "v_mov_b32_e32 v8, s12",
        "v_lshlrev_b64 v[6:7], 2, v[2:3]",
        "v_add_co_u32_e32 v6, vcc, s8, v6",
        "v_addc_co_u32_e32 v7, vcc, v4, v7, vcc",
        "v_cmp_gt_u64_e32 vcc, s[10:11], v[2:3]",
        "s_and_saveexec_b64 s[18:19], vcc",
        "global_store_dword v[6:7], v8, off",
        "s_waitcnt vmcnt(0)",
        "s_mov_b64 exec, s[18:19]",
        "s_endpgm",
    ];
    assert_eq!(rows, expected);
    assert_eq!(output.operations().count(), 21);
    assert_eq!(output.blocks().count(), 1);
    assert_eq!(output.assembly_template().lines().count(), 22);
}
#[test]
fn diamond_branches_are_actual_cfg_targets_and_fallthrough_emits_no_instruction() {
    let output = emit(true);
    let text = output.assembly_template();
    assert_eq!(text.lines().filter(|line| !line.ends_with(':')).count(), 25);
    assert!(text.contains("s_cbranch_scc1 .Lfe2o3_physical_${:uid}_b2\n"));
    assert!(text.contains("s_branch .Lfe2o3_physical_${:uid}_b3\n"));
    assert_eq!(text.matches("s_branch ").count(), 1);
    let blocks = output.blocks().collect::<Vec<_>>();
    assert_eq!(
        blocks[2].encoding,
        Gfx942PhysicalEntryBranchEncodingVNext::Fallthrough
    );
    assert_eq!(blocks[2].native_ordinal, None);
    assert_eq!(blocks[2].terminator_assembly_line, None);
    assert_eq!(
        blocks[3].encoding,
        Gfx942PhysicalEntryBranchEncodingVNext::Endpgm0
    );
    assert_eq!(output.operations().count(), 23);
}
#[test]
fn actual_canonical_site_result_descriptor_and_native_line_correspondence_is_complete() {
    for select in [false, true] {
        let module = fixture::module(select);
        let (owner, retained) = owner(&module);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
        budget.reserve_storage(retained).unwrap();
        let (output, receipt) =
            lower_canonical_v20_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
                .unwrap();
        assert_eq!(output.canonical_identity(), owner.identity());
        assert_eq!(budget.storage(), retained);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let body = owner.module().functions[0].body.as_ref().unwrap();
        let lines = output.assembly_template().lines().collect::<Vec<_>>();
        let mut ordinals = Vec::new();
        for row in output.operations() {
            let operation =
                &body.blocks[row.block.0 as usize].operations[usize::from(row.operation_ordinal)];
            assert_eq!(
                row.results.into_iter().flatten().collect::<Vec<_>>(),
                operation
                    .results
                    .iter()
                    .map(|value| value.id)
                    .collect::<Vec<_>>()
            );
            match &operation.kind {
                OperationKind::Gfx942PhysicalEntryDeclaration(value) => {
                    assert_eq!(row.source_site, value.begin_site);
                    assert_eq!(row.native_ordinal, None);
                    assert_eq!(row.assembly_line, None);
                }
                OperationKind::Gfx942PhysicalEntryStep(value) => {
                    assert_eq!(row.source_site, value.site);
                    assert_eq!(
                        row.instruction_descriptor,
                        Some(value.instruction.descriptor())
                    );
                    assert_eq!(row.native_ordinal, Some(value.native_ordinal));
                    let line = lines[usize::from(row.assembly_line.unwrap())];
                    assert!(!line.is_empty() && !line.ends_with(':'));
                    ordinals.push(value.native_ordinal);
                }
                _ => panic!("closed fixture"),
            }
        }
        for row in output.blocks() {
            if let Some(n) = row.native_ordinal {
                ordinals.push(n);
            }
            assert!(lines[usize::from(row.assembly_label_line)].ends_with(':'));
        }
        ordinals.sort_unstable();
        assert_eq!(
            ordinals,
            (0..if select { 25 } else { 21 }).collect::<Vec<_>>()
        );
        drop(output);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), retained);
    }
}
#[test]
fn ordinary_shell_has_six_unused_arguments_no_input_constraints_intrinsics_or_setup_ssa() {
    let output = emit(true);
    let llvm = output.llvm_ir();
    assert_eq!(llvm.matches("define ").count(), 1);
    assert_eq!(llvm.matches("call void asm sideeffect").count(), 1);
    assert_eq!(llvm.matches("unreachable").count(), 1);
    assert!(!llvm.contains("declare "));
    for forbidden in [
        " naked",
        " inreg",
        "@llvm.",
        "module asm",
        "ptrtoint",
        "getelementptr",
        "zext ",
        "trunc ",
        "lshr ",
        "%index",
        "ret void",
        "~{exec}",
    ] {
        assert!(!llvm.contains(forbidden), "{forbidden}");
    }
    for argument in ["%data", "%length", "%a", "%b", "%c", "%selector"] {
        assert_eq!(llvm.matches(argument).count(), 1);
    }
    for attribute in [
        "amdgpu-no-dispatch-ptr",
        "amdgpu-no-queue-ptr",
        "amdgpu-no-dispatch-id",
        "amdgpu-no-hostcall-ptr",
        "amdgpu-no-multigrid-sync-arg",
        "amdgpu-no-heap-ptr",
        "amdgpu-no-default-queue",
        "amdgpu-no-completion-action",
        "amdgpu-no-workgroup-id-y",
        "amdgpu-no-workgroup-id-z",
        "amdgpu-no-cluster-id-y",
        "amdgpu-no-cluster-id-z",
        "amdgpu-no-workitem-id-y",
        "amdgpu-no-workitem-id-z",
    ] {
        assert_eq!(llvm.matches(attribute).count(), 1);
    }
    assert!(llvm.contains("\"amdgpu-max-num-workgroups\"=\"2,1,1\""));
    assert!(llvm.contains("!1 = !{i32 1, !\"amdhsa_code_object_version\", i32 600}"));
    assert!(llvm.contains("\"~{s8},~{s9},~{s10},~{s11},~{s12},~{s13},~{s14},~{s15},~{s16},~{s18},~{s19},~{v2},~{v3},~{v4},~{v6},~{v7},~{v8},~{vcc},~{scc},~{memory}\"()"));
}
#[test]
fn register_clobbers_and_entry_symbol_come_from_same_verified_owner_not_fixture_names() {
    let mut module = fixture::module(false);
    module.functions[0].id = FunctionId::new("source_owned_entry_57");
    module.kernels[0].id = KernelId::new("source_owned_entry_57");
    module.kernels[0].entry = module.functions[0].id.clone();
    for operation in &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations {
        if let OperationKind::Gfx942PhysicalEntryStep(step) = &mut operation.kind
            && step.instruction.opcode == Gfx942PhysicalEntryOpcodeV20::LoadKernargDword
            && step.instruction.immediate == 24
        {
            step.instruction.destination = 63; // unused logical c, still an authored load/wait
        }
    }
    let (owner, retained) = owner(&module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(retained).unwrap();
    let (output, _) =
        lower_canonical_v20_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap();
    assert!(output.llvm_ir().contains("@source_owned_entry_57("));
    assert!(
        output
            .assembly_template()
            .contains("s_load_dword s63, s[0:1], 24")
    );
    assert!(output.llvm_ir().contains("~{s63}"));
    assert!(!output.llvm_ir().contains("~{s14}"));
    assert!(output.clobbered_register_units()[63]);
    assert!(!output.clobbered_register_units()[130]);
}
#[test]
fn nonphysical_v20_owner_is_a_named_profile_refusal_without_allocating_authority() {
    let (owner, retained) = owner(&Module::new("ordinary_empty"));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(retained + 73).unwrap();
    assert!(matches!(
        lower_canonical_v20_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget),
        Err(Gfx942PhysicalEntryCanonicalEmissionErrorV20::Profile(_))
    ));
    assert_eq!(budget.storage(), retained + 73);
}
#[test]
fn exact_and_one_short_render_resources_keep_owner_floor_and_cumulative_work() {
    let (owner, retained) = owner(&fixture::module(true));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(retained + 73).unwrap();
    drop(
        lower_canonical_v20_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap(),
    );
    let needed_work = budget.work();
    let needed_storage = budget.peak_storage();
    for case in 0..3 {
        let mut work =
            CanonicalKernelIrWorkBudgetV1::new(11 + needed_work - usize::from(case == 1));
        work.charge_work(11).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            needed_storage - usize::from(case == 2),
        );
        budget.reserve_storage(retained + 73).unwrap();
        assert_eq!(
            lower_canonical_v20_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
                .is_ok(),
            case == 0
        );
        assert_eq!(budget.storage(), retained + 73);
        if case == 0 {
            assert_eq!(budget.work(), 11 + needed_work);
        }
    }
}
