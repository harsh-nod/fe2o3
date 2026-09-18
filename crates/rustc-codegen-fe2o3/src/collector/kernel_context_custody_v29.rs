//! Same-session source custody, not callback, scope, graph or execution authority.

use super::{BoundCallV29, BoundContextEntryV29};
use crate::collector::workgroup_scope_custody_v29::{
    PendingWorkgroupScopesV29, RetainedWorkgroupScopesV29, ScopeCallableV29, ScopeEventV29,
};
use crate::production_semantic_body_v1::ProductionSemanticBodyErrorV1 as Error;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as VisitBudget,
    CanonicalKernelIrVerificationResourceErrorV1 as VisitResource,
};
use fe2o3_mir_model::semantic_mir_v1::*;

#[cfg(test)]
#[path = "kernel_context_custody_v29_tests.rs"]
mod tests;

fn mismatch() -> Error {
    Error::IdentityTableMismatch {
        table: "retained context entry custody",
    }
}

#[derive(Debug)]
pub(crate) struct CallBoundaryV29 {
    block: SemanticBlockIdV1,
    statements: usize,
    destination: SemanticLocalIdV1,
    destination_type: SemanticTypeIdV1,
    target: SemanticBlockIdV1,
    unwind: SemanticUnwindActionV1,
}

impl CallBoundaryV29 {
    pub(crate) fn location(&self) -> (SemanticBlockIdV1, usize) {
        (self.block, self.statements)
    }

    pub(crate) fn destination(&self) -> (SemanticLocalIdV1, SemanticTypeIdV1) {
        (self.destination, self.destination_type)
    }

    pub(crate) fn continuation(&self) -> SemanticBlockIdV1 {
        self.target
    }

    pub(crate) fn unwind(&self) -> SemanticUnwindActionV1 {
        self.unwind
    }

    fn from_consumed(
        call: BoundCallV29,
        destination_type: SemanticTypeIdV1,
    ) -> Result<Self, Error> {
        let unwind = match call.raw.unwind {
            rustc_middle::mir::UnwindAction::Continue => SemanticUnwindActionV1::Continue,
            rustc_middle::mir::UnwindAction::Unreachable => SemanticUnwindActionV1::Unreachable,
            _ => return Err(mismatch()),
        };
        Ok(Self {
            block: call.block,
            statements: call.raw.location.statement_index,
            destination: call.destination,
            destination_type,
            target: call.target,
            unwind,
        })
    }

    fn observe<'a>(
        &self,
        function: &'a SemanticFunctionDeclV1,
    ) -> Result<&'a SemanticDirectCallV1, Error> {
        let (block_id, statements) = self.location();
        let block = function
            .blocks()
            .get(block_id.index() as usize)
            .ok_or_else(mismatch)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            return Err(mismatch());
        };
        let destination = call.destination().ok_or_else(mismatch)?;
        if block.statements().len() != statements
            || (destination.place().local(), destination.place().ty()) != self.destination()
            || !destination.place().projections().is_empty()
            || destination.edge().target() != self.continuation()
            || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
            || call.unwind() != self.unwind()
            || !call.variadic_argument_abis().is_empty()
        {
            return Err(mismatch());
        }
        Ok(call)
    }
}

/// Only the consumed authenticated collector receipt constructs this value.
#[derive(Debug)]
pub(crate) struct CompletedContextEntryV29 {
    function: SemanticFunctionIdV1,
    helper: SemanticFunctionIdV1,
    issuer: SemanticCallableIdV1,
    issuance: CallBoundaryV29,
    helper_call: CallBoundaryV29,
    helper_argument: SemanticLocalIdV1,
    arguments: Vec<SemanticOperandV1>,
    root_identity: SemanticFunctionIdentityV1,
    helper_identity: SemanticFunctionIdentityV1,
    issuer_identity: SemanticFunctionIdentityV1,
    context_identity: SemanticTypeIdentityV1,
    _source_commitment: [u8; 32],
}

