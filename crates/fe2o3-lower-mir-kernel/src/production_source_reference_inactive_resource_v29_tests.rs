use super::inactive_tests::{InactiveCase, inactive_owner, joined_inactive, rebuilt_inactive};
use super::*;

#[test]
fn source_inactive_clone_has_independent_exact_and_one_short_work_and_storage() {
    run_cells(inactive_owner(InactiveCase::Move, false), |plan, budget| {
        let emission = SourceReferenceEmissionV29::new(plan, budget)?;
        let original = rebuilt_inactive(plan, &emission, budget)?;
        for pointer in [false, true] {
            let SemanticValueBindingV1::SourceInactive(mut fixture) = original.clone() else {
                panic!("inactive binding absent");
            };
            if pointer {
                fixture.values[0].ty = Type::pointer(
                    Type::Scalar(ScalarType::U64),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                );
            }
            let fixture = SemanticValueBindingV1::SourceInactive(fixture);
            // One binding dispatch, seven metadata fields, Vec constructor3,
            // one payload element and one work unit per type-tree node.
            let exact_work = 1 + 7 + 3 + 1 + 1 + usize::from(pointer);
            let exact_storage = std::mem::size_of::<SemanticSourceInactiveBindingV29>()
                + std::mem::size_of::<ValueDef>()
                + usize::from(pointer) * std::mem::size_of::<Type>();
            for short_work in [false, true] {
                for short_storage in [false, true] {
                    let floor = 37;
                    let mut work =
                        CanonicalKernelIrWorkBudgetV1::new(exact_work - usize::from(short_work));
                    let mut clone_budget = ArgumentBudgetV1::new(
                        &mut work,
                        floor + exact_storage - usize::from(short_storage),
                    );
                    clone_budget.reserve_storage(floor)?;
                    let result = emission_clone_binding_v1(&fixture, &mut clone_budget);
                    assert_eq!(result.is_err(), short_work || short_storage);
                    if short_work || short_storage {
                        assert_eq!(
                            clone_budget.storage(),
                            floor,
                            "partial clone backing must drop before refund"
                        );
                        assert!(matches!(
                            result,
                            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
                        ));
                    } else {
                        let SemanticValueBindingV1::SourceInactive(copy) = result.unwrap() else {
                            panic!("clone gained a value binding");
                        };
                        let SemanticValueBindingV1::SourceInactive(original) = &fixture else {
                            unreachable!();
                        };
                        assert_eq!(&copy, original);
                        assert_eq!(clone_budget.work(), exact_work);
                        assert_eq!(clone_budget.storage(), floor + exact_storage);
                        drop(copy);
                        clone_budget.release_storage(exact_storage)?;
                    }
                    assert_eq!(clone_budget.storage(), floor);
                }
            }
        }
        Ok(())
    })
    .unwrap();
}

macro_rules! inactive_scope {
    (|$plan:ident, $root:ident, $budget:ident| $body:block) => {{
        let owner = inactive_owner(InactiveCase::Loop, false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, 64 * 1024 * 1024);
        let demands =
            source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)
                .unwrap();
        let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
            &owner,
            demands.types(&owner, &mut budget).unwrap(),
            ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
            &mut budget,
        )
        .unwrap();
        let result = production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
                let floor = budget.storage();
                let result = source_storage_v29::with_source_storage_root_v29(
                    &mut layouts,
                    instances,
                    budget,
                    |$plan, $root, $budget| $body,
                );
                assert_eq!(budget.storage(), floor);
                assert!(layouts.permits_root_emission_refund(&owner, 0, budget));
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
            },
        )
        .unwrap();
        let cleanup = layouts.release(&mut budget);
        assert_eq!(cleanup.is_err(), result.is_err());
        demands.discard(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
        result
    }};
}

