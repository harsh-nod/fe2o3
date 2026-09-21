//! Actual preflight and body constructor replay with incoming collection work.
use super::*;
use crate::collector::CollectedFunctionRole;
use crate::production_semantic_fn_abi_v1::construct_production_semantic_fn_abis_v1;
use crate::production_semantic_types_v1::construct_production_semantic_types_v1;
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, canonical_target_layout_v1,
};
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, RetainedSemanticFunctionProducerV1, SourceClosureWorkV1,
    build_production_semantic_preflight_plan_with_work_v1,
};
use crate::semantic_layout_bridge::rustc_semantic_layout_target_v1;

pub(crate) fn preflight_and_construct<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    work: SourceClosureWorkV1,
) {
    let prior = work.validation_work_for_test();
    assert!(prior > 17);
    let identities = canonical_function_identities_v1(tcx, instance);
    let function = SemanticFunctionIdV1::from_index(0);
    let mut plan = build_production_semantic_preflight_plan_with_work_v1(
        tcx,
        canonical_target_layout_v1(&rustc_semantic_layout_target_v1(tcx).unwrap()),
        vec![RetainedSemanticFunctionProducerV1 {
            identities,
            instance,
            role: CollectedFunctionRole::InternalHelper,
            export_name: None,
            kernel_binding: None,
            generated_host_contract_identity: None,
            frontend_contract: None,
        }]
        .into_boxed_slice(),
        vec![function].into_boxed_slice(),
        [0x60; 32],
        DebugSourceCaptureRequestV2::Disabled,
        (None, work),
    )
    .unwrap();
    assert_eq!(plan.normalized_intrinsic_producers().len(), 1);
    assert!(plan.direct_call_producers().is_empty());
    assert!(plan.terminal_producers().is_empty());
    assert!(matches!(
        plan.normalized_intrinsic_producers()[0].operation,
        NormalizedCallV1::CheckedPrimitiveFrom(_)
    ));
    let types = construct_production_semantic_types_v1(tcx, plan.type_producers())
        .unwrap()
        .into_records();
    let abis = construct_production_semantic_fn_abis_v1(
        tcx,
        plan.function_abi_producers(),
        plan.type_producers(),
    )
    .unwrap()
    .into_records();
    let type_bindings = plan
        .type_producers()
        .iter()
        .enumerate()
        .map(|(i, t)| {
            ProductionSemanticTypeBindingV1::new(t.ty, SemanticTypeIdV1::from_index(i as u32))
        })
        .collect::<Vec<_>>();
    let incoming = plan.take_construction_work().unwrap();
    let mut owner = ProductionSemanticBodyRequestOwnerV1::with_preflight_work(
        incoming,
        types.len(),
        &[ProductionSemanticCallableOwnerEntryV1::defined(
            instance,
            SemanticCallableIdV1::from_index(0),
        )],
    )
    .unwrap();
    assert!(owner.totals.validation_work > prior);
    let body = &plan.body_producers()[0];
    let raw = tcx.instance_mir(instance.def);
    let locals = body
        .locals
        .iter()
        .map(|r| {
            ProductionSemanticLocalBindingV1::new(
                r.rustc_local,
                body.raw_to_semantic_locals[r.rustc_local as usize],
                r.identity,
                r.source.provenance,
            )
        })
        .collect::<Vec<_>>();
    let blocks = body
        .blocks
        .iter()
        .map(|r| {
            ProductionSemanticBlockBindingV1::new(
                r.rustc_block,
                body.raw_to_semantic_blocks[r.rustc_block as usize],
                r.identity,
                r.source.provenance,
                r.statements.iter().map(|s| s.provenance).collect(),
                r.terminator.provenance,
            )
        })
        .collect::<Vec<_>>();
    let calls = plan
        .normalized_intrinsic_producers()
        .iter()
        .map(|r| {
            ProductionSemanticNormalizedRustcIntrinsicRecipeV1::new(
                r.caller,
                r.block,
                r.instance,
                r.element_type,
                r.operation,
            )
        })
        .collect::<Vec<_>>();
    let before = owner.totals.validation_work;
    let result = construct_production_semantic_body_v1(
        ProductionSemanticBodyInputV1 {
            context_entry: None,
            tcx,
            instance,
            body: raw,
            function,
            identities: ProductionSemanticFunctionIdentitiesV1::new(
                identities.function(),
                identities.item_definition(),
                identities.monomorphization(),
                identities.generic_type_arguments(),
                identities.const_generic_arguments(),
            ),
            role: SemanticFunctionRoleV1::InternalHelper,
            export: ProductionSemanticFunctionExportV1::None,
            source: body.source.provenance,
            abi: abis[0].clone(),
            type_bindings: &type_bindings,
            local_bindings: &locals,
            block_bindings: &blocks,
            entry: body.entry,
            direct_calls: &[],
            terminal_expansions: &[],
            normalized_intrinsics: &calls,
        },
        &mut owner,
    )
    .unwrap();
    assert!(owner.totals.validation_work > before);
    assert_eq!(result.locals().len(), raw.local_decls.len());
    assert_eq!(result.blocks().len(), raw.basic_blocks.len());
    let call = &calls[0];
    let original =
        &raw.basic_blocks[rustc_middle::mir::BasicBlock::from_usize(call.rustc_block as usize)];
    let after =
        &result.blocks()[body.raw_to_semantic_blocks[call.rustc_block as usize].index() as usize];
    assert_eq!(after.statements().len(), original.statements.len() + 1);
    let SemanticStatementKindV1::Assign(assignment) = after.statements().last().unwrap().kind()
    else {
        unreachable!()
    };
    let SemanticRvalueKindV1::Cast {
        kind: SemanticCastKindV1::Integer,
        operand,
    } = assignment.value().kind()
    else {
        unreachable!()
    };
    let TerminatorKind::Call {
        args,
        destination,
        target: Some(target),
        ..
    } = &original.terminator().kind
    else {
        unreachable!()
    };
    let (input, actual) = match (&args[0].node, operand) {
        (Operand::Copy(a), SemanticOperandV1::Copy(b))
        | (Operand::Move(a), SemanticOperandV1::Move(b)) => (a, b),
        _ => unreachable!(),
    };
    assert!(actual.projections().is_empty());
    assert!(input.projection.is_empty());
    assert_eq!(
        actual.local(),
        body.raw_to_semantic_locals[input.local.index()]
    );
    assert_eq!(
        assignment.destination().local(),
        body.raw_to_semantic_locals[destination.local.index()]
    );
    let SemanticTerminatorKindV1::Goto(edge) = after.terminator().kind() else {
        unreachable!()
    };
    assert_eq!(edge.target(), body.raw_to_semantic_blocks[target.index()]);
    println!(
        "primitive-from actual-stage-work collection-and-closure={prior} pre-body={before} final={}",
        owner.totals.validation_work
    );
}
