//! Genuine rustc query products and explicitly labeled cloned-body hostiles.
use super::super::*;
use crate::production_safe_core_shift_v1::SafeCoreShiftV1;
use crate::test_temp_dir::TestTempDir;
#[path = "host_core_v1_tests.rs"]
mod host_core;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{
    self, BasicBlock, BasicBlockData, Const, ConstOperand, ConstValue, Local, LocalDecl, Operand,
    Place, ProjectionElem, Rvalue, Statement, StatementKind, SwitchTargets, TerminatorKind,
};

fn function<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    Instance::mono(
        tcx,
        tcx.iter_local_def_id()
            .find(|id| {
                tcx.def_kind(id.to_def_id()) == DefKind::Fn
                    && tcx.item_name(id.to_def_id()).as_str() == name
            })
            .unwrap()
            .to_def_id(),
    )
}
fn call<'tcx>(tcx: TyCtxt<'tcx>, caller: Instance<'tcx>) -> (BasicBlock, Instance<'tcx>) {
    let body = tcx.instance_mir(caller.def);
    let mut calls = body
        .basic_blocks
        .iter_enumerated()
        .filter_map(|(block, data)| {
            let TerminatorKind::Call { func, .. } = &data.terminator().kind else {
                return None;
            };
            Some((
                block,
                crate::closure_profile_v1::resolve_direct_call(tcx, caller, func).unwrap(),
            ))
        });
    let result = calls.next().unwrap();
    assert!(calls.next().is_none());
    result
}
fn stages() -> [Stage; 4] {
    [
        Stage::Collector,
        Stage::ClosureReplay,
        Stage::Preflight,
        Stage::BodyReplay,
    ]
}

fn checked_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    actual: CheckedPrimitiveFromV1<'tcx>,
    body: &Body<'tcx>,
) -> Result<(), ()> {
    checker::check(
        tcx,
        actual.instance,
        body,
        actual.input_type(),
        actual.output_type(),
        &mut Context {
            stage: Stage::BodyReplay,
            limits: SemanticMirLimitsV1::default(),
            counts: [0; 8],
            charge: &mut |_| Ok(()),
        },
    )
}

fn stage_limits<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) {
    let body = tcx.instance_mir(instance.def);
    let scratch = std::mem::size_of::<scratch::Scratch>()
        + body.local_decls.len() * std::mem::size_of::<scratch::Fact>()
        + body.local_decls.len() * std::mem::size_of::<bool>()
        + body.basic_blocks.len() * std::mem::size_of::<bool>();
    let exact = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, scratch as u64)
        .unwrap();
    let mut amounts = Vec::new();
    let mut needed = 17usize;
    for stage in stages() {
        let mut work = 17usize;
        check_primitive_from_v1(tcx, instance, stage, exact, &mut |n| {
            work += n;
            Ok::<_, ()>(())
        })
        .unwrap()
        .unwrap();
        amounts.push(work);
    }
    assert!(amounts.iter().all(|n| *n == amounts[0]));
    needed += (amounts[0] - 17) * 4;
    for limit in [needed, needed - 1] {
        let mut attempted = 17;
        for stage in stages() {
            let result = check_primitive_from_v1(tcx, instance, stage, exact, &mut |n| {
                attempted += n;
                if attempted > limit {
                    Err((attempted, limit))
                } else {
                    Ok(())
                }
            });
            if stage != Stage::BodyReplay || limit == needed {
                assert!(result.unwrap().is_some());
            } else {
                assert!(
                    matches!(result, Err(PrimitiveFromErrorV1::Work { stage: Stage::BodyReplay,
                source: (actual, maximum) }) if actual == needed && maximum == needed - 1)
                );
            }
        }
        assert_eq!(attempted, needed);
    }
    for stage in stages() {
        let one = amounts[0];
        let mut attempted = 17;
        let result = check_primitive_from_v1(tcx, instance, stage, exact, &mut |n| {
            attempted += n;
            if attempted >= one {
                Err((attempted, one - 1))
            } else {
                Ok(())
            }
        });
        assert!(
            matches!(result, Err(PrimitiveFromErrorV1::Work { stage: s, source: (actual, maximum) })
            if s == stage && actual == one && maximum == one - 1)
        );
        let short = exact
            .with_limit(SemanticMirResourceV1::CanonicalBytes, scratch as u64 - 1)
            .unwrap();
        assert!(
            matches!(check_primitive_from_v1(tcx, instance, stage, short, &mut |_| Ok::<_, ()>(())),
            Err(PrimitiveFromErrorV1::Resource { stage: s, source: Resource::ScratchLimit {
                phase: Phase { slab: Slab::Blocks, backing: Backing::Requested }, actual, maximum } })
            if s == stage && actual == scratch as u64 && maximum + 1 == actual)
        );
    }
}

