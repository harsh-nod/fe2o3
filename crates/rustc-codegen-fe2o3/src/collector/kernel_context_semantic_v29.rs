//! Consume entry custody at the canonical table and actual call boundaries.

use super::{AuthenticatedContextEntriesV1, flow};
use crate::collector::CollectedFunctionRole;
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
use crate::rustc_semantic_plan_v1::{
    DirectCallRecipeV1, ProductionSemanticPreflightErrorV1, RetainedSemanticBodyProducerV1,
    RetainedSemanticFunctionProducerV1, TerminalExpansionRecipeV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticCallableIdV1, SemanticFunctionIdV1, SemanticLocalIdV1,
};
use rustc_middle::mir::{Body, Const, ConstValue, Local, Operand, TerminatorKind, UnwindAction};
use rustc_middle::ty::{Instance, Ty};
use std::collections::{BTreeMap, HashMap};

type Error = ProductionSemanticPreflightErrorV1;

#[cfg(test)]
#[path = "../production_semantic_body_v1/context_entry_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "kernel_context_commitment_v29_tests.rs"]
mod commitment_tests;

#[path = "kernel_context_custody_v29.rs"]
mod custody;
pub(crate) use custody::{
    CallBoundaryV29, CompletedContextEntryV29, ContextRootVisitErrorV29, RetainedContextEntriesV29,
    RetainedContextEntryV29,
};

#[derive(Debug)]
struct BoundCallV29 {
    raw: flow::CallOccurrenceV1,
    block: SemanticBlockIdV1,
    destination: SemanticLocalIdV1,
    target: SemanticBlockIdV1,
    consumed: bool,
}

/// Not cloneable or constructible outside the authenticated collector.
#[derive(Debug)]
pub(crate) struct BoundContextEntryV29<'tcx> {
    root: Instance<'tcx>,
    function: SemanticFunctionIdV1,
    helper: Instance<'tcx>,
    helper_function: SemanticFunctionIdV1,
    issuer: Instance<'tcx>,
    context: Ty<'tcx>,
    optimized_body: &'tcx Body<'tcx>,
    original_mir_sha256: [u8; 32],
    original: flow::AuthenticatedFlowV1<'tcx>,
    issuance: BoundCallV29,
    helper_call: BoundCallV29,
    // None means the optimizer erased argument zero, not that it is optional.
    helper_argument: Option<Local>,
    semantic_helper_argument: SemanticLocalIdV1,
    arguments: usize,
}

