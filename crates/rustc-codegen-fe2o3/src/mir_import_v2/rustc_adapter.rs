use super::accounting::recompute_capture_accounting_v2;
use super::budget::{BudgetErrorV2, CaptureBudgetV2};
use super::normalized::*;
use super::preflight::{PreflightErrorV2, preflight_body_v2};
use super::type_preflight::{
    TypePreflightErrorV2, preflight_generic_args_v2, preflight_mir_const_v2, preflight_ty_const_v2,
    preflight_ty_v2,
};
use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_middle::mir::{
    AggregateKind, Body, Local, NonDivergingIntrinsic, Operand, Place, ProjectionElem, Rvalue,
    SourceInfo, SourceScope, StatementKind, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{
    EarlyBinder, FloatTy, GenericArgsRef, Instance, InstanceKind, IntTy, ReifyReason, Ty, TyCtxt,
    TyKind, TypingEnv, UintTy,
};
use rustc_span::Span;
use std::error::Error;
use std::fmt;

macro_rules! stable_hash {
    ($tcx:expr, $value:expr) => {{
        let fingerprint: Fingerprint = $tcx.with_stable_hashing_context(|mut context| {
            let mut hasher = StableHasher::new();
            ($value).hash_stable(&mut context, &mut hasher);
            hasher.finish()
        });
        fingerprint.to_le_bytes()
    }};
}

macro_rules! stable_compiler_value {
    ($context:expr, $label:expr, $value:expr $(,)?) => {{
        let stable_hash = stable_hash!($context.tcx, $value);
        let diagnostic = $context.budget.bounded_debug($label, $value)?;
        Ok::<StableCompilerValueV2, CaptureErrorV2>(StableCompilerValueV2 {
            stable_hash,
            diagnostic,
        })
    }};
}

struct CaptureContextV2<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &'a Body<'tcx>,
    limits: CaptureLimitsV2,
    budget: CaptureBudgetV2,
}

pub(crate) fn capture_instance_body_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    limits: CaptureLimitsV2,
) -> Result<CapturedBodyV2, CaptureErrorV2> {
    let captured = capture_instance_observation_v2(tcx, instance, limits)?;
    captured.validate(limits)?;
    Ok(captured)
}

pub(crate) fn capture_instance_observation_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    limits: CaptureLimitsV2,
) -> Result<CapturedBodyV2, CaptureErrorV2> {
    let mut budget = CaptureBudgetV2::new(limits);
    preflight_instance_v2("selected instance", instance, limits, &mut budget)?;
    if matches!(
        instance.def,
        InstanceKind::Intrinsic(_) | InstanceKind::Virtual(..)
    ) {
        return Err(CaptureErrorV2::new(
            "the selected instance has no independently callable MIR body",
        ));
    }
    let def_id = instance.def_id();
    if matches!(instance.def, InstanceKind::Item(_)) && !tcx.is_mir_available(def_id) {
        return Err(CaptureErrorV2::new(
            "MIR is unavailable for the selected stable DefId",
        ));
    }
    let body = tcx.instance_mir(instance.def);
    capture_body_v2(tcx, instance, body, limits, budget)
}

fn capture_body_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    limits: CaptureLimitsV2,
    mut budget: CaptureBudgetV2,
) -> Result<CapturedBodyV2, CaptureErrorV2> {
    let total_work = preflight_body_v2(body, limits)?;
    budget.charge_work("MIR preflight", total_work)?;
    let mut context = CaptureContextV2 {
        tcx,
        instance,
        body,
        limits,
        budget,
    };

    let mut locals = Vec::with_capacity(body.local_decls.len());
    for (local, declaration) in body.local_decls.iter_enumerated() {
        locals.push(LocalDeclV2 {
            index: local.as_usize(),
            role: if local.as_usize() == 0 {
                LocalRoleV2::Return
            } else if local.as_usize() <= body.arg_count {
                LocalRoleV2::Argument
            } else {
                LocalRoleV2::Temporary
            },
            ty: type_identity(&mut context, declaration.ty)?,
            mutable: matches!(declaration.mutability, rustc_ast::Mutability::Mut),
            source: source_span(&mut context, declaration.source_info)?,
            diagnostic_debug: context
                .budget
                .bounded_debug("local declaration", declaration)?,
        });
    }

    let mut blocks = Vec::with_capacity(body.basic_blocks.len());
    for (block_index, block) in body.basic_blocks.iter_enumerated() {
        let mut statements = Vec::with_capacity(block.statements.len());
        for (index, statement) in block.statements.iter().enumerate() {
            let kind = capture_statement(&mut context, &statement.kind, statement.source_info)?;
            statements.push(StatementV2 {
                index,
                source: source_span(&mut context, statement.source_info)?,
                diagnostic_debug: context.budget.bounded_debug("statement", &statement.kind)?,
                kind,
            });
        }
        let terminator = block.terminator.as_ref().ok_or_else(|| {
            CaptureErrorV2::new(format!("bb{} has no terminator", block_index.as_usize()))
        })?;
        let successor_count = terminator.successors().count();
        ensure_bound(
            "terminator successors",
            successor_count,
            limits.max_successors,
        )?;
        let mut successors = Vec::with_capacity(successor_count);
        successors.extend(
            terminator
                .successors()
                .map(|successor| successor.as_usize()),
        );
        let terminator_kind =
            capture_terminator(&mut context, &terminator.kind, terminator.source_info)?;
        blocks.push(BasicBlockV2 {
            index: block_index.as_usize(),
            cleanup: block.is_cleanup,
            statements,
            terminator: TerminatorV2 {
                source: source_span(&mut context, terminator.source_info)?,
                diagnostic_debug: context
                    .budget
                    .bounded_debug("terminator", &terminator.kind)?,
                kind: terminator_kind,
                successors,
            },
        });
    }

    let function = function_identity(&mut context, instance)?;
    let source = span_identity(&mut context, body.span, 0)?;
    let mut captured = CapturedBodyV2 {
        schema_version: NORMALIZED_MIR_SCHEMA_V2,
        authority: CaptureAuthorityV2::CompilerObservationOnly,
        function,
        source,
        arg_count: body.arg_count,
        capture_work_items: 0,
        capture_text_bytes: 0,
        locals,
        blocks,
    };
    let accounting = recompute_capture_accounting_v2(&captured, limits)?;
    captured.capture_work_items = accounting.work_items;
    captured.capture_text_bytes = accounting.text_bytes;
    Ok(captured)
}