fn structural_limits<'tcx>(tcx: TyCtxt<'tcx>, actual: CheckedPrimitiveFromV1<'tcx>) {
    use SemanticMirResourceV1 as R;
    let body = actual.body();
    let statements = body
        .basic_blocks
        .iter()
        .find(|b| !b.statements.is_empty())
        .unwrap()
        .statements
        .len();
    let switches = body
        .basic_blocks
        .iter()
        .find_map(|b| match &b.terminator().kind {
            TerminatorKind::SwitchInt { targets, .. } => Some(targets.all_targets().len()),
            _ => None,
        })
        .unwrap();
    let (arguments, literal_bytes) = body
        .basic_blocks
        .iter()
        .find_map(|b| {
            let TerminatorKind::Call { args, .. } = &b.terminator().kind else {
                return None;
            };
            let Operand::Constant(c) = &args[0].node else {
                unreachable!()
            };
            let Const::Val(ConstValue::Slice { alloc_id, .. }, _) = c.const_ else {
                unreachable!()
            };
            let mir::interpret::GlobalAlloc::Memory(a) = tcx.global_alloc(alloc_id) else {
                unreachable!()
            };
            Some((args.len(), a.inner().len()))
        })
        .unwrap();
    for stage in stages() {
        for (resource, actual_count) in [
            (R::Locals, body.local_decls.len()),
            (R::Blocks, body.basic_blocks.len()),
            (R::Statements, statements),
            (R::Operands, body.required_consts.as_ref().unwrap().len()),
            (R::CallArguments, arguments),
            (R::SwitchTargets, switches),
            (R::ConstantBytes, literal_bytes),
        ] {
            assert!(actual_count > 0);
            let limits = SemanticMirLimitsV1::default()
                .with_limit(resource, 0)
                .unwrap();
            assert!(
                matches!(check_primitive_from_v1(tcx, actual.instance, stage, limits, &mut |_| Ok::<_, ()>(())),
            Err(PrimitiveFromErrorV1::Resource { stage: s, source: Resource::Structural { resource: r, actual, maximum: 0 } })
                if s == stage && r == resource && actual == actual_count as u64)
            );
        }
    }
    let place = Place {
        local: Local::from_usize(1),
        projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
    };
    let mut work = 17;
    let mut cx = Context {
        stage: Stage::BodyReplay,
        limits: SemanticMirLimitsV1::default()
            .with_limit(R::Projections, 0)
            .unwrap(),
        counts: [0; 8],
        charge: &mut |n| {
            work += n;
            Ok::<_, ()>(())
        },
    };
    assert!(matches!(
        raw::place(place, body, RawSiteV1::Signature, &mut cx),
        Err(PrimitiveFromErrorV1::Resource {
            source: Resource::Structural {
                resource: R::Projections,
                actual: 1,
                maximum: 0
            },
            ..
        })
    ));
    assert_eq!(work, 18);
}

