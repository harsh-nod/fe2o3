use super::normalized::*;
use rustc_hir::def_id::DefId;
use rustc_middle::mir::{
    AggregateKind, Body, NonDivergingIntrinsic, Operand, Place, ProjectionElem, Rvalue, SourceInfo,
    StatementKind, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv};
use rustc_span::Span;
use std::error::Error;
use std::fmt;

pub(crate) fn capture_instance_body_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    limits: CaptureLimitsV2,
) -> Result<CapturedBodyV2, CaptureErrorV2> {
    let def_id = instance.def_id();
    if !tcx.is_mir_available(def_id) {
        return Err(CaptureErrorV2::new(format!(
            "MIR is unavailable for `{}`",
            tcx.def_path_str(def_id)
        )));
    }
    let body = tcx.instance_mir(instance.def);
    capture_body_v2(tcx, instance, body, limits)
}

fn capture_body_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    limits: CaptureLimitsV2,
) -> Result<CapturedBodyV2, CaptureErrorV2> {
    ensure_bound("locals", body.local_decls.len(), limits.max_locals)?;
    ensure_bound("blocks", body.basic_blocks.len(), limits.max_blocks)?;

    let locals = body
        .local_decls
        .iter_enumerated()
        .map(|(local, declaration)| {
            Ok(LocalDeclV2 {
                index: local.as_usize(),
                role: if local.as_usize() == 0 {
                    LocalRoleV2::Return
                } else if local.as_usize() <= body.arg_count {
                    LocalRoleV2::Argument
                } else {
                    LocalRoleV2::Temporary
                },
                ty: type_identity(tcx, instance, declaration.ty),
                mutable: matches!(declaration.mutability, rustc_ast::Mutability::Mut),
                source: source_span(tcx, declaration.source_info),
                rustc_debug: bounded_debug("local declaration", declaration, limits)?,
            })
        })
        .collect::<Result<Vec<_>, CaptureErrorV2>>()?;

    let mut total_statements = 0usize;
    let mut blocks = Vec::with_capacity(body.basic_blocks.len());
    for (block_index, block) in body.basic_blocks.iter_enumerated() {
        ensure_bound(
            format!("bb{}.statements", block_index.as_usize()),
            block.statements.len(),
            limits.max_statements_per_block,
        )?;
        total_statements = total_statements
            .checked_add(block.statements.len())
            .ok_or_else(|| CaptureErrorV2::new("statement count overflowed"))?;
        ensure_bound("statements", total_statements, limits.max_total_statements)?;

        let statements = block
            .statements
            .iter()
            .enumerate()
            .map(|(index, statement)| {
                Ok(StatementV2 {
                    index,
                    source: source_span(tcx, statement.source_info),
                    rustc_debug: bounded_debug("statement", &statement.kind, limits)?,
                    kind: capture_statement(
                        tcx,
                        instance,
                        &statement.kind,
                        statement.source_info,
                        limits,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, CaptureErrorV2>>()?;
        let terminator = block.terminator.as_ref().ok_or_else(|| {
            CaptureErrorV2::new(format!("bb{} has no terminator", block_index.as_usize()))
        })?;
        let successors = terminator
            .successors()
            .map(|successor| successor.as_usize())
            .collect::<Vec<_>>();
        ensure_bound(
            format!("bb{}.successors", block_index.as_usize()),
            successors.len(),
            limits.max_successors,
        )?;
        blocks.push(BasicBlockV2 {
            index: block_index.as_usize(),
            cleanup: block.is_cleanup,
            statements,
            terminator: TerminatorV2 {
                source: source_span(tcx, terminator.source_info),
                rustc_debug: bounded_debug("terminator", &terminator.kind, limits)?,
                kind: capture_terminator(
                    tcx,
                    instance,
                    &terminator.kind,
                    terminator.source_info,
                    limits,
                )?,
                successors,
            },
        });
    }

    let captured = CapturedBodyV2 {
        schema_version: NORMALIZED_MIR_SCHEMA_V2,
        authority: CaptureAuthorityV2::CompilerObservationOnly,
        function: function_identity(tcx, instance),
        source: span_identity(tcx, body.span, 0),
        arg_count: body.arg_count,
        locals,
        blocks,
    };
    captured.validate(limits)?;
    Ok(captured)
}

fn capture_statement<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    kind: &StatementKind<'tcx>,
    source_info: SourceInfo,
    limits: CaptureLimitsV2,
) -> Result<StatementKindV2, CaptureErrorV2> {
    let captured = match kind {
        StatementKind::Assign(assignment) => {
            let (destination, value) = &**assignment;
            StatementKindV2::Assign {
                destination: capture_place(tcx, instance, *destination, limits)?,
                value: capture_rvalue(tcx, instance, value, source_info, limits)?,
            }
        }
        StatementKind::StorageLive(local) => StatementKindV2::StorageLive {
            local: local.as_usize(),
        },
        StatementKind::StorageDead(local) => StatementKindV2::StorageDead {
            local: local.as_usize(),
        },
        StatementKind::SetDiscriminant {
            place,
            variant_index,
        } => StatementKindV2::SetDiscriminant {
            place: capture_place(tcx, instance, **place, limits)?,
            variant: variant_index.index(),
        },
        StatementKind::Intrinsic(intrinsic) => {
            StatementKindV2::Intrinsic(match intrinsic.as_ref() {
                NonDivergingIntrinsic::CopyNonOverlapping(copy) => {
                    IntrinsicStatementV2::CopyNonOverlapping {
                        source: capture_operand(tcx, instance, &copy.src, source_info, limits)?,
                        destination: capture_operand(
                            tcx,
                            instance,
                            &copy.dst,
                            source_info,
                            limits,
                        )?,
                        count: capture_operand(tcx, instance, &copy.count, source_info, limits)?,
                    }
                }
                NonDivergingIntrinsic::Assume(condition) => IntrinsicStatementV2::Assume {
                    condition: capture_operand(tcx, instance, condition, source_info, limits)?,
                },
            })
        }
        StatementKind::Retag(kind, place) => StatementKindV2::Retag {
            place: capture_place(tcx, instance, **place, limits)?,
            rustc_kind: bounded_debug("retag kind", kind, limits)?,
        },
        StatementKind::PlaceMention(place) => StatementKindV2::PlaceMention {
            place: capture_place(tcx, instance, **place, limits)?,
        },
        StatementKind::Coverage(coverage) => StatementKindV2::Coverage {
            rustc_kind: bounded_debug("coverage", coverage, limits)?,
        },
        StatementKind::Nop => StatementKindV2::Nop,
        other => StatementKindV2::CompilerOpaque {
            rustc_kind: bounded_debug("statement", other, limits)?,
        },
    };
    Ok(captured)
}

fn capture_rvalue<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    value: &Rvalue<'tcx>,
    source_info: SourceInfo,
    limits: CaptureLimitsV2,
) -> Result<RvalueV2, CaptureErrorV2> {
    let captured = match value {
        Rvalue::Use(operand) => RvalueV2::Use(capture_operand(
            tcx,
            instance,
            operand,
            source_info,
            limits,
        )?),
        Rvalue::Repeat(operand, count) => RvalueV2::Repeat {
            operand: capture_operand(tcx, instance, operand, source_info, limits)?,
            count: bounded_debug(
                "repeat count",
                &tcx.instantiate_and_normalize_erasing_regions(
                    instance.args,
                    TypingEnv::fully_monomorphized(),
                    EarlyBinder::bind(*count),
                ),
                limits,
            )?,
        },
        Rvalue::Ref(_, borrow_kind, place) => RvalueV2::Reference {
            borrow_kind: bounded_debug("borrow kind", borrow_kind, limits)?,
            place: capture_place(tcx, instance, *place, limits)?,
        },
        Rvalue::RawPtr(mutability, place) => RvalueV2::RawPointer {
            mutability: bounded_debug("raw pointer mutability", mutability, limits)?,
            place: capture_place(tcx, instance, *place, limits)?,
        },
        Rvalue::Cast(kind, operand, target) => RvalueV2::Cast {
            kind: bounded_debug("cast kind", kind, limits)?,
            operand: capture_operand(tcx, instance, operand, source_info, limits)?,
            target: type_identity(tcx, instance, *target),
        },
        Rvalue::BinaryOp(operation, operands) => RvalueV2::Binary {
            operation: bounded_debug("binary operation", operation, limits)?,
            lhs: capture_operand(tcx, instance, &operands.0, source_info, limits)?,
            rhs: capture_operand(tcx, instance, &operands.1, source_info, limits)?,
        },
        Rvalue::UnaryOp(operation, operand) => RvalueV2::Unary {
            operation: bounded_debug("unary operation", operation, limits)?,
            operand: capture_operand(tcx, instance, operand, source_info, limits)?,
        },
        Rvalue::Discriminant(place) => RvalueV2::Discriminant {
            place: capture_place(tcx, instance, *place, limits)?,
        },
        Rvalue::Aggregate(kind, operands) => {
            ensure_bound("aggregate operands", operands.len(), limits.max_operands)?;
            RvalueV2::Aggregate {
                kind: capture_aggregate_kind(tcx, instance, kind, limits)?,
                operands: operands
                    .iter()
                    .map(|operand| capture_operand(tcx, instance, operand, source_info, limits))
                    .collect::<Result<Vec<_>, _>>()?,
            }
        }
        Rvalue::CopyForDeref(place) => {
            RvalueV2::CopyForDeref(capture_place(tcx, instance, *place, limits)?)
        }
        Rvalue::ThreadLocalRef(def_id) => RvalueV2::ThreadLocalRef {
            definition: definition_identity(tcx, *def_id),
        },
        Rvalue::WrapUnsafeBinder(operand, target) => RvalueV2::WrapUnsafeBinder {
            operand: capture_operand(tcx, instance, operand, source_info, limits)?,
            target: type_identity(tcx, instance, *target),
        },
    };
    Ok(captured)
}

fn capture_aggregate_kind<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    kind: &rustc_middle::mir::AggregateKind<'tcx>,
    limits: CaptureLimitsV2,
) -> Result<AggregateKindV2, CaptureErrorV2> {
    let normalized_kind = tcx.instantiate_and_normalize_erasing_regions(
        instance.args,
        TypingEnv::fully_monomorphized(),
        EarlyBinder::bind(kind.clone()),
    );
    let rustc_kind = bounded_debug("aggregate kind", &normalized_kind, limits)?;
    let (class, definition, variant) = match kind {
        AggregateKind::Array(_) => (AggregateClassV2::Array, None, None),
        AggregateKind::Tuple => (AggregateClassV2::Tuple, None, None),
        AggregateKind::Adt(def_id, variant, ..) => (
            AggregateClassV2::Adt,
            Some(definition_identity(tcx, *def_id)),
            Some(variant.index()),
        ),
        AggregateKind::Closure(def_id, ..) => (
            AggregateClassV2::Closure,
            Some(definition_identity(tcx, *def_id)),
            None,
        ),
        AggregateKind::CoroutineClosure(def_id, ..) => (
            AggregateClassV2::CoroutineClosure,
            Some(definition_identity(tcx, *def_id)),
            None,
        ),
        AggregateKind::Coroutine(def_id, ..) => (
            AggregateClassV2::Coroutine,
            Some(definition_identity(tcx, *def_id)),
            None,
        ),
        AggregateKind::RawPtr(..) => (AggregateClassV2::RawPointer, None, None),
    };
    Ok(AggregateKindV2 {
        class,
        definition,
        variant,
        rustc_kind,
    })
}

fn capture_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    operand: &Operand<'tcx>,
    source_info: SourceInfo,
    limits: CaptureLimitsV2,
) -> Result<OperandV2, CaptureErrorV2> {
    match operand {
        Operand::Copy(place) => Ok(OperandV2::Copy(capture_place(
            tcx, instance, *place, limits,
        )?)),
        Operand::Move(place) => Ok(OperandV2::Move(capture_place(
            tcx, instance, *place, limits,
        )?)),
        Operand::Constant(constant) => Ok(OperandV2::Constant {
            ty: type_identity(tcx, instance, constant.const_.ty()),
            literal: bounded_debug(
                "constant",
                &tcx.instantiate_and_normalize_erasing_regions(
                    instance.args,
                    TypingEnv::fully_monomorphized(),
                    EarlyBinder::bind(constant.const_),
                ),
                limits,
            )?,
            source: span_identity(tcx, constant.span, source_info.scope.as_usize()),
        }),
        Operand::RuntimeChecks(kind) => Ok(OperandV2::RuntimeChecks {
            kind: bounded_debug("runtime checks", kind, limits)?,
        }),
    }
}

fn capture_place<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    place: Place<'tcx>,
    limits: CaptureLimitsV2,
) -> Result<PlaceV2, CaptureErrorV2> {
    ensure_bound(
        "place projection",
        place.projection.len(),
        limits.max_projection_depth,
    )?;
    let projection = place
        .projection
        .iter()
        .map(|element| capture_projection(tcx, instance, element, limits))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PlaceV2 {
        local: place.local.as_usize(),
        projection,
    })
}

