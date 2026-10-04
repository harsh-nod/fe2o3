use super::super::{
    call_transfers::CallContext,
    control::{Branch, LiveIn, SourceBlock},
    invocations::{Instance, InvocationPlan, Root},
};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use std::mem::align_of;

const LIMIT: usize = 100_000_000;
const FLOOR: usize = 37;

fn run(
    examine: impl FnOnce(&mut InvocationPlan<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::invocations::tests::run(LIMIT, LIMIT, examine)
}

fn inspect(plan: &mut InvocationPlan<'_, '_>, out: &mut Writer<'_, '_>) -> Result<()> {
    let transfers = CallTransfers::derive(plan, out)?;
    let model = InvocationBodies::derive(&transfers, out)?;
    assert_eq!(model.roots.len(), 2);
    let mut calls = 0;
    let mut returns = 0;
    for (root, range) in model.roots.iter().enumerate() {
        assert_eq!(*range, plan.root(root, out)?.instances);
        assert_eq!(model.bodies[range.clone()].iter().flatten().count(), 3);
        for (instance, body) in model.bodies[range.clone()].iter().enumerate() {
            let row = plan.instance(root, instance, out)?;
            assert_eq!(body.is_some(), row.active);
            let Some(body) = body else { continue };
            assert_eq!((body.root, body.instance), (root, instance));
            assert_eq!(body.locals, row.locals);
            assert_eq!(body.blocks, row.blocks);
            for (block, source) in body.control.blocks.iter().enumerate() {
                let Some(source) = source else { continue };
                match source.branch {
                    Branch::Call {
                        transfer,
                        continuation,
                    } => {
                        let transfer = model.transfer(transfer, out)?;
                        assert_eq!(transfer.site, body.blocks.start + block);
                        assert_eq!(
                            transfer.returned.continuation,
                            body.blocks.start + continuation.get() as usize
                        );
                        assert_ne!(transfer.child_locals, body.locals);
                        calls += 1;
                    }
                    Branch::Return if body.returned.is_some() => {
                        let transfer = model.transfer(body.returned.unwrap(), out)?;
                        assert_eq!(transfer.child, instance);
                        assert!(
                            transfer
                                .return_blocks
                                .contains(&(body.blocks.start + block))
                        );
                        assert!(!source.program.assignments.is_empty());
                        assert!(source.program.returned.is_some());
                        returns += 1;
                    }
                    Branch::Return => assert_eq!(instance, 0),
                    _ => (),
                }
            }
        }
    }
    assert_eq!((calls, returns), (4, 4));
    let start = out.text.len();
    model.emit_steps(out)?;
    let text = &out.text[start..];
    assert_eq!(text.matches("spec fn original_invocation_step_").count(), 2);
    assert_eq!(
        text.matches("spec fn original_invocation_block_enabled_")
            .count(),
        10
    );
    assert_eq!(
        text.matches("let post = OriginalControlStateV31").count(),
        10
    );
    assert_eq!(text.matches("(post)").count(), 8);
    for body in model.bodies.iter().flatten() {
        for (block, source) in body.control.blocks.iter().enumerate() {
            let Some(source) = source else { continue };
            let key = body.blocks.start + block;
            let marker = format!(
                " if s.pc == {key}int {{\n let n = original_invocation_body_trace_{key}_v30"
            );
            let section = &text[text.find(&marker).expect("exact body dispatch")..];
            let post = section.find("let post = OriginalControlStateV31").unwrap();
            let state = section.find("let state = ").unwrap();
            assert!(post < state);
            for (local, (&changed, node)) in source
                .changed
                .iter()
                .zip(&source.program.locals)
                .enumerate()
            {
                if let (true, Some(node)) = (changed, node) {
                    let assignment =
                        format!(".update({}int, n[{node}])", body.locals.start + local);
                    assert!(section.find(&assignment).unwrap() < post);
                }
            }
            if let Some(returned) = body.returned {
                let transfer = model.transfer(returned, out)?;
                if source.branch == Branch::Return {
                    assert!(section[state..].starts_with(&format!(
                        "let state = original_call_return_{}_v33(post)",
                        transfer.site
                    )));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn original_mir_invocation_bodies_bind_all_roots_instances_and_post_statement_returns() {
    let first = run(inspect);
    let second = run(inspect);
    first.0.unwrap();
    second.0.unwrap();
    assert_eq!((first.1, first.2, first.3), (second.1, FLOOR, second.3));
}

#[test]
fn original_mir_invocation_bodies_keep_exact_and_one_short_whole_model_resources() {
    let measured = run(inspect);
    measured.0.unwrap();
    let exact = super::super::invocations::tests::run(measured.1, measured.3, inspect);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (measured.1, FLOOR, measured.3));
    assert!(
        matches!(super::super::invocations::tests::run(measured.1 - 1, measured.3, inspect).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
        if error.actual() == measured.1 && error.limit() == measured.1 - 1)
    );
    assert!(
        matches!(super::super::invocations::tests::run(measured.1, measured.3 - 1, inspect).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
        if error.actual() == measured.3 && error.limit() == measured.3 - 1)
    );
}

#[test]
fn original_mir_body_context_rejects_equal_byte_foreign_function_types_and_plan() {
    for fault in 0..3 {
        let result = run(|plan, out| {
            let transfers = CallTransfers::derive(plan, out)?;
            let context = transfers.context(0, 1, out)?;
            let source = plan.source(out)?;
            let semantic = source.source_semantic(out.budget)?;
            let ssa = source.source_ssa(out.budget)?;
            let instance = plan.instance(0, 1, out)?;
            let function = &semantic.functions()[instance.function.index() as usize];
            let original_plan = ssa.plan_for_function(instance.function).unwrap().plan();
            context.check_original(semantic.types(), function, original_plan, out)?;
            let foreign_function = function.clone();
            let foreign_types = semantic.types().to_vec();
            let foreign_instance = plan.instance(0, 0, out)?;
            let other_plan = ssa
                .plan_for_function(foreign_instance.function)
                .unwrap()
                .plan();
            let rejected = match fault {
                0 => {
                    context.check_original(semantic.types(), &foreign_function, original_plan, out)
                }
                1 => context.check_original(&foreign_types, function, original_plan, out),
                _ => context.check_original(semantic.types(), function, other_plan, out),
            };
            assert!(matches!(
                rejected,
                Err(Error::Statement(
                    "original MIR call transfer differs from its exact invocation"
                ))
            ));
            context.check_original(semantic.types(), function, original_plan, out)?;
            Ok(())
        });
        result.0.unwrap();
        assert_eq!(result.2, FLOOR);
    }
}

#[test]
fn original_mir_calls_require_the_exact_instance_context_and_keep_unit_return_bodies() {
    let ordinary = run(|plan, out| {
        let transfers = CallTransfers::derive(plan, out)?;
        let source = plan.source(out)?;
        let semantic = source.source_semantic(out.budget)?;
        let ssa = source.source_ssa(out.budget)?;
        let row = plan.instance(0, 0, out)?;
        let function = &semantic.functions()[row.function.index() as usize];
        let original = ssa.plan_for_function(row.function).unwrap().plan();
        assert!(matches!(
            SourceControl::derive(semantic.types(), function, original, out),
            Err(Error::Statement(
                "original MIR memory, call or exceptional control is not modeled"
            ))
        ));
        let context = transfers.context(0, 0, out)?;
        let model =
            SourceControl::derive_instance(semantic.types(), function, original, &context, out)?;
        assert!(matches!(
            model.blocks[0].as_ref().unwrap().branch,
            Branch::Call { .. }
        ));
        assert!(model.blocks[3].is_none());
        Ok(())
    });
    ordinary.0.unwrap();
    let unit = super::super::invocations::tests::run_variant(LIMIT, LIMIT, true, |plan, out| {
        let transfers = CallTransfers::derive(plan, out)?;
        let model = InvocationBodies::derive(&transfers, out)?;
        for body in model
            .bodies
            .iter()
            .flatten()
            .filter(|body| body.returned.is_some())
        {
            let transfer = model.transfer(body.returned.unwrap(), out)?;
            assert_eq!(transfer.returned.source, None);
            assert_eq!(transfer.returned.scalar, super::super::ScalarV30::Unit);
            assert_eq!(
                body.control.blocks[0]
                    .as_ref()
                    .unwrap()
                    .program
                    .assignments
                    .len(),
                1
            );
        }
        model.emit_steps(out)
    });
    unit.0.unwrap();
    assert_eq!((ordinary.2, unit.2), (FLOOR, FLOOR));
}

#[test]
fn original_mir_invocation_body_queries_reject_funded_foreign_ledger() {
    let result = run(|plan, out| {
        let transfers = CallTransfers::derive(plan, out)?;
        let model = InvocationBodies::derive(&transfers, out)?;
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(out.budget.storage())?;
        let other = Writer::new(&mut budget)?;
        let before = (
            other.budget.work(),
            other.budget.storage(),
            other.budget.peak_storage(),
        );
        let rejected = model.check(&other);
        assert!(matches!(
            rejected,
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        assert_eq!(
            (
                other.budget.work(),
                other.budget.storage(),
                other.budget.peak_storage()
            ),
            before
        );
        rejected
    });
    assert!(matches!(
        result.0,
        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
    ));
}

#[test]
fn original_mir_invocation_body_frames_have_independent_field_and_envelope_oracles() {
    #[allow(dead_code)]
    struct BodyFields {
        root: usize,
        instance: usize,
        locals: Range<usize>,
        blocks: Range<usize>,
        returned: Option<usize>,
        control: SourceControl,
    }
    #[allow(dead_code)]
    struct OwnerFields<'a> {
        transfers: &'a CallTransfers<'a, 'a, 'a>,
        roots: Vec<Range<usize>>,
        bodies: Vec<Option<BodyFields>>,
        locals: usize,
        required: usize,
    }
    assert_eq!(
        (size_of::<Body>(), align_of::<Body>()),
        (size_of::<BodyFields>(), align_of::<BodyFields>())
    );
    assert_eq!(
        (
            size_of::<InvocationBodies<'_, '_, '_, '_>>(),
            align_of::<InvocationBodies<'_, '_, '_, '_>>()
        ),
        (size_of::<OwnerFields<'_>>(), align_of::<OwnerFields<'_>>())
    );
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    let expected = h::<OwnerFields<'_>>()
        + h::<BodyFields>()
        + h::<Vec<Option<BodyFields>>>()
        + h::<Vec<Range<usize>>>()
        + h::<SourceControl>()
        + h::<Range<usize>>()
        + h::<Option<usize>>()
        + h::<&Instance>()
        + h::<&Root>()
        + h::<&InvocationPlan<'_, '_>>()
        + h::<&super::super::call_transfers::DirectTransfer>()
        + h::<&[super::super::call_transfers::CallRow]>()
        + h::<&fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'_>>()
        + h::<&fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1>()
        + h::<&fe2o3_pliron::ProductionSemanticSsaOwnerV1>()
        + h::<(&CallTransfers<'_, '_, '_>, &mut Writer<'_, '_>)>()
        + 12 * size_of::<usize>()
        + 8 * size_of::<&()>();
    assert_eq!(headers(), expected);
    let generate = size_of::<(&OwnerFields<'_>, &mut Writer<'_, '_>)>()
        + size_of::<(&BodyFields, &SourceBlock)>()
        + size_of::<&super::super::call_transfers::DirectTransfer>()
        + size_of::<std::slice::Iter<'_, Option<BodyFields>>>()
        + size_of::<std::slice::Iter<'_, LiveIn>>()
        + size_of::<std::slice::Iter<'_, super::super::AssignmentV30>>()
        + 8 * size_of::<Result<()>>()
        + 14 * size_of::<usize>()
        + size_of::<u128>()
        + size_of::<super::super::ExpressionV30>();
    assert_eq!(generate::headers(), generate);
    type ContextFields<'a> = (&'a CallTransfers<'a, 'a, 'a>, usize, usize);
    assert_eq!(
        (
            size_of::<CallContext<'_, '_, '_, '_>>(),
            align_of::<CallContext<'_, '_, '_, '_>>()
        ),
        (
            size_of::<ContextFields<'_>>(),
            align_of::<ContextFields<'_>>()
        )
    );
    type ContextFrame<'a> = (
        ContextFields<'a>,
        Result<ContextFields<'a>>,
        Result<&'a Instance>,
        Result<(usize, &'a super::super::call_transfers::DirectTransfer)>,
        Result<&'a super::super::call_transfers::DirectTransfer>,
        Result<&'a [super::super::call_transfers::CallRow]>,
        Result<()>,
        &'a CallTransfers<'a, 'a, 'a>,
        &'a ContextFields<'a>,
        &'a [super::super::Type],
        &'a super::super::Function,
        &'a fe2o3_mir_model::SsaConstructionPlanV1,
        &'a mut Writer<'a, 'a>,
        [usize; 8],
    );
    assert_eq!(
        super::super::call_transfers::context_headers(),
        size_of::<ContextFrame<'_>>() + align_of::<ContextFrame<'_>>()
    );
}