fn hostile_bodies<'tcx>(tcx: TyCtxt<'tcx>, actual: CheckedPrimitiveFromV1<'tcx>) {
    let input = Place::from(Local::from_usize(1));
    let start = BasicBlock::from_usize(0);
    let mut cycle = actual.body().clone();
    cycle.basic_blocks.as_mut()[start]
        .terminator
        .as_mut()
        .unwrap()
        .kind = TerminatorKind::Goto { target: start };
    assert!(matches!(
        checked_body(tcx, actual, &cycle),
        Err(PrimitiveFromErrorV1::Semantics {
            reason: Semantic::Cycle,
            ..
        })
    ));
    let mut runtime = actual.body().clone();
    runtime.basic_blocks.as_mut()[start]
        .terminator
        .as_mut()
        .unwrap()
        .kind = TerminatorKind::SwitchInt {
        discr: Operand::Copy(input),
        targets: SwitchTargets::static_if(0, start, start),
    };
    assert!(matches!(
        checked_body(tcx, actual, &runtime),
        Err(PrimitiveFromErrorV1::Semantics {
            reason: Semantic::UnknownBranch,
            ..
        })
    ));
    let mut edge = actual.body().clone();
    let invalid = BasicBlock::from_usize(edge.basic_blocks.len() + 1);
    edge.basic_blocks.as_mut()[start]
        .terminator
        .as_mut()
        .unwrap()
        .kind = TerminatorKind::Goto { target: invalid };
    assert!(matches!(
        checked_body(tcx, actual, &edge),
        Err(PrimitiveFromErrorV1::RawPolicy {
            reason: Raw::Edge,
            ..
        })
    ));
    let (return_block, assignment) = actual.body().basic_blocks.iter_enumerated().find_map(|(b, data)| {
        data.statements.iter().position(|s| matches!(&s.kind, StatementKind::Assign(p) if p.0.local == Local::from_usize(0))).map(|s| (b, s))
    }).unwrap();
    for live in [true, false] {
        let mut killed = actual.body().clone();
        let info = killed.basic_blocks[return_block].statements[assignment].source_info;
        killed.basic_blocks.as_mut()[return_block]
            .statements
            .insert(
                assignment,
                Statement::new(
                    info,
                    if live {
                        StatementKind::StorageLive(Local::from_usize(1))
                    } else {
                        StatementKind::StorageDead(Local::from_usize(1))
                    },
                ),
            );
        assert!(matches!(
            checked_body(tcx, actual, &killed),
            Err(PrimitiveFromErrorV1::Semantics {
                reason: Semantic::Unsupported,
                ..
            })
        ));
    }
    let mut moved = actual.body().clone();
    let saved = moved
        .local_decls
        .push(LocalDecl::new(actual.input_type(), moved.span));
    let info = moved.basic_blocks[return_block].statements[assignment].source_info;
    moved.basic_blocks.as_mut()[return_block].statements.insert(
        assignment,
        Statement::new(
            info,
            StatementKind::Assign(Box::new((
                Place::from(saved),
                Rvalue::Use(Operand::Move(input)),
            ))),
        ),
    );
    assert!(matches!(
        checked_body(tcx, actual, &moved),
        Err(PrimitiveFromErrorV1::Semantics {
            reason: Semantic::Unsupported,
            ..
        })
    ));
    let mut projected = actual.body().clone();
    let StatementKind::Assign(pair) =
        &mut projected.basic_blocks.as_mut()[return_block].statements[assignment].kind
    else {
        unreachable!()
    };
    pair.0.projection = tcx.mk_place_elems(&[ProjectionElem::Deref]);
    assert!(matches!(
        checked_body(tcx, actual, &projected),
        Err(PrimitiveFromErrorV1::RawPolicy {
            reason: Raw::Place,
            ..
        })
    ));
    let mut wrong = actual.body().clone();
    let StatementKind::Assign(pair) =
        &mut wrong.basic_blocks.as_mut()[return_block].statements[assignment].kind
    else {
        unreachable!()
    };
    pair.1 = Rvalue::Use(Operand::Constant(Box::new(ConstOperand {
        span: wrong.span,
        user_ty: None,
        const_: Const::from_bits(
            tcx,
            0,
            TypingEnv::fully_monomorphized(),
            actual.output_type(),
        ),
    })));
    assert!(matches!(
        checked_body(tcx, actual, &wrong),
        Err(PrimitiveFromErrorV1::Semantics {
            reason: Semantic::WrongReturn,
            ..
        })
    ));
    let mut identity = actual.body().clone();
    identity.source.instance = function(tcx, "local_identity").def;
    assert!(matches!(
        checked_body(tcx, actual, &identity),
        Err(PrimitiveFromErrorV1::RawPolicy {
            reason: Raw::Body,
            ..
        })
    ));
    let mut required = actual.body().clone();
    required.required_consts = None;
    assert!(matches!(
        checked_body(tcx, actual, &required),
        Err(PrimitiveFromErrorV1::RawPolicy {
            reason: Raw::Body,
            ..
        })
    ));
    let mut required = actual.body().clone();
    required
        .required_consts
        .as_mut()
        .unwrap()
        .push(ConstOperand {
            span: required.span,
            user_ty: None,
            const_: Const::from_bits(tcx, 2, TypingEnv::fully_monomorphized(), tcx.types.bool),
        });
    assert!(matches!(
        checked_body(tcx, actual, &required),
        Err(PrimitiveFromErrorV1::RawPolicy {
            site: RawSiteV1::RequiredConstant(_),
            reason: Raw::ConstantValue,
            ..
        })
    ));
}

