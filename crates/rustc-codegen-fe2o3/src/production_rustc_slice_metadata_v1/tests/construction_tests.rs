//! Check the real producer against raw MIR, not a second normalization model.

use super::*;
use crate::collector::CollectedFunctionRole;
use crate::production_semantic_body_v1::*;
use crate::production_semantic_fn_abi_v1::construct_production_semantic_fn_abis_v1;
use crate::production_semantic_types_v1::construct_production_semantic_types_v1;
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, canonical_target_layout_v1,
};
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, RetainedSemanticFunctionProducerV1,
    build_production_semantic_preflight_plan_v1,
};
use crate::semantic_layout_bridge::rustc_semantic_layout_target_v1;
use fe2o3_mir_model::semantic_mir_v1::*;

pub(super) fn check<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) {
    let raw = tcx.instance_mir(instance.def);
    let metadata = derive(tcx, instance, raw);
    assert_eq!(metadata.pairs.len(), 1);
    let identities = canonical_function_identities_v1(tcx, instance);
    let function = SemanticFunctionIdV1::from_index(0);
    let plan = build_production_semantic_preflight_plan_v1(
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
        [0x79; 32],
        DebugSourceCaptureRequestV2::SourceVariables,
    )
    .unwrap();
    assert!(plan.direct_call_producers().is_empty());
    assert!(plan.terminal_producers().is_empty());
    assert!(plan.normalized_intrinsic_producers().is_empty());
    let types = construct_production_semantic_types_v1(tcx, plan.type_producers())
        .unwrap()
        .into_records();
    let abi = construct_production_semantic_fn_abis_v1(
        tcx,
        plan.function_abi_producers(),
        plan.type_producers(),
    )
    .unwrap()
    .into_records()
    .pop()
    .unwrap();
    let type_bindings = plan
        .type_producers()
        .iter()
        .enumerate()
        .map(|(index, producer)| {
            ProductionSemanticTypeBindingV1::new(
                producer.ty,
                SemanticTypeIdV1::from_index(index as u32),
            )
        })
        .collect::<Vec<_>>();
    let planned = &plan.body_producers()[0];
    let local_bindings = planned
        .locals
        .iter()
        .enumerate()
        .map(|(index, local)| {
            ProductionSemanticLocalBindingV1::new(
                local.rustc_local,
                SemanticLocalIdV1::from_index(index as u32),
                local.identity,
                local.source.provenance,
            )
        })
        .collect::<Vec<_>>();
    let block_bindings = planned
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            ProductionSemanticBlockBindingV1::new(
                block.rustc_block,
                SemanticBlockIdV1::from_index(index as u32),
                block.identity,
                block.source.provenance,
                block
                    .statements
                    .iter()
                    .map(|source| source.provenance)
                    .collect(),
                block.terminator.provenance,
            )
        })
        .collect::<Vec<_>>();
    let construct = |body: &Body<'tcx>, limits| {
        let mut owner = ProductionSemanticBodyRequestOwnerV1::new(
            limits,
            types.len(),
            &[ProductionSemanticCallableOwnerEntryV1::Defined {
                rustc_instance: instance,
                semantic_callable: SemanticCallableIdV1::from_index(0),
            }],
        )
        .unwrap();
        construct_production_semantic_body_v1(
            ProductionSemanticBodyInputV1 {
                tcx,
                instance,
                body,
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
                source: planned.source.provenance,
                abi: abi.clone(),
                type_bindings: &type_bindings,
                local_bindings: &local_bindings,
                block_bindings: &block_bindings,
                entry: planned.entry,
                direct_calls: &[],
                terminal_expansions: &[],
                normalized_intrinsics: &[],
            },
            &mut owner,
        )
    };
    let produced = construct(raw, SemanticMirLimitsV1::default()).unwrap();
    assert_eq!(produced.locals().len(), planned.locals.len());
    for (local, source) in produced.locals().iter().zip(&planned.locals) {
        assert_eq!(local.ty(), source.ty);
        assert_eq!(local.identity(), source.identity);
        assert_eq!(local.source(), source.source.provenance);
    }
    for (block, source) in produced.blocks().iter().zip(&planned.blocks) {
        assert_eq!(block.statements().len(), source.statements.len());
        for (statement, source) in block.statements().iter().zip(&source.statements) {
            assert_eq!(statement.source(), source.provenance);
        }
        assert_eq!(block.terminator().source(), source.terminator.provenance);
    }
    let pair = metadata.pairs[0];
    let raw_to_semantic = |local: Local| {
        SemanticLocalIdV1::from_index(
            planned
                .locals
                .iter()
                .position(|row| row.rustc_local == local.as_u32())
                .unwrap() as u32,
        )
    };
    let block = planned
        .blocks
        .iter()
        .position(|row| row.rustc_block == pair.producer.block.as_u32())
        .unwrap();
    let statements = produced.blocks()[block].statements();
    assert!(matches!(
        statements[pair.producer.statement_index].kind(),
        SemanticStatementKindV1::Nop
    ));
    let SemanticStatementKindV1::Assign(assignment) =
        statements[pair.producer.statement_index + 1].kind()
    else {
        panic!("metadata assignment")
    };
    let SemanticRvalueKindV1::Unary {
        operation: SemanticUnaryOpV1::PointerMetadata,
        operand: SemanticOperandV1::Copy(place),
    } = assignment.value().kind()
    else {
        panic!("metadata of shared reference")
    };
    assert_eq!(place.local(), raw_to_semantic(pair.slice));
    assert!(place.projections().is_empty());
    let raw_local = &produced.locals()[raw_to_semantic(pair.temporary).index() as usize];
    assert!(
        matches!(types[raw_local.ty().index() as usize].shape(), SemanticTypeShapeV1::Pointer(pointer)
        if pointer.kind() == SemanticPointerKindV1::Raw && pointer.metadata() == SemanticPointerMetadataV1::SliceLength)
    );
    assert!(produced.blocks().iter().any(|block| matches!(
        block.terminator().kind(),
        SemanticTerminatorKindV1::Assert {
            message: SemanticAssertMessageV1::BoundsCheck { .. },
            ..
        }
    )));
    assert!(produced.blocks().iter().flat_map(|block| block.statements()).any(|statement| {
        matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place))
                if place.projections().iter().any(|projection| matches!(projection.kind(), SemanticProjectionKindV1::Index(_)))))
    }), "indexed read is preserved, not normalized away");

    // Reuse the preflight bindings against a changed live body: the constructor
    // must independently refuse a pointer escape with an unchanged row count.
    let mut changed = raw.clone();
    let consumer = Location {
        statement_index: pair.producer.statement_index + 1,
        ..pair.producer
    };
    assignment_mut(&mut changed, consumer).1 =
        Rvalue::Use(Operand::Copy(Place::from(pair.temporary)));
    assert!(
        matches!(construct(&changed, SemanticMirLimitsV1::default()),
        Err(ProductionSemanticBodyErrorV1::Unsupported { construct, .. })
        if construct.contains("exact shared-slice metadata pair"))
    );
    assert_eq!(
        construct(raw, SemanticMirLimitsV1::default()).unwrap(),
        produced
    );
}
