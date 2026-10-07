use super::super::super::boundary::ControlInput;
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use fe2o3_mir_model::SsaDefinitionIdV1;
use std::collections::BTreeMap;

const LIMIT: usize = 256 * 1024 * 1024;

fn value(index: u32) -> Value {
    Value::Definition(SsaDefinitionIdV1::new(index))
}

#[test]
fn source_frame_events_preserve_uses_redefinitions_and_every_kill() {
    let a = Variable::new(0);
    let b = Variable::new(1);
    let events = [
        (
            0,
            Event::Use {
                variable: a,
                value: value(0),
            },
        ),
        (
            1,
            Event::Define {
                variable: b,
                value: value(1),
            },
        ),
        (
            2,
            Event::Kill {
                variable: a,
                previous: Some(value(0)),
            },
        ),
        (
            3,
            Event::Define {
                variable: a,
                value: value(2),
            },
        ),
        (
            4,
            Event::Kill {
                variable: b,
                previous: Some(value(1)),
            },
        ),
        (
            5,
            Event::Use {
                variable: a,
                value: value(2),
            },
        ),
    ];
    let mut values = [Some(value(0)), None];
    let mut work = Work::new(12);
    let mut budget = Budget::new(&mut work, 0);
    replay_events(&mut values, &events, &mut budget).unwrap();
    assert_eq!(values, [Some(value(2)), None]);
    assert_eq!(demanded_value(&values, a).unwrap(), value(2));
    assert!(matches!(
        demanded_value(&values, b),
        Err(Error::Statement(
            "original frame demand differs from its retained caller SSA"
        ))
    ));
    assert_eq!(budget.work(), 12);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn source_frame_event_budget_refuses_before_unpaid_mutation() {
    let a = Variable::new(0);
    let events = [(
        0,
        Event::Define {
            variable: a,
            value: value(1),
        },
    )];
    let mut values = [Some(value(0))];
    let mut work = Work::new(1);
    let mut budget = Budget::new(&mut work, 0);
    assert!(matches!(
        replay_events(&mut values, &events, &mut budget),
        Err(Error::Resource(_))
    ));
    assert_eq!(values, [Some(value(0))]);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn source_frame_events_reject_missing_or_out_of_range_demanded_locals() {
    let mut work = Work::new(32);
    let mut budget = Budget::new(&mut work, 0);
    for event in [
        Event::Define {
            variable: Variable::new(1),
            value: value(2),
        },
        Event::Kill {
            variable: Variable::new(1),
            previous: None,
        },
    ] {
        let mut values = [Some(value(0))];
        assert!(matches!(
            replay_events(&mut values, &[(0, event)], &mut budget),
            Err(Error::Statement(
                "original frame demand differs from its retained caller SSA"
            ))
        ));
        assert_eq!(values, [Some(value(0))]);
    }
    for (values, variable) in [([None], 0), ([Some(value(0))], 1)] {
        assert!(matches!(
            demanded_value(&values, Variable::new(variable)),
            Err(Error::Statement(
                "original frame demand differs from its retained caller SSA"
            ))
        ));
    }
}

fn fixture(
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &InvocationPlan<'_, '_>,
        &SourceSlots<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_variant(work, storage, true, |plan, out| {
        super::super::source_function::tests::with_slots(plan, out, |slots, out| {
            examine(plan, slots, out)
        })
    })
}

fn compare_original_demands(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let source = plan.source(out)?;
    let semantic = source.source_semantic(out.budget)?;
    let archive = source.source_ssa(out.budget)?;
    let mut cache = vector(semantic.functions().len(), out)?;
    cache.resize_with(semantic.functions().len(), || None);
    let mut observed = 0;
    let mut nonempty = 0;
    for root in 0..source.root_count(out.budget)? {
        for parent in 0..plan.root(root, out)?.instances.len() {
            let caller = plan.instance(root, parent, out)?;
            if !caller.active {
                continue;
            }
            let function = &semantic.functions()[caller.function.index() as usize];
            let ssa = archive.plan_for_function(caller.function).unwrap().plan();
            let successors: Vec<Vec<_>> = function
                .blocks()
                .iter()
                .map(|body| {
                    let mut edges = Vec::new();
                    body.terminator()
                        .kind()
                        .try_for_each_edge(|edge| {
                            edges.push(block(edge.target().index() as usize)?);
                            Ok::<_, Error>(())
                        })
                        .unwrap();
                    edges
                })
                .collect();
            let boundaries = Boundaries::derive(
                ssa,
                ControlInput {
                    entry: block(function.entry().index() as usize)?,
                    successors: &successors,
                },
                out,
            )?;
            for call in plan.calls(root, parent, out)? {
                let Some(child) = call.child else {
                    continue;
                };
                if !call.ssa_reachable || !plan.instance(root, child, out)?.active {
                    continue;
                }
                let Terminator::Call(original) = function.blocks()[call.block.index() as usize]
                    .terminator()
                    .kind()
                else {
                    continue;
                };
                let destination = original.destination().unwrap();
                assert!(destination.place().projections().is_empty());
                let at = block(call.block.index() as usize)?;
                let next = block(destination.edge().target().index() as usize)?;
                let mut expected = BTreeMap::new();
                for &variable in ssa.live_in(at).unwrap() {
                    expected.insert(variable, boundaries.value(at, variable, out)?);
                }
                for &(_, event) in ssa.resolved_events(at).unwrap() {
                    match event {
                        Event::Define { variable, value } => {
                            expected.insert(variable, value);
                        }
                        Event::Kill { variable, .. } => {
                            expected.remove(&variable);
                        }
                        Event::Use { .. } => (),
                    }
                }
                let expected: Vec<_> = ssa
                    .live_in(next)
                    .unwrap()
                    .iter()
                    .filter(|variable| variable.get() != destination.place().local().index())
                    .map(|variable| SourceDemand {
                        local: variable.get() as usize,
                        value: expected[variable],
                        components: ComponentCut::at(next.get() as usize),
                    })
                    .collect();
                let actual = caller_demands(
                    slots,
                    plan,
                    root,
                    parent,
                    call.block,
                    &boundaries,
                    &mut cache,
                    out,
                )?;
                assert_eq!(actual, expected);
                observed += 1;
                nonempty += usize::from(!actual.is_empty());
            }
        }
    }
    assert!(observed >= 4);
    assert!(nonempty > 0);
    Ok(())
}

#[test]
fn source_frame_caller_demands_match_independent_original_ssa_replay() {
    fixture(LIMIT, LIMIT, compare_original_demands).0.unwrap();
}

#[test]
fn source_frame_projected_return_keeps_only_unwritten_continuation_leaves() {
    use fe2o3_lower_mir_kernel::{
        ProductionPendingScopedSourceErrorV29 as Pending,
        ProductionSemanticKirErrorV1 as SemanticError,
    };
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticOperandV1 as Operand, SemanticProjectionKindV1 as Projection,
        SemanticScalarTypeV1 as Scalar, SemanticTerminatorKindV1 as Terminator,
        SemanticTypeShapeV1 as Shape,
    };
    let transform = |types: &mut Vec<_>, functions: &mut Vec<_>| {
        super::super::paired::aggregate_tests::call_transform(types, functions, false)
    };
    let mut callback = false;
    let refused = super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        transform,
        |_, _| {
            callback = true;
            Ok(())
        },
    );
    assert!(!callback);
    assert!(matches!(
        refused.0,
        Err(Error::Source(SourceError::Source(Pending::Source(
            SemanticError::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "typed allocation identity or representation requires its exact source contract"
            }
        ))))
    ));

    // The original SSA owner retains the projected tuple as storage. Verify that
    // boundary without fabricating scalar SSA or admitted SourceSlots custody.
    let owner =
        super::super::super::invocations::tests::try_source_ssa_transform(transform).unwrap();
    let semantic = owner.source_semantic();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT + headers())
        .unwrap();
    let mut out = Writer::new(&mut budget).unwrap();
    let mut projected = 0;
    for &root in semantic.roots() {
        let function = &semantic.functions()[root.index() as usize];
        let original = owner.plan_for_function(root).unwrap();
        let ssa = original.plan();
        assert!(
            original
                .retained_cross_edge_variables()
                .contains(&Variable::new(4))
        );
        assert!(!ssa.promoted_variables().contains(&Variable::new(4)));
        let successors: Vec<Vec<_>> = function
            .blocks()
            .iter()
            .map(|body| {
                let mut edges = Vec::new();
                body.terminator()
                    .kind()
                    .try_for_each_edge(|edge| {
                        edges.push(block(edge.target().index() as usize).unwrap());
                        Ok::<_, ()>(())
                    })
                    .unwrap();
                edges
            })
            .collect();
        let boundaries = Boundaries::derive(
            ssa,
            ControlInput {
                entry: block(function.entry().index() as usize).unwrap(),
                successors: &successors,
            },
            &mut out,
        )
        .unwrap();
        let Shape::Tuple(pair) =
            semantic.types()[function.locals()[4].ty().index() as usize].shape()
        else {
            panic!("original pair")
        };
        assert_eq!(pair.fields().len(), 2);
        assert_eq!(
            semantic.types()[pair.fields()[1].index() as usize].shape(),
            &Shape::Scalar(Scalar::Bool)
        );
        let Terminator::Assert {
            condition: Operand::Move(boolean),
            ..
        } = function.blocks()[2].terminator().kind()
        else {
            panic!("original bool consumer")
        };
        assert_eq!(boolean.local().index(), 4);
        assert_eq!(boolean.projections().len(), 1);
        assert_eq!(boolean.projections()[0].kind(), Projection::Field(1));
        assert_eq!(boolean.ty(), pair.fields()[1]);
        for site in 0..2 {
            let Terminator::Call(call) = function.blocks()[site].terminator().kind() else {
                panic!("original call")
            };
            let destination = call.destination().unwrap();
            assert_eq!(destination.place().local().index(), 4);
            assert_eq!(destination.place().projections().len(), 1);
            assert_eq!(
                destination.place().projections()[0].kind(),
                Projection::Field(0)
            );
            assert_eq!(destination.place().ty(), pair.fields()[0]);
            let next = destination.edge().target().index() as usize;
            assert_eq!(next, site + 1);
            let mut values = vec![None; function.locals().len()];
            for &variable in ssa.live_in(block(site).unwrap()).unwrap() {
                let value = boundaries
                    .value(block(site).unwrap(), variable, &mut out)
                    .unwrap();
                values[variable.get() as usize] = Some(value);
            }
            let events = ssa.resolved_events(block(site).unwrap()).unwrap();
            replay_events(&mut values, events, out.budget).unwrap();
            for &(_, event) in events {
                let variable = match event {
                    Event::Define { variable, .. }
                    | Event::Kill { variable, .. }
                    | Event::Use { variable, .. } => variable,
                };
                assert_ne!(variable, Variable::new(4));
            }
            assert!(
                !ssa.live_in(block(next).unwrap())
                    .unwrap()
                    .contains(&Variable::new(4))
            );
            assert_eq!(values[4], None);
            let cut = ComponentCut {
                block: next,
                overwritten: Some((0, 1)),
            };
            assert!(matches!(
                select_demand(&values, Variable::new(4), cut, true, true),
                Err(Error::Statement(
                    "original frame demand differs from its retained caller SSA"
                ))
            ));
            projected += 1;
        }
    }
    assert_eq!(projected, 4);

    // Synthetic selection-only inputs cover the pure branch; these values do
    // not represent the unsupported retained tuple above or an admitted owner.
    let values = [Some(value(0))];
    let variable = Variable::new(0);
    let cut = ComponentCut {
        block: 1,
        overwritten: Some((0, 1)),
    };
    let selected = select_demand(&values, variable, cut, true, true)
        .unwrap()
        .unwrap();
    assert_eq!(selected.value, value(0));
    assert_eq!(selected.components, cut);
    assert!(
        select_demand(&values, variable, cut, true, false)
            .unwrap()
            .is_none()
    );
    assert!(
        select_demand(&values, variable, ComponentCut::at(1), true, true)
            .unwrap()
            .is_none()
    );
}

