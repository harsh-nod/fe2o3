//! The actual body consumer, using ordinary test carriers (not issuer authority).
use super::*;
use crate::production_semantic_body_v1::*;
use crate::production_semantic_fn_abi_v1::construct_production_semantic_fn_abis_v1;
use crate::production_semantic_types_v1::construct_production_semantic_types_v1;
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, canonical_target_layout_v1,
};
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, build_production_semantic_preflight_plan_v1,
};
use crate::semantic_layout_bridge::rustc_semantic_layout_target_v1;
use crate::test_temp_dir::TestTempDir;
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::mir::Location;
use rustc_middle::ty::TyCtxt;

const SOURCE: &str = r#"
pub struct Context;
pub struct Tag;
#[inline(never)] pub fn issue() -> Context { Context }
#[inline(never)] pub fn helper(_: Context, a: u32, b: u32, _: Tag) -> u32 { a ^ b }
pub fn root(a: u32, b: u32, tag: Tag) -> u32 { helper(issue(), a, b, tag) }
"#;

#[derive(Default)]
struct ContextCallbacks {
    completed: bool,
    erased: bool,
}

impl Callbacks for ContextCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let find = |name: &str| {
            Instance::mono(
                tcx,
                tcx.hir_body_owners()
                    .find(|id| tcx.item_name(id.to_def_id()).as_str() == name)
                    .unwrap()
                    .to_def_id(),
            )
        };
        let root = find("root");
        let helper = find("helper");
        let issuer = find("issue");
        let mut functions =
            [root, helper, issuer].map(|instance| RetainedSemanticFunctionProducerV1 {
                identities: canonical_function_identities_v1(tcx, instance),
                instance,
                role: CollectedFunctionRole::InternalHelper,
                export_name: None,
                kernel_binding: None,
                generated_host_contract_identity: None,
                frontend_contract: None,
            });
        functions.sort_by_key(|row| row.identities.function());
        let id = |instance| {
            SemanticFunctionIdV1::from_index(
                functions
                    .iter()
                    .position(|row| row.instance == instance)
                    .unwrap() as u32,
            )
        };
        let function = id(root);
        let helper_function = id(helper);
        let issuer_function = id(issuer);
        let identities = canonical_function_identities_v1(tcx, root);
        let plan = build_production_semantic_preflight_plan_v1(
            tcx,
            canonical_target_layout_v1(&rustc_semantic_layout_target_v1(tcx).unwrap()),
            functions.to_vec().into_boxed_slice(),
            vec![function].into_boxed_slice(),
            [7; 32],
            DebugSourceCaptureRequestV2::Disabled,
            None,
        )
        .unwrap();
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
        let planned = &plan.body_producers()[function.index() as usize];
        let raw = tcx.instance_mir(root.def);
        let site = |callee| {
            let (block, data) = raw
                .basic_blocks
                .iter_enumerated()
                .find(|(_, data)| {
                    matches!(&data.terminator().kind, TerminatorKind::Call { func, .. }
                    if super::super::resolve_call(tcx, root, raw, func).unwrap() == callee)
                })
                .unwrap();
            let TerminatorKind::Call {
                destination,
                target,
                unwind,
                ..
            } = data.terminator().kind
            else {
                unreachable!()
            };
            flow::CallOccurrenceV1 {
                location: Location {
                    block,
                    statement_index: data.statements.len(),
                },
                destination: destination.as_local().unwrap(),
                target: target.unwrap(),
                unwind,
            }
        };
        let issuance = site(issuer);
        let helper_call = site(helper);
        let context = raw.local_decls[issuance.destination].ty;
        let erased = raw;
        let helper_argument =
            context_argument(raw, helper_call.location.block.index(), context).unwrap();
        assert_eq!(helper_argument.is_none(), self.erased);
        let receipt = || BoundContextEntryV29 {
            root,
            function,
            helper,
            helper_function,
            issuer,
            context,
            optimized_body: raw,
            original_mir_sha256: [9; 32],
            original: flow::AuthenticatedFlowV1 {
                issuer,
                issuance,
                helper_call,
            },
            issuance: BoundCallV29::bind(issuance, planned).unwrap(),
            helper_call: BoundCallV29::bind(helper_call, planned).unwrap(),
            helper_argument,
            semantic_helper_argument: planned.raw_to_semantic_locals
                [helper_argument.unwrap_or(issuance.destination).index()],
            arguments: raw.arg_count + 1,
        };
        let type_bindings = plan
            .type_producers()
            .iter()
            .enumerate()
            .map(|(i, row)| {
                ProductionSemanticTypeBindingV1::new(row.ty, SemanticTypeIdV1::from_index(i as u32))
            })
            .collect::<Vec<_>>();
        let local_bindings = planned
            .locals
            .iter()
            .enumerate()
            .map(|(i, row)| {
                ProductionSemanticLocalBindingV1::new(
                    row.rustc_local,
                    SemanticLocalIdV1::from_index(i as u32),
                    row.identity,
                    row.source.provenance,
                )
            })
            .collect::<Vec<_>>();
        let block_bindings = planned
            .blocks
            .iter()
            .enumerate()
            .map(|(i, row)| {
                ProductionSemanticBlockBindingV1::new(
                    row.rustc_block,
                    SemanticBlockIdV1::from_index(i as u32),
                    row.identity,
                    row.source.provenance,
                    row.statements.iter().map(|s| s.provenance).collect(),
                    row.terminator.provenance,
                )
            })
            .collect::<Vec<_>>();
        let direct_calls = plan
            .direct_call_producers()
            .iter()
            .filter(|call| call.caller == function)
            .map(|call| {
                ProductionSemanticDirectCallBindingV1::new(
                    call.caller,
                    call.block,
                    functions[call.callee.index() as usize].instance,
                )
            })
            .collect::<Vec<_>>();
        let owned = functions
            .iter()
            .enumerate()
            .map(|(i, row)| ProductionSemanticCallableOwnerEntryV1::Defined {
                rustc_instance: row.instance,
                semantic_callable: SemanticCallableIdV1::from_index(i as u32),
            })
            .collect::<Vec<_>>();
        let construct_with_owner =
            |body: &Body<'tcx>,
             context_entry,
             owner: &mut ProductionSemanticBodyRequestOwnerV1<'tcx>| {
                construct_production_semantic_body_v1(
                    ProductionSemanticBodyInputV1 {
                        context_entry,
                        tcx,
                        instance: root,
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
                        abi: abis[function.index() as usize].clone(),
                        type_bindings: &type_bindings,
                        local_bindings: &local_bindings,
                        block_bindings: &block_bindings,
                        entry: planned.entry,
                        direct_calls: &direct_calls,
                        terminal_expansions: &[],
                        normalized_intrinsics: &[],
                    },
                    owner,
                )
            };
        let construct = |body: &Body<'tcx>, context_entry, limits| {
            let mut owner = ProductionSemanticBodyRequestOwnerV1::new(limits, types.len(), &owned)?;
            construct_with_owner(body, context_entry, &mut owner)
        };
        let limits = SemanticMirLimitsV1::default();
        let produced = construct(erased, Some(receipt()), limits).unwrap();
        let helper_block =
            planned.raw_to_semantic_blocks[helper_call.location.block.index()].index() as usize;
        let SemanticTerminatorKindV1::Call(call) =
            produced.blocks()[helper_block].terminator().kind()
        else {
            panic!("helper call")
        };
        assert_eq!(call.callee().index(), helper_function.index());
        let SemanticOperandV1::Move(place) = &call.arguments()[0] else {
            panic!("restored move")
        };
        assert_eq!(
            place.local(),
            planned.raw_to_semantic_locals[helper_argument.unwrap_or(issuance.destination).index()]
        );
        assert!(place.projections().is_empty());
        let TerminatorKind::Call { args, .. } = &raw.basic_blocks[helper_call.location.block]
            .terminator()
            .kind
        else {
            unreachable!()
        };
        assert_eq!(
            matches!(call.arguments()[3], SemanticOperandV1::Constant(_)),
            matches!(args[3].node, Operand::Constant(_)),
            "ordinary ZST remains erased when rustc erases it"
        );
        let issuer_block =
            planned.raw_to_semantic_blocks[issuance.location.block.index()].index() as usize;
        let SemanticTerminatorKindV1::Call(issue) =
            produced.blocks()[issuer_block].terminator().kind()
        else {
            panic!("issuer call")
        };
        assert_eq!(issue.callee().index(), issuer_function.index());
        if helper_argument.is_none() {
            assert_eq!(issue.destination().unwrap().place(), place);
        }
        let ordinary = construct(erased, None, limits).unwrap();
        let SemanticTerminatorKindV1::Call(call) =
            ordinary.blocks()[helper_block].terminator().kind()
        else {
            panic!("ordinary call")
        };
        assert_eq!(
            matches!(call.arguments()[0], SemanticOperandV1::Constant(_)),
            self.erased,
            "ordinary ZST is not restored"
        );
        for mutation in 0..9 {
            let mut changed = receipt();
            match mutation {
                0 => changed.root = helper,
                1 => changed.function = helper_function,
                2 => changed.issuer = helper,
                3 => changed.helper = issuer,
                4 => changed.issuance.raw.location.statement_index += 1,
                5 => changed.issuance.raw.destination = Local::from_u32(0),
                6 => changed.helper_call.raw.target = helper_call.location.block,
                7 => {
                    changed.helper_call.raw.unwind = if helper_call.unwind == UnwindAction::Continue
                    {
                        UnwindAction::Unreachable
                    } else {
                        UnwindAction::Continue
                    }
                }
                8 => changed.issuance.destination = SemanticLocalIdV1::from_index(u32::MAX),
                _ => unreachable!(),
            }
            assert!(
                construct(erased, Some(changed), limits).is_err(),
                "mutation {mutation}"
            );
        }
        let mut both = receipt();
        assert_eq!(
            both.consume_call(
                erased,
                helper_call.location.block.index(),
                helper,
                raw.arg_count + 1
            ),
            Ok(helper_argument.is_none().then_some(issuance.destination))
        );
        assert_eq!(
            both.consume_call(erased, issuance.location.block.index(), issuer, 0),
            Ok(None),
            "construction order need not be execution order"
        );
        assert!(
            both.consume_call(erased, issuance.location.block.index(), issuer, 0)
                .is_err(),
            "duplicate occurrence"
        );
        let issuer_callable = SemanticCallableIdV1::from_index(issuer_function.index());
        let finish = |bound: BoundContextEntryV29<'tcx>| {
            bound.finish(
                tcx,
                issuer_callable,
                |local| planned.raw_to_semantic_locals.get(local.index()).copied(),
                |ty| {
                    plan.type_producers()
                        .iter()
                        .position(|binding| binding.ty == ty)
                        .map(|index| SemanticTypeIdV1::from_index(index as u32))
                },
                |_| Ok(()),
            )
        };
        finish(both).unwrap();
        for mutation in 0..3 {
            let mut bound = receipt();
            bound
                .consume_call(erased, issuance.location.block.index(), issuer, 0)
                .unwrap();
            bound
                .consume_call(
                    erased,
                    helper_call.location.block.index(),
                    helper,
                    raw.arg_count + 1,
                )
                .unwrap();
            let changed = bound
                .finish(
                    tcx,
                    issuer_callable,
                    |local| {
                        if mutation == 0 {
                            None
                        } else if mutation == 1 {
                            Some(SemanticLocalIdV1::from_index(u32::MAX))
                        } else {
                            planned.raw_to_semantic_locals.get(local.index()).copied()
                        }
                    },
                    |ty| {
                        if mutation == 2 {
                            None
                        } else {
                            plan.type_producers()
                                .iter()
                                .position(|binding| binding.ty == ty)
                                .map(|index| SemanticTypeIdV1::from_index(index as u32))
                        }
                    },
                    |_| Ok(()),
                )
                .and_then(|entry| entry.bind_function(&produced, |_| Ok(())));
            assert!(changed.is_err(), "source mapping substitution {mutation}");
        }
        assert!(finish(receipt()).is_err(), "unused occurrences");
        let mut missing = receipt();
        missing
            .consume_call(erased, issuance.location.block.index(), issuer, 0)
            .unwrap();
        assert!(finish(missing).is_err(), "missing helper");
        let mut wrong = erased.clone();
        let TerminatorKind::Call { args, .. } = &mut wrong.basic_blocks.as_mut()
            [helper_call.location.block]
            .terminator_mut()
            .kind
        else {
            unreachable!()
        };
        args.swap(1, 2);
        assert!(
            construct(&wrong, Some(receipt()), limits).is_err(),
            "same-typed physical argument substitution"
        );
        assert_eq!(
            construct(erased, Some(receipt()), limits).unwrap(),
            produced,
            "failed construction publishes no reusable receipt"
        );
        let mut owner =
            ProductionSemanticBodyRequestOwnerV1::new(limits, types.len(), &owned).unwrap();
        let mut late_failure = receipt();
        late_failure.helper_function = issuer_function;
        assert!(construct_with_owner(erased, Some(late_failure), &mut owner).is_err());
        assert_eq!(owner.retained_context_entry_count(), 0);
        construct_with_owner(erased, Some(receipt()), &mut owner).unwrap();
        assert_eq!(owner.retained_context_entry_count(), 1);
        assert!(
            construct_with_owner(erased, Some(receipt()), &mut owner).is_err(),
            "duplicate completed receipt"
        );
        assert_eq!(owner.retained_context_entry_count(), 1);
        for resource in [
            SemanticMirResourceV1::ValidationWork,
            SemanticMirResourceV1::Operands,
        ] {
            let mut low = 0;
            let mut high = 4096;
            while low < high {
                let middle = (low + high) / 2;
                if construct(
                    erased,
                    Some(receipt()),
                    limits.with_limit(resource, middle).unwrap(),
                )
                .is_ok()
                {
                    high = middle;
                } else {
                    low = middle + 1;
                }
            }
            assert!(low > 0 && low < 4096);
            assert_eq!(
                construct(
                    erased,
                    Some(receipt()),
                    limits.with_limit(resource, low).unwrap()
                )
                .unwrap(),
                produced
            );
            assert!(
                construct(
                    erased,
                    Some(receipt()),
                    limits.with_limit(resource, low - 1).unwrap()
                )
                .is_err(),
                "one-short {resource:?}"
            );
        }
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
fn authenticated_context_binding_is_consumed_by_the_actual_body_builder() {
    let directory = TestTempDir::create("fe2o3-context-body");
    let source = directory.path().join("fixture.rs");
    std::fs::write(&source, SOURCE).unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let sysroot = String::from_utf8(output.stdout).unwrap();
    for level in [0, 2] {
        let args = vec![
            "rustc".into(),
            "--crate-name".into(),
            "fe2o3_context_body_fixture".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "--emit=metadata".into(),
            format!("-Zmir-opt-level={level}"),
            "-Cpanic=abort".into(),
            "--sysroot".into(),
            sysroot.trim().into(),
            "-o".into(),
            directory.path().join("fixture.rmeta").display().to_string(),
            source.display().to_string(),
        ];
        let mut callbacks = ContextCallbacks {
            erased: level == 2,
            ..Default::default()
        };
        rustc_driver::run_compiler(&args, &mut callbacks);
        assert!(callbacks.completed);
    }
}