fn capture_statement<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    kind: &StatementKind<'tcx>,
    source_info: SourceInfo,
) -> Result<StatementKindV2, CaptureErrorV2> {
    match kind {
        StatementKind::Assign(assignment) => {
            let (destination, value) = &**assignment;
            Ok(StatementKindV2::Assign {
                destination: capture_place(context, *destination)?,
                value: Box::new(capture_rvalue(context, value, source_info)?),
            })
        }
        StatementKind::FakeRead(contents) => Ok(StatementKindV2::Unsupported(
            UnsupportedStatementV2::FakeRead {
                cause: stable_compiler_value!(context, "fake-read cause", &contents.0)?,
                place: capture_place(context, contents.1)?,
            },
        )),
        StatementKind::StorageLive(local) => Ok(StatementKindV2::StorageLive {
            local: local.as_usize(),
        }),
        StatementKind::StorageDead(local) => Ok(StatementKindV2::StorageDead {
            local: local.as_usize(),
        }),
        StatementKind::SetDiscriminant {
            place,
            variant_index,
        } => Ok(StatementKindV2::SetDiscriminant {
            place: capture_place(context, **place)?,
            variant: variant_index.index(),
        }),
        StatementKind::Intrinsic(intrinsic) => Ok(StatementKindV2::Intrinsic(Box::new(
            match intrinsic.as_ref() {
                NonDivergingIntrinsic::CopyNonOverlapping(copy) => {
                    IntrinsicStatementV2::CopyNonOverlapping {
                        source: Box::new(capture_operand(context, &copy.src, source_info)?),
                        destination: Box::new(capture_operand(context, &copy.dst, source_info)?),
                        count: Box::new(capture_operand(context, &copy.count, source_info)?),
                    }
                }
                NonDivergingIntrinsic::Assume(condition) => IntrinsicStatementV2::Assume {
                    condition: Box::new(capture_operand(context, condition, source_info)?),
                },
            },
        ))),
        StatementKind::Retag(kind, place) => Ok(StatementKindV2::Retag {
            place: capture_place(context, **place)?,
            kind: stable_compiler_value!(context, "retag kind", kind)?,
        }),
        StatementKind::PlaceMention(place) => Ok(StatementKindV2::PlaceMention {
            place: capture_place(context, **place)?,
        }),
        StatementKind::AscribeUserType(contents, variance) => Ok(StatementKindV2::Unsupported(
            UnsupportedStatementV2::AscribeUserType {
                place: capture_place(context, contents.0)?,
                projection: stable_compiler_value!(context, "user type projection", &contents.1,)?,
                variance: stable_compiler_value!(context, "user type variance", variance)?,
            },
        )),
        StatementKind::Coverage(coverage) => Ok(StatementKindV2::Coverage {
            kind: stable_compiler_value!(context, "coverage", coverage)?,
        }),
        StatementKind::ConstEvalCounter => Ok(StatementKindV2::Unsupported(
            UnsupportedStatementV2::ConstEvalCounter,
        )),
        StatementKind::Nop => Ok(StatementKindV2::Nop),
        StatementKind::BackwardIncompatibleDropHint { place, reason } => Ok(
            StatementKindV2::Unsupported(UnsupportedStatementV2::BackwardIncompatibleDropHint {
                place: capture_place(context, **place)?,
                reason: stable_compiler_value!(
                    context,
                    "backward-incompatible drop reason",
                    reason,
                )?,
            }),
        ),
    }
}

fn capture_rvalue<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    value: &Rvalue<'tcx>,
    source_info: SourceInfo,
) -> Result<RvalueV2, CaptureErrorV2> {
    match value {
        Rvalue::Use(operand) => Ok(RvalueV2::Use(capture_operand(
            context,
            operand,
            source_info,
        )?)),
        Rvalue::Repeat(operand, count) => {
            preflight_ty_const_v2("repeat count", *count, context.limits, &mut context.budget)?;
            let normalized = context
                .instance
                .try_instantiate_mir_and_normalize_erasing_regions(
                    context.tcx,
                    TypingEnv::fully_monomorphized(),
                    EarlyBinder::bind(*count),
                )
                .map_err(|_| CaptureErrorV2::normalization("repeat count"))?;
            preflight_ty_const_v2(
                "normalized repeat count",
                normalized,
                context.limits,
                &mut context.budget,
            )?;
            Ok(RvalueV2::Repeat {
                operand: capture_operand(context, operand, source_info)?,
                count: stable_compiler_value!(context, "repeat count", &normalized)?,
            })
        }
        Rvalue::Ref(_, borrow_kind, place) => Ok(RvalueV2::Reference {
            borrow_kind: stable_compiler_value!(context, "borrow kind", borrow_kind)?,
            place: capture_place(context, *place)?,
        }),
        Rvalue::RawPtr(kind, place) => Ok(RvalueV2::RawPointer {
            kind: stable_compiler_value!(context, "raw pointer kind", kind)?,
            place: capture_place(context, *place)?,
        }),
        Rvalue::Cast(kind, operand, target) => Ok(RvalueV2::Cast {
            kind: stable_compiler_value!(context, "cast kind", kind)?,
            operand: capture_operand(context, operand, source_info)?,
            target: Box::new(type_identity(context, *target)?),
        }),
        Rvalue::BinaryOp(operation, operands) => Ok(RvalueV2::Binary {
            operation: stable_compiler_value!(context, "binary operation", operation)?,
            lhs: capture_operand(context, &operands.0, source_info)?,
            rhs: Box::new(capture_operand(context, &operands.1, source_info)?),
        }),
        Rvalue::UnaryOp(operation, operand) => Ok(RvalueV2::Unary {
            operation: stable_compiler_value!(context, "unary operation", operation)?,
            operand: capture_operand(context, operand, source_info)?,
        }),
        Rvalue::Discriminant(place) => Ok(RvalueV2::Discriminant {
            place: capture_place(context, *place)?,
        }),
        Rvalue::Aggregate(kind, operands) => {
            ensure_bound(
                "aggregate operands",
                operands.len(),
                context.limits.max_operands,
            )?;
            let mut captured_operands = Vec::with_capacity(operands.len());
            for operand in operands {
                captured_operands.push(capture_operand(context, operand, source_info)?);
            }
            Ok(RvalueV2::Aggregate {
                kind: capture_aggregate_kind(context, kind)?,
                operands: captured_operands,
            })
        }
        Rvalue::CopyForDeref(place) => Ok(RvalueV2::CopyForDeref(capture_place(context, *place)?)),
        Rvalue::ThreadLocalRef(def_id) => Ok(RvalueV2::ThreadLocalRef {
            definition: definition_identity(context, *def_id)?,
        }),
        Rvalue::WrapUnsafeBinder(operand, target) => Ok(RvalueV2::WrapUnsafeBinder {
            operand: capture_operand(context, operand, source_info)?,
            target: type_identity(context, *target)?,
        }),
    }
}