impl CompletedContextEntryV29 {
    pub(super) fn from_consumed<'tcx>(
        bound: BoundContextEntryV29<'tcx>,
        tcx: rustc_middle::ty::TyCtxt<'tcx>,
        issuer: SemanticCallableIdV1,
        local: impl Fn(rustc_middle::mir::Local) -> Option<SemanticLocalIdV1>,
        ty: impl Fn(rustc_middle::ty::Ty<'tcx>) -> Option<SemanticTypeIdV1>,
        mut charge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        use rustc_middle::mir::{Const, ConstValue, Operand, TerminatorKind};
        charge(40)?;
        let TerminatorKind::Call { args, .. } = &bound.optimized_body.basic_blocks
            [bound.helper_call.raw.location.block]
            .terminator()
            .kind
        else {
            return Err(mismatch());
        };
        if args.len() != bound.arguments {
            return Err(mismatch());
        }
        charge(args.len())?;
        let mut arguments = Vec::new();
        arguments
            .try_reserve_exact(args.len())
            .map_err(|_| Error::Allocation {
                resource: SemanticMirResourceV1::CallArguments,
            })?;
        for (ordinal, argument) in args.iter().enumerate() {
            let value = match &argument.node {
                Operand::Move(place) | Operand::Copy(place) if place.as_local().is_some() => {
                    let place = SemanticPlaceV1::new(
                        local(place.local).ok_or_else(mismatch)?,
                        vec![],
                        ty(bound.optimized_body.local_decls[place.local].ty)
                            .ok_or_else(mismatch)?,
                    )
                    .map_err(|_| mismatch())?;
                    if matches!(argument.node, Operand::Move(_)) {
                        SemanticOperandV1::Move(place)
                    } else {
                        SemanticOperandV1::Copy(place)
                    }
                }
                Operand::Constant(value) => {
                    let Const::Val(ConstValue::ZeroSized, raw_ty) = value.const_ else {
                        return Err(mismatch());
                    };
                    let ty = ty(raw_ty).ok_or_else(mismatch)?;
                    if ordinal == 0 {
                        SemanticOperandV1::Move(
                            SemanticPlaceV1::new(bound.semantic_helper_argument, vec![], ty)
                                .map_err(|_| mismatch())?,
                        )
                    } else {
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            ty,
                            SemanticConstantValueV1::ZeroSized,
                        ))
                    }
                }
                _ => return Err(mismatch()),
            };
            arguments.push(value);
        }
        let mut digest = crate::rustc_semantic_adapter_v1::SemanticIdentityDigestV1::new(
            b"fe2o3/retained-context-source/v29",
        );
        bound.commitment(|field| {
            digest.field(field);
            Ok::<_, Error>(())
        })?;
        let issuance_type = ty(bound.optimized_body.local_decls[bound.issuance.raw.destination].ty)
            .ok_or_else(mismatch)?;
        let helper_result_type =
            ty(bound.optimized_body.local_decls[bound.helper_call.raw.destination].ty)
                .ok_or_else(mismatch)?;
        Ok(Self {
            function: bound.function,
            helper: bound.helper_function,
            issuer,
            issuance: CallBoundaryV29::from_consumed(bound.issuance, issuance_type)?,
            helper_call: CallBoundaryV29::from_consumed(bound.helper_call, helper_result_type)?,
            helper_argument: bound.semantic_helper_argument,
            arguments,
            root_identity: crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(
                tcx, bound.root,
            )
            .function(),
            helper_identity: crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(
                tcx,
                bound.helper,
            )
            .function(),
            issuer_identity: crate::rustc_semantic_adapter_v1::canonical_function_identities_v1(
                tcx,
                bound.issuer,
            )
            .function(),
            context_identity: crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(
                tcx,
                bound.context,
            ),
            _source_commitment: digest.finish(),
        })
    }

    pub(crate) fn bind_function(
        self,
        function: &SemanticFunctionDeclV1,
        mut charge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<RetainedContextEntryV29, Error> {
        charge(16 + self.arguments.len())?;
        let issuance = self.issuance.observe(function)?;
        let helper = self.helper_call.observe(function)?;
        let context = issuance.destination().ok_or_else(mismatch)?.place().ty();
        if function.identity() != self.root_identity
            || issuance.callee() != self.issuer
            || !issuance.arguments().is_empty()
            || helper.callee().index() != self.helper.index()
            || helper.arguments() != self.arguments
            || !matches!(helper.arguments().first(), Some(SemanticOperandV1::Move(place))
                if place.local() == self.helper_argument && place.projections().is_empty() && place.ty() == context)
        {
            return Err(mismatch());
        }
        Ok(RetainedContextEntryV29 {
            source: self,
            context,
        })
    }
}

