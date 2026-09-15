//! Source-only memory evidence; construction follows exact materializer replay.

use super::*;
use crate::production::ProductionSemanticLoadV2;
use dialect_kernel::SemanticTypedReadOp;
use fe2o3_kernel_analysis::{
    LivePlironSemanticMemoryProofV1, prove_live_pliron_semantic_memory_v1,
};

pub(super) struct SourceInitialReadsV1<'a> {
    proof: LivePlironSemanticMemoryProofV1,
    loads: BTreeMap<(u32, u32), &'a ProductionSemanticLoadV2>,
}

impl<'a> SourceInitialReadsV1<'a> {
    pub(super) fn prove(
        context: &pliron::context::Context,
        function: &FuncOp,
        kernel: &'a ProductionRankedKernelV1,
    ) -> Result<Self, ProductionSourceContractExportErrorV1> {
        use ProductionSourceContractExportErrorV1 as E;
        let plan = RankedSemanticReadsV1::new(kernel).map_err(|_| E::LiveMemoryRejected)?;
        let loads: BTreeMap<_, _> = plan
            .loads()
            .map(|load| ((load.block, load.operation), load))
            .collect();
        let proof = prove_live_pliron_semantic_memory_v1(context, function)
            .map_err(|_| E::LiveMemoryRejected)?;
        proof
            .with_live_reads(context, function, |reads| {
                let by_symbol: BTreeMap<_, _> = reads
                    .iter()
                    .map(|read| {
                        let producer = SemanticTypedReadOp::from_operation(read.producer());
                        (producer.symbol(context), read)
                    })
                    .collect();
                if reads.len() != loads.len() || by_symbol.len() != reads.len() {
                    return Err(E::LiveMemoryRejected);
                }
                for load in loads.values() {
                    let read = by_symbol
                        .get(&Some(load.proof_symbol()))
                        .ok_or(E::LiveMemoryRejected)?;
                    // Replay has already bound the actual view/index operands to
                    // the recipe. The independent theorem supplies memory version.
                    if !read.reads_initial_memory()
                        || read.allocation_origin() != load.allocation_origin
                        || read.scalar()
                            != typed_scalar(load.scalar).map_err(|_| E::LiveMemoryRejected)?
                    {
                        return Err(E::LiveMemoryRejected);
                    }
                }
                Ok(())
            })
            .map_err(|_| E::LiveMemoryRejected)??;
        Ok(Self { proof, loads })
    }

    pub(super) fn admits_site(&self, block: usize, operation: usize) -> bool {
        u32::try_from(block)
            .ok()
            .zip(u32::try_from(operation).ok())
            .is_some_and(|site| self.loads.contains_key(&site))
    }

    pub(super) fn admits_expression(&self, expression: &ProductionSemanticExpressionV2) -> bool {
        use ProductionSemanticExpressionV2 as X;
        match expression {
            X::Load(load) => self.loads.get(&(load.block, load.operation)).copied() == Some(load),
            X::Symbol { symbol, .. } => {
                *symbol < crate::production::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2
            }
            X::Constant { .. } => true,
            X::Unary { operand, .. } | X::Cast { operand, .. } => self.admits_expression(operand),
            X::Binary { lhs, rhs, .. } | X::Compare { lhs, rhs, .. } => {
                self.admits_expression(lhs) && self.admits_expression(rhs)
            }
            X::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                self.admits_expression(condition)
                    && self.admits_expression(when_true)
                    && self.admits_expression(when_false)
            }
        }
    }

    pub(super) fn revalidate(
        &self,
        context: &pliron::context::Context,
        function: &FuncOp,
    ) -> Result<(), ProductionSourceContractExportErrorV1> {
        self.proof
            .revalidate(context, function)
            .map_err(|_| ProductionSourceContractExportErrorV1::LiveMemoryRejected)
    }
}
