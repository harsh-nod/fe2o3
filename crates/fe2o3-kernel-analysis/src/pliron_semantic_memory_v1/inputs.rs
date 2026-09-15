use super::*;
use std::collections::HashMap;

/// The expression table retains this live owner for as long as it compares
/// expressions containing memory inputs. A commitment label alone is not an
/// expression input and is never exported as a public equality witness.
pub(crate) struct LivePlironInitialReadInputsV1<'ctx> {
    context: &'ctx Context,
    function: FuncOp,
    proof: LivePlironSemanticMemoryProofV1,
    leaves: HashMap<Ptr<Operation>, (u32, SemanticTypedScalarV1)>,
}

impl<'ctx> LivePlironInitialReadInputsV1<'ctx> {
    pub(crate) fn prove(
        context: &'ctx Context,
        function: &FuncOp,
    ) -> Result<Self, PlironSemanticMemoryErrorV1> {
        use PlironSemanticMemoryErrorV1 as E;
        let proof = prove_live_pliron_semantic_memory_v1(context, function)?;
        let leaves = proof.with_live_reads(context, function, |reads| {
            let mut leaves = HashMap::with_capacity(reads.len());
            for read in reads {
                if !read.reads_initial_memory() {
                    return Err(E::NonInitialRead { site: read.site() });
                }
                let operation = Operation::get_op_dyn(read.producer(), context);
                let producer = operation.downcast_ref::<SemanticTypedReadOp>().ok_or(E::InvalidGraph)?;
                if producer.result(context) != read.result()
                    || producer.scalar(context) != Some(read.scalar())
                    || leaves.insert(read.producer(), (
                        producer.symbol(context).ok_or(E::InvalidGraph)?, read.scalar(),
                    )).is_some()
                {
                    return Err(E::InvalidGraph);
                }
            }
            Ok(leaves)
        })??;
        Ok(Self {
            context, function: FuncOp::from_operation(function.get_operation()), proof, leaves,
        })
    }

    /// An internal encoding view, not an independently usable equality proof.
    /// The expression table retains this owner and revalidates before reporting.
    pub(crate) fn with_live_commitment_leaves<R>(
        &self,
        inspect: impl FnOnce(&HashMap<Ptr<Operation>, (u32, SemanticTypedScalarV1)>) -> R,
    ) -> Result<R, PlironSemanticMemoryErrorV1> {
        self.revalidate()?;
        let result = inspect(&self.leaves);
        self.revalidate()?;
        Ok(result)
    }

    /// Fast query guard; complete structure/producer replay still brackets
    /// construction and every successful scalar/effect report.
    pub(crate) fn epoch_is_current(&self) -> bool {
        require_context_identity(self.context).ok() == Some(self.proof.context)
            && self.function.get_operation() == self.proof.function
            && epoch(self.context).ok() == Some(self.proof.epoch)
    }

    pub(crate) fn revalidate(&self) -> Result<(), PlironSemanticMemoryErrorV1> {
        self.proof.revalidate(self.context, &self.function)
    }

    /// Used only to reconstruct a typed root's encoding. Equality remains
    /// conditional on this owner, including its exact producer and memory proof.
    pub(crate) fn commitment_leaf(
        &self,
        producer: Ptr<Operation>,
    ) -> Result<Option<(u32, SemanticTypedScalarV1)>, PlironSemanticMemoryErrorV1> {
        self.revalidate()?;
        Ok(self.leaves.get(&producer).copied())
    }
}