#[test]
fn source_inactive_repeated_cfg_and_call_replay_keeps_recipe_and_snapshot_storage_stable() {
    inactive_scope!(|plan, root, budget| {
        let retained = root.snapshot_statistics(budget)?;
        let floor = budget.storage();
        budget.reserve_storage(source_reference_headers_v29::<()>()?)?;
        let mut builder = SourceReferenceBuilderV29::new_with_root(
            plan.instances,
            SourceReferenceStorageV29::ScalarCells,
            Some(root),
            budget,
        )?;
        builder.function(plan.root, None, budget)?;
        let counts = (
            builder.plan.inactive_removals.len(),
            builder.plan.inactive_choices.len(),
            builder.plan.nodes.len(),
        );
        let capacities = (
            builder.plan.inactive_removals.capacity(),
            builder.plan.inactive_choices.capacity(),
            builder.plan.nodes.capacity(),
        );
        for _ in 0..8 {
            builder.function(plan.root, None, budget)?;
            assert_eq!(
                (
                    builder.plan.inactive_removals.len(),
                    builder.plan.inactive_choices.len(),
                    builder.plan.nodes.len()
                ),
                counts
            );
            assert_eq!(
                builder
                    .storage_root
                    .as_ref()
                    .unwrap()
                    .snapshot_statistics(budget)?,
                retained
            );
            assert_eq!(
                (
                    builder.plan.inactive_removals.capacity(),
                    builder.plan.inactive_choices.capacity(),
                    builder.plan.nodes.capacity()
                ),
                capacities
            );
        }
        drop(builder);
        budget.release_storage(budget.storage() - floor)?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn original_partial_snapshot_still_cannot_qualify_a_return_value() {
    let completed = std::cell::Cell::new(false);
    inactive_scope!(|plan, root, budget| {
        let floor = budget.storage();
        budget.reserve_storage(source_reference_headers_v29::<()>()?)?;
        let mut builder = SourceReferenceBuilderV29::new_with_root(
            plan.instances,
            SourceReferenceStorageV29::ScalarCells,
            Some(root),
            budget,
        )?;
        builder.function(plan.root, None, budget)?;
        let helper = capture_instance(&builder.plan, 0);
        let entry = builder
            .plan
            .blocks
            .iter()
            .find(|row| row.instance == helper && row.block.index() == 3)
            .unwrap()
            .entry;
        let local = SemanticLocalIdV1::from_index(5);
        let state = builder.plan.states[entry][5];
        assert!(state.storage.is_some());
        let node = state.node.unwrap();
        let count = builder.plan.nodes.len();
        // Replay an inert return request against an existing original partial
        // snapshot. This does not admit a new source return or rewrite its SSA.
        assert!(builder.frames[helper.index()].is_none());
        builder.frames[helper.index()] = Some(entry);
        let result = builder.return_storage_value(helper, local, node, budget);
        builder.frames[helper.index()] = None;
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source return value is not definitely initialized",
                    ..
                })
            ),
            "{result:?}"
        );
        assert_eq!(
            builder.plan.nodes.len(),
            count,
            "refusal publishes no return node"
        );
        drop(builder);
        budget.release_storage(budget.storage() - floor)?;
        completed.set(true);
        Ok(())
    })
    .unwrap();
    assert!(
        completed.get(),
        "all original snapshot and return assertions must complete"
    );
}

#[test]
fn source_inactive_canonical_recipe_reuse_has_an_independent_work_boundary() {
    for short in [false, true] {
        let checked = std::cell::Cell::new(false);
        let result = inactive_scope!(|plan, root, budget| {
            let floor = budget.storage();
            budget.reserve_storage(source_reference_headers_v29::<()>()?)?;
            let mut builder = SourceReferenceBuilderV29::new_with_root(
                plan.instances,
                SourceReferenceStorageV29::ScalarCells,
                Some(root),
                budget,
            )?;
            builder.function(plan.root, None, budget)?;
            let node = joined_inactive(&builder.plan);
            let row = builder.plan.nodes[node];
            let shape = row.inactive.unwrap();
            let choices =
                builder.plan.inactive_choices[shape.first..shape.first + shape.count].to_vec();
            // Two discriminants per prior node, plus each same-type absent
            // candidate's charged prefix comparison and length check. The
            // canonical row is the first exact recipe; no vector grows.
            let exact = (node + 1) * 2
                + builder.plan.nodes[..=node]
                    .iter()
                    .filter(|candidate| {
                        candidate.ty == row.ty
                            && candidate.kind == SourceReferenceNodeKindV29::Absent
                    })
                    .map(|candidate| candidate.inactive.unwrap().count.min(choices.len()) + 1)
                    .sum::<usize>();
            let before = budget.storage();
            budget.charge_work(usize::MAX - budget.work() - exact + usize::from(short))?;
            let before_work = budget.work();
            let result = builder.publish_inactive_node(row.ty, &choices, budget);
            assert_eq!(result.is_err(), short);
            assert_eq!(budget.storage(), before);
            // The final whole comparison charge is atomic. A one-short meter
            // retains the consumed prefix, not a partially debited comparison.
            assert_eq!(
                budget.work().checked_sub(before_work).unwrap(),
                exact - if short { choices.len() + 1 } else { 0 }
            );
            if !short {
                assert_eq!(result.as_ref().unwrap(), &node);
            }
            drop(builder);
            budget.release_storage(budget.storage() - floor)?;
            checked.set(true);
            result?;
            Err::<(), _>(source_reference_error_v29("completed inactive canonical boundary").into())
        });
        assert!(
            checked.get(),
            "resource assertions must finish without a caught panic"
        );
        if short {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "completed inactive canonical boundary",
                    ..
                })
            ));
        }
    }
}

