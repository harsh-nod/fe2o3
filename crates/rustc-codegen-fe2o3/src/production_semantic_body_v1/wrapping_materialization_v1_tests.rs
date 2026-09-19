//! Actual rustc producers and constructor replay, not full source admission.

use super::*;
use crate::collector::CollectedFunctionRole;
use crate::production_semantic_fn_abi_v1::construct_production_semantic_fn_abis_v1;
use crate::production_semantic_types_v1::construct_production_semantic_types_v1;
use crate::rustc_semantic_adapter_v1::canonical_target_layout_v1;
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, RetainedDebugSourceVariableClassV2,
    RetainedSemanticFunctionProducerV1, build_production_semantic_preflight_plan_v1,
};
use crate::semantic_layout_bridge::rustc_semantic_layout_target_v1;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;

fn assert_source_operand(
    raw: &Operand<'_>,
    semantic: &SemanticOperandV1,
    mapping: &[SemanticLocalIdV1],
) {
    let (raw, semantic) = match (raw, semantic) {
        (Operand::Copy(raw), SemanticOperandV1::Copy(semantic))
        | (Operand::Move(raw), SemanticOperandV1::Move(semantic)) => (raw, semantic),
        _ => panic!("source operand mode changed: {raw:?} -> {semantic:?}"),
    };
    assert!(raw.projection.is_empty());
    assert!(semantic.projections().is_empty());
    assert_eq!(semantic.local(), mapping[raw.local.index()]);
}

#[derive(Default)]
struct ConstructorCallbacks {
    completed: bool,
}

fn assert_mixed_scan_work(shift: SafeCoreShiftV1<'_>) {
    use crate::production_safe_core_shift_v1::locals::prepaid_rows_v1;
    let mut rows =
        vec![NormalizedCallV1::Rustc(ProductionRustcIntrinsicOperationV1::FabsF32); 1025];
    rows[1] = NormalizedCallV1::SafeCoreShift(shift);
    // Inert mixed recipe rows test the traversal shared by both producers;
    // they are not claims that duplicate recipes form an admitted source body.
    for scans in [1, 2, 3] {
        let exact = scans * rows.len();
        for limit in [exact, exact - 1] {
            let mut charged = 0;
            let mut visited = 0;
            let mut wrapping = 0;
            let result = (|| -> Result<(), &'static str> {
                for _ in 0..scans {
                    let iter = prepaid_rows_v1(&rows, |amount| {
                        let next = charged + amount;
                        if next > limit {
                            return Err("scan work exhausted");
                        }
                        charged = next;
                        Ok(())
                    })?;
                    for row in iter {
                        visited += 1;
                        wrapping += usize::from(matches!(row, NormalizedCallV1::SafeCoreShift(_)));
                    }
                }
                Ok(())
            })();
            if limit == exact {
                assert_eq!(result, Ok(()));
                assert_eq!((charged, visited, wrapping), (exact, exact, scans));
            } else {
                assert_eq!(result, Err("scan work exhausted"));
                assert_eq!(
                    (charged, visited, wrapping),
                    (
                        (scans - 1) * rows.len(),
                        (scans - 1) * rows.len(),
                        scans - 1
                    )
                );
            }
        }
    }
}

