//! Borrowed physical output order from the existing checked transition.

use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirBlockRefV1, CanonicalKirEdgeArgumentRefV1, CanonicalKirEdgeRefV1,
    CanonicalKirEffectRefV1, CanonicalKirFunctionRefV1, CanonicalKirOperationRefV1,
    CanonicalKirUseRefV1,
};
use fe2o3_kernel_ir::{
    CanonicalKirBlockSegmentV1, CanonicalKirEdgeArgumentTransitionV1,
    CanonicalKirEdgeTransitionV1, CanonicalKirOperationOriginV1,
    CanonicalKirTransitionCandidateV1, CanonicalKirUseTransitionV1,
};

/// One event in physical output order. The borrowed rows are observations, not
/// source-equivalence, memory-currentness, or generated-recipe authority.
pub enum ProductionOptimizedSourceCfgEventV18<'a, 'g> {
    /// Actual block entry followed by its checked original merge chain.
    Block {
        /// Exact block borrowed from the checked output inventory.
        actual: &'a CanonicalKirBlockRefV1<'g>,
        /// Original input segments in the checked physical merge order.
        segments: &'a [CanonicalKirBlockSegmentV1],
    },
    /// Actual operation and exact original operand roles. Synthesized constants
    /// retain their explicit ConstantFrom origin; they are not source operations.
    Operation {
        /// Exact operation borrowed from the checked output inventory.
        actual: &'a CanonicalKirOperationRefV1<'g>,
        /// Checked retained-operation or synthesized-constant lineage.
        origin: CanonicalKirOperationOriginV1,
        /// Actual operand uses in the operation's physical operand order.
        operands: &'a [CanonicalKirUseRefV1],
        /// Checked input-use relations aligned with `operands`.
        operand_origins: &'a [CanonicalKirUseTransitionV1],
        /// Actual physical effects attached to this output operation.
        effects: &'a [CanonicalKirEffectRefV1<'g>],
    },
    /// Actual terminator and successor occurrences in their executable order.
    /// Duplicate destinations remain separate edges and separate payload roles.
    Terminator {
        /// Exact output block whose terminator is being visited.
        actual: &'a CanonicalKirBlockRefV1<'g>,
        /// Actual terminator uses in physical operand order.
        operands: &'a [CanonicalKirUseRefV1],
        /// Checked input-use relations aligned with `operands`.
        operand_origins: &'a [CanonicalKirUseTransitionV1],
        /// Successor occurrences in terminator order, including duplicates.
        edges: &'a [CanonicalKirEdgeRefV1<'g>],
        /// Checked input-edge relations aligned with `edges`.
        edge_origins: &'a [CanonicalKirEdgeTransitionV1],
        /// Flattened successor payload bindings in edge and argument order.
        arguments: &'a [CanonicalKirEdgeArgumentRefV1],
        /// Checked input-payload relations aligned with `arguments`.
        argument_origins: &'a [CanonicalKirEdgeArgumentTransitionV1],
    },
}

/// Root-qualified borrowed output, constructed only from the existing checked
/// transition. It never copies the graph or constructs a second origin index.
/// The enclosing source scope and consumer meter retain owner/ledger custody.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgRootV18;
/// fn forge() -> ProductionOptimizedSourceCfgRootV18<'static, 'static> {
///     ProductionOptimizedSourceCfgRootV18 { root: 0, function: panic!(),
///         inventory: panic!(), rows: panic!() }
/// }
/// ```
pub struct ProductionOptimizedSourceCfgRootV18<'a, 'g> {
    root: usize,
    function: &'a CanonicalKirFunctionRefV1<'g>,
    inventory: &'a Inventory<'g>,
    rows: CanonicalKirTransitionCandidateV1<'a>,
}