fn capture_projection<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    element: ProjectionElem<rustc_middle::mir::Local, Ty<'tcx>>,
    _limits: CaptureLimitsV2,
) -> Result<ProjectionV2, CaptureErrorV2> {
    let captured = match element {
        ProjectionElem::Deref => ProjectionV2::Deref,
        ProjectionElem::Field(field, ty) => ProjectionV2::Field {
            index: field.index(),
            ty: type_identity(tcx, instance, ty),
        },
        ProjectionElem::Index(local) => ProjectionV2::Index {
            local: local.as_usize(),
        },
        ProjectionElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        } => ProjectionV2::ConstantIndex {
            offset,
            min_length,
            from_end,
        },
        ProjectionElem::Subslice { from, to, from_end } => {
            ProjectionV2::Subslice { from, to, from_end }
        }
        ProjectionElem::Downcast(name, variant) => ProjectionV2::Downcast {
            variant: variant.index(),
            name: name.map(|name| name.to_string()),
        },
        ProjectionElem::OpaqueCast(ty) => ProjectionV2::OpaqueCast {
            ty: type_identity(tcx, instance, ty),
        },
        ProjectionElem::UnwrapUnsafeBinder(ty) => ProjectionV2::UnwrapUnsafeBinder {
            ty: type_identity(tcx, instance, ty),
        },
    };
    Ok(captured)
}