fn capture_aggregate_kind<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    kind: &AggregateKind<'tcx>,
) -> Result<AggregateKindV2, CaptureErrorV2> {
    preflight_aggregate_kind(context, kind, "aggregate kind")?;
    let normalized = context
        .instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            context.tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(kind.clone()),
        )
        .map_err(|_| CaptureErrorV2::normalization("aggregate kind"))?;
    preflight_aggregate_kind(context, &normalized, "normalized aggregate kind")?;
    match normalized {
        AggregateKind::Array(element) => Ok(AggregateKindV2::Array {
            element: type_identity_normalized(context, element)?,
        }),
        AggregateKind::Tuple => Ok(AggregateKindV2::Tuple),
        AggregateKind::Adt(def_id, variant, args, user_type, active_field) => {
            Ok(AggregateKindV2::Adt {
                definition: definition_identity(context, def_id)?,
                generic_args_hash: generic_args_hash(context, "ADT aggregate arguments", args)?,
                variant: variant.index(),
                user_type_annotation: user_type.map(|index| index.index()),
                active_field: active_field.map(|field| field.index()),
            })
        }
        AggregateKind::Closure(def_id, args) => Ok(AggregateKindV2::Closure {
            definition: definition_identity(context, def_id)?,
            generic_args_hash: generic_args_hash(context, "closure aggregate arguments", args)?,
        }),
        AggregateKind::CoroutineClosure(def_id, args) => Ok(AggregateKindV2::CoroutineClosure {
            definition: definition_identity(context, def_id)?,
            generic_args_hash: generic_args_hash(
                context,
                "coroutine closure aggregate arguments",
                args,
            )?,
        }),
        AggregateKind::Coroutine(def_id, args) => Ok(AggregateKindV2::Coroutine {
            definition: definition_identity(context, def_id)?,
            generic_args_hash: generic_args_hash(context, "coroutine aggregate arguments", args)?,
        }),
        AggregateKind::RawPtr(pointee, mutability) => Ok(AggregateKindV2::RawPointer {
            pointee: type_identity_normalized(context, pointee)?,
            mutable: matches!(mutability, rustc_ast::Mutability::Mut),
        }),
    }
}

fn preflight_aggregate_kind<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    kind: &AggregateKind<'tcx>,
    label: &str,
) -> Result<(), CaptureErrorV2> {
    match kind {
        AggregateKind::Array(ty) | AggregateKind::RawPtr(ty, _) => {
            preflight_ty_v2(label, *ty, context.limits, &mut context.budget)?;
        }
        AggregateKind::Tuple => {}
        AggregateKind::Adt(_, _, args, _, _)
        | AggregateKind::Closure(_, args)
        | AggregateKind::CoroutineClosure(_, args)
        | AggregateKind::Coroutine(_, args) => {
            preflight_generic_args_v2(label, args, context.limits, &mut context.budget)?;
        }
    }
    Ok(())
}

fn generic_args_hash<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    label: &str,
    args: GenericArgsRef<'tcx>,
) -> Result<[u8; 16], CaptureErrorV2> {
    preflight_generic_args_v2(label, args, context.limits, &mut context.budget)?;
    Ok(stable_hash!(context.tcx, args))
}

fn preflight_instance_v2<'tcx>(
    label: &str,
    instance: Instance<'tcx>,
    limits: CaptureLimitsV2,
    budget: &mut CaptureBudgetV2,
) -> Result<(), CaptureErrorV2> {
    preflight_generic_args_v2(label, instance.args, limits, budget)?;
    match instance.def {
        InstanceKind::FnPtrShim(_, ty)
        | InstanceKind::CloneShim(_, ty)
        | InstanceKind::FnPtrAddrShim(_, ty)
        | InstanceKind::AsyncDropGlueCtorShim(_, ty)
        | InstanceKind::AsyncDropGlue(_, ty) => preflight_ty_v2(label, ty, limits, budget)?,
        InstanceKind::FutureDropPollShim(_, proxy, implementation) => {
            preflight_ty_v2(label, proxy, limits, budget)?;
            preflight_ty_v2(label, implementation, limits, budget)?;
        }
        InstanceKind::DropGlue(_, Some(ty)) => preflight_ty_v2(label, ty, limits, budget)?,
        InstanceKind::Item(_)
        | InstanceKind::Intrinsic(_)
        | InstanceKind::VTableShim(_)
        | InstanceKind::ReifyShim(..)
        | InstanceKind::Virtual(..)
        | InstanceKind::ClosureOnceShim { .. }
        | InstanceKind::ConstructCoroutineInClosureShim { .. }
        | InstanceKind::ThreadLocalShim(_)
        | InstanceKind::DropGlue(_, None) => {}
    }
    Ok(())
}