impl<'g> ProductionOptimizedSourceCorrespondenceV18<'g> {
    /// Resolves the actual output function by the checked declaration relation,
    /// never by assuming original function ordinals survive optimization.
    /// The caller reserves the returned fixed header before this query.
    pub fn output_root_cfg_v18<'a>(
        &'a self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedSourceCfgRootV18<'a, 'g>> {
        self.retain((|| {
            self.query(budget)?;
            let input_ordinal = self.original.source.root(root, budget)?.1;
            budget.charge_work(1)?;
            let input = self.checked.input().functions().get(input_ordinal)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("output CFG original root"))?;
            let inventory = self.checked.output();
            let function = inventory.function_for_name(input.function.id.as_str(), budget)
                .map_err(source_pointer_inventory_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("output CFG actual root"))?;
            budget.charge_work(2)?;
            let rows = self.checked.rows();
            let association = rows.functions.get(function.coordinate.0 as usize)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("output CFG function relation"))?;
            if association.input != input.coordinate || association.output != function.coordinate {
                return resources::binding("output CFG source/output function mismatch");
            }
            self.check(budget)?;
            Ok(ProductionOptimizedSourceCfgRootV18 { root, function, inventory, rows })
        })())
    }
}

impl<'a, 'g> ProductionOptimizedSourceCfgRootV18<'a, 'g> {
    /// Original root ordinal, not the output function ordinal.
    pub fn root(&self) -> usize { self.root }

    /// Exact actual function, including its original signature and output blocks.
    pub fn function(&self) -> &'a CanonicalKirFunctionRefV1<'g> { self.function }

    /// The actual checked inventory; this borrow grants no final source proof.
    pub fn inventory(&self) -> &'a Inventory<'g> { self.inventory }

    /// Streams actual blocks and operations, not source blocks or source order.
    /// One visit pays one work unit; consumers also pay for individual borrowed
    /// row inspections and any owned output they retain. The source-bound meter
    /// used by production checks the owner's first refusal at every debit.
    pub fn visit<E>(
        &self,
        meter: &mut dyn fe2o3_mir_model::SemanticAssertionMeterV1<Error = E>,
        mut consume: impl FnMut(
            ProductionOptimizedSourceCfgEventV18<'a, 'g>,
            &mut dyn fe2o3_mir_model::SemanticAssertionMeterV1<Error = E>,
        ) -> Result<(), E>,
    ) -> Result<(), E> {
        meter.charge_work(0)?;
        meter.reserve_storage(std::mem::size_of_val(&consume))?;
        meter.reserve_storage(std::mem::size_of::<ProductionOptimizedSourceCfgEventV18<'_, '_>>())?;
        meter.reserve_storage(3 * std::mem::size_of::<std::ops::Range<usize>>())?;
        for block_ordinal in self.function.blocks.clone() {
            meter.charge_work(1)?;
            let block = &self.inventory.blocks()[block_ordinal];
            let chain = self.rows.blocks[block_ordinal].segments;
            let first = chain.start as usize;
            let end = first + chain.len as usize;
            consume(ProductionOptimizedSourceCfgEventV18::Block {
                actual: block, segments: &self.rows.segments[first..end],
            }, meter)?;
            for operation_ordinal in block.operations.clone() {
                meter.charge_work(1)?;
                let actual = &self.inventory.operations()[operation_ordinal];
                consume(ProductionOptimizedSourceCfgEventV18::Operation {
                    actual,
                    origin: self.rows.operations[operation_ordinal].origin,
                    operands: &self.inventory.uses()[actual.operands.clone()],
                    operand_origins: &self.rows.uses[actual.operands.clone()],
                    effects: &self.inventory.effects()[actual.effects.clone()],
                }, meter)?;
            }
            meter.charge_work(1)?;
            let edges = &self.inventory.edges()[block.edges.clone()];
            let arguments = edges.first().zip(edges.last()).map_or(0..0,
                |(first, last)| first.bindings.start..last.bindings.end);
            consume(ProductionOptimizedSourceCfgEventV18::Terminator {
                actual: block,
                operands: &self.inventory.uses()[block.terminator_uses.clone()],
                operand_origins: &self.rows.uses[block.terminator_uses.clone()],
                edges,
                edge_origins: &self.rows.edges[block.edges.clone()],
                arguments: &self.inventory.edge_arguments()[arguments.clone()],
                argument_origins: &self.rows.edge_arguments[arguments],
            }, meter)?;
        }
        // A callback may swallow a debit error. The source-bound production
        // meter observes its first refusal before this traversal can succeed.
        meter.charge_work(0)
    }
}