fn dead_calls_and_literals<'tcx>(tcx: TyCtxt<'tcx>, actual: CheckedPrimitiveFromV1<'tcx>) {
    let probe = function(tcx, "panic_probe");
    let (probe_block, _) = call(tcx, probe);
    let mut term = tcx.instance_mir(probe.def).basic_blocks[probe_block]
        .terminator()
        .clone();
    let mut body = actual.body().clone();
    let never = body
        .local_decls
        .push(LocalDecl::new(tcx.types.never, body.span));
    let TerminatorKind::Call {
        destination,
        target,
        args,
        ..
    } = &mut term.kind
    else {
        unreachable!()
    };
    *destination = Place::from(never);
    assert!(target.is_none());
    assert_eq!(args.len(), 1);
    let Operand::Constant(literal) = &args[0].node else {
        unreachable!()
    };
    let literal = **literal;
    assert!(matches!(
        literal.const_,
        Const::Val(ConstValue::Slice { .. }, _)
    ));
    let dead = body
        .basic_blocks
        .as_mut()
        .push(BasicBlockData::new(Some(term), false));
    checked_body(tcx, actual, &body).unwrap();
    for destination in [true, false] {
        for projection in [true, false] {
            let mut bad = body.clone();
            let place = if projection {
                Place {
                    local: Local::from_usize(1),
                    projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
                }
            } else {
                Place::from(Local::from_usize(bad.local_decls.len() + 1))
            };
            let TerminatorKind::Call {
                destination: dest,
                args,
                ..
            } = &mut bad.basic_blocks.as_mut()[dead]
                .terminator
                .as_mut()
                .unwrap()
                .kind
            else {
                unreachable!()
            };
            if destination {
                *dest = place;
            } else {
                args[0].node = Operand::Copy(place);
            }
            assert!(
                matches!(checked_body(tcx, actual, &bad), Err(PrimitiveFromErrorV1::RawPolicy { site: RawSiteV1::Terminator(b), reason: Raw::Place, .. }) if b == dead.as_u32())
            );
        }
    }
    let mut live = body.clone();
    live.basic_blocks.as_mut()[BasicBlock::from_usize(0)]
        .terminator
        .as_mut()
        .unwrap()
        .kind = TerminatorKind::Goto { target: dead };
    assert!(matches!(
        checked_body(tcx, actual, &live),
        Err(PrimitiveFromErrorV1::Semantics {
            reason: Semantic::Unsupported,
            ..
        })
    ));
    let mut oversized = literal;
    let Const::Val(ConstValue::Slice { meta, .. }, _) = &mut oversized.const_ else {
        unreachable!()
    };
    *meta = u64::MAX;
    let mut cx = Context {
        stage: Stage::Collector,
        limits: SemanticMirLimitsV1::default(),
        counts: [0; 8],
        charge: &mut |_| Ok::<_, ()>(()),
    };
    assert!(matches!(
        raw::constant(
            tcx,
            &oversized,
            false,
            RawSiteV1::RequiredConstant(0),
            &mut cx
        ),
        Err(PrimitiveFromErrorV1::RawPolicy {
            reason: Raw::LiteralAllocation,
            ..
        })
    ));
    let mut mutable = literal;
    let ty::TyKind::Ref(_, target, _) = literal.const_.ty().kind() else {
        unreachable!()
    };
    let Const::Val(value, _) = literal.const_ else {
        unreachable!()
    };
    mutable.const_ = Const::Val(
        value,
        Ty::new_mut_ref(tcx, tcx.lifetimes.re_erased, *target),
    );
    assert!(matches!(
        raw::constant(
            tcx,
            &mutable,
            false,
            RawSiteV1::RequiredConstant(0),
            &mut cx
        ),
        Err(PrimitiveFromErrorV1::RawPolicy {
            reason: Raw::ConstantValue,
            ..
        })
    ));
    crate::collector::primitive_from_stage_tests::reject_original_panic(tcx, probe);
}