impl<'tcx> AuthenticatedContextEntriesV1<'tcx> {
    pub(crate) fn bind_semantic_v29(
        self,
        functions: &[RetainedSemanticFunctionProducerV1<'tcx>],
        bodies: &[RetainedSemanticBodyProducerV1],
        terminals: &[TerminalExpansionRecipeV1<'tcx>],
        calls: &[DirectCallRecipeV1],
        issuance_count: usize,
        mut charge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<BTreeMap<SemanticFunctionIdV1, BoundContextEntryV29<'tcx>>, Error> {
        if issuance_count != self.entries.len() {
            return Err(Error::IdentityTableMismatch);
        }
        let mut bound = BTreeMap::new();
        if self.entries.is_empty() {
            return Ok(bound);
        }
        charge(functions.len())?;
        let mut ids = HashMap::new();
        ids.try_reserve(functions.len())
            .map_err(|_| Error::IdentityTableMismatch)?;
        for (index, function) in functions.iter().enumerate() {
            let id = SemanticFunctionIdV1::from_index(
                u32::try_from(index).map_err(|_| Error::IdentityTableMismatch)?,
            );
            if ids.insert(function.instance, id).is_some() {
                return Err(Error::IdentityTableMismatch);
            }
        }
        for entry in self.entries {
            charge(1)?;
            let source = entry.source;
            let function = *ids.get(&source.root).ok_or(Error::IdentityTableMismatch)?;
            let helper_function = *ids
                .get(&source.helper)
                .ok_or(Error::IdentityTableMismatch)?;
            if functions[function.index() as usize].role != CollectedFunctionRole::KernelEntry
                || functions[helper_function.index() as usize].role
                    != CollectedFunctionRole::InternalHelper
            {
                return Err(Error::IdentityTableMismatch);
            }
            let body = bodies
                .get(function.index() as usize)
                .ok_or(Error::IdentityTableMismatch)?;
            if body.function != function {
                return Err(Error::IdentityTableMismatch);
            }
            let raw = entry.optimized_body;
            let issuance = BoundCallV29::bind(entry.optimized.issuance, body)?;
            let helper_call = BoundCallV29::bind(entry.optimized.helper_call, body)?;
            let helper_argument =
                context_argument(raw, helper_call.raw.location.block.index(), source.context)
                    .map_err(|_| Error::IdentityTableMismatch)?;
            let receipt = BoundContextEntryV29 {
                root: source.root,
                function,
                helper: source.helper,
                helper_function,
                issuer: entry.optimized.issuer,
                context: source.context,
                optimized_body: raw,
                original_mir_sha256: source.original_mir_sha256,
                original: source.flow.original,
                issuance,
                helper_call,
                helper_argument,
                semantic_helper_argument: *body
                    .raw_to_semantic_locals
                    .get(
                        helper_argument
                            .unwrap_or(entry.optimized.issuance.destination)
                            .index(),
                    )
                    .ok_or(Error::IdentityTableMismatch)?,
                arguments: raw.arg_count + 1,
            };
            if bound.insert(function, receipt).is_some() {
                return Err(Error::IdentityTableMismatch);
            }
        }
        // Scan existing tables once, rather than searching them for every root.
        for recipe in terminals {
            charge(1)?;
            if recipe.expansion != ProductionTerminalExpansionV1::ContextIssue {
                continue;
            }
            let entry = bound
                .get_mut(&recipe.caller)
                .ok_or(Error::IdentityTableMismatch)?;
            if recipe.instance != entry.issuer
                || recipe.arguments != 0
                || recipe.block as usize != entry.issuance.raw.location.block.index()
                || entry.issuance.consumed
            {
                return Err(Error::IdentityTableMismatch);
            }
            entry.issuance.consumed = true;
        }
        for recipe in calls {
            charge(1)?;
            if let Some(entry) = bound.get_mut(&recipe.caller)
                && recipe.block as usize == entry.helper_call.raw.location.block.index()
            {
                if recipe.callee != entry.helper_function || entry.helper_call.consumed {
                    return Err(Error::IdentityTableMismatch);
                }
                entry.helper_call.consumed = true;
            }
        }
        for entry in bound.values_mut() {
            charge(1)?;
            if !entry.issuance.consumed || !entry.helper_call.consumed {
                return Err(Error::IdentityTableMismatch);
            }
            entry.issuance.consumed = false;
            entry.helper_call.consumed = false;
        }
        Ok(bound)
    }
}

impl BoundCallV29 {
    fn bind(
        raw: flow::CallOccurrenceV1,
        body: &RetainedSemanticBodyProducerV1,
    ) -> Result<Self, Error> {
        let block = *body
            .raw_to_semantic_blocks
            .get(raw.location.block.index())
            .ok_or(Error::IdentityTableMismatch)?;
        let destination = *body
            .raw_to_semantic_locals
            .get(raw.destination.index())
            .ok_or(Error::IdentityTableMismatch)?;
        let target = *body
            .raw_to_semantic_blocks
            .get(raw.target.index())
            .ok_or(Error::IdentityTableMismatch)?;
        if body
            .blocks
            .get(block.index() as usize)
            .map(|row| row.rustc_block as usize)
            != Some(raw.location.block.index())
            || body
                .blocks
                .get(target.index() as usize)
                .map(|row| row.rustc_block as usize)
                != Some(raw.target.index())
            || body
                .locals
                .get(destination.index() as usize)
                .map(|row| row.rustc_local as usize)
                != Some(raw.destination.index())
        {
            return Err(Error::IdentityTableMismatch);
        }
        Ok(Self {
            raw,
            block,
            destination,
            target,
            consumed: false,
        })
    }

    fn consume(&mut self, body: &Body<'_>, block: usize) -> Result<(), &'static str> {
        let data = body
            .basic_blocks
            .get(rustc_middle::mir::BasicBlock::from_usize(block))
            .ok_or("context call block")?;
        let Some(terminator) = &data.terminator else {
            return Err("context call terminator");
        };
        let TerminatorKind::Call {
            destination,
            target,
            unwind,
            ..
        } = &terminator.kind
        else {
            return Err("context call kind");
        };
        if self.consumed
            || block != self.raw.location.block.index()
            || data.statements.len() != self.raw.location.statement_index
            || destination.as_local() != Some(self.raw.destination)
            || *target != Some(self.raw.target)
            || *unwind != self.raw.unwind
        {
            return Err("context call occurrence");
        }
        self.consumed = true;
        Ok(())
    }
}

impl<'tcx> BoundContextEntryV29<'tcx> {
    pub(crate) fn validate_body(
        &self,
        root: Instance<'tcx>,
        function: SemanticFunctionIdV1,
        body: &Body<'tcx>,
        local: impl Fn(usize) -> Option<SemanticLocalIdV1>,
        block: impl Fn(usize) -> Option<SemanticBlockIdV1>,
    ) -> Result<(), &'static str> {
        if root != self.root
            || function != self.function
            || !std::ptr::eq(body, self.optimized_body)
        {
            return Err("context physical root");
        }
        for site in [&self.issuance, &self.helper_call] {
            if local(site.raw.destination.index()) != Some(site.destination)
                || block(site.raw.location.block.index()) != Some(site.block)
                || block(site.raw.target.index()) != Some(site.target)
            {
                return Err("context canonical call mapping");
            }
        }
        if local(
            self.helper_argument
                .unwrap_or(self.issuance.raw.destination)
                .index(),
        ) != Some(self.semantic_helper_argument)
        {
            return Err("context canonical argument mapping");
        }
        Ok(())
    }

    pub(crate) fn consume_call(
        &mut self,
        body: &Body<'tcx>,
        block: usize,
        callee: Instance<'tcx>,
        arguments: usize,
    ) -> Result<Option<Local>, &'static str> {
        if !std::ptr::eq(body, self.optimized_body) {
            return Err("context optimized body custody");
        }
        if callee == self.issuer || block == self.issuance.raw.location.block.index() {
            if callee != self.issuer
                || arguments != 0
                || body
                    .local_decls
                    .get(self.issuance.raw.destination)
                    .map(|local| local.ty)
                    != Some(self.context)
            {
                return Err("context issuer signature");
            }
            self.issuance.consume(body, block)?;
        } else if callee == self.helper || block == self.helper_call.raw.location.block.index() {
            if callee != self.helper
                || arguments != self.arguments
                || context_argument(body, block, self.context)? != self.helper_argument
            {
                return Err("context helper argument binding");
            }
            self.helper_call.consume(body, block)?;
            if self.helper_argument.is_none() {
                return Ok(Some(self.issuance.raw.destination));
            }
        }
        Ok(None)
    }

    pub(crate) fn issuer(&self) -> Instance<'tcx> {
        self.issuer
    }

