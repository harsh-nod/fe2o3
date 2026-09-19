//! Actual constructor consumption of a retained generic call graph, not source admission.

use super::*;
use crate::closure_profile_v1::authenticate_once_shim_v1;
use crate::collector::CollectedFunctionRole;
use crate::production_semantic_fn_abi_v1::construct_production_semantic_fn_abis_v1;
use crate::production_semantic_types_v1::construct_production_semantic_types_v1;
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, canonical_source_provenance_v1, canonical_target_layout_v1,
};
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, RetainedDebugSourceVariableClassV2,
    RetainedSemanticFunctionProducerV1, build_production_semantic_preflight_plan_v1,
};
use crate::semantic_layout_bridge::rustc_semantic_layout_target_v1;
use crate::test_temp_dir::TestTempDir;
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{BasicBlock, Const, ConstOperand, VarDebugInfoContents};
use rustc_middle::ty::{ClosureKind, InstanceKind};

const SOURCE: &str = r#"
#[inline(never)]
fn invoke<F: FnOnce((u32, u32)) -> u32>(f: F, pair: (u32, u32)) -> u32 { f(pair) }
#[inline(never)]
fn through_fn<F: Fn((u32, u32)) -> u32>(f: F, pair: (u32, u32)) -> u32 { invoke(f, pair) }
#[inline(never)]
fn through_mut<F: FnMut((u32, u32)) -> u32>(f: F, pair: (u32, u32)) -> u32 { invoke(f, pair) }
pub fn shared(seed: u32, a: u32, b: u32) -> u32 {
    through_fn(move |pair| seed ^ pair.0 ^ pair.1, (a, b))
}
pub fn mutable(mut seed: u32, a: u32, b: u32) -> u32 {
    through_mut(move |pair| { seed ^= pair.0; seed ^ pair.1 }, (a, b))
}
"#;

fn root<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|definition| {
            tcx.def_kind(definition.to_def_id()) == DefKind::Fn
                && tcx.item_name(definition.to_def_id()).as_str() == name
        })
        .unwrap_or_else(|| panic!("missing fixture root {name}"));
    Instance::mono(tcx, definition.to_def_id())
}

fn assert_plain_operand(
    raw: &Operand<'_>,
    semantic: &SemanticOperandV1,
    mapping: &[SemanticLocalIdV1],
) {
    let (raw, semantic) = match (raw, semantic) {
        (Operand::Copy(raw), SemanticOperandV1::Copy(semantic))
        | (Operand::Move(raw), SemanticOperandV1::Move(semantic)) => (raw, semantic),
        _ => panic!("fixture operand kind changed: {raw:?} -> {semantic:?}"),
    };
    assert!(raw.projection.is_empty());
    assert!(semantic.projections().is_empty());
    assert_eq!(semantic.local(), mapping[raw.local.index()]);
}

#[derive(Default)]
struct ConstructionCallbacks {
    completed: bool,
}