fn capture_terminator<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    kind: &TerminatorKind<'tcx>,
    source_info: SourceInfo,
    limits: CaptureLimitsV2,
) -> Result<TerminatorKindV2, CaptureErrorV2> {
    let captured = match kind {
        TerminatorKind::Return => TerminatorKindV2::Return,
        TerminatorKind::Unreachable => TerminatorKindV2::Unreachable,
        TerminatorKind::Goto { target } => TerminatorKindV2::Goto {
            target: target.as_usize(),
        },
        TerminatorKind::SwitchInt { discr, targets } => TerminatorKindV2::SwitchInt {
            discriminant: capture_operand(tcx, instance, discr, source_info, limits)?,
            targets: targets
                .iter()
                .map(|(value, target)| SwitchTargetV2 {
                    value,
                    target: target.as_usize(),
                })
                .collect(),
            otherwise: targets.otherwise().as_usize(),
        },
        TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            unwind,
            call_source,
            fn_span,
        } => {
            ensure_bound("call arguments", args.len(), limits.max_operands)?;
            let (declared, resolved, intrinsic) = capture_callee_identity(tcx, instance, func);
            TerminatorKindV2::Call {
                function: capture_operand(tcx, instance, func, source_info, limits)?,
                declared,
                resolved,
                intrinsic,
                arguments: args
                    .iter()
                    .map(|argument| {
                        Ok(CallArgumentV2 {
                            operand: capture_operand(
                                tcx,
                                instance,
                                &argument.node,
                                source_info,
                                limits,
                            )?,
                            source: span_identity(tcx, argument.span, source_info.scope.as_usize()),
                        })
                    })
                    .collect::<Result<Vec<_>, CaptureErrorV2>>()?,
                destination: capture_place(tcx, instance, *destination, limits)?,
                target: target.map(|target| target.as_usize()),
                unwind: capture_unwind(unwind, limits)?,
                call_source: bounded_debug("call source", call_source, limits)?,
                function_span: span_identity(tcx, *fn_span, source_info.scope.as_usize()),
            }
        }
        TerminatorKind::TailCall {
            func,
            args,
            fn_span,
        } => {
            ensure_bound("tail-call arguments", args.len(), limits.max_operands)?;
            let (declared, resolved, intrinsic) = capture_callee_identity(tcx, instance, func);
            TerminatorKindV2::TailCall {
                function: capture_operand(tcx, instance, func, source_info, limits)?,
                declared,
                resolved,
                intrinsic,
                arguments: args
                    .iter()
                    .map(|argument| {
                        Ok(CallArgumentV2 {
                            operand: capture_operand(
                                tcx,
                                instance,
                                &argument.node,
                                source_info,
                                limits,
                            )?,
                            source: span_identity(tcx, argument.span, source_info.scope.as_usize()),
                        })
                    })
                    .collect::<Result<Vec<_>, CaptureErrorV2>>()?,
                function_span: span_identity(tcx, *fn_span, source_info.scope.as_usize()),
            }
        }
        TerminatorKind::Drop {
            place,
            target,
            unwind,
            replace,
            drop,
            async_fut,
        } => TerminatorKindV2::Drop {
            place: capture_place(tcx, instance, *place, limits)?,
            target: target.as_usize(),
            unwind: capture_unwind(unwind, limits)?,
            replace: *replace,
            async_drop: drop.map(|target| target.as_usize()),
            async_future_local: async_fut.map(|local| local.as_usize()),
        },
        TerminatorKind::Assert {
            cond,
            expected,
            msg,
            target,
            unwind,
        } => TerminatorKindV2::Assert {
            condition: capture_operand(tcx, instance, cond, source_info, limits)?,
            expected: *expected,
            target: target.as_usize(),
            message: bounded_debug("assert message", msg, limits)?,
            unwind: capture_unwind(unwind, limits)?,
        },
        TerminatorKind::InlineAsm { .. } => TerminatorKindV2::InlineAsm {
            rustc_kind: bounded_debug("inline assembly", kind, limits)?,
        },
        other => TerminatorKindV2::CompilerOpaque {
            rustc_kind: bounded_debug("terminator", other, limits)?,
        },
    };
    Ok(captured)
}