fn capture_operand<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    operand: &Operand<'tcx>,
    source_info: SourceInfo,
) -> Result<OperandV2, CaptureErrorV2> {
    match operand {
        Operand::Copy(place) => Ok(OperandV2::Copy(capture_place(context, *place)?)),
        Operand::Move(place) => Ok(OperandV2::Move(capture_place(context, *place)?)),
        Operand::Constant(constant) => {
            preflight_mir_const_v2(
                "constant",
                constant.const_,
                context.limits,
                &mut context.budget,
            )?;
            let normalized = context
                .instance
                .try_instantiate_mir_and_normalize_erasing_regions(
                    context.tcx,
                    TypingEnv::fully_monomorphized(),
                    EarlyBinder::bind(constant.const_),
                )
                .map_err(|_| CaptureErrorV2::normalization("constant"))?;
            preflight_mir_const_v2(
                "normalized constant",
                normalized,
                context.limits,
                &mut context.budget,
            )?;
            Ok(OperandV2::Constant {
                ty: Box::new(type_identity_normalized(context, normalized.ty())?),
                value: stable_compiler_value!(context, "constant", &normalized)?,
                source: Box::new(span_identity(
                    context,
                    constant.span,
                    source_info.scope.as_usize(),
                )?),
            })
        }
        Operand::RuntimeChecks(kind) => Ok(OperandV2::RuntimeChecks {
            kind: stable_compiler_value!(context, "runtime checks", kind)?,
        }),
    }
}

fn capture_place<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    place: Place<'tcx>,
) -> Result<PlaceV2, CaptureErrorV2> {
    ensure_bound(
        "place projection",
        place.projection.len(),
        context.limits.max_projection_depth,
    )?;
    let mut projection = Vec::with_capacity(place.projection.len());
    for element in place.projection {
        projection.push(capture_projection(context, element)?);
    }
    Ok(PlaceV2 {
        local: place.local.as_usize(),
        projection,
    })
}

fn capture_projection<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    element: ProjectionElem<rustc_middle::mir::Local, Ty<'tcx>>,
) -> Result<ProjectionV2, CaptureErrorV2> {
    match element {
        ProjectionElem::Deref => Ok(ProjectionV2::Deref),
        ProjectionElem::Field(field, ty) => Ok(ProjectionV2::Field {
            index: field.index(),
            ty: type_identity(context, ty)?,
        }),
        ProjectionElem::Index(local) => Ok(ProjectionV2::Index {
            local: local.as_usize(),
        }),
        ProjectionElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        } => Ok(ProjectionV2::ConstantIndex {
            offset,
            min_length,
            from_end,
        }),
        ProjectionElem::Subslice { from, to, from_end } => {
            Ok(ProjectionV2::Subslice { from, to, from_end })
        }
        ProjectionElem::Downcast(name, variant) => Ok(ProjectionV2::Downcast {
            variant: variant.index(),
            name: name
                .map(|name| context.budget.bounded_str("downcast name", name.as_str()))
                .transpose()?,
        }),
        ProjectionElem::OpaqueCast(ty) => Ok(ProjectionV2::OpaqueCast {
            ty: type_identity(context, ty)?,
        }),
        ProjectionElem::UnwrapUnsafeBinder(ty) => Ok(ProjectionV2::UnwrapUnsafeBinder {
            ty: type_identity(context, ty)?,
        }),
    }
}

fn capture_terminator<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    kind: &TerminatorKind<'tcx>,
    source_info: SourceInfo,
) -> Result<TerminatorKindV2, CaptureErrorV2> {
    match kind {
        TerminatorKind::Return => Ok(TerminatorKindV2::Return),
        TerminatorKind::Unreachable => Ok(TerminatorKindV2::Unreachable),
        TerminatorKind::Goto { target } => Ok(TerminatorKindV2::Goto {
            target: target.as_usize(),
        }),
        TerminatorKind::SwitchInt { discr, targets } => {
            let target_count = targets.iter().count();
            ensure_bound(
                "switch targets",
                target_count,
                context.limits.max_switch_targets,
            )?;
            let mut captured_targets = Vec::with_capacity(target_count);
            captured_targets.extend(targets.iter().map(|(value, target)| SwitchTargetV2 {
                value,
                target: target.as_usize(),
            }));
            Ok(TerminatorKindV2::SwitchInt {
                discriminant: capture_operand(context, discr, source_info)?,
                targets: captured_targets,
                otherwise: targets.otherwise().as_usize(),
            })
        }
        TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            unwind,
            call_source,
            fn_span,
        } => {
            ensure_bound("call arguments", args.len(), context.limits.max_operands)?;
            let callee = capture_callee_identity(context, func)?;
            let mut arguments = Vec::with_capacity(args.len());
            for argument in args {
                arguments.push(CallArgumentV2 {
                    operand: capture_operand(context, &argument.node, source_info)?,
                    source: span_identity(context, argument.span, source_info.scope.as_usize())?,
                });
            }
            Ok(TerminatorKindV2::Call {
                function: capture_operand(context, func, source_info)?,
                callee,
                arguments,
                destination: capture_place(context, *destination)?,
                target: target.map(|target| target.as_usize()),
                unwind: capture_unwind(context, unwind)?,
                call_source: stable_compiler_value!(context, "call source", call_source)?,
                function_span: span_identity(context, *fn_span, source_info.scope.as_usize())?,
            })
        }
        TerminatorKind::TailCall {
            func,
            args,
            fn_span,
        } => {
            ensure_bound(
                "tail-call arguments",
                args.len(),
                context.limits.max_operands,
            )?;
            let callee = capture_callee_identity(context, func)?;
            let mut arguments = Vec::with_capacity(args.len());
            for argument in args {
                arguments.push(CallArgumentV2 {
                    operand: capture_operand(context, &argument.node, source_info)?,
                    source: span_identity(context, argument.span, source_info.scope.as_usize())?,
                });
            }
            Ok(TerminatorKindV2::TailCall {
                function: capture_operand(context, func, source_info)?,
                callee,
                arguments,
                function_span: span_identity(context, *fn_span, source_info.scope.as_usize())?,
            })
        }
        TerminatorKind::Drop {
            place,
            target,
            unwind,
            replace,
            drop,
            async_fut,
        } => Ok(TerminatorKindV2::Drop {
            place: capture_place(context, *place)?,
            target: target.as_usize(),
            unwind: capture_unwind(context, unwind)?,
            replace: *replace,
            async_drop: drop.map(|target| target.as_usize()),
            async_future_local: async_fut.map(|local| local.as_usize()),
        }),
        TerminatorKind::Assert {
            cond,
            expected,
            msg,
            target,
            unwind,
        } => Ok(TerminatorKindV2::Assert {
            condition: capture_operand(context, cond, source_info)?,
            expected: *expected,
            target: target.as_usize(),
            message: stable_compiler_value!(context, "assert message", msg)?,
            unwind: capture_unwind(context, unwind)?,
        }),
        TerminatorKind::UnwindResume => Ok(TerminatorKindV2::UnwindResume),
        TerminatorKind::UnwindTerminate(reason) => Ok(TerminatorKindV2::UnwindTerminate {
            reason: stable_compiler_value!(context, "unwind termination", reason)?,
        }),
        TerminatorKind::Yield {
            value,
            resume,
            resume_arg,
            drop,
        } => Ok(TerminatorKindV2::Yield {
            value: capture_operand(context, value, source_info)?,
            resume: resume.as_usize(),
            resume_argument: capture_place(context, *resume_arg)?,
            drop: drop.map(|target| target.as_usize()),
        }),
        TerminatorKind::CoroutineDrop => Ok(TerminatorKindV2::CoroutineDrop),
        TerminatorKind::FalseEdge {
            real_target,
            imaginary_target,
        } => Ok(TerminatorKindV2::FalseEdge {
            real_target: real_target.as_usize(),
            imaginary_target: imaginary_target.as_usize(),
        }),
        TerminatorKind::FalseUnwind {
            real_target,
            unwind,
        } => Ok(TerminatorKindV2::FalseUnwind {
            real_target: real_target.as_usize(),
            unwind: capture_unwind(context, unwind)?,
        }),
        TerminatorKind::InlineAsm {
            template,
            operands,
            line_spans,
            targets,
            ..
        } => Ok(TerminatorKindV2::Unsupported(
            UnsupportedTerminatorV2::InlineAssembly {
                template_pieces: template.len(),
                operands: operands.len(),
                line_spans: line_spans.len(),
                targets: targets.len(),
            },
        )),
    }
}

