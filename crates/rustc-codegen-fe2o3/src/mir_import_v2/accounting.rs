use super::normalized::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CaptureAccountingV2 {
    pub(super) work_items: usize,
    pub(super) text_bytes: usize,
}

pub(super) fn recompute_capture_accounting_v2(
    body: &CapturedBodyV2,
    limits: CaptureLimitsV2,
) -> Result<CaptureAccountingV2, ValidationErrorV2> {
    let mut accounting = AccountingV2 {
        limits,
        work_items: 0,
        text_bytes: 0,
    };
    accounting.body(body)?;
    Ok(CaptureAccountingV2 {
        work_items: accounting.work_items,
        text_bytes: accounting.text_bytes,
    })
}

struct AccountingV2 {
    limits: CaptureLimitsV2,
    work_items: usize,
    text_bytes: usize,
}

impl AccountingV2 {
    fn work(&mut self, count: usize) -> Result<(), ValidationErrorV2> {
        self.work_items = self.work_items.checked_add(count).ok_or_else(|| {
            ValidationErrorV2::new("capture_work_items", "recomputed work count overflowed")
        })?;
        if self.work_items > self.limits.max_total_work_items {
            return Err(ValidationErrorV2::new(
                "capture_work_items",
                format!(
                    "recomputed work bound exceeded: {} > {}",
                    self.work_items, self.limits.max_total_work_items
                ),
            ));
        }
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<(), ValidationErrorV2> {
        if text.is_empty() {
            return Err(ValidationErrorV2::new(
                "capture_text_bytes",
                "captured text must not be empty",
            ));
        }
        if text.len() > self.limits.max_text_bytes {
            return Err(ValidationErrorV2::new(
                "capture_text_bytes",
                format!(
                    "captured text bound exceeded: {} > {}",
                    text.len(),
                    self.limits.max_text_bytes
                ),
            ));
        }
        if text.contains('\0') {
            return Err(ValidationErrorV2::new(
                "capture_text_bytes",
                "captured text contains a NUL byte",
            ));
        }
        self.text_bytes = self.text_bytes.checked_add(text.len()).ok_or_else(|| {
            ValidationErrorV2::new("capture_text_bytes", "recomputed text count overflowed")
        })?;
        if self.text_bytes > self.limits.max_total_text_bytes {
            return Err(ValidationErrorV2::new(
                "capture_text_bytes",
                format!(
                    "recomputed text bound exceeded: {} > {}",
                    self.text_bytes, self.limits.max_total_text_bytes
                ),
            ));
        }
        Ok(())
    }

    fn body(&mut self, body: &CapturedBodyV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.function(&body.function)?;
        if let Some(signature) = &body.caller_signature {
            self.work(1)?;
            self.signature(signature)?;
        }
        self.span(&body.source)?;
        self.work(body.source_scopes.len())?;
        for scope in &body.source_scopes {
            self.source_scope(scope)?;
        }
        self.work(body.locals.len())?;
        for local in &body.locals {
            self.local(local)?;
        }
        self.work(body.blocks.len())?;
        for block in &body.blocks {
            self.block(block)?;
        }
        Ok(())
    }

    fn function(&mut self, function: &FunctionIdentityV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.definition(&function.definition)?;
        self.instance(&function.instance)
    }

    fn definition(&mut self, definition: &DefinitionIdentityV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.text(&definition.diagnostic_crate_name)?;
        self.text(&definition.diagnostic_def_path)
    }

    fn intrinsic(&mut self, intrinsic: &IntrinsicIdentityV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.definition(&intrinsic.definition)?;
        self.text(&intrinsic.name)
    }

    fn signature(
        &mut self,
        signature: &FunctionSignatureIdentityV2,
    ) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.work(signature.inputs.len())?;
        for input in &signature.inputs {
            self.ty(input)?;
        }
        self.ty(&signature.output)?;
        self.text(&signature.abi.canonical_name)
    }

    fn instance(&mut self, instance: &InstanceIdentityV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.instance_kind(&instance.kind)?;
        self.text(&instance.diagnostic_generic_args)?;
        self.text(&instance.diagnostic_debug)
    }