    pub(crate) fn finish(
        self,
        tcx: rustc_middle::ty::TyCtxt<'tcx>,
        issuer_callable: SemanticCallableIdV1,
        local: impl Fn(Local) -> Option<SemanticLocalIdV1>,
        ty: impl Fn(Ty<'tcx>) -> Option<fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1>,
        charge: impl FnMut(
            usize,
        ) -> Result<
            (),
            crate::production_semantic_body_v1::ProductionSemanticBodyErrorV1,
        >,
    ) -> Result<
        CompletedContextEntryV29,
        crate::production_semantic_body_v1::ProductionSemanticBodyErrorV1,
    > {
        if !self.issuance.consumed || !self.helper_call.consumed {
            return Err(crate::production_semantic_body_v1::ProductionSemanticBodyErrorV1::IdentityTableMismatch {
                table: "unused context call occurrence",
            });
        }
        CompletedContextEntryV29::from_consumed(self, tcx, issuer_callable, local, ty, charge)
    }

    pub(crate) fn commitment<E>(
        &self,
        mut field: impl FnMut(&[u8]) -> Result<(), E>,
    ) -> Result<(), E> {
        field(b"fe2o3/semantic-mir/context-entry/v29")?;
        field(&self.original_mir_sha256)?;
        field(&self.function.index().to_le_bytes())?;
        field(&self.helper_function.index().to_le_bytes())?;
        for site in [
            &self.original.issuance,
            &self.original.helper_call,
            &self.issuance.raw,
            &self.helper_call.raw,
        ] {
            for value in [
                site.location.block.index(),
                site.location.statement_index,
                site.destination.index(),
                site.target.index(),
            ] {
                field(&(value as u64).to_le_bytes())?;
            }
            field(&[u8::from(site.unwind == UnwindAction::Unreachable)])?;
        }
        for site in [&self.issuance, &self.helper_call] {
            field(&site.block.index().to_le_bytes())?;
            field(&site.destination.index().to_le_bytes())?;
            field(&site.target.index().to_le_bytes())?;
        }
        field(&[u8::from(self.helper_argument.is_none())])?;
        field(
            &(self
                .helper_argument
                .unwrap_or(self.issuance.raw.destination)
                .index() as u64)
                .to_le_bytes(),
        )?;
        field(&(self.arguments as u64).to_le_bytes())
    }
}

fn context_argument<'tcx>(
    body: &Body<'tcx>,
    block: usize,
    context: Ty<'tcx>,
) -> Result<Option<Local>, &'static str> {
    let data = body
        .basic_blocks
        .get(rustc_middle::mir::BasicBlock::from_usize(block))
        .ok_or("context helper block")?;
    let Some(terminator) = &data.terminator else {
        return Err("context helper terminator");
    };
    let TerminatorKind::Call { args, .. } = &terminator.kind else {
        return Err("context helper call");
    };
    match args.first().map(|arg| &arg.node) {
        Some(Operand::Move(place))
            if place.as_local().is_some()
                && body.local_decls.get(place.local).map(|local| local.ty) == Some(context) =>
        {
            Ok(Some(place.local))
        }
        Some(Operand::Constant(constant)) if matches!(constant.const_, Const::Val(ConstValue::ZeroSized, ty) if ty == context) => {
            Ok(None)
        }
        _ => Err("context helper operand"),
    }
}
