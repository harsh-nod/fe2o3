use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1 as SourceBlock;
use std::panic::{AssertUnwindSafe, catch_unwind};

const LIMIT: usize = 512 * 1024 * 1024;

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

fn inspect(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let frames = FramePlan::derive(plan, slots, out)?;
    frames.check(plan, slots, out)?;
    let (mut suspended, mut inactive) = (0, 0);
    for (index, frame) in frames.frames.iter().enumerate() {
        let original = plan.instance(frame.root, frame.instance, out)?;
        assert_eq!(frame.locals, original.locals);
        assert_eq!(frame.function, original.function);
        assert_eq!(frame.active, original.active);
        inactive += usize::from(!frame.active);
        if let Some(parent) = frame.parent_call {
            let call = &frames.calls[parent];
            assert_eq!(call.root, frame.root);
            assert_eq!(call.child, Some(frame.instance));
            assert_eq!(call.child_active, frame.active);
            if !frame.active {
                assert!(call.demands.is_empty());
            }
            assert_eq!(
                original.incoming,
                Some((call.caller, SourceBlock::from_index(call.block as u32)))
            );
            assert!(call.caller < frame.instance);
            let parent = &frames.frames[frames.roots[frame.root].start + call.caller];
            assert_eq!(frame.depth, parent.depth + 1);
            if !call.demands.is_empty() {
                // Only the first call suspends arguments needed by the second;
                // the second continuation returns Unit without reading locals.
                assert_eq!((call.caller, call.block), (0, 0));
                assert_eq!(
                    frames.demands[call.demands.clone()]
                        .iter()
                        .map(|row| row.source.local)
                        .collect::<Vec<_>>(),
                    vec![1, 2]
                );
            }
            suspended += usize::from(!call.demands.is_empty());
        } else {
            assert_eq!((frame.instance, frame.depth), (0, 0));
        }
        for cut in &frames.cuts[frame.cuts.clone()] {
            assert_eq!((cut.root, cut.instance), (frame.root, frame.instance));
            if !frame.active {
                assert!(!cut.reachable && cut.demands.is_empty());
            }
            for demand in &frames.demands[cut.demands.clone()] {
                assert_eq!(demand.frame, index);
                assert!(demand.source.local < frame.locals.len());
                assert_eq!(demand.source.components.block, cut.block);
            }
        }
    }
    assert_eq!(suspended, 2);
    assert!(inactive > 0);
    Ok(())
}

#[test]
fn source_frame_plan_retains_root_qualified_shared_carries_and_current_ssa() {
    fixture(LIMIT, LIMIT, inspect).0.unwrap();
}

#[test]
fn source_frame_plan_complete_derivation_has_exact_and_one_short_budgets() {
    let baseline = fixture(LIMIT, LIMIT, inspect);
    baseline.0.unwrap();
    let exact = fixture(baseline.1, baseline.3, inspect);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (baseline.1, baseline.2, baseline.3)
    );
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    assert!(matches!(fixture(baseline.1 - 1, baseline.3, inspect).0,
        Err(Error::Resource(Resource::Work(error))) | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
        if error.actual() == baseline.1 && error.limit() == baseline.1 - 1));
    assert!(matches!(fixture(baseline.1, baseline.3 - 1, inspect).0,
        Err(Error::Resource(Resource::Storage(error))) | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
        if error.actual() == baseline.3 && error.limit() == baseline.3 - 1));
}