    fn instance_kind(&mut self, kind: &InstanceKindV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        match kind {
            InstanceKindV2::Item
            | InstanceKindV2::Intrinsic
            | InstanceKindV2::VTableShim
            | InstanceKindV2::ReifyShim { reason: None }
            | InstanceKindV2::Virtual { .. }
            | InstanceKindV2::ClosureOnceShim { .. }
            | InstanceKindV2::ThreadLocalShim => Ok(()),
            InstanceKindV2::ReifyShim { reason: Some(_) } => self.work(1),
            InstanceKindV2::ConstructCoroutineInClosureShim {
                coroutine_closure, ..
            } => self.definition(coroutine_closure),
            InstanceKindV2::FnPtrShim { fn_pointer }
            | InstanceKindV2::CloneShim { ty: fn_pointer }
            | InstanceKindV2::FnPtrAddrShim { ty: fn_pointer }
            | InstanceKindV2::AsyncDropGlueCtorShim { ty: fn_pointer }
            | InstanceKindV2::AsyncDropGlue { ty: fn_pointer } => self.ty(fn_pointer),
            InstanceKindV2::FutureDropPollShim {
                proxy_coroutine,
                implementation_coroutine,
            } => {
                self.ty(proxy_coroutine)?;
                self.ty(implementation_coroutine)
            }
            InstanceKindV2::DropGlue { ty: None } => Ok(()),
            InstanceKindV2::DropGlue { ty: Some(ty) } => {
                self.work(1)?;
                self.ty(ty)
            }
        }
    }

    fn span(&mut self, span: &SourceSpanV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.text(&span.remapped_file)?;
        self.text(&span.diagnostic_debug)?;
        self.expansion(&span.expansion)?;
        if span.source_scope_parent.is_some() {
            self.work(1)?;
        }
        if span.inlined_instance_hash.is_some() {
            self.work(1)?;
        }
        Ok(())
    }

    fn source_scope(&mut self, scope: &SourceScopeIdentityV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        if scope.parent.is_some() {
            self.work(1)?;
        }
        if scope.inlined_parent.is_some() {
            self.work(1)?;
        }
        if let Some(inlined) = &scope.inlined {
            self.work(1)?;
            self.function(inlined)?;
        }
        self.structural_span(&scope.scope_span)?;
        if let Some(callsite) = &scope.inlined_callsite {
            self.work(1)?;
            self.structural_span(callsite)?;
        }
        Ok(())
    }