#[derive(Debug)]
pub(crate) struct RetainedContextEntryV29 {
    source: CompletedContextEntryV29,
    context: SemanticTypeIdV1,
}

impl RetainedContextEntryV29 {
    pub(crate) fn function(&self) -> SemanticFunctionIdV1 {
        self.root().0
    }

    pub(crate) fn root(&self) -> (SemanticFunctionIdV1, SemanticFunctionIdentityV1) {
        (self.source.function, self.source.root_identity)
    }

    pub(crate) fn helper(&self) -> (SemanticFunctionIdV1, SemanticFunctionIdentityV1) {
        (self.source.helper, self.source.helper_identity)
    }

    pub(crate) fn issuer(&self) -> (SemanticCallableIdV1, SemanticFunctionIdentityV1) {
        (self.source.issuer, self.source.issuer_identity)
    }

    pub(crate) fn context(&self) -> (SemanticTypeIdV1, SemanticTypeIdentityV1) {
        (self.context, self.source.context_identity)
    }

    pub(crate) fn issuance(&self) -> &CallBoundaryV29 {
        &self.source.issuance
    }

    pub(crate) fn helper_call(&self) -> &CallBoundaryV29 {
        &self.source.helper_call
    }

    pub(crate) fn helper_argument(&self) -> SemanticLocalIdV1 {
        self.source.helper_argument
    }

    pub(crate) fn helper_operands(&self) -> &[SemanticOperandV1] {
        &self.source.arguments
    }

    fn check(
        &self,
        semantic: &AdmittedInertSemanticMirV1,
        charge: &mut impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<(), Error> {
        charge(20 + self.helper_operands().len())?;
        let (root_id, root_identity) = self.root();
        let (helper_id, helper_identity) = self.helper();
        let (issuer_id, issuer_identity) = self.issuer();
        let (context_id, context_identity) = self.context();
        let function = semantic
            .functions()
            .get(root_id.index() as usize)
            .ok_or_else(mismatch)?;
        let helper = semantic
            .functions()
            .get(helper_id.index() as usize)
            .ok_or_else(mismatch)?;
        let issuance = self.issuance().observe(function)?;
        let helper_call = self.helper_call().observe(function)?;
        if function.identity() != root_identity
            || function.role() != SemanticFunctionRoleV1::KernelRoot
            || function.kernel_entry().is_none()
            || helper.role() != SemanticFunctionRoleV1::InternalHelper
            || helper.identity() != helper_identity
            || issuance.callee() != issuer_id
            || !issuance.arguments().is_empty()
            || issuance.destination().ok_or_else(mismatch)?.place().ty() != context_id
            || helper_call.arguments() != self.helper_operands()
            || !matches!(helper_call.arguments().first(), Some(SemanticOperandV1::Move(place))
                if place.local() == self.helper_argument() && place.projections().is_empty() && place.ty() == context_id)
            || !matches!(semantic.callables().get(helper_call.callee().index() as usize),
                Some(SemanticCallableDeclV1::Defined { function }) if *function == helper_id)
            || !matches!(semantic.callables().get(issuer_id.index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic { binding,
                    operation: SemanticCompilerIntrinsicOperationV1::Execution(
                        SemanticExecutionOperationV29::ContextIssue { context }), ..
                }) if *context == context_id && binding.identity() == issuer_identity)
            || semantic
                .types()
                .get(context_id.index() as usize)
                .map(SemanticTypeDeclV1::identity)
                != Some(context_identity)
            || !matches!(
                semantic
                    .types()
                    .get(context_id.index() as usize)
                    .map(SemanticTypeDeclV1::rust_type_kind),
                Some(SemanticRustTypeKindV1::Execution(
                    SemanticExecutionRoleV29::KernelContext
                ))
            )
        {
            return Err(mismatch());
        }
        Ok(())
    }
}

/// Kept beside the one source/SSA owner in private production bindings.
/// There is no public constructor, decoder, clone, or authority conversion.
pub(crate) struct RetainedContextEntriesV29 {
    entries: Vec<RetainedContextEntryV29>,
    semantic_sha256: [u8; 32],
    scopes: Option<RetainedWorkgroupScopesV29>,
}

/// Borrowed constructor data, not a lifecycle proof or executable authority.
/// Its lifetime belongs only to the receipt, so an owning consumer can move SSA.
pub(crate) struct RetainedExecutionSourceV29<'receipt> {
    semantic_sha256: &'receipt [u8; 32],
    roots: &'receipt [RetainedContextEntryV29],
    scopes: &'receipt RetainedWorkgroupScopesV29,
}

