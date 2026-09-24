//! Inert ranked component controls only. No source owner is minted here.
use super::*;
use crate::physical_lds_exchange_materialization_v22::tests::fixture;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, ExplicitLaunchExtent, FormalIndexWidth, Module,
    derive_physical_lds_exchange_memory_obligations_v22,
};
fn owner(module: &Module) -> VerifiedCanonicalKernelIrModuleV22 {
    let mut work = Work::new(16_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 32_000_000);
    VerifiedCanonicalKernelIrModuleV22::from_module_ref_with_verification_budget_v22(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}
fn formal(
    owner: &VerifiedCanonicalKernelIrModuleV22,
    count: u64,
) -> PhysicalLdsExchangeMemoryObligationsV22 {
    let mut work = Work::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 4_000_000);
    derive_physical_lds_exchange_memory_obligations_v22(
        owner,
        &owner.module().kernels[0].id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [count, 1, 1],
        },
        FormalIndexWidth::Bits64,
        &mut budget,
    )
    .unwrap()
    .0
}
fn projected(owner: &VerifiedCanonicalKernelIrModuleV22) -> Recipe {
    let formal = formal(owner, 128);
    let mut work = Work::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 4_000_000);
    budget.reserve_storage(83).unwrap();
    let result = {
        let scope = PhysicalLdsExchangeLedgerScopeV22::new(&mut budget);
        recipe(owner, &formal, 7, scope.budget).unwrap()
    };
    assert_eq!(budget.storage(), 83);
    assert_eq!(budget.work(), WORK);
    result
}
#[test]
fn lds_exchange_ranked_fixed_payload_and_auxiliary_error_are_bounded() {
    assert!(storage_bound().unwrap() <= STORAGE);
    assert_eq!(vector::<ValueR>(5).unwrap().capacity(), 5);
    assert!(std::mem::size_of::<PhysicalLdsExchangeAuxErrorV22>() < 128);
}
#[test]
fn lds_exchange_ranked_unguarded_input_precedes_conditional_output_tail_and_uses_two_arguments() {
    let r = projected(&owner(&fixture::module()));
    assert_eq!(r.argument_count(), 2);
    assert_eq!(r.blocks().len(), 3);
    let entry = r.blocks()[0].operations();
    let input=entry.iter().filter(|op|matches!(op,OpR::Access{kind:Access::Read,view,..}if *view==ValueR::Local(INPUT_PREFIX))).count();
    assert_eq!(input, 1);
    assert!(matches!(
        r.blocks()[0].terminator(),
        EndR::IndexLessThanArgs {
            true_block: 1,
            false_block: 2,
            ..
        }
    ));
    assert!(
        matches!(r.blocks()[1].operations(),[OpR::Access{kind:Access::Write,view,..}]if *view==ValueR::Local(OUTPUT))
    );
    assert!(entry.iter().any(
        |op| matches!(op,OpR::ViewInSpace{result,shape,dynamic_extents,writable:false,..}
        if *result==INPUT_PREFIX&&shape==&[128]&&dynamic_extents.is_empty())
    ));
    assert!(entry.iter().any(
        |op| matches!(op,OpR::ViewInSpace{result,shape,dynamic_extents,writable:true,..}
        if *result==OUTPUT&&shape==&[128]&&dynamic_extents.is_empty())
    ));
}
#[test]
fn lds_exchange_ranked_output_prefix_does_not_rewrite_actual_mask_or_discharge_conditions() {
    for renamed in [false, true] {
        let owner = owner(&fixture::module_with_registers(renamed));
        let actual = owner.module().clone();
        let report = formal(&owner, 128);
        let _projection = projected(&owner);
        assert_eq!(owner.module(), &actual);
        assert_eq!(report.runtime_requirements().minimum_output_bytes(), 512);
        assert!(report.runtime_requirements().requires_output_writable());
        assert!(
            !report
                .runtime_requirements()
                .grants_runtime_binding_authority()
        );
        let block = &actual.functions[0].body.as_ref().unwrap().blocks[0];
        let store = report.output_store();
        let comparisons: Vec<_> = block
            .operations
            .iter()
            .filter_map(|operation| match operation.kind {
                Op::Gfx942PhysicalLdsExchangeStep(step)
                    if step.instruction.opcode == Opcode::VectorCompareGtU64 =>
                {
                    Some(step)
                }
                _ => None,
            })
            .collect();
        assert_eq!(comparisons.len(), 1);
        assert_eq!(&comparisons[0].operands[..2], &store.length().map(Some));
        for opcode in [Opcode::SaveAndMaskExec, Opcode::RestoreExec] {
            assert_eq!(
                block
                    .operations
                    .iter()
                    .filter(|operation| matches!(
                        operation.kind, Op::Gfx942PhysicalLdsExchangeStep(step)
                        if step.instruction.opcode == opcode
                    ))
                    .count(),
                1
            );
        }
        assert!(store.comparison_at().operation_index < store.mask_at().operation_index);
        assert!(store.mask_at().operation_index < store.access().location().operation_index);
        assert!(store.access().location().operation_index < store.restore_at().operation_index);
    }
}
#[test]
fn lds_exchange_ranked_unresolved_dynamic_output_still_fails_mandatory_workgroup_analysis() {
    let r = projected(&owner(&fixture::module()));
    let mut operations = r.blocks()[0].operations().to_vec();
    let output = operations
        .iter_mut()
        .find(|operation| matches!(operation, OpR::ViewInSpace { result, .. } if *result == OUTPUT))
        .unwrap();
    let OpR::ViewInSpace {
        shape,
        dynamic_extents,
        ..
    } = output
    else {
        unreachable!()
    };
    *shape = vec![dialect_kernel::DYNAMIC_EXTENT];
    *dynamic_extents = vec![ValueR::Argument(1)];
    let mut blocks = r.blocks().to_vec();
    blocks[0] = Block::new(operations, r.blocks()[0].terminator().clone());
    let changed = Recipe::new("lds_unresolved_output_control", 2, blocks).unwrap();
    let construction =
        ProductionConstructionV1::ranked_kernel("lds_unresolved_output_control", changed).unwrap();
    let error = compile_ranked_kernel_for_gfx942_lowering_v1(
        construction,
        ProductionSessionLimitsV1::default(),
        std::iter::empty(),
    )
    .err()
    .expect("mutated ranked recipe must fail");
    let detail = format!("{error:?}");
    assert!(detail.contains("RankedWorkgroup"));
    assert!(detail.contains("branch in block 0 has an unresolved condition"));
}
#[test]
fn lds_exchange_ranked_output_prefix_does_not_admit_one_past_end_store() {
    let r = projected(&owner(&fixture::module()));
    let Some(OpR::Dimension { result, .. }) = r.blocks()[0].operations().last() else {
        panic!("projection must end in the output-prefix dimension");
    };
    let index = IdR::new(result.get().checked_add(1).unwrap());
    let mut blocks = r.blocks().to_vec();
    blocks[1] = Block::with_index_arguments(
        1,
        vec![
            OpR::IndexConstant {
                result: index,
                value: 128,
            },
            OpR::Access {
                kind: Access::Write,
                view: ValueR::Local(OUTPUT),
                indices: vec![ValueR::Local(index)],
            },
        ],
        EndR::Branch { target: 2 },
    );
    let changed = Recipe::new("lds_output_one_past_control", 2, blocks).unwrap();
    let construction =
        ProductionConstructionV1::ranked_kernel("lds_output_one_past_control", changed).unwrap();
    let error = compile_ranked_kernel_for_gfx942_lowering_v1(
        construction,
        ProductionSessionLimitsV1::default(),
        std::iter::empty(),
    )
    .err()
    .expect("mutated ranked recipe must fail");
    assert!(format!("{error:?}").contains("RankedBounds"));
}
#[test]
fn lds_exchange_ranked_actual_local_x_uses_exact_single_workgroup_affine_invocation() {
    for renamed in [false, true] {
        let owner = owner(&fixture::module_with_registers(renamed));
        let report = formal(&owner, 128);
        assert_eq!(
            report.runtime_requirements().required_workgroup(),
            [128, 1, 1]
        );
        assert_eq!(
            report.runtime_requirements().required_workgroups(),
            [1, 1, 1]
        );
        assert!(required_conditions(&report));
        let declaration =
            &owner.module().functions[0].body.as_ref().unwrap().blocks[0].operations[0];
        let mut values = Values::new();
        let mut operations = Operations::new().unwrap();
        entry_values(declaration, &mut values, &mut operations).unwrap();
        let invocations: Vec<_> = operations
            .rows
            .iter()
            .filter_map(|operation| match operation {
                OpR::InvocationIndex {
                    result,
                    dimension: 0,
                    launch_extent: 128,
                } => Some(*result),
                _ => None,
            })
            .collect();
        assert_eq!(invocations.len(), 1);
        assert_eq!(declaration.results[3].id, report.lds_frame().local_x());
        assert_eq!(
            values.get(report.lds_frame().local_x()).unwrap(),
            ValueR::Local(invocations[0])
        );
        for global_x in 0u64..128 {
            assert_eq!(global_x, global_x % 128);
            assert_eq!((global_x + 64) % 128, global_x ^ 64);
        }
    }
}
#[test]
fn lds_exchange_ranked_has_four_abi_reads_and_one_full_exec_global_read() {
    let r = projected(&owner(&fixture::module_with_registers(true)));
    let reads: Vec<_> = r
        .blocks()
        .iter()
        .flat_map(|b| b.operations())
        .filter_map(|op| match op {
            OpR::Access {
                kind: Access::Read,
                view,
                indices,
            } => Some((*view, indices.len())),
            _ => None,
        })
        .collect();
    assert_eq!(reads.len(), 6);
    assert_eq!(
        reads
            .iter()
            .filter(|(v, _)| *v == ValueR::Local(KERNARG64))
            .count(),
        4
    );
    assert_eq!(
        reads
            .iter()
            .filter(|(v, _)| *v == ValueR::Local(INPUT_PREFIX))
            .count(),
        1
    );
    assert!(reads.iter().all(|(_, n)| *n == 1));
}
#[test]
fn lds_exchange_ranked_load_data_is_opaque_not_a_deterministic_address_join() {
    let r = projected(&owner(&fixture::module()));
    let ops = r.blocks()[0].operations();
    let read=ops.iter().position(|op|matches!(op,OpR::Access{kind:Access::Read,view,..}if *view==ValueR::Local(INPUT_PREFIX))).unwrap();
    let OpR::IndexUnknown { result: loaded } = ops[read + 1] else {
        panic!("opaque load result");
    };
    let unknowns: Vec<_> = ops
        .iter()
        .filter_map(|op| match op {
            OpR::IndexUnknown { result } => Some(ValueR::Local(*result)),
            _ => None,
        })
        .collect();
    assert_eq!(unknowns.len(), 4); // ABI base halves, opaque global input and opaque peer LDS data
    assert!(ops[read+2..].iter().any(|op|matches!(op,OpR::DeterministicJoin{dependencies,..}if dependencies.contains(&ValueR::Local(loaded)))));
    for op in r.blocks().iter().flat_map(|b| b.operations()) {
        if let OpR::Access { indices, .. } = op {
            assert!(indices.iter().all(|i| !unknowns.contains(i)));
        }
    }
}
#[test]
fn lds_exchange_ranked_conditional_recipe_passes_existing_mandatory_session() {
    for extra in [false, true] {
        let r = projected(&owner(&fixture::module_with_registers(extra)));
        let construction =
            ProductionConstructionV1::ranked_kernel("lds_exchange_inert_conditional", r).unwrap();
        let checked = compile_ranked_kernel_for_gfx942_lowering_v1(
            construction,
            ProductionSessionLimitsV1::default(),
            std::iter::empty(),
        )
        .unwrap();
        assert!(checked.all_mandatory_reports_are_clean());
        assert!(!checked.has_retained_policy_checked_refinement_staging());
    }
}
#[test]
fn lds_exchange_ranked_without_required_full_input_prefix_fails_normal_bounds_check() {
    for dynamic in [false, true] {
        let r = projected(&owner(&fixture::module()));
        let mut ops = r.blocks()[0].operations().to_vec();
        let op = ops
            .iter_mut()
            .find(|op| matches!(op,OpR::ViewInSpace{result,..}if *result==INPUT_PREFIX))
            .unwrap();
        let OpR::ViewInSpace {
            shape,
            dynamic_extents,
            ..
        } = op
        else {
            unreachable!();
        };
        if dynamic {
            *shape = vec![dialect_kernel::DYNAMIC_EXTENT];
            *dynamic_extents = vec![ValueR::Argument(0)];
        } else {
            *shape = vec![127];
        }
        let mut blocks = r.blocks().to_vec();
        blocks[0] = Block::new(ops, r.blocks()[0].terminator().clone());
        let changed = Recipe::new("inert_bad_input_prefix", 2, blocks).unwrap();
        let construction =
            ProductionConstructionV1::ranked_kernel("lds_exchange_inert_bad_prefix", changed)
                .unwrap();
        assert!(
            compile_ranked_kernel_for_gfx942_lowering_v1(
                construction,
                ProductionSessionLimitsV1::default(),
                std::iter::empty()
            )
            .is_err()
        );
    }
}
#[test]
fn lds_exchange_ranked_rejects_foreign_complete_memory_subject() {
    let first = owner(&fixture::module());
    let other = owner(&fixture::module_with_registers(true));
    let report = formal(&other, 128);
    {
        let mut work = Work::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 4_000_000);
        budget.reserve_storage(83).unwrap();
        {
            let scope = PhysicalLdsExchangeLedgerScopeV22::new(&mut budget);
            assert!(matches!(
                recipe(&first, &report, 7, scope.budget),
                Err(PhysicalLdsExchangeAuxErrorV22::Relation(
                    "LDS exchange ranked complete memory subject or conditions differ"
                ))
            ));
        }
        assert_eq!(budget.storage(), 83);
        assert_eq!(budget.work(), WORK);
    }
}
#[test]
fn lds_exchange_ranked_register_rename_keeps_safety_projection_but_not_subject_identity() {
    let before = owner(&fixture::module());
    let after = owner(&fixture::module_with_registers(true));
    assert_ne!(before.identity(), after.identity());
    assert_eq!(projected(&before), projected(&after)); // safety only, not native identity
    assert_ne!(
        formal(&before, 128).canonical_identity(),
        formal(&after, 128).canonical_identity()
    );
}
#[test]
fn lds_exchange_ranked_exact_short_resources_and_repeated_failures_keep_caller_floor() {
    let owner = owner(&fixture::module());
    let report = formal(&owner, 128);
    for (work_limit, storage_limit, success) in [
        (11 + WORK, 83 + STORAGE, true),
        (10 + WORK, 83 + STORAGE, false),
        (11 + WORK, 82 + STORAGE, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.charge_work(11).unwrap();
        budget.reserve_storage(83).unwrap();
        {
            let scope = PhysicalLdsExchangeLedgerScopeV22::new(&mut budget);
            assert_eq!(recipe(&owner, &report, 7, scope.budget).is_ok(), success);
        }
        assert_eq!(budget.storage(), 83);
        if success {
            assert_eq!(budget.work(), 11 + WORK);
            assert_eq!(budget.peak_storage(), 83 + STORAGE);
        }
        if storage_limit < 83 + STORAGE {
            assert_eq!(budget.failed_storage(), Some(83 + STORAGE));
        }
    }
    let other = self::owner(&fixture::module_with_registers(true));
    let bad = formal(&other, 128);
    let mut work = Work::new(7 + WORK * 2);
    let mut budget = ArgumentBudgetV1::new(&mut work, 4_000_000);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(83).unwrap();
    for attempt in 1..=2 {
        {
            let scope = PhysicalLdsExchangeLedgerScopeV22::new(&mut budget);
            assert!(recipe(&owner, &bad, 7, scope.budget).is_err());
        }
        assert_eq!(budget.storage(), 83);
        assert_eq!(budget.work(), 7 + attempt * WORK);
    }
}
#[test]
fn lds_exchange_ranked_value_map_and_operation_bounds_refuse_without_inventing_values() {
    let mut values = Values::new();
    assert!(values.get(ValueId(999)).is_err());
    values.insert(ValueId(4), ValueR::Argument(0)).unwrap();
    assert!(values.insert(ValueId(4), ValueR::Argument(1)).is_err());
    let mut ops = Operations::new().unwrap();
    assert!(ops.joined(&[]).is_err());
    assert!(ops.joined(&[ValueR::Argument(0); 6]).is_err());
    for _ in 0..OPERATIONS {
        ops.unknown().unwrap();
    }
    assert!(ops.unknown().is_err());
}
#[test]
fn lds_exchange_ranked_inspection_names_all_unresolved_memory_conditions() {
    let r = projected(&owner(&fixture::module()));
    let text = inspection_text(&r).unwrap();
    assert!(text.starts_with("physical-lds-exchange-v22-safety-projection conditional-full-input-prefix512-readable-initialized-output-prefix512-writable-nonalias-kernarg-live-immutable-lds512-wg128-publish1 "));
    assert!(text.len() < TEXT_LIMIT);
    assert_eq!(text, inspection_text(&r).unwrap());
}

#[path = "production_physical_lds_exchange_ranked_memory_v22_tests.rs"]
mod lds_tests;