fn actual_constants_and_raw_calls<'tcx>(tcx: TyCtxt<'tcx>, actual: CheckedPrimitiveFromV1<'tcx>) {
    use crate::production_raw_call_audit_v1::{RawCallRefusalV1, audit_raw_call_v1};
    let required = actual.body().required_consts.as_ref().unwrap();
    assert_eq!(
        required.len(),
        4,
        "pinned u32-to-u64 instance required-constant roster"
    );
    let u32_ty = scalar::ty(tcx.types.u32).unwrap();
    let u64_ty = scalar::ty(tcx.types.u64).unwrap();
    let expected = [
        (u64_ty, 0),
        (u32_ty, 0),
        (u32_ty, u128::from(u32::MAX)),
        (u64_ty, u128::from(u64::MAX)),
    ];
    for (index, (constant, (ty, bits))) in required.iter().zip(expected).enumerate() {
        let Const::Unevaluated(unevaluated, constant_ty) = constant.const_ else {
            unreachable!()
        };
        assert!(unevaluated.args.is_empty());
        assert!(unevaluated.promoted.is_none());
        let mut charges = Vec::new();
        let value = raw::constant(
            tcx,
            constant,
            false,
            RawSiteV1::RequiredConstant(index as u32),
            &mut Context {
                stage: Stage::BodyReplay,
                limits: SemanticMirLimitsV1::default(),
                counts: [0; 8],
                charge: &mut |n| {
                    charges.push(n);
                    Ok::<_, ()>(())
                },
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!((value.ty, value.bits), (ty, bits));
        assert_eq!(
            charges,
            [1, 1, 1],
            "occurrence, actual definition audit, actual CTFE query"
        );
        // This synthetic carrier violates Const::eval's normalized-Ty precondition.
        let alternate = ConstOperand {
            const_: Const::Ty(
                constant_ty,
                ty::Const::new_unevaluated(
                    tcx,
                    ty::UnevaluatedConst {
                        def: unevaluated.def,
                        args: unevaluated.args,
                    },
                ),
            ),
            ..*constant
        };
        // Query a real valtree from the same definition for the valid Ty carrier.
        let valtree = tcx
            .const_eval_resolve_for_typeck(
                TypingEnv::fully_monomorphized(),
                ty::UnevaluatedConst {
                    def: unevaluated.def,
                    args: unevaluated.args,
                },
                constant.span,
            )
            .unwrap()
            .unwrap();
        let normalized = ConstOperand {
            const_: Const::Ty(constant_ty, ty::Const::new_value(tcx, valtree, constant_ty)),
            ..*constant
        };
        for stage in stages() {
            assert!(tcx.dcx().has_errors_or_delayed_bugs().is_none());
            let mut denied_charges = Vec::new();
            {
                let mut cx = Context {
                    stage,
                    limits: SemanticMirLimitsV1::default(),
                    counts: [0; 8],
                    charge: &mut |n| {
                        denied_charges.push(n);
                        Ok::<_, ()>(())
                    },
                };
                assert!(matches!(
                    raw::constant(
                        tcx,
                        &alternate,
                        false,
                        RawSiteV1::RequiredConstant(index as u32),
                        &mut cx
                    ),
                    Err(PrimitiveFromErrorV1::RawPolicy {
                        stage: actual_stage,
                        site: RawSiteV1::RequiredConstant(actual_index),
                        reason: Raw::ConstantEvaluation,
                    }) if actual_stage == stage && actual_index == index as u32
                ));
            }
            assert_eq!(
                denied_charges,
                [1, 1, 1],
                "occurrence, definition audit, evaluation precondition; no CTFE query"
            );
            assert!(tcx.dcx().has_errors_or_delayed_bugs().is_none());
            let mut value_charges = Vec::new();
            {
                let mut cx = Context {
                    stage,
                    limits: SemanticMirLimitsV1::default(),
                    counts: [0; 8],
                    charge: &mut |n| {
                        value_charges.push(n);
                        Ok::<_, ()>(())
                    },
                };
                assert_eq!(
                    raw::constant(
                        tcx,
                        &normalized,
                        false,
                        RawSiteV1::RequiredConstant(index as u32),
                        &mut cx
                    )
                    .unwrap(),
                    Some(value)
                );
            }
            assert_eq!(
                value_charges,
                [1, 1],
                "occurrence and normalized evaluation"
            );
            assert!(tcx.dcx().has_errors_or_delayed_bugs().is_none());
        }
        let mut cx = Context {
            stage: Stage::Preflight,
            limits: SemanticMirLimitsV1::default(),
            counts: [0; 8],
            charge: &mut |_| Ok::<_, ()>(()),
        };
        let promoted = ConstOperand {
            const_: Const::Unevaluated(
                mir::UnevaluatedConst {
                    promoted: Some(mir::Promoted::from_usize(0)),
                    ..unevaluated
                },
                constant_ty,
            ),
            ..*constant
        };
        assert!(matches!(
            raw::constant(
                tcx,
                &promoted,
                false,
                RawSiteV1::RequiredConstant(index as u32),
                &mut cx
            ),
            Err(PrimitiveFromErrorV1::RawPolicy {
                reason: Raw::ConstantIdentity,
                ..
            })
        ));
    }
    for (name, expected, charges) in [
        ("panic_probe", None, 12),
        ("spin_probe", None, 9),
        ("unsafe_probe", Some(RawCallRefusalV1::Signature), 5),
        ("fake_probe", Some(RawCallRefusalV1::Core), 2),
    ] {
        let caller = function(tcx, name);
        let (block, _) = call(tcx, caller);
        let TerminatorKind::Call { func, .. } = &tcx.instance_mir(caller.def).basic_blocks[block]
            .terminator()
            .kind
        else {
            unreachable!()
        };
        if expected.is_none() {
            let resolved = call(tcx, caller).1;
            let signature =
                crate::rustc_semantic_plan_v1::source_signature_v1(tcx, resolved).unwrap();
            let abi = tcx
                .fn_abi_of_instance(
                    TypingEnv::fully_monomorphized().as_query_input((resolved, ty::List::empty())),
                )
                .unwrap();
            let tracked = name == "panic_probe";
            assert_eq!(resolved.def.requires_caller_location(tcx), tracked);
            assert_eq!(signature.inputs().len(), usize::from(tracked));
            assert_eq!(
                usize::try_from(abi.fixed_count).unwrap(),
                signature.inputs().len()
            );
            assert_eq!(
                abi.args.len(),
                signature.inputs().len() + usize::from(tracked)
            );
            assert_eq!(abi.conv, rustc_abi::CanonAbi::Rust);
            assert!(!abi.c_variadic);
            assert_eq!(abi.ret.layout.ty, signature.output());
            for (argument, source) in abi.args.iter().zip(signature.inputs()) {
                assert_eq!(argument.layout.ty, *source);
            }
            if tracked {
                let location = tcx
                    .layout_of(
                        TypingEnv::fully_monomorphized().as_query_input(tcx.caller_location_ty()),
                    )
                    .unwrap();
                assert_eq!(abi.args.last().unwrap().layout.ty, location.ty);
            } else {
                assert!(
                    abi.args.is_empty(),
                    "ordinary zero-argument core call has no hidden tail"
                );
            }
        }
        if name == "panic_probe" {
            let Operand::Constant(constant) = func else {
                unreachable!()
            };
            let ty::TyKind::FnDef(definition, args) = constant.const_.ty().kind() else {
                unreachable!()
            };
            let original = tcx.instantiate_bound_regions_with_erased(
                tcx.fn_sig(*definition).instantiate(tcx, args),
            );
            let resolved =
                crate::rustc_semantic_plan_v1::source_signature_v1(tcx, call(tcx, caller).1)
                    .unwrap();
            assert_ne!(original, resolved, "actual static-region regression");
            assert_eq!(
                tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), original)
                    .unwrap(),
                resolved,
            );
        }
        let mut work = Vec::new();
        let result = audit_raw_call_v1(tcx, caller, func, &mut |n| {
            work.push(n);
            Ok::<_, ()>(())
        })
        .unwrap();
        if let Some(expected) = expected {
            assert_eq!(result.unwrap_err(), expected);
        } else {
            assert_eq!(result.unwrap(), call(tcx, caller).1);
        }
        assert_eq!(work, vec![1; charges]);
        if expected.is_none() {
            for denied in 1..=charges {
                let limit = 17 + denied - 1;
                let mut accepted = 17;
                let mut attempts = Vec::new();
                let result = audit_raw_call_v1(tcx, caller, func, &mut |n| {
                    attempts.push(n);
                    let actual = accepted + n;
                    if actual > limit {
                        Err((actual, limit))
                    } else {
                        accepted = actual;
                        Ok(())
                    }
                });
                assert_eq!(result.unwrap_err(), (17 + denied, limit));
                assert_eq!(accepted, limit);
                assert_eq!(attempts, vec![1; denied]);
            }
        }
    }
    let mut work = 17;
    assert_eq!(
        audit_raw_call_v1(
            tcx,
            actual.instance,
            &Operand::Copy(Place::from(Local::from_usize(1))),
            &mut |n| {
                work += n;
                Ok::<_, ()>(())
            }
        )
        .unwrap()
        .unwrap_err(),
        RawCallRefusalV1::Operand
    );
    assert_eq!(work, 18);
    let other = check_primitive_from_v1(
        tcx,
        call(tcx, function(tcx, "from_u32_u128")).1,
        Stage::Preflight,
        SemanticMirLimitsV1::default(),
        &mut |_| Ok::<_, ()>(()),
    )
    .unwrap()
    .unwrap();
    assert!(!actual.same_producers(other));
    for substituted in [
        CheckedPrimitiveFromV1 {
            instance: other.instance,
            ..actual
        },
        CheckedPrimitiveFromV1 {
            body: other.body,
            ..actual
        },
        CheckedPrimitiveFromV1 {
            abi: other.abi,
            ..actual
        },
    ] {
        assert!(
            !actual.same_producers(substituted),
            "each actual query-product identity is necessary"
        );
    }
}

