use fe2o3_pliron::{ProductionSemanticMirLimitsV1, ProductionSemanticSsaLimitsV1};
mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-pliron/src/production/semantic_ssa/defined_matrix_results/fixture.rs"
    ));

    pub(super) fn killed_context() -> AdmittedInertSemanticMirV1 {
        let base = source(false, true);
        let old = &base.functions()[0];
        let mut blocks = old.blocks().to_vec();
        let block = &blocks[1];
        let mut statements = block.statements().to_vec();
        statements.push(SemanticStatementV1::new(
            location(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
        ));
        blocks[1] = SemanticBasicBlockV1::new(
            block.identity(),
            block.source(),
            statements,
            block.terminator().clone(),
        )
        .unwrap();
        let mut functions = base.functions().to_vec();
        functions[0] = function(135, old.abi().clone(), old.locals().to_vec(), blocks, true)
            .with_kernel_entry(old.kernel_entry().unwrap().clone());
        InertSemanticMirRequestV1::new_with_callables(
            base.target(),
            base.types().to_vec(),
            vec![],
            vec![],
            vec![],
            functions,
            base.callables().to_vec(),
            base.roots().to_vec(),
        )
        .unwrap()
        .admit_exact_v23(SemanticMirLimitsV1::default())
        .unwrap()
    }
}

fn inputs() -> Vec<ProductionKernelContextLoweringInputV1> {
    vec![ProductionKernelContextLoweringInputV1::new(
        SemanticFunctionIdV1::from_index(0),
        [2; 32],
        [3; 32],
        [4; 32],
        [5; 32],
        [6; 32],
    )]
}
fn mir(source: AdmittedInertSemanticMirV1) -> ProductionSemanticMirOwnerV1 {
    ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default()).unwrap()
}

#[test]
fn matrix_getter_receipts_lower_only_at_original_return_transfers() {
    for repeated in [false, true] {
        let lowered = ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            mir(fixture::source(repeated, true)),
            ProductionSemanticKirLimitsV1::default(),
            inputs(),
        )
        .unwrap();
        lowered.verify_equivalence().unwrap();
        lowered
            .canonical_kernel_ir_v13()
            .unwrap()
            .revalidate()
            .unwrap();
        assert!(
            !lowered
                .module()
                .functions
                .iter()
                .filter_map(|f| f.body.as_ref())
                .flat_map(|body| &body.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(operation.kind, OperationKind::Matrix(_))),
            "a getter result is not a numerical operation"
        );
    }
}

#[test]
fn matrix_getter_rejects_changed_context_issuance_identity() {
    let changed = vec![ProductionKernelContextLoweringInputV1::new(
        SemanticFunctionIdV1::from_index(0),
        [2; 32],
        [3; 32],
        [4; 32],
        [5; 32],
        [99; 32],
    )];
    assert!(
        ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            mir(fixture::source(false, true)),
            ProductionSemanticKirLimitsV1::default(),
            changed,
        )
        .is_err()
    );
}

#[test]
fn matrix_getter_keeps_context_borrow_storage_death_obligation() {
    let source = fixture::killed_context();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        mir(fixture::killed_context()),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    let error = ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
        mir(source),
        ProductionSemanticKirLimitsV1::default(),
        inputs(),
    )
    .unwrap_err();
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
        if detail.contains("loan") || detail.contains("storage")),
        "{error:?}"
    );
}

#[test]
fn matrix_receipt_requires_exact_use_define_move_kill_order() {
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        mir(fixture::source(false, true)),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let plan = owner
        .execution_plan_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let [result] = plan.defined_matrix_results() else {
        panic!("one Matrix receipt")
    };
    let events = plan
        .plan()
        .resolved_events(SsaBlockIdV1::new(result.block().index()))
        .unwrap();
    matrix_bridge_events_v1(*result, events).unwrap();
    let start = events.windows(2).position(|w| matches!(w,
        [(_, SsaResolvedEventV1::Use { variable: a, .. }), (_, SsaResolvedEventV1::Use { variable: b, .. })]
        if a.get() == result.receiver().index() && b.get() == result.current().index())).unwrap();
    for index in start..start + 6 {
        let mut changed = events.to_vec();
        changed.remove(index);
        assert!(matrix_bridge_events_v1(*result, &changed).is_err());
    }
    let mut changed = events.to_vec();
    changed.swap(start, start + 1);
    assert!(matrix_bridge_events_v1(*result, &changed).is_err());
    changed = events.to_vec();
    changed.extend_from_slice(events);
    assert!(matrix_bridge_events_v1(*result, &changed).is_err());
}