fn capture_unwind<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    unwind: &UnwindAction,
) -> Result<UnwindActionV2, CaptureErrorV2> {
    match unwind {
        UnwindAction::Continue => Ok(UnwindActionV2::Continue),
        UnwindAction::Unreachable => Ok(UnwindActionV2::Unreachable),
        UnwindAction::Terminate(reason) => Ok(UnwindActionV2::Terminate {
            reason: stable_compiler_value!(context, "unwind action", reason)?,
        }),
        UnwindAction::Cleanup(target) => Ok(UnwindActionV2::Cleanup {
            target: target.as_usize(),
        }),
    }
}

fn capture_callee_identity<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    operand: &Operand<'tcx>,
) -> Result<CalleeIdentityV2, CaptureErrorV2> {
    let raw_callable_ty = operand.ty(context.body, context.tcx);
    preflight_ty_v2(
        "callable type",
        raw_callable_ty,
        context.limits,
        &mut context.budget,
    )?;
    let callable_ty = context
        .instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            context.tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(raw_callable_ty),
        )
        .map_err(|_| CaptureErrorV2::normalization("callable type"))?;
    preflight_ty_v2(
        "normalized callable type",
        callable_ty,
        context.limits,
        &mut context.budget,
    )?;
    if let TyKind::FnDef(def_id, args) = callable_ty.kind() {
        let resolved = require_direct_resolution(Instance::try_resolve(
            context.tcx,
            TypingEnv::fully_monomorphized(),
            *def_id,
            args,
        ))?;
        let declared = definition_identity(context, *def_id)?;
        let declared_generic_args_hash =
            generic_args_hash(context, "declared callee arguments", args)?;
        let declared_generic_arg_count = args.len();
        let declared_signature =
            function_signature_identity(context, *def_id, args, "declared callee signature")?;
        let resolved_signature = resolved_signature_identity(context, resolved)?;
        let resolved_identity = function_identity(context, resolved)?;
        let intrinsic = match resolved.def {
            InstanceKind::Intrinsic(def_id) => {
                let metadata = context.tcx.intrinsic(def_id).ok_or_else(|| {
                    CaptureErrorV2::new(
                        "resolved intrinsic instance has no compiler intrinsic metadata",
                    )
                })?;
                let mut captured = IntrinsicIdentityV2 {
                    definition: definition_identity(context, def_id)?,
                    name: context
                        .budget
                        .bounded_str("intrinsic name", metadata.name.as_str())?,
                    must_be_overridden: metadata.must_be_overridden,
                    const_stable: metadata.const_stable,
                    binding_hash: [0; 32],
                };
                captured.binding_hash = intrinsic_binding_hash_v2(&captured)?;
                Some(captured)
            }
            _ => None,
        };
        let callable_type = type_identity_normalized(context, callable_ty)?;
        let resolution_binding_hash = resolution_binding_hash_v2(
            &callable_type,
            &declared,
            &declared_generic_args_hash,
            declared_generic_arg_count,
            &declared_signature,
            &resolved_identity,
            &resolved_signature,
            intrinsic.as_ref(),
        )?;
        return Ok(CalleeIdentityV2::Direct {
            declared,
            declared_generic_args_hash,
            declared_generic_arg_count,
            declared_signature,
            resolved: Box::new(resolved_identity),
            resolved_signature,
            intrinsic,
            resolution_binding_hash,
        });
    }
    if !matches!(callable_ty.kind(), TyKind::FnPtr(..)) {
        return Err(CaptureErrorV2::new(
            "a non-FnDef call operand was not a legitimate function pointer",
        ));
    }
    Ok(CalleeIdentityV2::Indirect {
        callable_type: type_identity_normalized(context, callable_ty)?,
    })
}

