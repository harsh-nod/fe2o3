use super::*;

fn correlated_node(plan: &SourceReferencePlanV29<'_, '_>) -> usize {
    plan.nodes
        .iter()
        .enumerate()
        .find_map(|(node, row)| {
            let SourceReferenceNodeKindV29::EnumView(index) = row.kind else {
                return None;
            };
            (row.ty == REFERENCE && plan.enum_views[index].child_count > 1).then_some(node)
        })
        .expect("genuine correlated reference view")
}

fn object_node(plan: &SourceReferencePlanV29<'_, '_>) -> (usize, usize) {
    plan.nodes
        .iter()
        .enumerate()
        .find_map(|(node, row)| {
            let SourceReferenceNodeKindV29::Loan(loan) = row.kind else {
                return None;
            };
            matches!(
                plan.cells.strategies[loan],
                SourceReferenceCellStrategyV29::Object(_)
            )
            .then_some((node, loan))
        })
        .expect("genuine object-backed loan node")
}

fn rebuild(
    plan: &SourceReferencePlanV29<'_, '_>,
    node: usize,
    bad_type: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SemanticSourceReferenceBindingV29, ProductionSemanticKirErrorV1> {
    let references = SourceReferenceEmissionV29::new(plan, budget)?;
    let types = source_reference_node_types_v29(plan, node, budget)?;
    assert!(!types.is_empty());
    let mut input = types
        .into_iter()
        .enumerate()
        .map(|(index, ty)| ValueDef::new(ValueId(1700 + index as u32), ty))
        .collect::<Vec<_>>();
    if bad_type {
        assert_ne!(input[0].ty, Type::BOOL);
        input[0].ty = Type::BOOL;
    }
    let mut values = input.iter();
    let mut leaves = [].iter();
    let mut nodes = 0;
    let result = source_reference_rebuild_node_v29(
        &references,
        node,
        false,
        &mut leaves,
        &mut values,
        &mut nodes,
        budget,
    );
    if bad_type {
        assert!(
            matches!(
                &result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "execution CFG transport differs from its captured SSA state",
                })
            ),
            "{result:?}"
        );
        assert_eq!(
            values.len(),
            input.len() - 1,
            "reject the actual first payload type before moving it"
        );
    } else {
        assert!(values.next().is_none());
        assert!(leaves.next().is_none());
    }
    let SemanticValueBindingV1::SourceReference(binding) = result? else {
        panic!("exact Loan/EnumView source node")
    };
    assert_eq!(binding.values.len(), input.len());
    for (rebuilt, original) in binding.values.iter().zip(&input) {
        assert_eq!(rebuilt, original);
    }
    drop(input);
    // The owned payload must outlive the input and the internal type iterator.
    source_reference_validate_binding_v29(plan, &binding, budget)?;
    Ok(binding)
}

#[test]
fn owned_reference_type_rebuild_keeps_correlated_view_and_value_types_after_input_drop() {
    run_enum(EnumCase::CorrelatedLoans, |plan, budget| {
        let node = correlated_node(plan);
        let binding = rebuild(plan, node, false, budget)?;
        assert_eq!(
            binding.origin,
            SourceReferenceBindingOriginV29::EnumView(node)
        );
        assert_eq!(binding.source_type, REFERENCE);
        assert_eq!(
            (binding.source, binding.ssa, binding.root),
            (plan.source, plan.ssa, plan.root)
        );
        assert!(binding.origin.single_loan().is_err());
        let floor = budget.storage();
        for _ in 0..4 {
            source_reference_validate_binding_v29(plan, &binding, budget)?;
            assert_eq!(budget.storage(), floor);
            assert!(!binding.values.is_empty());
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn owned_reference_type_rebuild_keeps_object_pointer_and_original_arena_custody() {
    run_enum_with_original_demands(
        enum_owner(EnumCase::RetainedCorrelatedLoans),
        |plan, budget| {
            let (node, loan) = object_node(plan);
            let binding = rebuild(plan, node, false, budget)?;
            assert_eq!(
                binding.origin,
                SourceReferenceBindingOriginV29::SingleLoan(loan)
            );
            assert_eq!(binding.source_type, plan.loans[loan].source_type);
            assert_eq!(binding.values.len(), 1);
            let Type::Pointer(pointer) = &binding.values[0].ty else {
                panic!("actual object pointer")
            };
            let address = pointer.pointee.as_ref() as *const Type as usize;
            budget.charge_work(source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29)?;
            let root = plan.storage_root.as_ref().expect("original storage arena");
            let growth = root.capture_retained_growth().expect("live custody");
            let floor = budget.storage();
            for _ in 0..4 {
                source_reference_validate_binding_v29(plan, &binding, budget)?;
                assert_eq!(budget.storage(), floor);
                assert!(root.retains_custody(plan.instances, &plan.failure, budget));
                assert!(growth.permits_refund(floor, floor, budget.storage(), 0));
                let Type::Pointer(pointer) = &binding.values[0].ty else {
                    unreachable!()
                };
                assert_eq!(pointer.pointee.as_ref() as *const Type as usize, address);
            }
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn owned_reference_type_rebuild_retains_actual_loan_and_view_payload_mismatch_refusals() {
    for object in [false, true] {
        let reached = std::cell::Cell::new(false);
        let consume = |plan: &SourceReferencePlanV29<'_, '_>, budget: &mut ArgumentBudgetV1<'_>| {
            let node = if object {
                object_node(plan).0
            } else {
                correlated_node(plan)
            };
            reached.set(true);
            rebuild(plan, node, true, budget).map(|_| ())
        };
        let result = if object {
            run_enum_with_original_demands(enum_owner(EnumCase::RetainedCorrelatedLoans), consume)
        } else {
            run_enum(EnumCase::CorrelatedLoans, consume)
        };
        assert!(reached.get());
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "execution CFG transport differs from its captured SSA state",
                })
            ),
            "{result:?}"
        );
    }
}