#[test]
fn source_inactive_join_reuses_the_published_recipe_at_exact_and_one_short_storage() {
    for short in [false, true] {
        let result = inactive_scope!(|plan, root, budget| {
            let floor = budget.storage();
            budget.reserve_storage(source_reference_headers_v29::<()>()?)?;
            let mut builder = SourceReferenceBuilderV29::new_with_root(
                plan.instances,
                SourceReferenceStorageV29::ScalarCells,
                Some(root),
                budget,
            )?;
            builder.function(plan.root, None, budget)?;
            let joined = joined_inactive(&builder.plan);
            let shape = builder.plan.nodes[joined].inactive.unwrap();
            assert_eq!(shape.count, 2);
            let choices = &builder.plan.inactive_choices[shape.first..shape.first + shape.count];
            let singleton = |choice| {
                builder
                    .plan
                    .nodes
                    .iter()
                    .position(|node| {
                        node.ty == REFERENCE
                            && node.kind == SourceReferenceNodeKindV29::Absent
                            && node.inactive.is_some_and(|shape| {
                                shape.count == 1
                                    && builder.plan.inactive_choices[shape.first] == choice
                            })
                    })
                    .unwrap()
            };
            let left = singleton(choices[0]);
            let right = singleton(choices[1]);
            let counts = (
                builder.plan.nodes.len(),
                builder.plan.inactive_choices.len(),
                builder.plan.inactive_removals.len(),
            );
            // Union scratch consists of one Vec header and two distinct
            // existing receipt indices; canonical publication allocates nothing.
            let header = std::mem::size_of::<Vec<usize>>();
            let exact = header + 2 * std::mem::size_of::<usize>();
            let padding = budget.storage_limit() - budget.storage() - exact + usize::from(short);
            budget.reserve_storage(padding)?;
            let before = budget.storage();
            let result = builder.merge_inactive_nodes(left, right, budget);
            assert_eq!(result.is_err(), short);
            assert_eq!(budget.storage(), before + if short { header } else { 0 });
            assert_eq!(
                (
                    builder.plan.nodes.len(),
                    builder.plan.inactive_choices.len(),
                    builder.plan.inactive_removals.len()
                ),
                counts
            );
            if !short {
                assert_eq!(result.as_ref().unwrap(), &joined);
            }
            budget.release_storage(padding)?;
            drop(builder);
            // Any failed construction remains paid until all builder scratch is
            // gone; this is the same enclosing owner boundary as the source run.
            budget.release_storage(budget.storage() - floor)?;
            result?;
            Ok(())
        });
        if short {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            ));
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn source_inactive_every_alternative_must_agree_on_the_checked_physical_type() {
    inactive_scope!(|plan, root, budget| {
        let floor = budget.storage();
        budget.reserve_storage(source_reference_headers_v29::<()>()?)?;
        let mut builder = SourceReferenceBuilderV29::new_with_root(
            plan.instances, SourceReferenceStorageV29::ScalarCells, Some(root), budget,
        )?;
        builder.function(plan.root, None, budget)?;
        builder.finish_effects(budget)?;
        builder.plan_scalar_cells(budget)?;
        let joined = joined_inactive(&builder.plan);
        let shape = builder.plan.nodes[joined].inactive.unwrap();
        let first = builder.plan.inactive_choices[shape.first];
        let second = builder.plan.inactive_choices[shape.first + 1];
        // Deliberately corrupt only the later retained recipe's prior physical
        // representation. This is a hostile graph test, not source admission.
        // The original source locator/type remains valid, but the first recipe's
        // scalar referent cannot silently stand for the second recipe's pointer.
        let prior = builder.node(REFERENCE, SourceReferenceNodeKindV29::Plain(None), budget)?;
        let old = &builder.plan.inactive_removals[second];
        budget.reserve_storage(std::mem::size_of::<SourceReferenceInactiveRemovalV29>())?;
        let receipt = SourceReferenceInactiveRemovalV29 {
            site: old.site, source: old.source, source_local: old.source_local, kind: old.kind,
            instance: old.instance, local: old.local, generation: old.generation, ty: old.ty,
            projections: old.projections.clone(), prior, after: old.after,
        };
        let altered = builder.plan.inactive_removals.len();
        emission_push_v1(&mut builder.plan.inactive_removals, receipt, budget)?;
        let node = builder.publish_inactive_node(REFERENCE, &[first, altered], budget)?;
        let error = source_reference_cfg_node_types_v29(&builder.plan, node, budget).unwrap_err();
        assert!(matches!(error, ProductionSemanticKirErrorV1::Unsupported {
            detail: "inactive source alternatives require a common checked backing representation", ..
        }), "{error:?}");
        drop(builder);
        budget.release_storage(budget.storage() - floor)?;
        Ok(())
    }).unwrap();
}