fn function_signature_identity<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    def_id: DefId,
    args: GenericArgsRef<'tcx>,
    label: &str,
) -> Result<FunctionSignatureIdentityV2, CaptureErrorV2> {
    preflight_generic_args_v2(label, args, context.limits, &mut context.budget)?;
    if !matches!(context.tcx.def_kind(def_id), DefKind::Fn | DefKind::AssocFn) {
        return Err(CaptureErrorV2::new(format!(
            "{label} does not refer to a function-signature DefId"
        )));
    }
    let raw = context.tcx.fn_sig(def_id);
    preflight_signature_types(
        context,
        raw.skip_binder().inputs_and_output().skip_binder(),
        label,
    )?;
    let normalized = context
        .tcx
        .try_instantiate_and_normalize_erasing_regions(args, TypingEnv::fully_monomorphized(), raw)
        .map_err(|_| CaptureErrorV2::normalization(label))?;
    let signature = context
        .tcx
        .instantiate_bound_regions_with_erased(normalized);
    preflight_signature_types(context, signature.inputs_and_output, label)?;
    let signature_types = signature.inputs_and_output.iter().collect::<Vec<_>>();
    Ok(FunctionSignatureIdentityV2 {
        stable_hash: stable_hash!(context.tcx, signature),
        shape_hash: stable_hash!(context.tcx, signature_types),
        input_count: signature.inputs().len(),
    })
}

fn resolved_signature_identity<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    instance: Instance<'tcx>,
) -> Result<FunctionSignatureIdentityV2, CaptureErrorV2> {
    if matches!(
        context.tcx.def_kind(instance.def_id()),
        DefKind::Fn | DefKind::AssocFn
    ) {
        return function_signature_identity(
            context,
            instance.def_id(),
            instance.args,
            "resolved callee signature",
        );
    }
    preflight_instance_v2(
        "generated resolved signature",
        instance,
        context.limits,
        &mut context.budget,
    )?;
    let body = context.tcx.instance_mir(instance.def);
    let interface_count = body
        .arg_count
        .checked_add(1)
        .ok_or_else(|| CaptureErrorV2::new("generated signature arity overflowed"))?;
    ensure_bound(
        "generated signature type arity",
        interface_count,
        context.limits.max_type_arity,
    )?;
    if interface_count > body.local_decls.len() {
        return Err(CaptureErrorV2::new(
            "generated signature does not fit in its MIR local table",
        ));
    }
    let mut types = Vec::with_capacity(interface_count);
    for index in (1..=body.arg_count).chain(std::iter::once(0)) {
        let raw = body.local_decls[Local::from_usize(index)].ty;
        preflight_ty_v2(
            "generated signature type",
            raw,
            context.limits,
            &mut context.budget,
        )?;
        let normalized = instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                context.tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(raw),
            )
            .map_err(|_| CaptureErrorV2::normalization("generated signature type"))?;
        preflight_ty_v2(
            "normalized generated signature type",
            normalized,
            context.limits,
            &mut context.budget,
        )?;
        types.push(normalized);
    }
    Ok(FunctionSignatureIdentityV2 {
        stable_hash: stable_hash!(context.tcx, instance),
        shape_hash: stable_hash!(context.tcx, types),
        input_count: body.arg_count,
    })
}

fn preflight_signature_types<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    types: &[Ty<'tcx>],
    label: &str,
) -> Result<(), CaptureErrorV2> {
    ensure_bound(
        "signature type arity",
        types.len(),
        context.limits.max_type_arity,
    )?;
    for ty in types {
        preflight_ty_v2(label, *ty, context.limits, &mut context.budget)?;
    }
    Ok(())
}

fn require_direct_resolution<T, E>(result: Result<Option<T>, E>) -> Result<T, CaptureErrorV2> {
    match result {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err(CaptureErrorV2::new(
            "monomorphic direct-call resolution returned no concrete instance",
        )),
        Err(_) => Err(CaptureErrorV2::new(
            "direct-call instance resolution reported a compiler error",
        )),
    }
}

fn function_identity<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    instance: Instance<'tcx>,
) -> Result<FunctionIdentityV2, CaptureErrorV2> {
    preflight_instance_v2(
        "function instance",
        instance,
        context.limits,
        &mut context.budget,
    )?;
    let definition = definition_identity(context, instance.def_id())?;
    let kind = match instance.def {
        InstanceKind::Item(_) => InstanceKindV2::Item,
        InstanceKind::Intrinsic(_) => InstanceKindV2::Intrinsic,
        InstanceKind::VTableShim(_) => InstanceKindV2::VTableShim,
        InstanceKind::ReifyShim(_, reason) => InstanceKindV2::ReifyShim {
            reason: reason.map(|reason| match reason {
                ReifyReason::FnPtr => ReifyReasonV2::FunctionPointer,
                ReifyReason::Vtable => ReifyReasonV2::Vtable,
            }),
        },
        InstanceKind::FnPtrShim(_, ty) => InstanceKindV2::FnPtrShim {
            fn_pointer: Box::new(type_identity(context, ty)?),
        },
        InstanceKind::Virtual(_, vtable_index) => InstanceKindV2::Virtual { vtable_index },
        InstanceKind::ClosureOnceShim { track_caller, .. } => {
            InstanceKindV2::ClosureOnceShim { track_caller }
        }
        InstanceKind::ConstructCoroutineInClosureShim {
            coroutine_closure_def_id,
            receiver_by_ref,
        } => InstanceKindV2::ConstructCoroutineInClosureShim {
            coroutine_closure: definition_identity(context, coroutine_closure_def_id)?,
            receiver_by_ref,
        },
        InstanceKind::ThreadLocalShim(_) => InstanceKindV2::ThreadLocalShim,
        InstanceKind::FutureDropPollShim(_, proxy, implementation) => {
            InstanceKindV2::FutureDropPollShim {
                proxy_coroutine: Box::new(type_identity(context, proxy)?),
                implementation_coroutine: Box::new(type_identity(context, implementation)?),
            }
        }
        InstanceKind::DropGlue(_, ty) => InstanceKindV2::DropGlue {
            ty: ty
                .map(|ty| type_identity(context, ty).map(Box::new))
                .transpose()?,
        },
        InstanceKind::CloneShim(_, ty) => InstanceKindV2::CloneShim {
            ty: Box::new(type_identity(context, ty)?),
        },
        InstanceKind::FnPtrAddrShim(_, ty) => InstanceKindV2::FnPtrAddrShim {
            ty: Box::new(type_identity(context, ty)?),
        },
        InstanceKind::AsyncDropGlueCtorShim(_, ty) => InstanceKindV2::AsyncDropGlueCtorShim {
            ty: Box::new(type_identity(context, ty)?),
        },
        InstanceKind::AsyncDropGlue(_, ty) => InstanceKindV2::AsyncDropGlue {
            ty: Box::new(type_identity(context, ty)?),
        },
    };
    Ok(FunctionIdentityV2 {
        definition,
        instance: InstanceIdentityV2 {
            kind,
            generic_args_hash: generic_args_hash(
                context,
                "function instance arguments",
                instance.args,
            )?,
            generic_arg_count: instance.args.len(),
            instance_hash: stable_hash!(context.tcx, instance),
            diagnostic_generic_args: context
                .budget
                .bounded_debug("instance generic arguments", &instance.args)?,
            diagnostic_debug: context
                .budget
                .bounded_debug("instance kind", &instance.def)?,
        },
    })
}

