//! Same-session source custody, not callback, scope, graph or execution authority.

use super::{BoundCallV29, BoundContextEntryV29};
use crate::production_semantic_body_v1::ProductionSemanticBodyErrorV1 as Error;
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
struct CallBoundaryV29 {
    block: SemanticBlockIdV1,
    statements: usize,
    destination: SemanticLocalIdV1,
    destination_type: SemanticTypeIdV1,
    target: SemanticBlockIdV1,
    unwind: SemanticUnwindActionV1,
}

impl CallBoundaryV29 {
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
        let block = function
            .blocks()
            .get(self.block.index() as usize)
            .ok_or_else(mismatch)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            return Err(mismatch());
        };
        let destination = call.destination().ok_or_else(mismatch)?;
        if block.statements().len() != self.statements
            || destination.place().local() != self.destination
            || destination.place().ty() != self.destination_type
            || !destination.place().projections().is_empty()
            || destination.edge().target() != self.target
            || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
            || call.unwind() != self.unwind
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
        self.source.function
    }

    fn check(
        &self,
        semantic: &AdmittedInertSemanticMirV1,
        charge: &mut impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let source = &self.source;
        charge(20 + source.arguments.len())?;
        let function = semantic
            .functions()
            .get(source.function.index() as usize)
            .ok_or_else(mismatch)?;
        let helper = semantic
            .functions()
            .get(source.helper.index() as usize)
            .ok_or_else(mismatch)?;
        let issuance = source.issuance.observe(function)?;
        let helper_call = source.helper_call.observe(function)?;
        if function.identity() != source.root_identity
            || function.role() != SemanticFunctionRoleV1::KernelRoot
            || function.kernel_entry().is_none()
            || helper.role() != SemanticFunctionRoleV1::InternalHelper
            || helper.identity() != source.helper_identity
            || issuance.callee() != source.issuer
            || !issuance.arguments().is_empty()
            || issuance.destination().ok_or_else(mismatch)?.place().ty() != self.context
            || helper_call.arguments() != source.arguments
            || !matches!(semantic.callables().get(helper_call.callee().index() as usize),
                Some(SemanticCallableDeclV1::Defined { function }) if *function == source.helper)
            || !matches!(semantic.callables().get(source.issuer.index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic { binding,
                    operation: SemanticCompilerIntrinsicOperationV1::Execution(
                        SemanticExecutionOperationV29::ContextIssue { context }), ..
                }) if *context == self.context && binding.identity() == source.issuer_identity)
            || semantic
                .types()
                .get(self.context.index() as usize)
                .map(SemanticTypeDeclV1::identity)
                != Some(source.context_identity)
            || !matches!(
                semantic
                    .types()
                    .get(self.context.index() as usize)
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
}

impl RetainedContextEntriesV29 {
    pub(crate) fn seal(
        entries: Vec<RetainedContextEntryV29>,
        semantic: &AdmittedInertSemanticMirV1,
        mut charge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<Self, Error> {
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
                    || !entry
                        .is_some_and(|entry| entry.source.issuance.block.index() as usize == block)
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
