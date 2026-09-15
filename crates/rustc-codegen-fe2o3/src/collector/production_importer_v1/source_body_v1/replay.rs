//! Exact reconstructed-body correspondence, not a shape-based normalization exception.
use super::*;
use crate::rustc_semantic_plan_v1::RetainedSemanticBodyProducerV1;
use fe2o3_mir_model::semantic_mir_v1::{SemanticBlockIdV1, SemanticLocalIdV1};

fn mismatch() -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::KernelContextBinding(
        "original source body replay correspondence",
    )
}

fn spend(work: &mut usize, amount: usize) -> Result<(), ProductionSemanticImportErrorV1> {
    *work = work.checked_sub(amount).ok_or_else(|| {
        ProductionSemanticImportErrorV1::KernelContextBinding(
            "original source body replay work ceiling",
        )
    })?;
    Ok(())
}

pub(in crate::collector::production_importer_v1) struct Replay<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    plan: &'a ProductionSemanticPreflightPlanV1<'tcx>,
    type_bindings: Vec<ProductionSemanticTypeBindingV1<'tcx>>,
    owner: ProductionSemanticBodyRequestOwnerV1<'tcx>,
}

impl<'a, 'tcx> Replay<'a, 'tcx> {
    pub(in crate::collector::production_importer_v1) fn new(
        tcx: TyCtxt<'tcx>,
        plan: &'a ProductionSemanticPreflightPlanV1<'tcx>,
        work: &mut usize,
    ) -> Result<Self, ProductionSemanticImportErrorV1> {
        for count in [
            plan.type_producers().len(),
            plan.function_producers().len(),
            plan.terminal_producers().len(),
            plan.terminal_expansion_producers().len(),
        ] {
            spend(work, count)?;
        }
        let type_bindings = plan
            .type_producers()
            .iter()
            .enumerate()
            .map(|(index, producer)| {
                Ok(ProductionSemanticTypeBindingV1::new(
                    producer.ty,
                    SemanticTypeIdV1::from_index(u32::try_from(index).map_err(|_| mismatch())?),
                ))
            })
            .collect::<Result<Vec<_>, ProductionSemanticImportErrorV1>>()?;
        let owner = build_body_request_owner_v1(
            plan,
            type_bindings.len(),
            u32::try_from(plan.function_producers().len()).map_err(|_| mismatch())?,
        )?;
        Ok(Self {
            tcx,
            plan,
            type_bindings,
            owner,
        })
    }

    /// The caller supplies an ABI already checked by the live source roster.
    /// Reuse one construction budget across every replay in the observer.
    pub(in crate::collector::production_importer_v1) fn check<'b>(
        &mut self,
        observed_plan: &ProductionSemanticPreflightPlanV1<'tcx>,
        function: SemanticFunctionIdV1,
        semantic: &'b SemanticFunctionDeclV1,
        work: &mut usize,
    ) -> Result<Correspondence<'a, 'b>, ProductionSemanticImportErrorV1> {
        let plan = self.plan;
        if !std::ptr::eq(plan, observed_plan) {
            return Err(mismatch());
        }
        let producer = plan
            .function_producers()
            .get(function.index() as usize)
            .ok_or_else(mismatch)?;
        let original = plan
            .body_producers()
            .get(function.index() as usize)
            .filter(|body| body.function == function)
            .ok_or_else(mismatch)?;
        if !matches!(
            producer.instance.def,
            rustc_middle::ty::InstanceKind::Item(_)
        ) || !self.tcx.is_mir_available(producer.instance.def_id())
            || !plan.function_mir(function).is_some_and(|body| {
                std::ptr::eq(body, self.tcx.instance_mir(producer.instance.def))
            })
        {
            return Err(mismatch());
        }
        // The shared constructor accounts all recursive operand/projection work.
        // Charge the source-roster scans separately; only one replay body is live.
        for count in [
            plan.direct_call_producers().len(),
            plan.terminal_expansion_producers().len(),
            plan.normalized_intrinsic_producers().len(),
            original.locals.len(),
            original.blocks.len(),
        ] {
            spend(work, count)?;
        }
        for block in &original.blocks {
            spend(work, block.statements.len().saturating_add(1))?;
        }
        let expected = construct(
            self.tcx,
            plan,
            function,
            semantic.abi().clone(),
            &self.type_bindings,
            &mut self.owner,
        )?;
        if expected.identity() != semantic.identity()
            || expected.role() != semantic.role()
            || expected.item_definition_identity() != semantic.item_definition_identity()
            || expected.monomorphization_identity() != semantic.monomorphization_identity()
            || expected.generic_type_arguments_identity()
                != semantic.generic_type_arguments_identity()
            || expected.const_generic_arguments_identity()
                != semantic.const_generic_arguments_identity()
            || expected.abi() != semantic.abi()
            || expected.source() != semantic.source()
            || expected.entry() != semantic.entry()
            || expected.locals() != semantic.locals()
            || expected.blocks() != semantic.blocks()
        {
            return Err(mismatch());
        }
        Ok(Correspondence { original, semantic })
    }
}

pub(in crate::collector::production_importer_v1) struct Correspondence<'a, 'b> {
    original: &'a RetainedSemanticBodyProducerV1,
    semantic: &'b SemanticFunctionDeclV1,
}

impl Correspondence<'_, '_> {
    pub(in crate::collector::production_importer_v1) fn local(
        &self,
        raw: u32,
    ) -> Result<SemanticLocalIdV1, ProductionSemanticImportErrorV1> {
        let before = self
            .original
            .raw_to_semantic_locals
            .get(raw as usize)
            .ok_or_else(mismatch)?;
        let binding = self
            .original
            .locals
            .get(before.index() as usize)
            .filter(|binding| binding.rustc_local == raw)
            .ok_or_else(mismatch)?;
        let after = self
            .semantic
            .locals()
            .binary_search_by_key(&binding.identity, |l| l.identity())
            .map_err(|_| mismatch())?;
        let local = &self.semantic.locals()[after];
        if local.ty() != binding.ty || local.source() != binding.source.provenance {
            return Err(mismatch());
        }
        Ok(SemanticLocalIdV1::from_index(
            u32::try_from(after).map_err(|_| mismatch())?,
        ))
    }

    pub(in crate::collector::production_importer_v1) fn block(
        &self,
        raw: u32,
    ) -> Result<SemanticBlockIdV1, ProductionSemanticImportErrorV1> {
        let before = self
            .original
            .raw_to_semantic_blocks
            .get(raw as usize)
            .ok_or_else(mismatch)?;
        let binding = self
            .original
            .blocks
            .get(before.index() as usize)
            .filter(|binding| binding.rustc_block == raw)
            .ok_or_else(mismatch)?;
        let after = self
            .semantic
            .blocks()
            .binary_search_by_key(&binding.identity, |b| b.identity())
            .map_err(|_| mismatch())?;
        if self.semantic.blocks()[after].source() != binding.source.provenance {
            return Err(mismatch());
        }
        Ok(SemanticBlockIdV1::from_index(
            u32::try_from(after).map_err(|_| mismatch())?,
        ))
    }
}