fn capture_unwind(
    unwind: &UnwindAction,
    limits: CaptureLimitsV2,
) -> Result<UnwindActionV2, CaptureErrorV2> {
    match unwind {
        UnwindAction::Continue => Ok(UnwindActionV2::Continue),
        UnwindAction::Unreachable => Ok(UnwindActionV2::Unreachable),
        UnwindAction::Terminate(reason) => Ok(UnwindActionV2::Terminate {
            reason: bounded_debug("unwind termination", reason, limits)?,
        }),
        UnwindAction::Cleanup(target) => Ok(UnwindActionV2::Cleanup {
            target: target.as_usize(),
        }),
    }
}

fn capture_callee_identity<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    operand: &Operand<'tcx>,
) -> (
    Option<DefinitionIdentityV2>,
    Option<FunctionIdentityV2>,
    Option<IntrinsicIdentityV2>,
) {
    let Operand::Constant(constant) = operand else {
        return (None, None, None);
    };
    let TyKind::FnDef(def_id, args) = constant.const_.ty().kind() else {
        return (None, None, None);
    };
    let declared = Some(definition_identity(tcx, *def_id));
    let normalized_args = tcx.instantiate_and_normalize_erasing_regions(
        caller.args,
        TypingEnv::fully_monomorphized(),
        EarlyBinder::bind(*args),
    );
    let resolved = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        *def_id,
        normalized_args,
    )
    .ok()
    .flatten()
    .map(|instance| function_identity(tcx, instance));
    let intrinsic = tcx.intrinsic(*def_id).map(|intrinsic| IntrinsicIdentityV2 {
        definition: definition_identity(tcx, *def_id),
        name: intrinsic.name.to_string(),
        must_be_overridden: intrinsic.must_be_overridden,
        const_stable: intrinsic.const_stable,
    });
    (declared, resolved, intrinsic)
}