impl Callbacks for ConstructionCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let roots = [root(tcx, "shared"), root(tcx, "mutable")];
        for (root, expected) in roots.iter().zip([ClosureKind::Fn, ClosureKind::FnMut]) {
            let actual = tcx
                .instance_mir(root.def)
                .local_decls
                .iter()
                .find_map(|local| {
                    if let TyKind::Closure(_, arguments) = local.ty.kind() {
                        Some(arguments.as_closure().kind())
                    } else {
                        None
                    }
                });
            assert_eq!(
                actual,
                Some(expected),
                "actual source-inferred closure kind"
            );
        }
        let mut instances = roots.to_vec();
        let mut cursor = 0;
        while cursor < instances.len() {
            let instance = instances[cursor];
            let body = tcx.instance_mir(instance.def);
            for data in body.basic_blocks.iter() {
                if let TerminatorKind::Call { func, .. } = &data.terminator().kind {
                    let callee = resolve_direct_call_v1(tcx, instance, body, func).unwrap();
                    if !instances.contains(&callee) {
                        instances.push(callee);
                    }
                }
            }
            cursor += 1;
            assert!(instances.len() <= 12, "unexpected fixture call expansion");
        }
        assert_eq!(
            instances.len(),
            10,
            "two roots, bound forwarders, FnOnce helpers, shims and closure Items"
        );
        let mut functions = instances
            .into_iter()
            .map(|instance| RetainedSemanticFunctionProducerV1 {
                identities: canonical_function_identities_v1(tcx, instance),
                instance,
                role: CollectedFunctionRole::InternalHelper,
                export_name: None,
                kernel_binding: None,
                generated_host_contract_identity: None,
                frontend_contract: None,
            })
            .collect::<Vec<_>>();
        functions.sort_by_key(|row| row.identities.function());
        let mut root_ids = roots.map(|root| {
            SemanticFunctionIdV1::from_index(
                functions
                    .iter()
                    .position(|row| row.instance == root)
                    .unwrap() as u32,
            )
        });
        root_ids.sort();
        let mut plan = build_production_semantic_preflight_plan_v1(
            tcx,
            canonical_target_layout_v1(&rustc_semantic_layout_target_v1(tcx).unwrap()),
            functions.into_boxed_slice(),
            root_ids.to_vec().into_boxed_slice(),
            [0x7c; 32],
            DebugSourceCaptureRequestV2::SourceVariables,
            None,
        )
        .unwrap();
        assert_eq!(plan.direct_call_producers().len(), 8);
        assert!(plan.terminal_producers().is_empty());
        assert!(plan.normalized_intrinsic_producers().is_empty());
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
            .map(|(index, row)| {
                ProductionSemanticTypeBindingV1::new(
                    row.ty,
                    SemanticTypeIdV1::from_index(index as u32),
                )
            })
            .collect::<Vec<_>>();
        let owned = plan
            .function_producers()
            .iter()
            .enumerate()
            .map(
                |(index, row)| ProductionSemanticCallableOwnerEntryV1::Defined {
                    rustc_instance: row.instance,
                    semantic_callable: SemanticCallableIdV1::from_index(index as u32),
                },
            )
            .collect::<Vec<_>>();
        let mut owner = ProductionSemanticBodyRequestOwnerV1::with_preflight_work(
            plan.take_construction_work().unwrap(),
            types.len(),
            &owned,
        )
        .unwrap();
        assert!(matches!(
            plan.take_construction_work(),
            Err(crate::rustc_semantic_plan_v1::ProductionSemanticPreflightErrorV1::IdentityTableMismatch)
        ));
        let fresh_owner = || {
            ProductionSemanticBodyRequestOwnerV1::new(
                SemanticMirLimitsV1::default(),
                types.len(),
                &owned,
            )
            .unwrap()
        };
        let (mut fn_shims, mut fn_mut_shims, mut captures, mut debug_locals) = (0, 0, 0, 0);
        for (index, producer) in plan.function_producers().iter().enumerate() {
            let instance = producer.instance;
            let function = SemanticFunctionIdV1::from_index(index as u32);
            let identities = producer.identities;
            let planned = &plan.body_producers()[index];
            let raw = tcx.instance_mir(instance.def);
            let local_bindings = planned
                .locals
                .iter()
                .map(|row| {
                    ProductionSemanticLocalBindingV1::new(
                        row.rustc_local,
                        planned.raw_to_semantic_locals[row.rustc_local as usize],
                        row.identity,
                        row.source.provenance,
                    )
                })
                .collect::<Vec<_>>();
            let block_bindings = planned
                .blocks
                .iter()
                .map(|row| {
                    ProductionSemanticBlockBindingV1::new(
                        row.rustc_block,
                        planned.raw_to_semantic_blocks[row.rustc_block as usize],
                        row.identity,
                        row.source.provenance,
                        row.statements
                            .iter()
                            .map(|source| source.provenance)
                            .collect(),
                        row.terminator.provenance,
                    )
                })
                .collect::<Vec<_>>();
            let calls = plan
                .direct_call_producers()
                .iter()
                .filter(|call| call.caller == function)
                .map(|call| {
                    ProductionSemanticDirectCallBindingV1::new(
                        function,
                        call.block,
                        plan.function_producers()[call.callee.index() as usize].instance,
                    )
                })
                .collect::<Vec<_>>();
            let construct =
                |body: &Body<'tcx>, locals: &[_], blocks: &[_], calls: &[_], owner: &mut _| {
                    construct_production_semantic_body_v1(
                        ProductionSemanticBodyInputV1 {
                            context_entry: None,
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
                            abi: abis[index].clone(),
                            type_bindings: &type_bindings,
                            local_bindings: locals,
                            block_bindings: blocks,
                            entry: planned.entry,
                            direct_calls: calls,
                            terminal_expansions: &[],
                            normalized_intrinsics: &[],
                        },
                        owner,
                    )
                };
            let produced = construct(raw, &local_bindings, &block_bindings, &calls, &mut owner)
                .unwrap_or_else(|error| panic!("construct {instance:?}: {error}"));
            let observation =
                receiver_reborrow_v1::derive_fn_receiver_reborrow_v1(tcx, instance, raw, |_| {
                    Ok::<_, ()>(())
                })
                .unwrap();
            assert_eq!(planned.locals.len(), raw.local_decls.len());
            assert_eq!(planned.raw_to_semantic_locals.len(), raw.local_decls.len());
            assert_eq!(
                produced.locals().len(),
                raw.local_decls.len() + usize::from(observation.is_some())
            );
            assert_eq!(produced.blocks().len(), raw.basic_blocks.len());
            assert!(
                produced
                    .locals()
                    .windows(2)
                    .all(|pair| pair[0].identity() < pair[1].identity())
            );
            for row in &planned.locals {
                let local = &produced.locals()
                    [planned.raw_to_semantic_locals[row.rustc_local as usize].index() as usize];
                assert_eq!(local.identity(), row.identity);
                assert_eq!(local.ty(), row.ty);
                assert_eq!(local.source(), row.source.provenance);
            }
            if roots.contains(&instance) {
                assert!(planned.debug_capture_gap.is_none());
            }
            for variable in &planned.debug_variables {
                if let RetainedDebugSourceVariableClassV2::Local(local) = variable.class {
                    debug_locals += 1;
                    assert_eq!(variable.function, function);
                    assert!(local.index() < produced.locals().len() as u32);
                    assert!(
                        raw.var_debug_info.iter().any(|info| {
                            variable.name.as_deref() == Some(info.name.as_str())
                                && matches!(&info.value, VarDebugInfoContents::Place(place)
                                if place.as_local().is_some_and(|raw|
                                    planned.raw_to_semantic_locals[raw.index()] == local))
                        }),
                        "debug variable must retain its raw source local"
                    );
                }
            }
            for row in &planned.blocks {
                let semantic = &produced.blocks()
                    [planned.raw_to_semantic_blocks[row.rustc_block as usize].index() as usize];
                let original = &raw.basic_blocks[BasicBlock::from_usize(row.rustc_block as usize)];
                let inserted = observation.is_some_and(|site| site.block() == row.rustc_block);
                assert_eq!(
                    semantic.statements().len(),
                    original.statements.len() + usize::from(inserted)
                );
                assert_eq!(semantic.terminator().source(), row.terminator.provenance);
                for (ordinal, statement) in original.statements.iter().enumerate() {
                    assert_eq!(
                        semantic.statements()[ordinal].source(),
                        row.statements[ordinal].provenance
                    );
                    if let Some((destination, Rvalue::Aggregate(kind, operands))) =
                        statement.kind.as_assign()
                        && matches!(kind.as_ref(), AggregateKind::Closure(..))
                    {
                        let SemanticStatementKindV1::Assign(assignment) =
                            semantic.statements()[ordinal].kind()
                        else {
                            panic!("closure capture assignment");
                        };
                        let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind()
                        else {
                            panic!("closure environment aggregate");
                        };
                        assert!(matches!(
                            aggregate.kind(),
                            SemanticAggregateKindV1::Aggregate
                        ));
                        assert_eq!(operands.len(), 1, "exact scalar capture");
                        assert_eq!(aggregate.operands().len(), 1);
                        let environment =
                            planned.raw_to_semantic_locals[destination.as_local().unwrap().index()];
                        assert_eq!(assignment.destination().local(), environment);
                        assert_eq!(
                            assignment.value().result_type(),
                            produced.locals()[environment.index() as usize].ty()
                        );
                        assert_plain_operand(
                            &operands[rustc_abi::FieldIdx::from_usize(0)],
                            &aggregate.operands()[0],
                            &planned.raw_to_semantic_locals,
                        );
                        captures += 1;
                    }
                }
                if let SemanticTerminatorKindV1::Call(call) = semantic.terminator().kind() {
                    let expected = abis[call.callee().index() as usize].source_input_types();
                    assert_eq!(call.arguments().len(), expected.len());
                    for (argument, expected) in call.arguments().iter().zip(expected) {
                        assert_eq!(argument.ty(), *expected, "constructed call source type");
                    }
                    if let Some(destination) = call.destination() {
                        assert_eq!(
                            destination.place().ty(),
                            abis[call.callee().index() as usize].source_output_type()
                        );
                    }
                }
            }
            let Some(actual) = authenticate_once_shim_v1(tcx, instance).unwrap() else {
                assert!(!matches!(
                    instance.def,
                    InstanceKind::ClosureOnceShim { .. }
                ));
                continue;
            };
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].expected_callee, actual);
            let raw_block = BasicBlock::from_usize(calls[0].rustc_block as usize);
            let semantic_block = planned.raw_to_semantic_blocks[raw_block.index()];
            let block = &produced.blocks()[semantic_block.index() as usize];
            let TerminatorKind::Call {
                args,
                destination,
                target,
                unwind,
                ..
            } = &raw.basic_blocks[raw_block].terminator().kind
            else {
                panic!("live shim call");
            };
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                panic!("produced shim call");
            };
            let actual_index = plan
                .function_producers()
                .iter()
                .position(|row| row.instance == actual)
                .unwrap();
            assert_eq!(call.callee().index() as usize, actual_index);
            assert_plain_operand(
                &args[1].node,
                &call.arguments()[1],
                &planned.raw_to_semantic_locals,
            );
            assert_eq!(
                call.destination().unwrap().place().local(),
                planned.raw_to_semantic_locals[destination.local.index()]
            );
            assert!(call.destination().unwrap().place().projections().is_empty());
            assert_eq!(
                call.destination().unwrap().edge().target(),
                planned.raw_to_semantic_blocks[target.unwrap().index()]
            );
            assert_eq!(
                call.destination().unwrap().edge().role(),
                SemanticEdgeRoleV1::CallReturn
            );
            assert_eq!(
                call.unwind(),
                match unwind {
                    UnwindAction::Continue => SemanticUnwindActionV1::Continue,
                    UnwindAction::Unreachable => SemanticUnwindActionV1::Unreachable,
                    other => panic!("unexpected fixture unwind {other:?}"),
                }
            );
            let Operand::Move(receiver) = &args[0].node else {
                panic!("live receiver move");
            };
            let receiver_local = planned.raw_to_semantic_locals[receiver.local.index()];
            let owned_local = planned.raw_to_semantic_locals[1];
            let owned_ty = produced.locals()[owned_local.index() as usize].ty();
            assert_eq!(
                produced.locals()[owned_local.index() as usize].role(),
                SemanticLocalRoleV1::Argument(0)
            );
            let mutable_ty = produced.locals()[receiver_local.index() as usize].ty();
            assert!(
                matches!(types[mutable_ty.index() as usize].shape(), SemanticTypeShapeV1::Pointer(pointer)
                if pointer.mutability() == SemanticMutabilityV1::Mutable && pointer.pointee() == owned_ty)
            );
            let borrow_index = raw.basic_blocks[raw_block]
                .statements
                .iter()
                .position(|statement| {
                    statement
                        .kind
                        .as_assign()
                        .is_some_and(|(destination, _)| destination == receiver)
                })
                .unwrap();
            let SemanticStatementKindV1::Assign(original_borrow) =
                block.statements()[borrow_index].kind()
            else {
                panic!("original mutable receiver borrow");
            };
            assert_eq!(original_borrow.destination().local(), receiver_local);
            assert_eq!(original_borrow.value().result_type(), mutable_ty);
            assert!(
                matches!(original_borrow.value().kind(), SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable, place,
            } if place.local() == owned_local && place.projections().is_empty() && place.ty() == owned_ty)
            );
            let TyKind::Closure(_, closure_args) = instance.args.type_at(0).kind() else {
                unreachable!();
            };
            let Some(observation) = observation else {
                assert_eq!(closure_args.as_closure().kind(), ClosureKind::FnMut);
                assert_plain_operand(
                    &args[0].node,
                    &call.arguments()[0],
                    &planned.raw_to_semantic_locals,
                );
                fn_mut_shims += 1;
                continue;
            };
            fn_shims += 1;
            assert_eq!(closure_args.as_closure().kind(), ClosureKind::Fn);
            let inserted = ReceiverLocalV1::derive(
                identities.function(),
                block.identity(),
                planned.locals.iter().map(|row| row.identity),
            )
            .unwrap();
            let temporary = &produced.locals()[inserted.local.index() as usize];
            assert_eq!(temporary.identity(), inserted.identity);
            assert_eq!(temporary.role(), SemanticLocalRoleV1::Temporary);
            assert_eq!(temporary.ty(), abis[actual_index].source_input_types()[0]);
            assert_eq!(temporary.source(), block.terminator().source());
            for (ordinal, row) in planned.locals.iter().enumerate() {
                assert_eq!(
                    planned.raw_to_semantic_locals[row.rustc_local as usize],
                    inserted
                        .remap(SemanticLocalIdV1::from_index(ordinal as u32))
                        .unwrap()
                );
            }
            assert!(planned.debug_variables.iter().all(|variable| variable.class
                != RetainedDebugSourceVariableClassV2::Local(inserted.local)));
            let statement = block.statements().last().unwrap();
            assert_eq!(statement.source(), block.terminator().source());
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                panic!("shared receiver assignment immediately before call");
            };
            assert_eq!(assignment.destination().local(), inserted.local);
            assert!(assignment.destination().projections().is_empty());
            assert_eq!(assignment.value().result_type(), temporary.ty());
            let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } = assignment.value().kind()
            else {
                panic!("explicit shared reborrow");
            };
            assert_eq!(place.local(), receiver_local);
            assert_eq!(place.ty(), owned_ty);
            assert_eq!(place.projections().len(), 1);
            assert_eq!(
                place.projections()[0].kind(),
                SemanticProjectionKindV1::Dereference
            );
            assert_eq!(place.projections()[0].result_type(), owned_ty);
            assert!(
                matches!(&call.arguments()[0], SemanticOperandV1::Move(place)
                if place == assignment.destination())
            );

            let bounded_owner = |maximum| {
                ProductionSemanticBodyRequestOwnerV1::new(
                    SemanticMirLimitsV1::default()
                        .with_limit(SemanticMirResourceV1::ValidationWork, maximum)
                        .unwrap(),
                    types.len(),
                    &owned,
                )
                .unwrap()
            };
            let mut measured = fresh_owner();
            assert_eq!(
                construct(raw, &local_bindings, &block_bindings, &calls, &mut measured).unwrap(),
                produced
            );
            let exact_work = measured.totals.validation_work;
            assert_eq!(
                construct(
                    raw,
                    &local_bindings,
                    &block_bindings,
                    &calls,
                    &mut bounded_owner(exact_work)
                )
                .unwrap(),
                produced
            );
            assert!(matches!(
                construct(
                    raw,
                    &local_bindings,
                    &block_bindings,
                    &calls,
                    &mut bounded_owner(exact_work - 1)
                ),
                Err(ProductionSemanticBodyErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::ValidationWork,
                    ..
                })
            ));

            let interior =
                receiver_materialization_v1::assert_receiver_source_guards_v1(tcx, &bounded_owner);
            let mut changed = raw.clone();
            changed.basic_blocks.as_mut()[raw_block]
                .terminator_mut()
                .source_info
                .span = interior;
            assert!(matches!(
                construct(&changed, &local_bindings, &block_bindings, &calls, &mut fresh_owner()),
                Err(ProductionSemanticBodyErrorV1::IdentityTableMismatch { table })
                    if table == "shared receiver borrowed body identity"
            ));

            // Check the prehash guard's own boundary, not just the constructor's
            // last charge. It sees the supplied polymorphic shim, not its FnSig.
            let mut fingerprint_work = 0_usize;
            receiver_materialization_v1::charge_once_shim_fingerprint_v1(tcx, raw, |amount| {
                fingerprint_work = fingerprint_work.checked_add(amount).unwrap();
                Ok::<_, ()>(())
            })
            .unwrap();
            for maximum in [fingerprint_work, fingerprint_work - 1] {
                let mut work = 0_usize;
                let result = receiver_materialization_v1::charge_once_shim_fingerprint_v1(
                    tcx,
                    raw,
                    |amount| {
                        work = work.checked_add(amount).unwrap();
                        if work > maximum { Err(()) } else { Ok(()) }
                    },
                );
                if maximum == fingerprint_work {
                    assert!(result.is_ok());
                } else {
                    assert!(matches!(
                        result,
                        Err(receiver_reborrow_v1::ReceiverReborrowErrorV1::Resource(()))
                    ));
                }
            }

            let terminator_span = raw.basic_blocks[raw_block].terminator().source_info.span;
            for span in [terminator_span, rustc_span::DUMMY_SP] {
                let expected = canonical_source_provenance_v1(
                    tcx,
                    if span.is_dummy() { raw.span } else { span },
                    256,
                )
                .unwrap()
                .provenance();
                let mut measured = fresh_owner();
                assert_eq!(
                    receiver_materialization_v1::reobserve_receiver_source_v1(
                        tcx,
                        span,
                        raw.span,
                        &mut measured,
                    )
                    .unwrap(),
                    expected
                );
                let exact_work = measured.totals.validation_work;
                assert_eq!(
                    receiver_materialization_v1::reobserve_receiver_source_v1(
                        tcx,
                        span,
                        raw.span,
                        &mut bounded_owner(exact_work),
                    )
                    .unwrap(),
                    expected
                );
                assert!(matches!(
                    receiver_materialization_v1::reobserve_receiver_source_v1(
                        tcx,
                        span,
                        raw.span,
                        &mut bounded_owner(exact_work - 1),
                    ),
                    Err(ProductionSemanticBodyErrorV1::LimitExceeded {
                        resource: SemanticMirResourceV1::ValidationWork,
                        ..
                    })
                ));
            }
            let mut wrong_source = block_bindings.clone();
            let call_binding = wrong_source
                .iter_mut()
                .find(|row| row.rustc_block == raw_block.as_u32())
                .unwrap();
            let unrelated_source = plan.body_producers()[root_ids[0].index() as usize]
                .source
                .provenance;
            assert_ne!(call_binding.terminator_source, unrelated_source);
            call_binding.terminator_source = unrelated_source;
            assert!(matches!(
                construct(raw, &local_bindings, &wrong_source, &calls, &mut fresh_owner()),
                Err(ProductionSemanticBodyErrorV1::IdentityTableMismatch { table })
                    if table == "shared receiver terminator provenance"
            ));

            for mutation in 0..4 {
                let mut changed = raw.clone();
                let expected = match mutation {
                    0 => {
                        changed.source_scopes.push(
                            raw.source_scopes[rustc_middle::mir::OUTERMOST_SOURCE_SCOPE].clone(),
                        );
                        // The scope count is charged before even inspecting a
                        // scope payload, including on this rejection path.
                        for maximum in [2_usize, 3] {
                            let mut work = 0_usize;
                            let result =
                                receiver_materialization_v1::charge_once_shim_fingerprint_v1(
                                    tcx,
                                    &changed,
                                    |amount| {
                                        work += amount;
                                        if work > maximum { Err(()) } else { Ok(()) }
                                    },
                                );
                            if maximum == 2 {
                                assert!(matches!(
                                    result,
                                    Err(
                                        receiver_reborrow_v1::ReceiverReborrowErrorV1::Resource(())
                                    )
                                ));
                            } else {
                                assert!(matches!(
                                    result,
                                    Err(
                                        receiver_reborrow_v1::ReceiverReborrowErrorV1::Unsupported(
                                            "closure once shim fingerprint source scopes"
                                        )
                                    )
                                ));
                            }
                        }
                        "closure once shim fingerprint source scopes"
                    }
                    1 => {
                        changed.source_scopes[rustc_middle::mir::OUTERMOST_SOURCE_SCOPE].inlined =
                            Some((roots[0], raw.span));
                        "closure once shim fingerprint source scope metadata"
                    }
                    2 => {
                        changed.var_debug_info.push(
                            roots
                                .iter()
                                .find_map(|root| tcx.instance_mir(root.def).var_debug_info.first())
                                .expect("live fixture debug metadata")
                                .clone(),
                        );
                        "closure once shim fingerprint metadata"
                    }
                    _ => {
                        let local = rustc_middle::mir::Local::from_usize(1);
                        changed.local_decls[local].ty =
                            Ty::new_tup(tcx, &[raw.local_decls[local].ty]);
                        "closure once shim fingerprint nested type"
                    }
                };
                assert_eq!(changed.local_decls.len(), raw.local_decls.len());
                assert_eq!(changed.basic_blocks.len(), raw.basic_blocks.len());
                assert!(
                    changed
                        .basic_blocks
                        .iter()
                        .zip(raw.basic_blocks.iter())
                        .all(|(changed, raw)| changed.statements.len() == raw.statements.len())
                );
                assert!(matches!(
                    construct(&changed, &local_bindings, &block_bindings, &calls, &mut fresh_owner()),
                    Err(ProductionSemanticBodyErrorV1::Unsupported { construct, .. })
                        if construct == expected
                ));
            }

            // Both changes retain the observer's relation and all row counts;
            // the consumer must reject them against its original-body commitment.
            for mutation in 0..2 {
                let mut changed = raw.clone();
                if mutation == 0 {
                    let TerminatorKind::Call { unwind, .. } = &mut changed.basic_blocks.as_mut()
                        [raw_block]
                        .terminator_mut()
                        .kind
                    else {
                        unreachable!();
                    };
                    *unwind = if *unwind == UnwindAction::Continue {
                        UnwindAction::Unreachable
                    } else {
                        UnwindAction::Continue
                    };
                } else {
                    let data = changed
                        .basic_blocks
                        .as_mut()
                        .iter_mut()
                        .find(|data| matches!(data.terminator().kind, TerminatorKind::Return))
                        .unwrap();
                    data.terminator_mut().kind = TerminatorKind::Unreachable;
                }
                assert_eq!(changed.local_decls.len(), raw.local_decls.len());
                assert_eq!(changed.basic_blocks.len(), raw.basic_blocks.len());
                assert!(
                    changed
                        .basic_blocks
                        .iter()
                        .zip(raw.basic_blocks.iter())
                        .all(|(changed, raw)| changed.statements.len() == raw.statements.len())
                );
                assert_eq!(
                    receiver_reborrow_v1::derive_fn_receiver_reborrow_v1(
                        tcx,
                        instance,
                        &changed,
                        |_| Ok::<_, ()>(()),
                    )
                    .unwrap(),
                    Some(observation)
                );
                assert!(
                    matches!(construct(&changed, &local_bindings, &block_bindings, &calls, &mut fresh_owner()),
                    Err(ProductionSemanticBodyErrorV1::IdentityTableMismatch { table })
                        if table == "shared receiver borrowed body identity")
                );
            }
            let mut changed = raw.clone();
            let TerminatorKind::Call { func, .. } = &mut changed.basic_blocks.as_mut()[raw_block]
                .terminator_mut()
                .kind
            else {
                unreachable!();
            };
            *func = Operand::Constant(Box::new(ConstOperand {
                span: raw.basic_blocks[raw_block].terminator().source_info.span,
                user_ty: None,
                const_: Const::zero_sized(Ty::new_fn_def(tcx, roots[0].def_id(), roots[0].args)),
            }));
            assert!(
                construct(
                    &changed,
                    &local_bindings,
                    &block_bindings,
                    &calls,
                    &mut fresh_owner()
                )
                .is_err(),
                "changed live callee against frozen binding"
            );
            let mut wrong_calls = calls.clone();
            wrong_calls[0].expected_callee = roots[0];
            assert!(
                construct(
                    raw,
                    &local_bindings,
                    &block_bindings,
                    &wrong_calls,
                    &mut fresh_owner()
                )
                .is_err()
            );
            wrong_calls[0] = calls[0];
            wrong_calls[0].caller =
                SemanticFunctionIdV1::from_index((index as u32 + 1) % owned.len() as u32);
            assert!(
                construct(
                    raw,
                    &local_bindings,
                    &block_bindings,
                    &wrong_calls,
                    &mut fresh_owner()
                )
                .is_err()
            );
            assert!(
                construct(
                    raw,
                    &local_bindings,
                    &block_bindings,
                    &[],
                    &mut fresh_owner()
                )
                .is_err()
            );
            let mut wrong_blocks = block_bindings.clone();
            wrong_blocks
                .iter_mut()
                .find(|row| row.rustc_block == raw_block.as_u32())
                .unwrap()
                .identity = SemanticBlockIdentityV1::from_sha256([0x5d; 32]);
            assert!(
                construct(
                    raw,
                    &local_bindings,
                    &wrong_blocks,
                    &calls,
                    &mut fresh_owner()
                )
                .is_err()
            );
            let mut wrong_locals = local_bindings.clone();
            wrong_locals[0].rustc_local = local_bindings[1].rustc_local;
            wrong_locals[1].rustc_local = local_bindings[0].rustc_local;
            assert!(
                construct(
                    raw,
                    &wrong_locals,
                    &block_bindings,
                    &calls,
                    &mut fresh_owner()
                )
                .is_err()
            );
            let mut wrong_locals = local_bindings.clone();
            wrong_locals[0].semantic_local = local_bindings[1].semantic_local;
            wrong_locals[1].semantic_local = local_bindings[0].semantic_local;
            assert!(
                construct(
                    raw,
                    &wrong_locals,
                    &block_bindings,
                    &calls,
                    &mut fresh_owner()
                )
                .is_err()
            );
            let mut stale = local_bindings.clone();
            for (ordinal, binding) in stale.iter_mut().enumerate() {
                binding.semantic_local = SemanticLocalIdV1::from_index(ordinal as u32);
            }
            if stale != local_bindings {
                assert!(
                    construct(raw, &stale, &block_bindings, &calls, &mut fresh_owner()).is_err()
                );
            }
            assert_eq!(
                construct(
                    raw,
                    &local_bindings,
                    &block_bindings,
                    &calls,
                    &mut fresh_owner()
                )
                .unwrap(),
                produced
            );
        }
        assert_eq!((fn_shims, fn_mut_shims, captures), (1, 1, 2));
        assert!(
            debug_locals > 0,
            "debug mapping checks must observe actual source locals"
        );
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
fn fn_receiver_construction_consumes_retained_graph_and_frozen_bindings() {
    let directory = TestTempDir::create("fe2o3-receiver-construction");
    let source = directory.path().join("fixture.rs");
    std::fs::write(&source, SOURCE).unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let sysroot = String::from_utf8(output.stdout).unwrap();
    let args = vec![
        "rustc".into(),
        "--crate-name".into(),
        "fe2o3_receiver_construction_fixture".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zmir-opt-level=0".into(),
        "-Cpanic=abort".into(),
        "--sysroot".into(),
        sysroot.trim().into(),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = ConstructionCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}