#[test]
fn source_frame_complete_extraction_has_exact_and_short_resource_replay() {
    let (result, work, restored, peak) = fixture(LIMIT, LIMIT, compare_original_demands);
    result.unwrap();
    assert_eq!(restored, super::super::super::invocations::tests::FLOOR);
    let exact = fixture(work, peak, compare_original_demands);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (work, restored, peak));
    assert!(
        matches!(fixture(work - 1, peak, compare_original_demands).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == work && error.limit() == work - 1)
    );
    assert!(
        matches!(fixture(work, peak - 1, compare_original_demands).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == peak && error.limit() == peak - 1)
    );
}

#[test]
fn source_frame_component_cache_rejects_foreign_slots_and_other_functions() {
    fixture(LIMIT, LIMIT, |plan, slots, out| {
        let function = plan.instance(0, 0, out)?.function;
        let other = plan.instance(1, 0, out)?.function;
        assert_ne!(function, other);
        let demands = ComponentDemandsV42::derive(slots, function, out)?;
        demands.check_owner_v281(slots, function, out)?;
        assert!(matches!(
            demands.check_owner_v281(slots, other, out),
            Err(Error::Statement(
                "original aggregate component demand differs from its source CFG"
            ))
        ));
        super::super::source_function::tests::with_slots(plan, out, |foreign, out| {
            assert!(!std::ptr::eq(slots, foreign));
            assert!(matches!(
                demands.check_owner_v281(foreign, function, out),
                Err(Error::Statement(
                    "original aggregate component demand differs from its source CFG"
                ))
            ));
            Ok(())
        })
    })
    .0
    .unwrap();
}