fn definition_identity<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    def_id: DefId,
) -> Result<DefinitionIdentityV2, CaptureErrorV2> {
    let hash = context.tcx.def_path_hash(def_id);
    let def_path_hash = hash.0.to_le_bytes();
    let stable_crate_id = stable_hash!(context.tcx, hash.stable_crate_id());
    let local_def_path_hash = hash.local_hash().as_u64().to_le_bytes();
    Ok(DefinitionIdentityV2 {
        diagnostic_crate_name: context
            .budget
            .bounded_str("crate name", context.tcx.crate_name(def_id.krate).as_str())?,
        diagnostic_def_path: context
            .budget
            .bounded_debug("definition path", &context.tcx.def_path(def_id))?,
        def_path_hash,
        stable_crate_id,
        local_def_path_hash,
    })
}

fn type_identity<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    ty: Ty<'tcx>,
) -> Result<TypeIdentityV2, CaptureErrorV2> {
    preflight_ty_v2(
        "type before normalization",
        ty,
        context.limits,
        &mut context.budget,
    )?;
    let normalized = context
        .instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            context.tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(ty),
        )
        .map_err(|_| CaptureErrorV2::normalization("type"))?;
    type_identity_normalized(context, normalized)
}

fn type_identity_normalized<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    ty: Ty<'tcx>,
) -> Result<TypeIdentityV2, CaptureErrorV2> {
    preflight_ty_v2("normalized type", ty, context.limits, &mut context.budget)?;
    let class = match ty.kind() {
        TyKind::Bool => TypeClassV2::Bool,
        TyKind::Char => TypeClassV2::Char,
        TyKind::Int(width) => TypeClassV2::SignedInteger(integer_width_signed(*width)),
        TyKind::Uint(width) => TypeClassV2::UnsignedInteger(integer_width_unsigned(*width)),
        TyKind::Float(width) => TypeClassV2::Float(float_width(*width)),
        TyKind::Adt(definition, args) => TypeClassV2::Adt {
            definition: definition_identity(context, definition.did())?,
            generic_args_hash: generic_args_hash(context, "ADT type arguments", args)?,
        },
        TyKind::Foreign(def_id) => TypeClassV2::Foreign {
            definition: definition_identity(context, *def_id)?,
        },
        TyKind::Str => TypeClassV2::StringSlice,
        TyKind::Array(..) => TypeClassV2::Array,
        TyKind::Pat(..) => TypeClassV2::Pattern,
        TyKind::Slice(_) => TypeClassV2::Slice,
        TyKind::RawPtr(_, mutability) => TypeClassV2::RawPointer {
            mutable: matches!(mutability, rustc_ast::Mutability::Mut),
        },
        TyKind::Ref(_, _, mutability) => TypeClassV2::Reference {
            mutable: matches!(mutability, rustc_ast::Mutability::Mut),
        },
        TyKind::FnDef(def_id, args) => TypeClassV2::FunctionDefinition {
            definition: definition_identity(context, *def_id)?,
            generic_args_hash: generic_args_hash(context, "function type arguments", args)?,
            generic_arg_count: args.len(),
        },
        TyKind::FnPtr(..) => TypeClassV2::FunctionPointer,
        TyKind::UnsafeBinder(_) => TypeClassV2::UnsafeBinder,
        TyKind::Dynamic(..) => TypeClassV2::Dynamic,
        TyKind::Closure(def_id, args) => TypeClassV2::Closure {
            definition: definition_identity(context, *def_id)?,
            generic_args_hash: generic_args_hash(context, "closure type arguments", args)?,
        },
        TyKind::CoroutineClosure(def_id, args) => TypeClassV2::CoroutineClosure {
            definition: definition_identity(context, *def_id)?,
            generic_args_hash: generic_args_hash(
                context,
                "coroutine closure type arguments",
                args,
            )?,
        },
        TyKind::Coroutine(def_id, args) => TypeClassV2::Coroutine {
            definition: definition_identity(context, *def_id)?,
            generic_args_hash: generic_args_hash(context, "coroutine type arguments", args)?,
        },
        TyKind::CoroutineWitness(def_id, args) => TypeClassV2::CoroutineWitness {
            definition: definition_identity(context, *def_id)?,
            generic_args_hash: generic_args_hash(
                context,
                "coroutine witness type arguments",
                args,
            )?,
        },
        TyKind::Never => TypeClassV2::Never,
        TyKind::Tuple(types) => TypeClassV2::Tuple { arity: types.len() },
        TyKind::Alias(..) => TypeClassV2::Unsupported(UnresolvedTypeClassV2::Alias),
        TyKind::Param(_) => TypeClassV2::Unsupported(UnresolvedTypeClassV2::Parameter),
        TyKind::Bound(..) => TypeClassV2::Unsupported(UnresolvedTypeClassV2::Bound),
        TyKind::Placeholder(_) => TypeClassV2::Unsupported(UnresolvedTypeClassV2::Placeholder),
        TyKind::Infer(_) => TypeClassV2::Unsupported(UnresolvedTypeClassV2::Inference),
        TyKind::Error(_) => TypeClassV2::Unsupported(UnresolvedTypeClassV2::Error),
    };
    Ok(TypeIdentityV2 {
        stable_hash: stable_hash!(context.tcx, ty),
        class,
        diagnostic_display: context.budget.bounded_display("type", &ty)?,
        diagnostic_debug: context.budget.bounded_debug("type kind", &ty.kind())?,
    })
}