#[test]
fn source_frame_plan_rejects_equal_source_foreign_plan_slots_and_demands() {
    fixture(LIMIT, LIMIT, |plan, slots, out| {
        let frames = FramePlan::derive(plan, slots, out)?;
        let foreign = InvocationPlan::derive(plan.source(out)?, out)?;
        assert!(matches!(
            frames.check(&foreign, slots, out),
            Err(Error::Statement(
                "original frame plan differs from its retained owner or call ancestry"
            ))
        ));
        super::super::source_function::tests::with_slots(plan, out, |foreign, out| {
            assert!(matches!(
                frames.check(plan, foreign, out),
                Err(Error::Statement(
                    "original frame plan differs from its retained owner or call ancestry"
                ))
            ));
            Ok(())
        })?;
        let first = frames
            .demands
            .iter()
            .position(|demand| demand.frame == 0)
            .unwrap();
        assert!(matches!(
            frames.leaf_required(1, first, 0, out),
            Err(Error::Statement(
                "original frame plan differs from its retained owner or call ancestry"
            ))
        ));
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn source_frame_plan_rejects_foreign_and_refunded_accounts_before_querying_demands() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    for foreign in [false, true] {
        let mut reached = false;
        let result = fixture(LIMIT, LIMIT, |plan, slots, out| {
            let frames = FramePlan::derive(plan, slots, out)?;
            reached = true;
            let error = if foreign {
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(out.budget.storage())?;
                let mut writer = Writer::new(&mut budget)?;
                frames.check(plan, slots, &mut writer).unwrap_err()
            } else {
                out.budget.release_storage(1)?;
                frames.check(plan, slots, out).unwrap_err()
            };
            assert!(matches!(
                error,
                Error::Resource(Resource::Accounting)
                    | Error::Source(SourceError::Resource(Resource::Accounting))
            ));
            assert!(frames.check(plan, slots, out).is_err());
            Err(error)
        });
        assert!(reached && result.0.is_err());
    }
}

#[test]
fn source_frame_inactive_child_keeps_reachable_call_without_suspended_bindings() {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticBasicBlockV1, SemanticFunctionDeclV1, SemanticTerminatorKindV1,
        SemanticTerminatorV1,
    };
    let mut observed = 0;
    let result = super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |_, functions| {
            // A genuine nonreturning first helper leaves each later call in the
            // original SSA CFG, but its retained child is authenticated inactive.
            let helper = functions.last_mut().unwrap();
            let source = helper.source();
            let old = &helper.blocks()[0];
            let blocks = vec![
                SemanticBasicBlockV1::new(
                    old.identity(),
                    source,
                    old.statements().to_vec(),
                    SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Abort),
                )
                .unwrap(),
            ];
            *helper = SemanticFunctionDeclV1::new(
                helper.identity(),
                helper.role(),
                helper.item_definition_identity(),
                helper.monomorphization_identity(),
                helper.generic_type_arguments_identity(),
                helper.const_generic_arguments_identity(),
                source,
                helper.abi().clone(),
                helper.locals().to_vec(),
                helper.entry(),
                blocks,
            )
            .unwrap();
        },
        |plan, out| {
            super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let frames = FramePlan::derive(plan, slots, out)?;
                let source = plan.source(out)?;
                for frame in &frames.frames {
                    for call in &frames.calls[frame.calls.clone()] {
                        let Some(child) = call.child else { continue };
                        let original = plan.instance(call.root, child, out)?;
                        assert_eq!(
                            call.child_active,
                            source.instance_active(call.root, child, out.budget)?
                        );
                        assert_eq!(call.child_active, original.active);
                        if !frame.active || !call.reachable || call.child_active {
                            continue;
                        }
                        let retained = &frames.frames[frames.roots[call.root].start + child];
                        assert!(!retained.active);
                        assert!(retained.parent_call.is_some());
                        assert!(call.demands.is_empty());
                        assert_eq!(call.block, 1);
                        let start = out.text.len();
                        super::super::expanded_frame_contract::emit_carry(
                            frame,
                            call,
                            out,
                            |_| panic!("inactive child must not request target bindings"),
                        )?;
                        let emitted = &out.text[start..];
                        assert!(emitted.starts_with("spec fn invocation_expanded_carry_"));
                        assert!(emitted.ends_with("-> bool { false }\n"));
                        observed += 1;
                    }
                }
                Ok(())
            })
        },
    );
    result.0.unwrap();
    assert_eq!(observed, 2);
}