fn function_identity(tcx: TyCtxt<'_>, instance: Instance<'_>) -> FunctionIdentityV2 {
    let rustc_kind = format!("{:?}", instance.def);
    FunctionIdentityV2 {
        definition: definition_identity(tcx, instance.def_id()),
        instance: InstanceIdentityV2 {
            kind: if matches!(instance.def, InstanceKind::Item(_)) {
                InstanceKindV2::Item
            } else {
                InstanceKindV2::GeneratedCallable {
                    rustc_kind: rustc_kind.clone(),
                }
            },
            generic_args: format!("{:?}", instance.args),
            rustc_debug: rustc_kind,
        },
    }
}

fn definition_identity(tcx: TyCtxt<'_>, def_id: DefId) -> DefinitionIdentityV2 {
    DefinitionIdentityV2 {
        crate_name: tcx.crate_name(def_id.krate).to_string(),
        def_path: tcx.def_path_str(def_id),
        def_path_hash: tcx.def_path_hash(def_id).0.to_le_bytes(),
    }
}

fn type_identity<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    ty: Ty<'tcx>,
) -> TypeIdentityV2 {
    let ty = tcx.instantiate_and_normalize_erasing_regions(
        instance.args,
        TypingEnv::fully_monomorphized(),
        EarlyBinder::bind(ty),
    );
    TypeIdentityV2 {
        rust: ty.to_string(),
        rustc_kind: format!("{:?}", ty.kind()),
    }
}

