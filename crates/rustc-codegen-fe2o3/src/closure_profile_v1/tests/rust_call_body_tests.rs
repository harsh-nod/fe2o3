//! Exercise the production body constructor, not a surrogate argument mapper.

use super::{TyCtxt, local_function};
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
use rustc_middle::mir::{Body, Local};
use rustc_middle::ty::{Instance, InstanceKind, Ty, TyKind};

pub(super) fn check<'tcx>(tcx: TyCtxt<'tcx>) -> (usize, usize) {
    let cases = [
        ("body_fn", 1),
        ("body_fn_mut", 1),
        ("body_fn_mut_ref", 1),
        ("body_fn_once", 1),
        ("device_zero", 0),
        ("device_unit", 1),
        ("body_multiple", 2),
        ("body_mixed", 2),
        ("body_tuple", 1),
        ("packed", 2),
    ];
    let mut rejected = 0;
    for (name, arity) in cases {
        let caller = Instance::mono(tcx, local_function(tcx, name));
        let expanded = name != "packed";
        let instance = if expanded {
            tcx.instance_mir(caller.def)
                .local_decls
                .iter()
                .find_map(|local| match local.ty.kind() {
                    TyKind::Closure(def_id, args) => Some(Instance {
                        def: InstanceKind::Item(*def_id),
                        args,
                    }),
                    _ => None,
                })
                .expect("actual closure instance")
        } else {
            caller
        };
        let identities = canonical_function_identities_v1(tcx, instance);
        let function = SemanticFunctionIdV1::from_index(0);
        let target = canonical_target_layout_v1(&rustc_semantic_layout_target_v1(tcx).unwrap());
        let plan = build_production_semantic_preflight_plan_v1(
            tcx,
            target,
            vec![RetainedSemanticFunctionProducerV1 {
                identities: identities.clone(),
                instance,
                role: CollectedFunctionRole::InternalHelper,
                export_name: None,
                kernel_binding: None,
                generated_host_contract_identity: None,
                frontend_contract: None,
            }]
            .into_boxed_slice(),
            vec![function].into_boxed_slice(),
            [0x78; 32],
            DebugSourceCaptureRequestV2::Disabled,
        )
        .unwrap_or_else(|error| panic!("{name} preflight: {error}"));
        assert!(plan.direct_call_producers().is_empty(), "{name}");
        assert!(plan.terminal_producers().is_empty(), "{name}");
        assert!(plan.normalized_intrinsic_producers().is_empty(), "{name}");
        let types = construct_production_semantic_types_v1(tcx, plan.type_producers())
            .unwrap_or_else(|error| panic!("{name} types: {error}"))
            .into_records();
        let abi = construct_production_semantic_fn_abis_v1(
            tcx,
            plan.function_abi_producers(),
            plan.type_producers(),
        )
        .unwrap_or_else(|error| panic!("{name} FnAbi: {error}"))
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
        let construct = |body: &Body<'tcx>, abi: SemanticFunctionAbiV1| {
            let mut owner = ProductionSemanticBodyRequestOwnerV1::new(
                SemanticMirLimitsV1::default(),
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
                    abi,
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
        let body = tcx.instance_mir(instance.def);
        let produced = construct(body, abi.clone())
            .unwrap_or_else(|error| panic!("{name} body construction: {error}"));
        assert_eq!(produced.abi(), &abi, "{name}");
        assert_eq!(produced.blocks().len(), planned.blocks.len(), "{name}");
        assert_eq!(produced.locals().len(), planned.locals.len(), "{name}");
        for (local, planned) in produced.locals().iter().zip(&planned.locals) {
            let raw = planned.rustc_local;
            let expected = match raw {
                0 => SemanticLocalRoleV1::Return,
                1 => SemanticLocalRoleV1::Argument(0),
                _ if raw as usize <= body.arg_count && expanded => {
                    SemanticLocalRoleV1::RustCallTupleField {
                        argument: 1,
                        field: raw - 2,
                    }
                }
                _ if raw as usize <= body.arg_count => SemanticLocalRoleV1::Argument(raw - 1),
                _ => SemanticLocalRoleV1::Temporary,
            };
            assert_eq!(local.role(), expected, "{name} local {raw}");
            assert_eq!(local.ty(), planned.ty, "{name} local {raw}");
            if name == "body_tuple" && raw == 2 {
                assert!(matches!(types[local.ty().index() as usize].shape(),
                    SemanticTypeShapeV1::Tuple(fields) if fields.fields().len() == 2));
            }
        }
        assert_eq!(
            produced
                .locals()
                .iter()
                .filter(|local| matches!(
                    local.role(),
                    SemanticLocalRoleV1::RustCallTupleField { .. }
                ))
                .count(),
            if expanded { arity } else { 0 },
            "{name}"
        );
        let receiver = body.local_decls[Local::from_usize(1)].ty;
        match name {
            "body_fn" => assert!(matches!(
                receiver.kind(),
                TyKind::Ref(_, _, rustc_hir::Mutability::Not)
            )),
            "body_fn_mut" | "body_fn_mut_ref" => assert!(matches!(
                receiver.kind(),
                TyKind::Ref(_, _, rustc_hir::Mutability::Mut)
            )),
            "body_fn_once" => assert!(matches!(receiver.kind(), TyKind::Closure(..))),
            _ => {}
        }
        let mut reject = |changed: &Body<'tcx>, changed_abi, table| {
            let error = construct(changed, changed_abi).expect_err("substituted body must reject");
            assert!(
                matches!(error, ProductionSemanticBodyErrorV1::IdentityTableMismatch {
                table: actual
            } if actual == table),
                "{name}: expected {table}, got {error}"
            );
            rejected += 1;
        };
        let mut changed = body.clone();
        changed.arg_count += 1;
        reject(&changed, abi.clone(), "body argument count");
        let mut changed = body.clone();
        changed.arg_count -= 1;
        reject(&changed, abi.clone(), "body argument count");
        let mut changed = body.clone();
        changed.spread_arg = if expanded {
            Some(Local::from_usize(2))
        } else {
            None
        };
        reject(&changed, abi.clone(), "RustCall body argument form");
        if !expanded {
            let mut changed = body.clone();
            changed.spread_arg = Some(Local::from_usize(1));
            reject(&changed, abi.clone(), "RustCall body argument form");
        }
        let mut changed = body.clone();
        changed.local_decls[Local::from_usize(0)].ty = tcx.types.bool;
        reject(&changed, abi.clone(), "body source signature");
        let environment = match receiver.kind() {
            TyKind::Ref(_, environment, _) => *environment,
            _ => receiver,
        };
        for substituted in [
            environment,
            Ty::new_ref(
                tcx,
                tcx.lifetimes.re_erased,
                environment,
                rustc_hir::Mutability::Not,
            ),
            Ty::new_ref(
                tcx,
                tcx.lifetimes.re_erased,
                environment,
                rustc_hir::Mutability::Mut,
            ),
        ] {
            if substituted != receiver {
                let mut changed = body.clone();
                changed.local_decls[Local::from_usize(1)].ty = substituted;
                reject(&changed, abi.clone(), "body argument local type");
            }
        }
        for raw in 2..=body.arg_count {
            let mut changed = body.clone();
            let argument = &mut changed.local_decls[Local::from_usize(raw)];
            argument.ty = if argument.ty == tcx.types.bool {
                tcx.types.u32
            } else {
                tcx.types.bool
            };
            reject(&changed, abi.clone(), "body argument local type");
        }
        // Rebuild a structurally valid ordinary ABI, rather than mutating only
        // its tag and failing the ABI constructor before reaching BodyProducer.
        let ordinary = SemanticFunctionAbiV1::new(
            abi.identity(),
            abi.layout_identity(),
            SemanticCanonAbiV1::Rust,
            abi.can_unwind(),
            false,
            abi.source_input_types()
                .iter()
                .map(|ty| SemanticAbiValueV1::new(*ty, abi.arguments()[0].mode().clone()))
                .collect(),
            abi.return_value().clone(),
        )
        .unwrap();
        reject(body, ordinary, "body RustCall ABI disagreement");
    }
    (cases.len(), rejected)
}

#[test]
fn real_rust_call_bodies_construct_and_reject_argument_substitution() {
    assert_eq!(super::compiler_results().rust_call_body_cases, (10, 82));
}
