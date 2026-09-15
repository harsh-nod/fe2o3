//! Live source-unit checks, not Workgroup lifetime or physical LDS authority.
use super::*;
use fe2o3_mir_model::SsaValueV1;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1, ProductionSemanticSsaSourceQueryErrorV1 as QueryError,
};

#[path = "consumer_tests.rs"]
mod consumer_tests;

const WORK: usize = 1_048_576;

fn error<T>(result: PlanResult<T>) -> PlanError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("changed source/SSA custody was accepted"),
    }
}

fn mapped<'a, 'tcx>(tcx: TyCtxt<'tcx>, auth: &Authentication<'a, 'tcx>) -> MappedPlan<'a> {
    let mut work = WORK;
    SourcePlan::observe(tcx, auth, &mut work)
        .expect("observe the complete real HIR/MIR source-use roster")
        .replay(tcx, auth, &mut work)
        .expect("reconstruct every original body and replay the same source-use roster")
}

pub(in super::super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    retained: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    owner: &ProductionSemanticSsaOwnerV1,
) {
    owner.verify_replay().unwrap();
    let [selected] = owner.source_semantic().roots() else {
        panic!("one registered real-source root")
    };
    let root = contexts
        .roots
        .iter()
        .find(|root| root.selected_root == *selected)
        .expect("the source importer, not this test, authenticated the root");
    let auth = Authentication {
        semantic: owner.source_semantic(),
        root,
        contexts,
        retained,
    };
    let view = owner.execution_view_for_root(*selected).unwrap();
    let query = owner.source_query_for_root(*selected, view.body()).unwrap();
    let mut work = WORK;
    let source = SourcePlan::observe(tcx, &auth, &mut work).unwrap();
    let mapping = source.replay(tcx, &auth, &mut work).unwrap();
    assert_eq!(mapping.flows.len(), 1);
    let row = &mapping.flows[0];
    assert_eq!(
        row.bodies.map(|(id, _)| id),
        [row.issue.0, row.closure_call.0, row.stage.0]
    );
    assert_eq!(row.issue.0, row.capture.0);
    assert_eq!(row.issue.0, row.publish.0);
    assert_eq!(
        row.workgroup_borrows.len(),
        3,
        "subgroup plus both pre-Publish epoch projections"
    );
    assert!(
        row.workgroup_borrows
            .windows(2)
            .all(|pair| pair[0] < pair[1])
    );
    for (function, body) in row.bodies {
        assert!(std::ptr::eq(
            body,
            &owner.source_semantic().functions()[function.index() as usize]
        ));
    }
    let binding = row.source_binding;
    let outer_function = row.issue.0;
    assert_ne!(binding, [0; 32]);
    let bound = mapping
        .bind(owner, &mut work)
        .expect("bind each retained original flow to real SSA events");
    let flows = bound.into_flows(owner).unwrap();
    let [flow] = flows.as_slice() else {
        panic!("one exact expanded outer instance")
    };
    assert_eq!(flow.source_binding, binding);
    assert_eq!(
        view.instances()[flow.outer.index() as usize].function(),
        outer_function
    );
    assert_eq!(
        flow.capture.value, flow.stage.value,
        "source capture retains its actual Issue producer"
    );
    assert!(matches!(flow.capture.value, SsaValueV1::Definition(_)));
    assert!(matches!(flow.publish.value, SsaValueV1::Definition(_)));
    assert_ne!(
        flow.stage.value,
        flow.closure_return.value(),
        "Stage produces a new actual result"
    );
    assert_ne!(flow.closure_return.value(), flow.helper_return.value());
    assert_ne!(flow.helper_return.value(), flow.publish.value);
    for input in [&flow.capture, &flow.stage, &flow.publish] {
        assert!(
            input.erased,
            "the measured optimized source carries these owned inputs as ZST constants"
        );
        assert!(matches!(input.operand, SemanticOperandV1::Constant(c)
            if matches!(c.value(), SemanticConstantValueV1::ZeroSized)));
        assert!(
            matches!(
                query.operand_use(input.site, input.operand, &mut || bounded::charge(
                    &mut work, 1
                )
                .is_ok()),
                Err(QueryError::UnsupportedOperand)
            ),
            "source evidence must not create a generic SSA use at a constant"
        );
    }
    for use_ in [&flow.closure_return, &flow.helper_return] {
        assert!(use_.belongs_to(&query));
        assert_eq!(use_.agreeing_uses(), 1);
    }
    assert!(matches!(
        flow.closure_return.operand(),
        SemanticOperandV1::Move(_)
    ));
    assert!(matches!(
        flow.helper_return.operand(),
        SemanticOperandV1::Move(_)
    ));
    assert!(flow.workgroup.belongs_to(&query));
    assert!(matches!(
        flow.workgroup.operand(),
        SemanticOperandV1::Copy(_)
    ));
    let workgroup_variable =
        fe2o3_mir_model::SsaVariableIdV1::new(flow.workgroup.local().index());
    assert!(
        query.plan().plan().promoted_variables().contains(&workgroup_variable),
        "the closed actual Workgroup borrow component is now SSA-promoted",
    );
    let workgroup_use = query.operand_use(
        flow.workgroup.site(), flow.workgroup.operand(),
        &mut || bounded::charge(&mut work, 1).is_ok(),
    ).expect("Publish must consume its actual promoted Workgroup SSA value");
    assert!(workgroup_use.belongs_to(&query));
    assert!(std::ptr::eq(workgroup_use.operand(), flow.workgroup.operand()));
    assert_eq!(workgroup_use.site(), flow.workgroup.site());
    assert_eq!(workgroup_use.variable(), workgroup_variable);
    assert_eq!(workgroup_use.agreeing_uses(), 1);
    assert!(matches!(workgroup_use.value(), SsaValueV1::Definition(_)));
    let range = workgroup_use.event_range();
    assert_eq!(
        query.plan().plan().resolved_events(
            fe2o3_mir_model::SsaBlockIdV1::new(workgroup_use.site().block().index()),
        ).unwrap().iter().filter(|(event, resolved)| {
            range.contains(&(*event as usize))
                && matches!(resolved, fe2o3_mir_model::SsaResolvedEventV1::Use { variable, value }
                    if *variable == workgroup_variable && *value == workgroup_use.value())
        }).count(),
        1,
        "the source receipt must match one existing Publish Use, not synthesize it",
    );
    assert_eq!(flow.workgroup_borrows.len(), 3);
    for use_ in &flow.workgroup_borrows {
        assert!(use_.belongs_to(&query));
        assert!(matches!(
            use_.operand(),
            SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_)
        ));
    }
    // All three source relations are now consumed against real events. The
    // typed Workgroup endpoint must still discharge these exact loan demands.
    drop(flows);

    plan::tests::check(tcx, &auth);
    consumer_tests::check(tcx, &auth, owner);
    canonical::tests::check(tcx, &auth, owner);
    production::tests::check(contexts, owner);
    typed::tests::check(contexts, owner);

    let mut zero = 0;
    assert!(matches!(
        error(SourcePlan::observe(tcx, &auth, &mut zero)),
        PlanError::Source(Error::Work)
    ));
    assert_eq!(zero, 0);

    let mut changed = mapped(tcx, &auth);
    changed.flows[0].capture_field = u32::MAX;
    let mut work = WORK;
    assert!(matches!(
        error(changed.bind(owner, &mut work)),
        PlanError::Source(Error::Source("transpose capture field is absent"))
    ));

    let mut changed = mapped(tcx, &auth);
    changed.flows[0].workgroup_local = SemanticLocalIdV1::from_index(u32::MAX);
    let mut work = WORK;
    assert!(matches!(
        error(changed.bind(owner, &mut work)),
        PlanError::Source(Error::Source(
            "Publish Workgroup changed its exact source binding"
        ))
    ));

    let mut changed = mapped(tcx, &auth);
    changed.root = SemanticFunctionIdV1::from_index(u32::MAX);
    let mut work = WORK;
    assert!(matches!(
        error(changed.bind(owner, &mut work)),
        PlanError::Source(Error::Source("transpose source root has no execution view"))
    ));

    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        owner.source_semantic().canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let foreign = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(decoded, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = WORK;
    assert!(matches!(
        error(mapped(tcx, &auth).bind(&foreign, &mut work)),
        PlanError::Source(Error::Source("transpose source and SSA owners differ"))
    ));
    let mut work = WORK;
    let bound = mapped(tcx, &auth).bind(owner, &mut work).unwrap();
    assert!(matches!(
        error(bound.into_flows(&foreign)),
        PlanError::Source(Error::Source("transpose bound source owner changed"))
    ));
}