fn cloned_branch_and_alias_obligations<'tcx>(
    tcx: TyCtxt<'tcx>,
    actual: CheckedPrimitiveFromV1<'tcx>,
) {
    let start = BasicBlock::from_usize(0);
    let return_block = actual
        .body()
        .basic_blocks
        .iter_enumerated()
        .find_map(|(b, data)| matches!(data.terminator().kind, TerminatorKind::Return).then_some(b))
        .unwrap();
    let boolean = |bits| {
        Operand::Constant(Box::new(ConstOperand {
            span: actual.body().span,
            user_ty: None,
            const_: Const::from_bits(tcx, bits, TypingEnv::fully_monomorphized(), tcx.types.bool),
        }))
    };
    for bits in [0, 1] {
        let mut same_join = actual.body().clone();
        same_join.basic_blocks.as_mut()[start]
            .terminator
            .as_mut()
            .unwrap()
            .kind = TerminatorKind::SwitchInt {
            discr: boolean(bits),
            targets: SwitchTargets::static_if(0, return_block, return_block),
        };
        checked_body(tcx, actual, &same_join).unwrap();
    }
    let mut across_edge = actual.body().clone();
    let flag = across_edge
        .local_decls
        .push(LocalDecl::new(tcx.types.bool, across_edge.span));
    let mut branch = across_edge.basic_blocks[start].terminator().clone();
    branch.kind = TerminatorKind::SwitchInt {
        discr: Operand::Copy(Place::from(flag)),
        targets: SwitchTargets::static_if(0, return_block, return_block),
    };
    let next = across_edge
        .basic_blocks
        .as_mut()
        .push(BasicBlockData::new(Some(branch), false));
    let data = &mut across_edge.basic_blocks.as_mut()[start];
    data.statements.push(Statement::new(
        data.terminator().source_info,
        StatementKind::Assign(Box::new((Place::from(flag), Rvalue::Use(boolean(1))))),
    ));
    data.terminator.as_mut().unwrap().kind = TerminatorKind::Goto { target: next };
    assert!(
        matches!(checked_body(tcx, actual, &across_edge), Err(PrimitiveFromErrorV1::Semantics {
        site: RawSiteV1::Terminator(b), reason: Semantic::UnknownBranch, .. }) if b == next.as_u32())
    );
    for raw_pointer in [false, true] {
        let mut alias = actual.body().clone();
        let mut dead = BasicBlockData::new(
            Some(alias.basic_blocks[return_block].terminator().clone()),
            false,
        );
        let input = Place::from(Local::from_usize(1));
        dead.statements.push(Statement::new(
            dead.terminator().source_info,
            StatementKind::Assign(Box::new((
                input,
                if raw_pointer {
                    Rvalue::RawPtr(mir::RawPtrKind::Const, input)
                } else {
                    Rvalue::Ref(tcx.lifetimes.re_erased, mir::BorrowKind::Shared, input)
                },
            ))),
        ));
        let block = alias.basic_blocks.as_mut().push(dead);
        assert!(
            matches!(checked_body(tcx, actual, &alias), Err(PrimitiveFromErrorV1::RawPolicy {
            site: RawSiteV1::Statement { block: b, statement: 0 }, reason: Raw::Rvalue, .. }) if b == block.as_u32())
        );
    }
    let mut overwritten = actual.body().clone();
    let data = &mut overwritten.basic_blocks.as_mut()[return_block];
    data.statements.insert(
        0,
        Statement::new(
            data.terminator().source_info,
            StatementKind::Assign(Box::new((
                Place::from(Local::from_usize(1)),
                Rvalue::Use(Operand::Constant(Box::new(ConstOperand {
                    span: actual.body().span,
                    user_ty: None,
                    const_: Const::from_bits(
                        tcx,
                        0,
                        TypingEnv::fully_monomorphized(),
                        actual.input_type(),
                    ),
                }))),
            ))),
        ),
    );
    assert!(matches!(
        checked_body(tcx, actual, &overwritten),
        Err(PrimitiveFromErrorV1::Semantics {
            reason: Semantic::WrongReturn,
            ..
        })
    ));
}