fn ancestry(mutation: usize, work: usize) -> (Result<(Option<usize>, usize)>, usize) {
    use super::super::super::invocations::{Instance, Root};
    let root = Root {
        function: Function::from_index(0),
        physical: 0,
        instances: 0..2,
    };
    let mut row = Instance {
        function: Function::from_index(1),
        incoming: Some((0, SourceBlock::from_index(3))),
        active: true,
        locals: 4..8,
        blocks: 4..8,
        calls: 1..1,
    };
    let mut frames = [Frame {
        root: 0,
        instance: 0,
        function: root.function,
        active: true,
        locals: 0..4,
        cuts: 0..4,
        calls: 0..1,
        parent_call: None,
        depth: 0,
    }];
    let mut calls = [Call {
        root: 0,
        caller: 0,
        block: 3,
        child: Some(1),
        child_active: true,
        kind: CallKind::Direct,
        reachable: true,
        continuation: Some(4),
        demands: 0..1,
    }];
    match mutation {
        0 => (),
        1 => row.incoming = None,
        2 => row.incoming = Some((1, SourceBlock::from_index(3))),
        3 => row.incoming = Some((2, SourceBlock::from_index(3))),
        4 => calls[0].child = None,
        5 => calls[0].root = 1,
        6 => frames[0].root = 1,
        7 => calls[0].block = 4,
        8 => calls[0].reachable = false,
        9 => frames[0].active = false,
        10 => calls[0].kind = CallKind::Tail,
        11 => calls[0].continuation = None,
        12 => {
            row.active = false;
            calls[0].reachable = false;
            calls[0].child_active = false;
        }
        13 => calls[0].child_active = false,
        _ => unreachable!(),
    }
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, 0);
    let result = parent_link(0, 1, &root, &row, &frames, &calls, &mut budget);
    assert_eq!(budget.storage(), 0);
    (result, budget.work())
}

#[test]
fn source_frame_ancestry_rejects_missing_foreign_cyclic_and_noncontinuing_edges() {
    assert_eq!(ancestry(0, LIMIT).0.unwrap(), (Some(0), 1));
    for mutation in 1..=11 {
        assert!(
            matches!(
                ancestry(mutation, LIMIT).0,
                Err(Error::Statement(
                    "original frame plan differs from its retained owner or call ancestry"
                ))
            ),
            "mutation {mutation}"
        );
    }
    // Inactive provenance is retained, without declaring a suspended frame.
    assert_eq!(ancestry(12, LIMIT).0.unwrap(), (Some(0), 1));
    assert!(matches!(ancestry(13, LIMIT).0, Err(Error::Statement(_))));
    let measured = ancestry(0, LIMIT).1;
    assert_eq!(ancestry(0, measured).0.unwrap(), (Some(0), 1));
    assert!(
        matches!(ancestry(0, measured - 1).0, Err(Error::Resource(Resource::Work(error))) if error.actual() == measured)
    );
}

#[test]
fn source_frame_plan_unwind_keeps_original_account_and_outer_scope_settlement() {
    let result = fixture(LIMIT, LIMIT, |plan, slots, out| {
        let slot = std::ptr::from_ref(&*out.budget) as usize;
        let ledger = out.budget.work_ledger_identity_v1();
        let floor = out.budget.storage();
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let frames = FramePlan::derive(plan, slots, out).unwrap();
            assert!(!frames.demands.is_empty());
            std::panic::panic_any(281u32);
        }))
        .unwrap_err();
        assert_eq!(panic.downcast_ref::<u32>(), Some(&281));
        assert_eq!(slot, std::ptr::from_ref(&*out.budget) as usize);
        assert!(ledger == out.budget.work_ledger_identity_v1());
        assert!(out.budget.storage() >= floor);
        Ok(())
    });
    result.0.unwrap();
    assert_eq!(result.2, super::super::super::invocations::tests::FLOOR);
}
