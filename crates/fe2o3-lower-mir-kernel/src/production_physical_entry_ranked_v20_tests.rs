//! Canonical/ranked component tests only. No compiler/source owner is minted.
use super::*;
use crate::physical_entry_materialization_v20::tests::fixture;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, ExplicitLaunchExtent, FormalIndexWidth, Module,
    PhysicalEntryMemoryObligationsV20, derive_physical_entry_memory_obligations_v20,
};

fn owner(module: &Module) -> VerifiedCanonicalKernelIrModuleV20 {
    let mut work = Work::new(16_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 32_000_000);
    VerifiedCanonicalKernelIrModuleV20::from_module_ref_with_verification_budget_v20(
        module,
        &mut budget,
    )
    .unwrap()
    .0
}
fn formal(owner: &VerifiedCanonicalKernelIrModuleV20) -> PhysicalEntryMemoryObligationsV20 {
    let mut work = Work::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 4_000_000);
    derive_physical_entry_memory_obligations_v20(
        owner,
        &owner.module().kernels[0].id,
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [128, 1, 1],
        },
        FormalIndexWidth::Bits64,
        &mut budget,
    )
    .unwrap()
    .0
}
fn projected(owner: &VerifiedCanonicalKernelIrModuleV20) -> Recipe {
    let formal = formal(owner);
    let mut work = Work::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 4_000_000);
    budget.reserve_storage(83).unwrap();
    let result = {
        let scope = PhysicalEntryLedgerScopeV20::new(&mut budget);
        recipe(owner, &formal, 7, scope.budget).unwrap()
    };
    assert_eq!(budget.storage(), 83);
    assert_eq!(budget.work(), WORK);
    result
}
#[test]
fn physical_ranked_fixed_payload_bound_covers_all_preallocated_vectors() {
    assert!(storage_bound().unwrap() <= STORAGE);
    assert_eq!(vector::<ValueR>(6).unwrap().capacity(), 6);
}
#[test]
fn physical_ranked_one_and_diamond_keep_authored_blocks_and_actual_merge_edges() {
    for select in [false, true] {
        let owner = owner(&fixture::module(select));
        let recipe = projected(&owner);
        assert_eq!(recipe.argument_count(), 5); // no invented kernarg source argument
        assert_eq!(recipe.blocks().len(), if select { 6 } else { 3 });
        if select {
            // The normal ranked constructor canonicalizes empty edge-argument
            // lists to IndexEqual while preserving operands and target blocks.
            let EndR::IndexEqual {
                lhs: ValueR::Local(selector),
                rhs: ValueR::Local(zero),
                true_block: 2,
                false_block: 1,
            } = recipe.blocks()[0].terminator()
            else {
                panic!(
                    "actual selector edge: {:?}",
                    recipe.blocks()[0].terminator()
                );
            };
            assert!(
                recipe.blocks()[0]
                    .operations()
                    .iter()
                    .any(|operation| matches!(
                        operation,
                        OpR::IndexUnsignedCast {
                            result,
                            source: ValueR::Argument(4),
                            bit_width: 32,
                        } if result == selector
                    ))
            );
            assert!(
                recipe.blocks()[0]
                    .operations()
                    .iter()
                    .any(|operation| matches!(
                        operation,
                        OpR::IndexConstant { result, value: 0 } if result == zero
                    ))
            );
            assert_eq!(recipe.blocks()[3].index_argument_count(), 1);
            let edges = [1usize, 2].map(|i| match recipe.blocks()[i].terminator() {
                EndR::BranchArgs {
                    target: 3,
                    arguments,
                } => arguments.clone(),
                other => panic!("actual edge: {other:?}"),
            });
            assert_eq!(edges[0].len(), 1);
            assert_eq!(edges[1].len(), 1);
            assert_ne!(edges[0], edges[1]);
        }
    }
}
#[test]
fn physical_ranked_reads_cover_every_actual_abi_load_with_the_real_width() {
    for select in [false, true] {
        let owner = owner(&fixture::module(select));
        let report = formal(&owner);
        let recipe = projected(&owner);
        let reads: Vec<_> = recipe
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
        assert_eq!(reads.len(), report.kernarg_reads().len());
        assert_eq!(
            reads
                .iter()
                .filter(|(view, _)| *view == ValueR::Local(KERNARG64))
                .count(),
            2
        );
        assert_eq!(
            reads
                .iter()
                .filter(|(view, _)| *view == ValueR::Local(KERNARG32))
                .count(),
            4
        );
        assert!(reads.iter().all(|(_, n)| *n == 1));
        let abi: Vec<_> = recipe.blocks()[0]
            .operations()
            .iter()
            .filter_map(|op| match op {
                OpR::ViewInSpace {
                    result,
                    writable,
                    allocation_origin,
                    noalias_class,
                    ..
                } if *result == KERNARG32 || *result == KERNARG64 => {
                    Some((*writable, *allocation_origin, *noalias_class))
                }
                _ => None,
            })
            .collect();
        assert_eq!(abi, [(false, 2, 2), (false, 2, 2)]);
        assert!(report.kernarg_abi().requires_immutable_kernarg());
        assert!(!report.grants_artifact_or_launch_authority());
    }
}
#[test]
fn physical_ranked_exact_projection_passes_existing_mandatory_checker_conditionally() {
    for select in [false, true] {
        let owner = owner(&fixture::module(select));
        let recipe = projected(&owner);
        let construction =
            ProductionConstructionV1::ranked_kernel("physical_ranked_inert", recipe).unwrap();
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
fn physical_ranked_rejects_a_memory_report_from_another_canonical_subject() {
    let first = owner(&fixture::module(false));
    let second = owner(&fixture::module(true));
    let report = formal(&second);
    let mut work = Work::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 4_000_000);
    budget.reserve_storage(83).unwrap();
    {
        let scope = PhysicalEntryLedgerScopeV20::new(&mut budget);
        assert!(matches!(
            recipe(&first, &report, 7, scope.budget),
            Err(PhysicalEntryAuxErrorV20::Relation(
                "physical ranked combined memory subject or ABI obligation differs"
            ))
        ));
    }
    assert_eq!(budget.storage(), 83);
}
#[test]
fn physical_ranked_actual_abi_load_edit_changes_the_projection() {
    let old = fixture::module(false);
    let mut changed = old.clone();
    let body = changed.functions[0].body.as_mut().unwrap();
    let operation=body.blocks[0].operations.iter_mut().find(|op|matches!(op.kind,
        Op::Gfx942PhysicalEntryStep(s)if s.instruction.opcode==Opcode::LoadKernargDword&&s.instruction.immediate==24)).unwrap();
    let Op::Gfx942PhysicalEntryStep(step) = &mut operation.kind else {
        panic!("step")
    };
    step.instruction.immediate = 20;
    assert_ne!(projected(&owner(&old)), projected(&owner(&changed)));
}
#[test]
fn physical_ranked_opaque_pointer_dependencies_are_never_numerical_access_indices() {
    let recipe = projected(&owner(&fixture::module(false)));
    let unknowns: Vec<_> = recipe.blocks()[0]
        .operations()
        .iter()
        .filter_map(|op| match op {
            OpR::IndexUnknown { result } => Some(ValueR::Local(*result)),
            _ => None,
        })
        .collect();
    assert_eq!(unknowns.len(), 2);
    for op in recipe.blocks().iter().flat_map(|b| b.operations()) {
        if let OpR::Access { indices, .. } = op {
            assert!(indices.iter().all(|i| !unknowns.contains(i)));
        }
    }
}
#[test]
fn physical_ranked_value_and_edge_maps_reject_undefined_duplicate_or_excessive_ids() {
    let mut values = Values::new();
    assert!(values.get(ValueId(999)).is_err());
    values.insert(ValueId(4), ValueR::Argument(2)).unwrap();
    assert!(values.insert(ValueId(4), ValueR::Argument(3)).is_err());
    assert_eq!(edge(&values, &[ValueId(4)]).unwrap(), [ValueR::Argument(2)]);
    assert!(edge(&values, &[ValueId(4); 132]).is_err());
    assert!(edge(&values, &[ValueId(5)]).is_err());
}
#[test]
fn physical_ranked_work_and_storage_refusal_preserve_the_caller_account() {
    let owner = owner(&fixture::module(false));
    let report = formal(&owner);
    for (work_limit, storage_limit) in [(0, 4_000_000), (1_000_000, 128)] {
        let mut work = Work::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(83).unwrap();
        {
            let scope = PhysicalEntryLedgerScopeV20::new(&mut budget);
            assert!(matches!(
                recipe(&owner, &report, 7, scope.budget),
                Err(PhysicalEntryAuxErrorV20::Resource(_))
            ));
        }
        assert_eq!(budget.storage(), 83);
    }
}
#[test]
fn physical_ranked_text_names_the_unresolved_abi_conditions() {
    let recipe = projected(&owner(&fixture::module(true)));
    let text = inspection_text(&recipe).unwrap();
    assert!(text.starts_with(
        "physical-entry-v20-safety-projection conditional-kernarg-immutable-and-output-disjoint "
    ));
    assert!(text.len() < TEXT_LIMIT);
    assert_eq!(text, inspection_text(&recipe).unwrap());
}