impl<'receipt> RetainedExecutionSourceV29<'receipt> {
    pub(crate) fn semantic_sha256(&self) -> &'receipt [u8; 32] {
        self.semantic_sha256
    }

    pub(crate) fn roots(&self) -> &'receipt [RetainedContextEntryV29] {
        self.roots
    }

    pub(crate) fn classes(&self) -> &'receipt [ScopeCallableV29] {
        self.scopes.classes()
    }

    pub(crate) fn events(&self) -> &'receipt [ScopeEventV29] {
        self.scopes.events()
    }
}

#[derive(Debug)]
pub(crate) enum ContextRootVisitErrorV29<E> {
    Source(Error),
    Resource(VisitResource),
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "The production view has no callback")
    )]
    Consumer(E),
}

impl RetainedContextEntriesV29 {
    /// Validate source custody and prepay the complete borrowed roster before use.
    /// No source or ledger borrow escapes, and no storage is allocated or refunded.
    pub(crate) fn materialization_source_v29<'receipt>(
        &'receipt self,
        semantic: &AdmittedInertSemanticMirV1,
        budget: &mut VisitBudget<'_>,
    ) -> Result<
        Option<RetainedExecutionSourceV29<'receipt>>,
        ContextRootVisitErrorV29<std::convert::Infallible>,
    > {
        budget
            .charge_work(1)
            .map_err(ContextRootVisitErrorV29::Resource)?;
        self.validate_source(semantic)
            .map_err(ContextRootVisitErrorV29::Source)?;
        let scopes = match (self.entries.is_empty(), self.scopes.as_ref()) {
            (true, None) => return Ok(None),
            (false, Some(scopes)) => scopes,
            _ => return Err(ContextRootVisitErrorV29::Source(mismatch())),
        };
        let source = RetainedExecutionSourceV29 {
            semantic_sha256: &self.semantic_sha256,
            roots: &self.entries,
            scopes,
        };
        for count in [
            source.roots().len(),
            source.classes().len(),
            source.events().len(),
        ] {
            budget
                .charge_work(count)
                .map_err(ContextRootVisitErrorV29::Resource)?;
        }
        Ok(Some(source))
    }

    /// Borrows authenticated anchors in canonical root order, never cloned authority.
    /// Source identity and enumeration work are checked before the first callback.
    /// Consumers share this budget and must discard partial plans on any error.
    #[cfg(test)]
    pub(crate) fn visit_root_anchors_v29<'w, E>(
        &self,
        semantic: &AdmittedInertSemanticMirV1,
        budget: &mut VisitBudget<'w>,
        mut visit: impl FnMut(&RetainedContextEntryV29, &mut VisitBudget<'w>) -> Result<(), E>,
    ) -> Result<(), ContextRootVisitErrorV29<E>> {
        budget
            .charge_work(1)
            .map_err(ContextRootVisitErrorV29::Resource)?;
        self.validate_source(semantic)
            .map_err(ContextRootVisitErrorV29::Source)?;
        budget
            .charge_work(self.entries.len())
            .map_err(ContextRootVisitErrorV29::Resource)?;
        for entry in &self.entries {
            visit(entry, budget).map_err(ContextRootVisitErrorV29::Consumer)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn seal(
        entries: Vec<RetainedContextEntryV29>,
        semantic: &AdmittedInertSemanticMirV1,
        charge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        Self::seal_with_scopes(entries, None, semantic, charge)
    }

    pub(crate) fn seal_with_scopes(
        entries: Vec<RetainedContextEntryV29>,
        scopes: Option<(
            PendingWorkgroupScopesV29,
            SemanticDeclarationTablesCommitmentV1,
        )>,
        semantic: &AdmittedInertSemanticMirV1,
        mut charge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        let mut scope_census = scopes
            .map(|(scopes, declarations)| scopes.begin_census(semantic, declarations, &mut charge))
            .transpose()?;
        let mut previous = None;
        for entry in &entries {
            charge(1)?;
            if previous.is_some_and(|id| id >= entry.function()) {
                return Err(mismatch());
            }
            previous = Some(entry.function());
            entry.check(semantic, &mut charge)?;
            charge(semantic.roots().len())?;
            if !semantic.roots().contains(&entry.function()) {
                return Err(mismatch());
            }
        }
        // Every actual issuance must belong to exactly one consumed source
        // occurrence; matching capability type/layout alone supplies no receipt.
        let mut issued = 0_usize;
        let mut next_entry = 0;
        for (function, body) in semantic.functions().iter().enumerate() {
            charge(1)?;
            let entry = entries
                .get(next_entry)
                .filter(|entry| entry.function().index() as usize == function);
            if entry.is_some() {
                next_entry += 1;
            }
            for (block, data) in body.blocks().iter().enumerate() {
                charge(1)?;
                if let Some(census) = &mut scope_census {
                    census.observe(
                        SemanticFunctionIdV1::from_index(
                            u32::try_from(function).map_err(|_| mismatch())?,
                        ),
                        SemanticBlockIdV1::from_index(
                            u32::try_from(block).map_err(|_| mismatch())?,
                        ),
                        data,
                        &mut charge,
                    )?;
                }
                let (callee, tail) = match data.terminator().kind() {
                    SemanticTerminatorKindV1::Call(call) => (call.callee(), false),
                    SemanticTerminatorKindV1::TailCall(call) => (call.callee(), true),
                    _ => continue,
                };
                if !matches!(
                    semantic.callables().get(callee.index() as usize),
                    Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::Execution(
                            SemanticExecutionOperationV29::ContextIssue { .. }
                        ),
                        ..
                    })
                ) {
                    continue;
                }
                if tail
                    || entry
                        .is_none_or(|entry| entry.source.issuance.block.index() as usize != block)
                {
                    return Err(mismatch());
                }
                issued = issued.checked_add(1).ok_or_else(mismatch)?;
            }
        }
        if issued != entries.len() {
            return Err(mismatch());
        }
        Ok(Self {
            entries,
            semantic_sha256: *semantic.semantic_sha256().as_bytes(),
            scopes: scope_census.map(|census| census.finish()).transpose()?,
        })
    }

    pub(crate) fn validate_source(
        &self,
        semantic: &AdmittedInertSemanticMirV1,
    ) -> Result<(), Error> {
        if self.semantic_sha256 != *semantic.semantic_sha256().as_bytes()
            || self.entries.len() > semantic.roots().len()
        {
            return Err(mismatch());
        }
        Ok(())
    }
}
