//! Inert ranked component controls only. No source owner is minted here.
use super::*;
use crate::physical_global_copy_materialization_v21::tests::fixture;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, ExplicitLaunchExtent, FormalIndexWidth, Module,
    derive_physical_global_copy_memory_obligations_v21,
};
fn owner(module: &Module) -> VerifiedCanonicalKernelIrModuleV21 {
    let mut work = Work::new(16_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 32_000_000);
    VerifiedCanonicalKernelIrModuleV21::from_module_ref_with_verification_budget_v21(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}
fn formal(
    owner: &VerifiedCanonicalKernelIrModuleV21,
    count: u64,
) -> PhysicalGlobalCopyMemoryObligationsV21 {
    let mut work = Work::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 4_000_000);
    derive_physical_global_copy_memory_obligations_v21(
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
fn projected(owner: &VerifiedCanonicalKernelIrModuleV21) -> Recipe {
    let formal = formal(owner, 128);
    let mut work = Work::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 4_000_000);
    budget.reserve_storage(83).unwrap();
    let result = {
        let scope = PhysicalGlobalCopyLedgerScopeV21::new(&mut budget);
        recipe(owner, &formal, 7, scope.budget).unwrap()
    };
    assert_eq!(budget.storage(), 83);
    assert_eq!(budget.work(), WORK);
    result
}
#[test]
fn global_copy_ranked_fixed_payload_and_auxiliary_error_are_bounded() {
    assert!(storage_bound().unwrap() <= STORAGE);
    assert_eq!(vector::<ValueR>(5).unwrap().capacity(), 5);
    assert!(std::mem::size_of::<PhysicalGlobalCopyAuxErrorV21>() < 128);
}
#[test]
fn global_copy_ranked_unguarded_input_precedes_actual_output_guard_and_uses_two_arguments() {
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
    assert!(entry.iter().any(|op|matches!(op,OpR::ViewInSpace{result,shape,dynamic_extents,writable:true,..}
        if *result==OUTPUT&&shape==&[dialect_kernel::DYNAMIC_EXTENT]&&dynamic_extents==&[ValueR::Argument(1)])));
}
#[test]
fn global_copy_ranked_has_four_abi_reads_and_one_full_exec_global_read() {
    let r = projected(&owner(&fixture::module_with_extra_zero(true)));
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
    assert_eq!(reads.len(), 5);
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
fn global_copy_ranked_load_data_is_opaque_not_a_deterministic_address_join() {
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
    assert_eq!(unknowns.len(), 3); // two symbolic ABI base halves plus real input data
    assert!(ops[read+2..].iter().any(|op|matches!(op,OpR::DeterministicJoin{dependencies,..}if dependencies.contains(&ValueR::Local(loaded)))));
    for op in r.blocks().iter().flat_map(|b| b.operations()) {
        if let OpR::Access { indices, .. } = op {
            assert!(indices.iter().all(|i| !unknowns.contains(i)));
        }
    }
}
#[test]
fn global_copy_ranked_conditional_recipe_passes_existing_mandatory_session() {
    for extra in [false, true] {
        let r = projected(&owner(&fixture::module_with_extra_zero(extra)));
        let construction =
            ProductionConstructionV1::ranked_kernel("global_copy_inert_conditional", r).unwrap();
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
fn global_copy_ranked_without_required_full_input_prefix_fails_normal_bounds_check() {
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
            ProductionConstructionV1::ranked_kernel("global_copy_inert_bad_prefix", changed)
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
fn global_copy_ranked_rejects_foreign_subject_and_insufficient_conditional_launch() {
    let first = owner(&fixture::module());
    let other = owner(&fixture::module_with_extra_zero(true));
    for report in [formal(&other, 128), formal(&first, 64)] {
        let mut work = Work::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 4_000_000);
        budget.reserve_storage(83).unwrap();
        {
            let scope = PhysicalGlobalCopyLedgerScopeV21::new(&mut budget);
            assert!(matches!(
                recipe(&first, &report, 7, scope.budget),
                Err(PhysicalGlobalCopyAuxErrorV21::Relation(
                    "global copy ranked complete memory subject or conditions differ"
                ))
            ));
        }
        assert_eq!(budget.storage(), 83);
        assert_eq!(budget.work(), WORK);
    }
}
#[test]
fn global_copy_ranked_register_rename_keeps_safety_projection_but_not_subject_identity() {
    let mut module = fixture::module();
    let before = owner(&module);
    for op in &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations {
        if let Op::Gfx942PhysicalGlobalCopyStep(s) = &mut op.kind {
            match s.instruction.opcode {
                Opcode::GlobalLoadDword => s.instruction.destination = 22,
                Opcode::GlobalStoreDword => s.instruction.source1 = 22,
                _ => {}
            }
        }
    }
    let after = owner(&module);
    assert_ne!(before.identity(), after.identity());
    assert_eq!(projected(&before), projected(&after)); // safety only, not native identity
    assert_ne!(
        formal(&before, 128).canonical_identity(),
        formal(&after, 128).canonical_identity()
    );
}
#[test]
fn global_copy_ranked_exact_short_resources_and_repeated_failures_keep_caller_floor() {
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
            let scope = PhysicalGlobalCopyLedgerScopeV21::new(&mut budget);
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
    let bad = formal(&owner, 64);
    let mut work = Work::new(7 + WORK * 2);
    let mut budget = ArgumentBudgetV1::new(&mut work, 4_000_000);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(83).unwrap();
    for attempt in 1..=2 {
        {
            let scope = PhysicalGlobalCopyLedgerScopeV21::new(&mut budget);
            assert!(recipe(&owner, &bad, 7, scope.budget).is_err());
        }
        assert_eq!(budget.storage(), 83);
        assert_eq!(budget.work(), 7 + attempt * WORK);
    }
}
#[test]
fn global_copy_ranked_value_map_and_operation_bounds_refuse_without_inventing_values() {
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
fn global_copy_ranked_inspection_names_all_unresolved_memory_conditions() {
    let r = projected(&owner(&fixture::module()));
    let text = inspection_text(&r).unwrap();
    assert!(text.starts_with("physical-global-copy-v21-safety-projection conditional-full-input-prefix512-readable-initialized-output-writable-nonalias-kernarg-live-immutable "));
    assert!(text.len() < TEXT_LIMIT);
    assert_eq!(text, inspection_text(&r).unwrap());
}