    fn structural_span(
        &mut self,
        span: &StructuralSpanIdentityV2,
    ) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.expansion(&span.expansion)
    }

    fn expansion(&mut self, expansion: &MacroExpansionIdentityV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.work(expansion.frames.len())?;
        for frame in &expansion.frames {
            self.work(1)?;
            if frame.macro_definition.is_some() {
                self.work(1)?;
            }
            if frame.parent_module.is_some() {
                self.work(1)?;
            }
        }
        Ok(())
    }

    fn ty(&mut self, ty: &TypeIdentityV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.text(&ty.diagnostic_display)?;
        self.text(&ty.diagnostic_debug)?;
        match &ty.class {
            TypeClassV2::Adt { definition, .. }
            | TypeClassV2::FunctionDefinition { definition, .. }
            | TypeClassV2::Closure { definition, .. }
            | TypeClassV2::CoroutineClosure { definition, .. }
            | TypeClassV2::Coroutine { definition, .. }
            | TypeClassV2::CoroutineWitness { definition, .. }
            | TypeClassV2::Foreign { definition } => self.definition(definition),
            TypeClassV2::Bool
            | TypeClassV2::Char
            | TypeClassV2::SignedInteger(_)
            | TypeClassV2::UnsignedInteger(_)
            | TypeClassV2::Float(_)
            | TypeClassV2::StringSlice
            | TypeClassV2::Array
            | TypeClassV2::Pattern
            | TypeClassV2::Slice
            | TypeClassV2::RawPointer { .. }
            | TypeClassV2::Reference { .. }
            | TypeClassV2::FunctionPointer
            | TypeClassV2::UnsafeBinder
            | TypeClassV2::Dynamic
            | TypeClassV2::Never
            | TypeClassV2::Tuple { .. }
            | TypeClassV2::Unsupported(_) => Ok(()),
        }
    }

    fn stable_value(&mut self, value: &StableCompilerValueV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.text(&value.diagnostic)
    }

    fn local(&mut self, local: &LocalDeclV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.ty(&local.ty)?;
        self.span(&local.source)?;
        self.text(&local.diagnostic_debug)
    }

    fn block(&mut self, block: &BasicBlockV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.work(block.statements.len())?;
        for statement in &block.statements {
            self.statement(statement)?;
        }
        self.terminator(&block.terminator)
    }

    fn statement(&mut self, statement: &StatementV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.span(&statement.source)?;
        self.text(&statement.diagnostic_debug)?;
        self.statement_kind(&statement.kind)
    }

    fn statement_kind(&mut self, kind: &StatementKindV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        match kind {
            StatementKindV2::Assign { destination, value } => {
                self.place(destination)?;
                self.rvalue(value)
            }
            StatementKindV2::StorageLive { .. }
            | StatementKindV2::StorageDead { .. }
            | StatementKindV2::SetDiscriminant { .. }
            | StatementKindV2::PlaceMention { .. } => match kind {
                StatementKindV2::SetDiscriminant { place, .. }
                | StatementKindV2::PlaceMention { place } => self.place(place),
                StatementKindV2::StorageLive { .. } | StatementKindV2::StorageDead { .. } => Ok(()),
                _ => unreachable!(),
            },
            StatementKindV2::Intrinsic(intrinsic) => self.intrinsic_statement(intrinsic),
            StatementKindV2::Retag { place, kind } => {
                self.place(place)?;
                self.stable_value(kind)
            }
            StatementKindV2::Coverage { kind } => self.stable_value(kind),
            StatementKindV2::Nop => Ok(()),
            StatementKindV2::Unsupported(kind) => self.unsupported_statement(kind),
        }
    }

    fn unsupported_statement(
        &mut self,
        kind: &UnsupportedStatementV2,
    ) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        match kind {
            UnsupportedStatementV2::FakeRead { cause, place } => {
                self.stable_value(cause)?;
                self.place(place)
            }
            UnsupportedStatementV2::AscribeUserType {
                place,
                projection,
                variance,
            } => {
                self.place(place)?;
                self.stable_value(projection)?;
                self.stable_value(variance)
            }
            UnsupportedStatementV2::ConstEvalCounter => Ok(()),
            UnsupportedStatementV2::BackwardIncompatibleDropHint { place, reason } => {
                self.place(place)?;
                self.stable_value(reason)
            }
        }
    }

    fn intrinsic_statement(
        &mut self,
        intrinsic: &IntrinsicStatementV2,
    ) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        match intrinsic {
            IntrinsicStatementV2::CopyNonOverlapping {
                source,
                destination,
                count,
            } => {
                self.operand(source)?;
                self.operand(destination)?;
                self.operand(count)
            }
            IntrinsicStatementV2::Assume { condition } => self.operand(condition),
        }
    }

    fn place(&mut self, place: &PlaceV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.work(place.projection.len())?;
        for projection in &place.projection {
            self.projection(projection)?;
        }
        Ok(())
    }

    fn projection(&mut self, projection: &ProjectionV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        match projection {
            ProjectionV2::Field { ty, .. }
            | ProjectionV2::OpaqueCast { ty }
            | ProjectionV2::UnwrapUnsafeBinder { ty } => self.ty(ty),
            ProjectionV2::Downcast {
                name: Some(name), ..
            } => {
                self.work(1)?;
                self.text(name)
            }
            ProjectionV2::Deref
            | ProjectionV2::Index { .. }
            | ProjectionV2::ConstantIndex { .. }
            | ProjectionV2::Subslice { .. }
            | ProjectionV2::Downcast { name: None, .. } => Ok(()),
        }
    }

    fn operand(&mut self, operand: &OperandV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        match operand {
            OperandV2::Copy(place) | OperandV2::Move(place) => self.place(place),
            OperandV2::Constant { ty, value, source } => {
                self.ty(ty)?;
                self.stable_value(value)?;
                self.span(source)
            }
            OperandV2::RuntimeChecks { kind } => self.stable_value(kind),
        }
    }

    fn rvalue(&mut self, value: &RvalueV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        match value {
            RvalueV2::Use(operand) => self.operand(operand),
            RvalueV2::Repeat { operand, count } => {
                self.operand(operand)?;
                self.stable_value(count)
            }
            RvalueV2::Reference { borrow_kind, place } => {
                self.stable_value(borrow_kind)?;
                self.place(place)
            }
            RvalueV2::RawPointer { kind, place } => {
                self.stable_value(kind)?;
                self.place(place)
            }
            RvalueV2::Cast {
                kind,
                operand,
                target,
            } => {
                self.stable_value(kind)?;
                self.operand(operand)?;
                self.ty(target)
            }
            RvalueV2::Binary {
                operation,
                lhs,
                rhs,
            } => {
                self.stable_value(operation)?;
                self.operand(lhs)?;
                self.operand(rhs)
            }
            RvalueV2::Unary { operation, operand } => {
                self.stable_value(operation)?;
                self.operand(operand)
            }
            RvalueV2::Discriminant { place } | RvalueV2::CopyForDeref(place) => self.place(place),
            RvalueV2::Aggregate { kind, operands } => {
                self.aggregate_kind(kind)?;
                self.work(operands.len())?;
                for operand in operands {
                    self.operand(operand)?;
                }
                Ok(())
            }
            RvalueV2::ThreadLocalRef { definition } => self.definition(definition),
            RvalueV2::WrapUnsafeBinder { operand, target } => {
                self.operand(operand)?;
                self.ty(target)
            }
        }
    }

    fn aggregate_kind(&mut self, kind: &AggregateKindV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        match kind {
            AggregateKindV2::Array { element } => self.ty(element),
            AggregateKindV2::Tuple => Ok(()),
            AggregateKindV2::Adt { definition, .. }
            | AggregateKindV2::Closure { definition, .. }
            | AggregateKindV2::CoroutineClosure { definition, .. }
            | AggregateKindV2::Coroutine { definition, .. } => self.definition(definition),
            AggregateKindV2::RawPointer { pointee, .. } => self.ty(pointee),
        }
    }

    fn terminator(&mut self, terminator: &TerminatorV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        self.span(&terminator.source)?;
        self.text(&terminator.diagnostic_debug)?;
        self.work(terminator.successors.len())?;
        self.terminator_kind(&terminator.kind)
    }

    fn terminator_kind(&mut self, kind: &TerminatorKindV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        match kind {
            TerminatorKindV2::Return
            | TerminatorKindV2::Unreachable
            | TerminatorKindV2::Goto { .. }
            | TerminatorKindV2::UnwindResume
            | TerminatorKindV2::CoroutineDrop
            | TerminatorKindV2::FalseEdge { .. } => Ok(()),
            TerminatorKindV2::SwitchInt {
                discriminant,
                targets,
                ..
            } => {
                self.operand(discriminant)?;
                self.work(targets.len())
            }
            TerminatorKindV2::Call {
                function,
                callee,
                arguments,
                destination,
                unwind,
                call_source,
                function_span,
                ..
            } => {
                self.operand(function)?;
                self.callee(callee)?;
                self.arguments(arguments)?;
                self.place(destination)?;
                self.unwind(unwind)?;
                self.stable_value(call_source)?;
                self.span(function_span)
            }
            TerminatorKindV2::TailCall {
                function,
                callee,
                arguments,
                function_span,
                ..
            } => {
                self.operand(function)?;
                self.callee(callee)?;
                self.arguments(arguments)?;
                self.span(function_span)
            }
            TerminatorKindV2::Drop {
                place,
                unwind,
                async_drop,
                async_future_local,
                ..
            } => {
                self.place(place)?;
                self.unwind(unwind)?;
                if async_drop.is_some() {
                    self.work(1)?;
                }
                if async_future_local.is_some() {
                    self.work(1)?;
                }
                Ok(())
            }
            TerminatorKindV2::Assert {
                condition,
                message,
                unwind,
                ..
            } => {
                self.operand(condition)?;
                self.stable_value(message)?;
                self.unwind(unwind)
            }
            TerminatorKindV2::UnwindTerminate { reason } => self.stable_value(reason),
            TerminatorKindV2::Yield {
                value,
                resume_argument,
                drop,
                ..
            } => {
                self.operand(value)?;
                self.place(resume_argument)?;
                if drop.is_some() {
                    self.work(1)?;
                }
                Ok(())
            }
            TerminatorKindV2::FalseUnwind { unwind, .. } => self.unwind(unwind),
            TerminatorKindV2::Unsupported(_) => self.work(1),
        }
    }

    fn arguments(&mut self, arguments: &[CallArgumentV2]) -> Result<(), ValidationErrorV2> {
        self.work(arguments.len())?;
        for argument in arguments {
            self.work(1)?;
            self.operand(&argument.operand)?;
            self.span(&argument.source)?;
        }
        Ok(())
    }

    fn callee(&mut self, callee: &CalleeIdentityV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        match callee {
            CalleeIdentityV2::Direct {
                declared,
                declared_signature,
                resolved,
                resolved_signature,
                intrinsic,
                ..
            } => {
                self.definition(declared)?;
                self.signature(declared_signature)?;
                self.function(resolved)?;
                self.signature(resolved_signature)?;
                if let Some(intrinsic) = intrinsic {
                    self.work(1)?;
                    self.intrinsic(intrinsic)?;
                }
                Ok(())
            }
            CalleeIdentityV2::Indirect {
                callable_type,
                signature,
                ..
            } => {
                self.ty(callable_type)?;
                self.signature(signature)
            }
        }
    }

    fn unwind(&mut self, unwind: &UnwindActionV2) -> Result<(), ValidationErrorV2> {
        self.work(1)?;
        match unwind {
            UnwindActionV2::Continue
            | UnwindActionV2::Unreachable
            | UnwindActionV2::Cleanup { .. } => Ok(()),
            UnwindActionV2::Terminate { reason } => self.stable_value(reason),
        }
    }
}