fn source_span(tcx: TyCtxt<'_>, source_info: SourceInfo) -> SourceSpanV2 {
    span_identity(tcx, source_info.span, source_info.scope.as_usize())
}

fn span_identity(tcx: TyCtxt<'_>, span: Span, source_scope: usize) -> SourceSpanV2 {
    let source_map = tcx.sess.source_map();
    let start = source_map.lookup_char_pos(span.lo());
    let end = source_map.lookup_char_pos(span.hi());
    SourceSpanV2 {
        file: start
            .file
            .name
            .prefer_remapped_unconditionally()
            .to_string_lossy()
            .into_owned(),
        start_line: start.line,
        start_column: start.col.0 + 1,
        end_line: end.line,
        end_column: end.col.0 + 1,
        source_scope,
        rustc_debug: format!("{span:?}"),
    }
}

fn bounded_debug(
    label: impl Into<String>,
    value: &impl fmt::Debug,
    limits: CaptureLimitsV2,
) -> Result<String, CaptureErrorV2> {
    let label = label.into();
    let text = format!("{value:?}");
    ensure_bound(label, text.len(), limits.max_text_bytes)?;
    if text.is_empty() || text.contains('\0') {
        return Err(CaptureErrorV2::new(
            "compiler debug identity is empty or contains NUL",
        ));
    }
    Ok(text)
}

fn ensure_bound(
    label: impl Into<String>,
    actual: usize,
    limit: usize,
) -> Result<(), CaptureErrorV2> {
    if actual > limit {
        return Err(CaptureErrorV2::new(format!(
            "{} bound exceeded: {actual} > {limit}",
            label.into()
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