impl Callbacks for ConstructorCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let mut rejected_controls = 0;
        for definition in tcx.iter_local_def_id() {
            if tcx.def_kind(definition.to_def_id()) != DefKind::Fn
                || !matches!(
                    tcx.item_name(definition.to_def_id()).as_str(),
                    "local_fake" | "unsupported_128"
                )
            {
                continue;
            }
            let instance = Instance::mono(tcx, definition.to_def_id());
            let body = tcx.instance_mir(instance.def);
            let mut found = false;
            for data in body.basic_blocks.iter() {
                if let TerminatorKind::Call { func, .. } = &data.terminator().kind {
                    let callee = resolve_direct_call_v1(tcx, instance, body, func).unwrap();
                    if tcx.item_name(callee.def_id()).as_str() == "wrapping_shl" {
                        assert!(!matches!(
                            SafeCoreShiftV1::classify(tcx, callee),
                            Ok(Some(_))
                        ));
                        found = true;
                    }
                }
            }
            assert!(found, "control must observe a real retained method call");
            rejected_controls += 1;
        }
        assert_eq!(rejected_controls, 2);
        let mut functions = tcx
            .iter_local_def_id()
            .filter(|id| {
                tcx.def_kind(id.to_def_id()) == DefKind::Fn
                    && tcx.item_name(id.to_def_id()).as_str().starts_with("s_")
            })
            .map(|id| {
                let instance = Instance::mono(tcx, id.to_def_id());
                assert!(
                    SafeCoreShiftV1::classify(tcx, instance).unwrap().is_none(),
                    "local names are not core authority"
                );
                RetainedSemanticFunctionProducerV1 {
                    identities: canonical_function_identities_v1(tcx, instance),
                    instance,
                    role: CollectedFunctionRole::InternalHelper,
                    export_name: None,
                    kernel_binding: None,
                    generated_host_contract_identity: None,
                    frontend_contract: None,
                }
            })
            .collect::<Vec<_>>();
        functions.sort_by_key(|row| row.identities.function());
        assert_eq!(functions.len(), 17);
        let roots = (0..17)
            .map(SemanticFunctionIdV1::from_index)
            .collect::<Vec<_>>();
        let plan = build_production_semantic_preflight_plan_v1(
            tcx,
            canonical_target_layout_v1(&rustc_semantic_layout_target_v1(tcx).unwrap()),
            functions.into_boxed_slice(),
            roots.into_boxed_slice(),
            [0x71; 32],
            DebugSourceCaptureRequestV2::SourceVariables,
            None,
        )
        .unwrap();
        assert_eq!(plan.normalized_intrinsic_producers().len(), 18);
        assert!(plan.direct_call_producers().is_empty());
        assert!(plan.terminal_producers().is_empty());
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
            .map(|(index, row)| {
                ProductionSemanticCallableOwnerEntryV1::defined(
                    row.instance,
                    SemanticCallableIdV1::from_index(index as u32),
                )
            })
            .collect::<Vec<_>>();
        let fresh = |limits| {
            ProductionSemanticBodyRequestOwnerV1::new(limits, types.len(), &owned).unwrap()
        };
        let mut directions = [0; 2];
        let mut debug_checks = 0;
        for (index, producer) in plan.function_producers().iter().enumerate() {
            let function = SemanticFunctionIdV1::from_index(index as u32);
            let planned = &plan.body_producers()[index];
            let raw = tcx.instance_mir(producer.instance.def);
            let identities = producer.identities;
            let locals = planned
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
            let blocks = planned
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
                .normalized_intrinsic_producers()
                .iter()
                .filter(|call| call.caller == function)
                .map(|call| {
                    ProductionSemanticNormalizedRustcIntrinsicRecipeV1::new(
                        function,
                        call.block,
                        call.instance,
                        call.element_type,
                        call.operation,
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(
                calls.len(),
                if tcx.item_name(producer.instance.def_id()).as_str() == "s_multiple" {
                    2
                } else {
                    1
                }
            );
            let NormalizedCallV1::SafeCoreShift(shift) = calls[0].operation else {
                panic!("wrong recipe");
            };
            if index == 0 {
                assert_mixed_scan_work(shift);
            }
            assert!(std::ptr::eq(
                shift.body(),
                tcx.instance_mir(calls[0].expected_callee.def)
            ));
            assert!(
                shift.same_producers(
                    SafeCoreShiftV1::classify(tcx, calls[0].expected_callee)
                        .unwrap()
                        .unwrap()
                )
            );
            for call in &calls {
                let NormalizedCallV1::SafeCoreShift(actual) = call.operation else {
                    panic!("wrong recipe");
                };
                assert!(
                    actual.same_producers(
                        SafeCoreShiftV1::classify(tcx, call.expected_callee)
                            .unwrap()
                            .unwrap()
                    )
                );
                directions[usize::from(actual.direction() == DirectionV1::Right)] += 1;
            }
            let construct =
                |body: &Body<'tcx>, locals: &[_], blocks: &[_], calls: &[_], owner: &mut _| {
                    construct_production_semantic_body_v1(
                        ProductionSemanticBodyInputV1 {
                            context_entry: None,
                            tcx,
                            instance: producer.instance,
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
                            direct_calls: &[],
                            terminal_expansions: &[],
                            normalized_intrinsics: calls,
                        },
                        owner,
                    )
                };
            let mut measured = fresh(SemanticMirLimitsV1::default());
            let output = construct(raw, &locals, &blocks, &calls, &mut measured).unwrap();
            assert_eq!(
                output.locals().len(),
                raw.local_decls.len() + 2 * calls.len()
            );
            assert!(
                output
                    .locals()
                    .windows(2)
                    .all(|pair| pair[0].identity() < pair[1].identity())
            );
            for variable in &planned.debug_variables {
                if let RetainedDebugSourceVariableClassV2::Local(local) = variable.class {
                    assert!(
                        planned.locals.iter().any(|row| row.identity
                            == output.locals()[local.index() as usize].identity())
                    );
                    debug_checks += 1;
                }
            }
            for call in &calls {
                let NormalizedCallV1::SafeCoreShift(shift) = call.operation else {
                    panic!("wrong recipe");
                };
                let raw_block = call.rustc_block as usize;
                let block =
                    &output.blocks()[planned.raw_to_semantic_blocks[raw_block].index() as usize];
                let before = raw.basic_blocks[rustc_middle::mir::BasicBlock::from_usize(raw_block)]
                    .statements
                    .len();
                assert_eq!(block.statements().len(), before + 3);
                let assign = |ordinal: usize| match block.statements()[before + ordinal].kind() {
                    SemanticStatementKindV1::Assign(assignment) => assignment,
                    _ => panic!("normalization did not produce an assignment"),
                };
                let saved = assign(0);
                let mask = assign(1);
                let shifted = assign(2);
                let TerminatorKind::Call {
                    args: raw_args,
                    destination: raw_destination,
                    target: raw_target,
                    ..
                } = &raw.basic_blocks[rustc_middle::mir::BasicBlock::from_usize(raw_block)]
                    .terminator()
                    .kind
                else {
                    panic!("source call");
                };
                let SemanticRvalueKindV1::Use(saved_operand) = saved.value().kind() else {
                    panic!("saved value");
                };
                assert_source_operand(
                    &raw_args[0].node,
                    saved_operand,
                    &planned.raw_to_semantic_locals,
                );
                let SemanticRvalueKindV1::Binary {
                    operation,
                    left: count_operand,
                    right,
                } = mask.value().kind()
                else {
                    panic!("mask");
                };
                assert_eq!(*operation, SemanticBinaryOpV1::BitAnd);
                assert_source_operand(
                    &raw_args[1].node,
                    count_operand,
                    &planned.raw_to_semantic_locals,
                );
                let SemanticOperandV1::Constant(constant) = right else {
                    panic!("mask literal");
                };
                assert_eq!(
                    mask.value().result_type(),
                    type_bindings
                        .iter()
                        .find(|binding| binding.rustc_type == tcx.types.u32)
                        .unwrap()
                        .semantic_type
                );
                assert_eq!(
                    constant.value(),
                    &SemanticConstantValueV1::Scalar(
                        SemanticScalarValueV1::new(u128::from(shift.width() - 1), 4).unwrap()
                    )
                );
                let SemanticRvalueKindV1::Binary {
                    operation,
                    left,
                    right,
                } = shifted.value().kind()
                else {
                    panic!("shift");
                };
                assert_eq!(
                    *operation,
                    match shift.direction() {
                        DirectionV1::Left => SemanticBinaryOpV1::ShiftLeft,
                        DirectionV1::Right => SemanticBinaryOpV1::ShiftRight,
                    }
                );
                assert!(
                    matches!(left, SemanticOperandV1::Move(place) if place == saved.destination())
                );
                assert!(
                    matches!(right, SemanticOperandV1::Move(place) if place == mask.destination())
                );
                assert!(raw_destination.projection.is_empty());
                assert_eq!(
                    shifted.destination().local(),
                    planned.raw_to_semantic_locals[raw_destination.local.index()]
                );
                let SemanticTerminatorKindV1::Goto(edge) = block.terminator().kind() else {
                    panic!("normalized return edge");
                };
                assert_eq!(
                    edge.target(),
                    planned.raw_to_semantic_blocks[raw_target.unwrap().index()]
                );
            }
            let raw_block = calls[0].rustc_block as usize;
            for (resource, exact) in [
                (
                    SemanticMirResourceV1::ValidationWork,
                    measured.totals.validation_work,
                ),
                (SemanticMirResourceV1::Locals, measured.totals.locals),
                (
                    SemanticMirResourceV1::Statements,
                    measured.totals.statements,
                ),
                (SemanticMirResourceV1::Operands, measured.totals.operands),
            ] {
                let bounded = |maximum| {
                    fresh(
                        SemanticMirLimitsV1::default()
                            .with_limit(resource, maximum)
                            .unwrap(),
                    )
                };
                assert_eq!(
                    construct(raw, &locals, &blocks, &calls, &mut bounded(exact)).unwrap(),
                    output
                );
                assert!(
                    matches!(construct(raw, &locals, &blocks, &calls, &mut bounded(exact - 1)),
                    Err(ProductionSemanticBodyErrorV1::LimitExceeded { resource: actual, .. }) if actual == resource)
                );
            }
            let assert_refused = |calls: &[_], blocks: &[_]| {
                assert!(matches!(
                    construct(
                        raw,
                        &locals,
                        blocks,
                        calls,
                        &mut fresh(SemanticMirLimitsV1::default())
                    ),
                    Err(ProductionSemanticBodyErrorV1::IdentityTableMismatch { .. })
                ));
            };
            let mut wrong = calls.clone();
            wrong[0].caller = SemanticFunctionIdV1::from_index(17);
            assert_refused(&wrong, &blocks);
            wrong = calls.clone();
            wrong[0].expected_element_type = tcx.types.bool;
            assert_refused(&wrong, &blocks);
            wrong = calls.clone();
            wrong[0].expected_callee = producer.instance;
            assert_refused(&wrong, &blocks);
            wrong = calls.clone();
            wrong[0].operation = plan
                .normalized_intrinsic_producers()
                .iter()
                .find_map(|candidate| {
                    let NormalizedCallV1::SafeCoreShift(other) = candidate.operation else {
                        return None;
                    };
                    (other.value_type() == shift.value_type()
                        && other.direction() != shift.direction())
                    .then_some(candidate.operation)
                })
                .unwrap();
            assert_refused(&wrong, &blocks);
            wrong = calls.clone();
            wrong.push(calls[0]);
            assert_refused(&wrong, &blocks);
            wrong = calls.clone();
            wrong[0].operation =
                NormalizedCallV1::Rustc(ProductionRustcIntrinsicOperationV1::FabsF32);
            assert_refused(&wrong, &blocks);
            let mut changed_blocks = blocks.clone();
            let slot = changed_blocks
                .iter_mut()
                .find(|block| block.rustc_block == calls[0].rustc_block)
                .unwrap();
            slot.identity = SemanticBlockIdentityV1::from_sha256([0xd2; 32]);
            assert_refused(&calls, &changed_blocks);
            let mut changed = raw.clone();
            let call = changed.basic_blocks.as_mut()
                [rustc_middle::mir::BasicBlock::from_usize(raw_block)]
            .terminator_mut();
            let TerminatorKind::Call { args, .. } = &mut call.kind else {
                panic!("actual call");
            };
            args.swap(0, 1);
            assert!(matches!(
                construct(
                    &changed,
                    &locals,
                    &blocks,
                    &calls,
                    &mut fresh(SemanticMirLimitsV1::default())
                ),
                Err(ProductionSemanticBodyErrorV1::IdentityTableMismatch { .. })
            ));
        }
        assert_eq!(directions, [9, 9]);
        assert!(debug_checks > 0);
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
fn actual_safe_core_shift_calls_replay_into_typed_masked_assignments() {
    let directory = TestTempDir::create("fe2o3-wrapping-construction");
    let source = directory.path().join("fixture.rs");
    let mut text = String::new();
    text.push_str("#[inline(never)] pub fn s_multiple(value:u32, count:u32)->u32 { value.wrapping_shl(count).wrapping_shr(count) }\n");
    text.push_str("pub struct Forged; impl Forged { #[inline(never)] pub fn wrapping_shl(self, _:u32)->Self { self } }\n\
        pub fn local_fake(value:Forged, count:u32)->Forged { value.wrapping_shl(count) }\n\
        pub fn unsupported_128(value:u128, count:u32)->u128 { value.wrapping_shl(count) }\n");
    for ty in ["i8", "u8", "i16", "u16", "i32", "u32", "i64", "u64"] {
        for direction in ["shl", "shr"] {
            text.push_str(&format!("#[inline(never)] pub fn s_{ty}_{direction}(value:{ty}, count:u32)->{ty} {{ value.wrapping_{direction}(count) }}\n"));
        }
    }
    std::fs::write(&source, text).unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let sysroot = String::from_utf8(output.stdout).unwrap();
    let args = vec![
        "rustc".into(),
        "--crate-name=fe2o3_wrapping_construction_fixture".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zmir-opt-level=0".into(),
        "-Zinline-mir=no".into(),
        "-Coverflow-checks=on".into(),
        "-Cpanic=abort".into(),
        "--sysroot".into(),
        sysroot.trim().into(),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = ConstructorCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}
