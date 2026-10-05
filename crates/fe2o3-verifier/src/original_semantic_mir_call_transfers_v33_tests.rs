use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticConstantV1, SemanticConstantValueV1, SemanticPlaceV1, SemanticScalarValueV1,
};

const LIMIT: usize = 100_000_000;
const FLOOR: usize = 37;

#[test]
fn original_mir_call_transfer_direct_work_and_backing_have_independent_oracles() {
    run(LIMIT, LIMIT, |plan, out| {
        let source = plan.source(out)?;
        let semantic = source.source_semantic(out.budget)?;
        let caller = plan.instance(0, 0, out)?;
        let child = plan.calls(0, 0, out)?[0].child.unwrap();
        let callee = plan.instance(0, child, out)?;
        let caller_body = &semantic.functions()[caller.function.index() as usize];
        let callee_body = &semantic.functions()[callee.function.index() as usize];
        let Terminator::Call(call) = caller_body.blocks()[0].terminator().kind() else {
            unreachable!()
        };
        assert_eq!(
            (
                caller_body.locals().len(),
                callee_body.locals().len(),
                call.arguments().len(),
                callee_body.blocks().len()
            ),
            (4, 4, 2, 1)
        );
        // Fixed checks, two separately paid caller-local initialization walks,
        // parameter setup, callee roles, two arguments, reads, and return census.
        let work = 8 + 4 + 4 + 2 + 3 * 4 + 2 * (4 + 1 + 3) + 2 * 4 + 1;
        assert_eq!(work, 55);
        let payload = 6 * size_of::<NodeV30>()
            + (4 + 2) * size_of::<Option<usize>>()
            + 4 * size_of::<bool>()
            + 2 * size_of::<(usize, usize)>()
            + (4 + 4 + 1) * size_of::<usize>();
        let floor = 29 + super::super::super::super::SOURCE_LIMIT + headers();
        for (work_limit, storage_limit, failure) in [
            (work, floor + payload, 0),
            (work - 1, floor + payload, 1),
            (work, floor + payload - 1, 2),
        ] {
            let mut ledger = Work::new(work_limit);
            let mut budget = Budget::new(&mut ledger, storage_limit);
            budget.reserve_storage(floor)?;
            let result = (|| {
                let mut writer = Writer::new(&mut budget)?;
                let result = direct(
                    semantic.types(),
                    caller_body,
                    caller,
                    callee_body,
                    callee,
                    child,
                    0,
                    call,
                    &mut writer,
                )?;
                assert_eq!(result.arguments.len(), 2);
                assert_eq!(result.return_blocks.len(), 1);
                drop(result);
                Ok(())
            })();
            match failure {
                0 => {
                    result?;
                    assert_eq!((budget.work(), budget.storage()), (work, floor + payload));
                }
                1 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
                _ => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            }
            budget.release_storage(budget.storage() - floor)?;
            assert_eq!(budget.storage(), floor);
        }
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_call_transfer_census_pays_leaf_and_inactive_instance_scans() {
    run(LIMIT, LIMIT, |plan, out| {
        // Each root retains two reachable leaf invocations. The unreachable
        // original call remains a census row without inventing a child instance.
        assert_eq!(plan.root(0, out)?.instances.len(), 3);
        assert_eq!(plan.root(1, out)?.instances.len(), 3);
        for root in 0..2 {
            let calls = plan.calls(root, 0, out)?;
            assert_eq!(calls.len(), 3);
            assert!(!calls[2].ssa_reachable);
            assert_eq!(calls[2].child, None);
        }
        let before = out.budget.work();
        let transfers = CallTransfers::derive(plan, out)?;
        // Two source queries, two root passes, two per-instance scans, six
        // call-site checks, and four independently costed direct transfers.
        let expected = 2 + 2 * 2 + 6 * (1 + 2) + 6 * 5 + 4 * 55;
        assert_eq!(expected, 274);
        assert_eq!(out.budget.work() - before, expected);
        assert_eq!(transfers.rows(out)?.len(), 6);
        assert_eq!(
            transfers
                .rows(out)?
                .iter()
                .filter(|row| matches!(row.transfer, Transfer::Inactive))
                .count(),
            2
        );
        Ok(())
    })
    .0
    .unwrap();
}

fn run(
    work: usize,
    storage: usize,
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::invocations::tests::run(work, storage, examine)
}

fn check(plan: &mut InvocationPlan<'_, '_>, out: &mut Writer<'_, '_>) -> Result<()> {
    let transfers = CallTransfers::derive(plan, out)?;
    let rows = transfers.rows(out)?;
    assert_eq!(rows.len(), 6);
    for root in 0..2 {
        let calls = plan.calls(root, 0, out)?;
        let selected: Vec<_> = rows.iter().filter(|row| row.root == root).collect();
        assert_eq!(selected.len(), 3);
        assert!(matches!(selected[2].transfer, Transfer::Inactive));
        for (ordinal, row) in selected[..2].iter().enumerate() {
            assert_eq!(row.site, calls[ordinal]);
            let Transfer::Direct(transfer) = &row.transfer else {
                panic!("reachable direct call lost its transfer")
            };
            let caller = plan.instance(root, 0, out)?;
            let callee = plan.instance(root, row.site.child.unwrap(), out)?;
            assert_eq!(transfer.child, row.site.child.unwrap());
            assert_eq!(transfer.site, caller.blocks.start + ordinal);
            assert_eq!(transfer.entry, callee.blocks.start);
            assert_eq!(transfer.child_locals, callee.locals);
            assert_eq!(transfer.caller_locals, caller.locals);
            assert_eq!(
                transfer.reads,
                [caller.locals.start + 1, caller.locals.start + 2]
            );
            assert!(transfer.moved.is_empty());
            assert_eq!(
                transfer.arguments,
                [(callee.locals.start + 1, 0), (callee.locals.start + 2, 1)]
            );
            assert_eq!(transfer.return_blocks, [callee.blocks.start]);
            assert_eq!(
                transfer.returned.continuation,
                caller.blocks.start + ordinal + 1
            );
            assert_eq!(transfer.returned.destination, caller.locals.start + 3);
            assert_eq!(transfer.returned.source, Some(callee.locals.start));
            assert_eq!(
                transfer.returned.scalar,
                ScalarV30::Integer {
                    width: 32,
                    signed: false
                }
            );
        }
    }
    transfers.emit_steps(out)?;
    Ok(())
}

#[test]
fn original_mir_call_transfers_keep_all_roots_repeated_callees_and_inactive_sites() {
    let first = run(LIMIT, LIMIT, check);
    let second = run(LIMIT, LIMIT, check);
    first.0.unwrap();
    second.0.unwrap();
    assert_eq!((first.1, first.2, first.3), (second.1, FLOOR, second.3));
}

#[test]
fn original_mir_call_transfers_have_exact_and_one_short_cumulative_resources() {
    let measured = run(LIMIT, LIMIT, check);
    measured.0.unwrap();
    let exact = run(measured.1, measured.3, check);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (measured.1, FLOOR, measured.3));
    assert!(matches!(run(measured.1 - 1, measured.3, check).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1));
    assert!(matches!(run(measured.1, measured.3 - 1, check).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1));
}

fn altered(
    plan: &InvocationPlan<'_, '_>,
    out: &mut Writer<'_, '_>,
    arguments: impl FnOnce(&Call) -> Vec<Operand>,
) -> Result<DirectTransfer> {
    out.budget.reserve_storage(headers())?;
    let source = plan.source(out)?;
    let semantic = source.source_semantic(out.budget)?;
    let site = plan.calls(0, 0, out)?[0];
    let caller = plan.instance(0, 0, out)?;
    let child = site.child.unwrap();
    let callee = plan.instance(0, child, out)?;
    let caller_body = &semantic.functions()[caller.function.index() as usize];
    let callee_body = &semantic.functions()[callee.function.index() as usize];
    let Terminator::Call(call) = caller_body.blocks()[0].terminator().kind() else {
        unreachable!()
    };
    // This exercises only the private original-operand interpreter. It does
    // not issue a source-owned call receipt for the altered standalone call.
    let call = Call::new_callable(
        call.callee(),
        arguments(call),
        call.destination().cloned(),
        call.unwind(),
    )
    .unwrap();
    direct(
        semantic.types(),
        caller_body,
        caller,
        callee_body,
        callee,
        child,
        0,
        &call,
        out,
    )
}

fn copied_place(operand: &Operand) -> SemanticPlaceV1 {
    let Operand::Copy(place) = operand else {
        panic!("copy fixture")
    };
    place.clone()
}

#[test]
fn original_mir_call_argument_order_preserves_copy_then_move_and_refuses_use_after_move() {
    let result = run(LIMIT, LIMIT, |plan, out| {
        let transfer = altered(plan, out, |call| {
            let place = copied_place(&call.arguments()[0]);
            vec![Operand::Copy(place.clone()), Operand::Move(place)]
        })?;
        assert_eq!(transfer.operands.nodes.len(), 1);
        assert_eq!(transfer.arguments[0].1, transfer.arguments[1].1);
        assert_eq!(transfer.reads, [transfer.caller_locals.start + 1]);
        assert_eq!(transfer.moved, transfer.reads);
        let rejected = altered(plan, out, |call| {
            let place = copied_place(&call.arguments()[0]);
            vec![Operand::Move(place.clone()), Operand::Copy(place)]
        });
        assert!(matches!(
            rejected,
            Err(Error::Statement("original MIR scalar use is undefined"))
        ));
        let constants = altered(plan, out, |call| {
            call.arguments()
                .iter()
                .map(|operand| {
                    Operand::Constant(SemanticConstantV1::new(
                        operand.ty(),
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(17, 4).unwrap()),
                    ))
                })
                .collect()
        })?;
        assert!(constants.reads.is_empty() && constants.moved.is_empty());
        assert!(
            constants
                .operands
                .nodes
                .iter()
                .all(|node| node.expression == ExpressionV30::Constant(17))
        );
        Ok(())
    });
    result.0.unwrap();
    assert_eq!(result.2, FLOOR);
}

#[test]
fn original_mir_call_steps_reset_callee_frame_and_bind_exact_return_control() {
    let result = run(LIMIT, LIMIT, |plan, out| {
        let transfer = altered(plan, out, |call| {
            let place = copied_place(&call.arguments()[0]);
            vec![Operand::Copy(place.clone()), Operand::Move(place)]
        })?;
        let start = out.text.len();
        out.budget.reserve_storage(generate::headers())?;
        generate::direct(&transfer, out)?;
        let text = &out.text[start..];
        let caller = transfer.caller_locals.start;
        let child = transfer.child_locals.start;
        let end = transfer.child_locals.end;
        let key = transfer.site;
        assert!(text.contains(&format!(
            "s.pc == {key}int && s.values.len() == s.defined.len()"
        )));
        assert!(text.contains(&format!(
            "s.defined[{}] && 0 <= s.values[{}] < 4294967296int",
            caller + 1,
            caller + 1
        )));
        assert!(text.contains(&format!(
            "let values = s.values.update({}int, n[0]).update({}int, n[0]);",
            child + 1,
            child + 2
        )));
        assert!(text.contains(&format!("if {child} <= i < {end} {{ false }} else {{ s.defined[i] }}).update({}int, false).update({}int, true).update({}int, true);", caller + 1, child + 1, child + 2)));
        assert!(text.contains(&format!(
            "&& (false || s.pc == {}int)",
            transfer.return_blocks[0]
        )));
        assert!(text.contains(&format!(
            "let values = s.values.update({}int, s.values[{child}]);",
            caller + 3
        )));
        assert!(text.contains(&format!("pc: {}int, values, defined, cells: s.cells, initialized: s.initialized, external: s.external", transfer.returned.continuation)));
        assert_eq!(
            text.matches("cells: s.cells, initialized: s.initialized, external: s.external")
                .count(),
            2
        );
        Ok(())
    });
    result.0.unwrap();
    assert_eq!(result.2, FLOOR);
}

#[test]
fn original_mir_call_unit_return_uses_original_unit_destination_without_a_scalar_leaf() {
    let result = super::super::invocations::tests::run_variant(LIMIT, LIMIT, true, |plan, out| {
        let transfers = CallTransfers::derive(plan, out)?;
        for row in transfers.rows(out)? {
            if let Transfer::Direct(transfer) = &row.transfer {
                assert_eq!(transfer.returned.scalar, ScalarV30::Unit);
                assert_eq!(transfer.returned.source, None);
                assert_eq!(transfer.returned.destination, transfer.caller_locals.start);
            }
        }
        transfers.emit_steps(out)?;
        for row in transfers.rows(out)? {
            if let Transfer::Direct(transfer) = &row.transfer {
                assert!(out.text.contains(&format!(
                    "let values = s.values.update({}int, 0int);",
                    transfer.returned.destination
                )));
            }
        }
        Ok(())
    });
    result.0.unwrap();
    assert_eq!(result.2, FLOOR);
}

#[test]
fn original_mir_call_transfer_rejects_funded_foreign_ledger_before_emit_or_debit() {
    let result = run(LIMIT, LIMIT, |plan, out| {
        let transfers = CallTransfers::derive(plan, out)?;
        let mut work = Work::new(LIMIT);
        let mut foreign = Budget::new(&mut work, LIMIT);
        foreign.reserve_storage(transfers.required + super::super::super::super::SOURCE_LIMIT)?;
        let mut writer = Writer::new(&mut foreign)?;
        let before = (
            writer.budget.work(),
            writer.budget.storage(),
            writer.budget.peak_storage(),
        );
        let result = transfers.emit_steps(&mut writer);
        assert!(matches!(
            result,
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        assert_eq!(
            (
                writer.budget.work(),
                writer.budget.storage(),
                writer.budget.peak_storage()
            ),
            before
        );
        assert!(writer.text.is_empty());
        result
    });
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn original_mir_call_transfer_query_rejects_lost_retained_floor_before_work() {
    let result = run(LIMIT, LIMIT, |plan, out| {
        let transfers = CallTransfers::derive(plan, out)?;
        out.budget.release_storage(1)?;
        let before = (
            out.budget.work(),
            out.budget.storage(),
            out.budget.peak_storage(),
        );
        let result = transfers.rows(out).map(|_| ());
        assert!(matches!(
            result,
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        assert_eq!(
            (
                out.budget.work(),
                out.budget.storage(),
                out.budget.peak_storage()
            ),
            before
        );
        result
    });
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn original_mir_call_transfer_frames_have_independent_field_and_result_oracles() {
    type ReturnFields = (usize, usize, Option<usize>, ScalarV30);
    type DirectFields = (
        usize,
        usize,
        usize,
        Range<usize>,
        Range<usize>,
        SourceProgramV30,
        Vec<(usize, usize)>,
        Vec<usize>,
        Vec<usize>,
        Vec<usize>,
        ReturnFields,
    );
    #[allow(dead_code)]
    enum TransferFields {
        Inactive,
        Direct(DirectFields),
    }
    type RowFields = (usize, CallSite, TransferFields);
    type OwnerFields<'a> = (&'a InvocationPlan<'a, 'a>, Vec<CallRow>, usize);
    assert_eq!(size_of::<ReturnTransfer>(), size_of::<ReturnFields>());
    assert_eq!(size_of::<DirectTransfer>(), size_of::<DirectFields>());
    assert_eq!(size_of::<Transfer>(), size_of::<TransferFields>());
    assert_eq!(size_of::<CallRow>(), size_of::<RowFields>());
    assert_eq!(
        size_of::<CallTransfers<'_, '_, '_>>(),
        size_of::<OwnerFields<'_>>()
    );
    type Frame<'a> = (
        OwnerFields<'a>,
        RowFields,
        DirectFields,
        ReturnFields,
        Result<CallTransfers<'a, 'a, 'a>>,
        Result<DirectTransfer>,
        Result<&'a [CallRow]>,
        Result<&'a Source<'a>>,
        Vec<CallRow>,
        Vec<(usize, usize)>,
        [Vec<usize>; 3],
        Vec<Option<usize>>,
        Vec<bool>,
        SourceProgramV30,
        NodeV30,
        Option<usize>,
        Result<usize>,
        [usize; 16],
        [&'a Function; 2],
        [&'a Instance; 2],
        &'a Call,
        &'a [Type],
        &'a mut Writer<'a, 'a>,
    );
    fn vector<T>() -> usize {
        size_of::<Vec<T>>() + 2 * size_of::<Result<Vec<T>>>()
    }
    let interpreter = size_of::<(
        &mut SourceProgramV30,
        &[Type],
        &Function,
        usize,
        &super::super::Statement,
        &mut Writer<'_, '_>,
        &super::super::Place,
        ScalarV30,
        usize,
        super::super::AssignmentV30,
        Result<()>,
    )>() + size_of::<(
        &mut SourceProgramV30,
        &[Type],
        &Function,
        &mut Writer<'_, '_>,
        usize,
        Option<usize>,
        Result<()>,
    )>();
    let expected = size_of::<Frame<'_>>()
        + std::mem::align_of::<Frame<'_>>()
        + vector::<CallRow>()
        + vector::<NodeV30>()
        + vector::<Option<usize>>()
        + vector::<bool>()
        + vector::<(usize, usize)>()
        + 3 * vector::<usize>()
        + size_of::<Result<ScalarV30>>()
        + size_of::<std::slice::Iter<'_, CallSite>>()
        + size_of::<
            std::iter::Enumerate<
                std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>,
            >,
        >()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, Operand>>>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, bool>>>()
        + size_of::<
            std::iter::Enumerate<
                std::slice::Iter<'_, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
            >,
        >()
        + 8 * size_of::<usize>()
        + interpreter;
    assert_eq!(headers(), expected);
    let emitter = 4 * size_of::<&()>()
        + 3 * size_of::<Result<()>>()
        + 11 * size_of::<usize>()
        + size_of::<u128>()
        + size_of::<std::slice::Iter<'_, CallRow>>()
        + size_of::<std::slice::Iter<'_, (usize, usize)>>()
        + size_of::<std::slice::Iter<'_, usize>>()
        + size_of::<std::slice::Iter<'_, NodeV30>>();
    assert_eq!(generate::headers(), emitter);
}