#[derive(Default)]
struct Capture {
    completed: bool,
}
impl Callbacks for Capture {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let shift = SafeCoreShiftV1::classify(tcx, call(tcx, function(tcx, "shift_probe")).1)
            .unwrap()
            .unwrap();
        let mut count = 0;
        for definition in tcx.iter_local_def_id().filter(|id| {
            tcx.def_kind(id.to_def_id()) == DefKind::Fn
                && tcx.item_name(id.to_def_id()).as_str().starts_with("from_")
        }) {
            let caller = Instance::mono(tcx, definition.to_def_id());
            let (block, callee) = call(tcx, caller);
            let actual = check_primitive_from_v1(
                tcx,
                callee,
                Stage::Collector,
                SemanticMirLimitsV1::default(),
                &mut |_| Ok::<_, ()>(()),
            )
            .unwrap_or_else(|error| {
                for data in tcx.instance_mir(callee.def).basic_blocks.iter() {
                    if let TerminatorKind::Call { func, .. } = &data.terminator().kind {
                        let raw = crate::closure_profile_v1::resolve_direct_call(tcx, callee, func)
                            .unwrap();
                        let abi = tcx
                            .fn_abi_of_instance(
                                TypingEnv::fully_monomorphized()
                                    .as_query_input((raw, ty::List::empty())),
                            )
                            .unwrap();
                        eprintln!(
                            "actual raw call {raw:?}: signature={:?}; ABI={abi:?}",
                            crate::rustc_semantic_plan_v1::source_signature_v1(tcx, raw)
                        );
                    }
                }
                panic!("actual primitive From {callee:?}: {error:?}");
            })
            .unwrap();
            assert!(std::ptr::eq(actual.body(), tcx.instance_mir(callee.def)));
            assert!(
                actual.same_producers(
                    check_primitive_from_v1(
                        tcx,
                        callee,
                        Stage::BodyReplay,
                        SemanticMirLimitsV1::default(),
                        &mut |_| Ok::<_, ()>(())
                    )
                    .unwrap()
                    .unwrap()
                )
            );
            let work = crate::collector::primitive_from_stage_tests::collect_and_replay(
                tcx,
                caller,
                tcx.instance_mir(caller.def),
                block,
            );
            crate::production_semantic_body_v1::primitive_from_stage_tests::preflight_and_construct(
                tcx, caller, work,
            );
            if tcx.item_name(definition.to_def_id()).as_str() == "from_u32_u64" {
                stage_limits(tcx, callee);
                structural_limits(tcx, actual);
                hostile_bodies(tcx, actual);
                dead_calls_and_literals(tcx, actual);
                actual_constants_and_raw_calls(tcx, actual);
                cloned_branch_and_alias_obligations(tcx, actual);
                super::layout::check_actual(tcx, actual, shift);
            }
            count += 1;
        }
        assert_eq!(count, 30);
        for name in ["identity_probe", "bool_probe", "fake_probe"] {
            let callee = call(tcx, function(tcx, name)).1;
            assert!(
                check_primitive_from_v1(
                    tcx,
                    callee,
                    Stage::Collector,
                    SemanticMirLimitsV1::default(),
                    &mut |_| Ok::<_, ()>(())
                )
                .unwrap()
                .is_none()
            );
        }
        assert!(
            check_primitive_from_v1(
                tcx,
                function(tcx, "local_identity"),
                Stage::Collector,
                SemanticMirLimitsV1::default(),
                &mut |_| Ok::<_, ()>(())
            )
            .unwrap()
            .is_none()
        );
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
fn primitive_from_actual_rustc_all_endpoints_four_stages_and_hostile_raw_bodies() {
    let directory = TestTempDir::create("fe2o3-checked-primitive-from");
    let source = directory.path().join("fixture.rs");
    let mut text = String::from(
        "#![no_std]\n#![feature(panic_internals)]\n\
        #[inline(never)] pub fn panic_probe() -> ! { core::panicking::panic(\"primitive From audit literal\") }\n\
        #[inline(never)] pub fn spin_probe() { core::hint::spin_loop() }\n\
        #[inline(never)] pub unsafe fn unsafe_probe() -> ! { unsafe { core::hint::unreachable_unchecked() } }\n\
        #[inline(never)] pub fn shift_probe(x:u32)->u32 { x.wrapping_shl(5) }\n\
        #[inline(never)] pub fn local_identity(x:u32)->u64 { x as u64 }\n\
        #[inline(never)] pub fn identity_probe(x:u32)->u32 { u32::from(x) }\n\
        #[inline(never)] pub fn bool_probe(x:bool)->u8 { u8::from(x) }\n\
        pub struct Fake; impl Fake { #[inline(never)] pub fn from(x:u32)->u64 { x as u64 } }\n\
        #[inline(never)] pub fn fake_probe(x:u32)->u64 { Fake::from(x) }\n",
    );
    for a in [8, 16, 32, 64, 128] {
        for b in [8, 16, 32, 64, 128] {
            if b > a {
                for (source, target) in [("u", "u"), ("i", "i"), ("u", "i")] {
                    text.push_str(&format!("#[inline(never)] pub fn from_{source}{a}_{target}{b}(x:{source}{a})->{target}{b} {{ {target}{b}::from(x) }}\n"));
                }
            }
        }
    }
    std::fs::write(&source, text).unwrap();
    let args = host_core::fixture_arguments(&directory, &source);
    let mut callbacks = Capture::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.completed);
}
