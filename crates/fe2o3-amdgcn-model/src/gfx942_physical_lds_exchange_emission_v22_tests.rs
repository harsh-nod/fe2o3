//! Inert same-owner text qualification; no native/source/runtime authority.
use super::*;
use fe2o3_kernel_ir as physical_lds_exchange_fixture_ir;
use fe2o3_kernel_ir::*;
#[path = "../../fe2o3-kernel-ir/tests/fixtures/physical_lds_exchange_v22.rs"]
pub(super) mod fixture;
fn owner(module: &Module) -> (VerifiedCanonicalKernelIrModuleV22, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV22::from_module_ref_with_verification_budget_v22(
            module,
            &mut budget,
        )
        .unwrap();
    (owner, receipt.retained_storage())
}
fn emit() -> Gfx942PhysicalLdsExchangeCanonicalEmissionV22 {
    let (owner, retained) = owner(&fixture::module());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(retained).unwrap();
    lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
        .unwrap()
        .0
}
#[test]
fn exact_authored_32_instruction_exchange_with_three_lgkm_waits() {
    let output = emit();
    let rows = output
        .assembly_template()
        .lines()
        .filter(|line| !line.ends_with(':'))
        .collect::<Vec<_>>();
    assert_eq!(
        rows,
        [
            "s_load_dwordx2 s[8:9], s[0:1], 0",
            "s_load_dwordx2 s[10:11], s[0:1], 8",
            "s_load_dwordx2 s[12:13], s[0:1], 16",
            "s_load_dwordx2 s[14:15], s[0:1], 24",
            "s_waitcnt lgkmcnt(0)",
            "s_lshl_b32 s16, s2, 7",
            "v_add_u32_e32 v2, s16, v0",
            "v_mov_b32_e32 v3, 0",
            "v_mov_b32_e32 v4, s9",
            "v_lshlrev_b64 v[6:7], 2, v[2:3]",
            "v_add_co_u32_e32 v6, vcc, s8, v6",
            "v_addc_co_u32_e32 v7, vcc, v4, v7, vcc",
            "global_load_dword v8, v[6:7], off",
            "s_waitcnt vmcnt(0)",
            "v_lshlrev_b32_e32 v16, 2, v0",
            "ds_write_b32 v16, v8",
            "s_waitcnt lgkmcnt(0)",
            "s_barrier",
            "v_xor_b32_e32 v17, 64, v0",
            "v_lshlrev_b32_e32 v17, 2, v17",
            "ds_read_b32 v18, v17",
            "s_waitcnt lgkmcnt(0)",
            "v_mov_b32_e32 v5, s13",
            "v_lshlrev_b64 v[10:11], 2, v[2:3]",
            "v_add_co_u32_e32 v10, vcc, s12, v10",
            "v_addc_co_u32_e32 v11, vcc, v5, v11, vcc",
            "v_cmp_gt_u64_e32 vcc, s[14:15], v[2:3]",
            "s_and_saveexec_b64 s[18:19], vcc",
            "global_store_dword v[10:11], v18, off",
            "s_waitcnt vmcnt(0)",
            "s_mov_b64 exec, s[18:19]",
            "s_endpgm",
        ]
    );
    assert_eq!(output.operations().count(), 32);
    assert_eq!(output.blocks().count(), 1);
    assert_eq!(output.assembly_template().lines().count(), 33);
    assert_eq!(
        output
            .assembly_template()
            .matches("s_waitcnt vmcnt(0)")
            .count(),
        2
    );
}
#[test]
fn canonical_sites_results_and_instruction_lines_refer_to_the_same_retained_owner() {
    let (owner, retained) = owner(&fixture::module());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(retained).unwrap();
    let (output, receipt) =
        lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap();
    assert_eq!(output.canonical_identity(), owner.identity());
    assert_eq!(budget.storage(), retained);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let lines = output.assembly_template().lines().collect::<Vec<_>>();
    let mut native = Vec::new();
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
            OperationKind::Gfx942PhysicalLdsExchangeDeclaration(d) => {
                assert_eq!(row.source_site, d.begin_site);
                assert_eq!(row.native_ordinal, None);
                assert_eq!(row.assembly_line, None);
            }
            OperationKind::Gfx942PhysicalLdsExchangeStep(step) => {
                assert_eq!(row.source_site, step.site);
                assert_eq!(
                    row.instruction_descriptor,
                    Some(step.instruction.descriptor())
                );
                assert_eq!(row.native_ordinal, Some(step.native_ordinal));
                assert!(!lines[usize::from(row.assembly_line.unwrap())].ends_with(':'));
                native.push(step.native_ordinal);
            }
            _ => panic!("closed profile"),
        }
    }
    for row in output.blocks() {
        assert_eq!(
            row.encoding,
            Gfx942PhysicalEntryBranchEncodingVNext::Endpgm0
        );
        native.push(row.native_ordinal.unwrap());
        assert_eq!(
            lines[usize::from(row.terminator_assembly_line.unwrap())],
            "s_endpgm"
        );
    }
    assert_eq!(native, (0u8..32).collect::<Vec<_>>());
    drop(output);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), retained);
}
#[test]
fn exact_four_slot_ordinary_shell_has_no_outside_setup_ssa_or_forged_noalias() {
    let output = emit();
    let llvm = output.llvm_ir();
    assert!(llvm.contains("(ptr addrspace(1) %input_data, i64 %input_length, ptr addrspace(1) %output_data, i64 %output_length)"));
    for arg in [
        "%input_data",
        "%input_length",
        "%output_data",
        "%output_length",
    ] {
        assert_eq!(llvm.matches(arg).count(), 1);
    }
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
        " noalias",
        " readonly",
    ] {
        assert!(!llvm.contains(forbidden), "{forbidden}");
    }
    assert_eq!(llvm.matches("define ").count(), 1);
    assert_eq!(llvm.matches("call void asm sideeffect").count(), 1);
    assert_eq!(llvm.matches("unreachable").count(), 1);
    assert!(!llvm.contains("declare "));
    for attr in super::emit::ABSENCE_ATTRIBUTES {
        assert_eq!(llvm.matches(attr).count(), 1);
    }
    assert!(llvm.contains("\"amdgpu-max-num-workgroups\"=\"1,1,1\""));
    assert!(llvm.contains("!1 = !{i32 1, !\"amdhsa_code_object_version\", i32 600}"));
    assert!(llvm.contains("~{v5}"));
    assert!(llvm.contains("~{v10}"));
    assert!(llvm.contains("~{v11}"));
    assert!(!output.clobbered_register_units()[130]);
    assert_eq!(llvm.matches("\"amdgpu-lds-size\"=\"512,512\"").count(), 1);
    assert!(llvm.contains("\"amdgpu-flat-work-group-size\"=\"128,128\""));
    assert!(llvm.contains("!0 = !{i32 128, i32 1, i32 1}"));
    assert_eq!(output.lds_frame().byte_length, 512);
    assert!(!llvm.contains("addrspace(3)"));
    assert!(!llvm.contains("m0"));
}
#[test]
fn unused_input_length_pair_register_edit_and_entry_name_are_owner_derived() {
    let mut module = fixture::module();
    module.functions[0].id = FunctionId::new("inert_edited_lds_exchange");
    module.kernels[0].id = KernelId::new("inert_edited_lds_exchange");
    module.kernels[0].entry = module.functions[0].id.clone();
    for op in &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations {
        if let OperationKind::Gfx942PhysicalLdsExchangeStep(step) = &mut op.kind
            && step.instruction.opcode == Gfx942PhysicalLdsExchangeOpcodeV1::LoadKernargPair
            && step.instruction.immediate == 8
        {
            step.instruction.destination = 62;
        }
    }
    let (owner, retained) = owner(&module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(retained).unwrap();
    let (output, _) =
        lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap();
    assert!(output.llvm_ir().contains("@inert_edited_lds_exchange("));
    assert!(
        output
            .assembly_template()
            .contains("s_load_dwordx2 s[62:63], s[0:1], 8")
    );
    assert!(output.clobbered_register_units()[62] && output.clobbered_register_units()[63]);
    assert!(!output.clobbered_register_units()[10] && !output.clobbered_register_units()[11]);
}
#[test]
fn exact_and_one_short_render_budgets_keep_owner_and_work_floors() {
    let (owner, retained) = owner(&fixture::module());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(retained + 73).unwrap();
    drop(
        lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap(),
    );
    let need_work = budget.work();
    let need_storage = budget.peak_storage();
    for case in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + need_work - usize::from(case == 1));
        work.charge_work(11).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            need_storage - usize::from(case == 2),
        );
        budget.reserve_storage(retained + 73).unwrap();
        let result =
            lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget);
        assert_eq!(result.is_ok(), case == 0);
        if case == 1 {
            assert!(
                matches!(result, Err(Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22::Resource(
                Resource::Work(error))) if error.actual() == 11 + need_work && error.limit() + 1 == error.actual())
            );
        } else if case == 2 {
            assert!(
                matches!(result, Err(Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22::Resource(
                Resource::Storage(error))) if error.actual() == need_storage && error.limit() + 1 == error.actual())
            );
            assert_eq!(budget.failed_storage(), Some(need_storage));
        }
        assert_eq!(budget.storage(), retained + 73);
        if case == 0 {
            assert_eq!(budget.work(), 11 + need_work);
        }
    }
}
#[test]
fn ordinary_v22_owner_is_an_explicit_profile_refusal() {
    let (owner, retained) = owner(&Module::new("ordinary"));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(retained + 73).unwrap();
    assert!(matches!(
        lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget),
        Err(Gfx942PhysicalLdsExchangeCanonicalEmissionErrorV22::Profile(
            _
        ))
    ));
    assert_eq!(budget.storage(), retained + 73);
}

#[test]
fn complete_register_edit_keeps_real_lds_def_use_and_frame() {
    let (owner, retained) = owner(&fixture::module_with_registers(true));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(16_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16_000_000);
    budget.reserve_storage(retained).unwrap();
    let (out, _) =
        lower_canonical_v22_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner, &mut budget)
            .unwrap();
    for text in [
        "global_load_dword v22, v[6:7], off",
        "ds_write_b32 v24, v22",
        "v_xor_b32_e32 v25, 64, v0",
        "ds_read_b32 v26, v25",
        "global_store_dword v[10:11], v26, off",
    ] {
        assert!(out.assembly_template().contains(text), "{text}");
    }
    assert_eq!(
        out.lds_frame(),
        &gfx942_physical_lds_exchange_declaration_v22(&owner)
            .unwrap()
            .lds_frame
    );
}