fn integer_width_signed(width: IntTy) -> IntegerWidthV2 {
    match width {
        IntTy::Isize => IntegerWidthV2::Pointer,
        IntTy::I8 => IntegerWidthV2::Bits8,
        IntTy::I16 => IntegerWidthV2::Bits16,
        IntTy::I32 => IntegerWidthV2::Bits32,
        IntTy::I64 => IntegerWidthV2::Bits64,
        IntTy::I128 => IntegerWidthV2::Bits128,
    }
}

fn integer_width_unsigned(width: UintTy) -> IntegerWidthV2 {
    match width {
        UintTy::Usize => IntegerWidthV2::Pointer,
        UintTy::U8 => IntegerWidthV2::Bits8,
        UintTy::U16 => IntegerWidthV2::Bits16,
        UintTy::U32 => IntegerWidthV2::Bits32,
        UintTy::U64 => IntegerWidthV2::Bits64,
        UintTy::U128 => IntegerWidthV2::Bits128,
    }
}

fn float_width(width: FloatTy) -> FloatWidthV2 {
    match width {
        FloatTy::F16 => FloatWidthV2::Bits16,
        FloatTy::F32 => FloatWidthV2::Bits32,
        FloatTy::F64 => FloatWidthV2::Bits64,
        FloatTy::F128 => FloatWidthV2::Bits128,
    }
}

fn source_span<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    source_info: SourceInfo,
) -> Result<SourceSpanV2, CaptureErrorV2> {
    span_identity(context, source_info.span, source_info.scope.as_usize())
}

fn span_identity<'tcx>(
    context: &mut CaptureContextV2<'_, 'tcx>,
    span: Span,
    source_scope: usize,
) -> Result<SourceSpanV2, CaptureErrorV2> {
    let canonical = span.source_callsite();
    let source_map = context.tcx.sess.source_map();
    let start = source_map.lookup_char_pos(canonical.lo());
    let end = source_map.lookup_char_pos(canonical.hi());
    let same_file = start.file.stable_id == end.file.stable_id;
    let valid_position =
        start.line > 0 && end.line > 0 && (start.line, start.col.0) <= (end.line, end.col.0);
    let authority = if span.is_dummy() || canonical.is_dummy() {
        SourceAuthorityV2::Unauthoritative(SourceRejectionV2::DummySpan)
    } else if !same_file {
        SourceAuthorityV2::Unauthoritative(SourceRejectionV2::CrossFileSpan)
    } else if !valid_position {
        SourceAuthorityV2::Unauthoritative(SourceRejectionV2::InvalidPosition)
    } else {
        SourceAuthorityV2::CanonicalRemapped
    };

    let (source_scope_hash, source_scope_parent, inlined_instance_hash) = if let Some(scope_data) =
        context
            .body
            .source_scopes
            .get(SourceScope::from_usize(source_scope))
    {
        if let Some((instance, _)) = scope_data.inlined {
            preflight_instance_v2(
                "inlined source-scope instance",
                instance,
                context.limits,
                &mut context.budget,
            )?;
        }
        (
            stable_hash!(context.tcx, scope_data),
            scope_data.parent_scope.map(|scope| scope.as_usize()),
            scope_data
                .inlined
                .map(|(instance, _)| stable_hash!(context.tcx, instance)),
        )
    } else {
        return Err(CaptureErrorV2::new(
            "source scope index is outside the MIR scope table",
        ));
    };
    Ok(SourceSpanV2 {
        authority,
        remapped_file: context.budget.bounded_display(
            "remapped source file",
            &start.file.name.prefer_remapped_unconditionally(),
        )?,
        source_file_hash: stable_hash!(context.tcx, start.file.stable_id),
        span_hash: stable_hash!(context.tcx, canonical),
        start_line: start.line,
        start_column: start.col.0 + 1,
        end_line: end.line,
        end_column: end.col.0 + 1,
        source_scope,
        source_scope_hash,
        source_scope_parent,
        inlined_instance_hash,
        diagnostic_debug: context.budget.bounded_debug("source span", &span)?,
    })
}

fn ensure_bound(label: &str, actual: usize, limit: usize) -> Result<(), CaptureErrorV2> {
    if actual > limit {
        return Err(CaptureErrorV2::new(format!(
            "{label} bound exceeded: {actual} > {limit}"
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CaptureErrorV2 {
    reason: String,
}

impl CaptureErrorV2 {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    fn normalization(subject: &str) -> Self {
        Self::new(format!(
            "fallible MIR instantiation/normalization failed for {subject}"
        ))
    }
}

impl fmt::Display for CaptureErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "rustc MIR V2 capture failed: {}", self.reason)
    }
}

impl Error for CaptureErrorV2 {}

impl From<ValidationErrorV2> for CaptureErrorV2 {
    fn from(error: ValidationErrorV2) -> Self {
        Self::new(error.to_string())
    }
}

impl From<BudgetErrorV2> for CaptureErrorV2 {
    fn from(error: BudgetErrorV2) -> Self {
        Self::new(error.to_string())
    }
}

impl From<PreflightErrorV2> for CaptureErrorV2 {
    fn from(error: PreflightErrorV2) -> Self {
        Self::new(error.to_string())
    }
}

impl From<TypePreflightErrorV2> for CaptureErrorV2 {
    fn from(error: TypePreflightErrorV2) -> Self {
        Self::new(error.to_string())
    }
}

#[cfg(test)]
mod resolution_tests {
    use super::*;

    #[test]
    fn resolution_errors_and_monomorphic_none_are_distinct_fail_closed_results() {
        let compiler_error = require_direct_resolution::<u8, _>(Err("compiler error"))
            .unwrap_err()
            .to_string();
        let unavailable = require_direct_resolution::<u8, ()>(Ok(None))
            .unwrap_err()
            .to_string();
        assert!(compiler_error.contains("reported a compiler error"));
        assert!(unavailable.contains("no concrete instance"));
        assert_ne!(compiler_error, unavailable);
    }
}
