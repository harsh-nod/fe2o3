//! One actual typed SSA producer per retained ranked read site.

use super::super::{ProductionSemanticLoadV2, ProductionSemanticReadModeV2};
use super::*;
use dialect_kernel::{SemanticReadOrderingAttr, SemanticReadVolatilityAttr, SemanticTypedReadOp};

struct ReadSite<'a> {
    load: &'a ProductionSemanticLoadV2,
    result: Option<Value>,
    consumers: Vec<(u32, u32)>,
}

pub(super) struct RankedSemanticReadsV1<'a> {
    sites: BTreeMap<(u32, u32), ReadSite<'a>>,
    work: usize,
}

impl<'a> RankedSemanticReadsV1<'a> {
    pub(super) fn new(
        kernel: &'a ProductionRankedKernelV1,
    ) -> Result<Self, ProductionRankedKernelErrorV1> {
        let mut reads = Self {
            sites: BTreeMap::new(),
            work: 0,
        };
        for (block, recipe) in kernel.blocks.iter().enumerate() {
            for (operation, op) in recipe.operations.iter().enumerate() {
                reads.charge()?;
                if let ProductionRankedOperationV1::SemanticExpression { expression, .. } = op {
                    expression
                        .validate()
                        .map_err(ProductionRankedKernelErrorV1::InvalidSemanticExpression)?;
                    reads.collect(expression, block as u32, operation as u32)?;
                }
            }
        }
        Ok(reads)
    }

    fn charge(&mut self) -> Result<(), ProductionRankedKernelErrorV1> {
        self.work = self
            .work
            .checked_add(1)
            .ok_or_else(|| Self::limit(usize::MAX))?;
        if self.work > MAX_RANKED_BOUNDS_OPERATIONS {
            return Err(Self::limit(self.work));
        }
        Ok(())
    }

    fn limit(actual: usize) -> ProductionRankedKernelErrorV1 {
        ProductionRankedKernelErrorV1::ResourceLimit {
            resource: "semantic read materialization work",
            limit: MAX_RANKED_BOUNDS_OPERATIONS,
            actual,
        }
    }

    fn collect(
        &mut self,
        expression: &'a ProductionSemanticExpressionV2,
        block: u32,
        operation: u32,
    ) -> Result<(), ProductionRankedKernelErrorV1> {
        self.charge()?;
        match expression {
            ProductionSemanticExpressionV2::Load(load) => {
                // Construction readiness does not prove cross-block dominance.
                // The completed native CFG and live memory proof check that.
                if load.block == block && load.operation >= operation {
                    return Err(reject(
                        "semantic read does not precede its consumer in the same block",
                    ));
                }
                let site = (load.block, load.operation);
                if let Some(previous) = self.sites.get_mut(&site) {
                    if previous.load != load {
                        return Err(reject("one source read site has conflicting load metadata"));
                    }
                    previous.consumers.push((block, operation));
                } else {
                    self.sites.insert(
                        site,
                        ReadSite {
                            load,
                            result: None,
                            consumers: vec![(block, operation)],
                        },
                    );
                }
                Ok(())
            }
            ProductionSemanticExpressionV2::Unary { operand, .. }
            | ProductionSemanticExpressionV2::Cast { operand, .. } => {
                self.collect(operand, block, operation)
            }
            ProductionSemanticExpressionV2::Binary { lhs, rhs, .. }
            | ProductionSemanticExpressionV2::Compare { lhs, rhs, .. } => {
                self.collect(lhs, block, operation)?;
                self.collect(rhs, block, operation)
            }
            ProductionSemanticExpressionV2::Select {
                condition,
                when_true,
                when_false,
                ..
            } => {
                self.collect(condition, block, operation)?;
                self.collect(when_true, block, operation)?;
                self.collect(when_false, block, operation)
            }
            ProductionSemanticExpressionV2::Symbol { .. }
            | ProductionSemanticExpressionV2::Constant { .. } => Ok(()),
        }
    }

    /// Called immediately after the original ranked operation is appended.
    /// The original access remains the single event consumed by memory checks.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn emit_after(
        &mut self,
        context: &mut pliron::context::Context,
        block: Ptr<BasicBlock>,
        source_site: (u32, u32),
        operation: Ptr<Operation>,
        arguments: &[Value],
        locals: &RankedLocalValuesV1,
        block_arguments: &HashMap<(u32, u32), Value>,
    ) -> Result<(), ProductionRankedKernelErrorV1> {
        let Some(site) = self.sites.get_mut(&source_site) else {
            return Ok(());
        };
        if site.result.is_some() {
            return Err(reject("semantic read site was materialized more than once"));
        }
        let load = site.load;
        let view = resolve_value(load.view, arguments, locals, block_arguments)?;
        let indices = load
            .indices
            .iter()
            .map(|value| resolve_value(*value, arguments, locals, block_arguments))
            .collect::<Result<Vec<_>, _>>()?;
        let raw = Operation::get_op_dyn(operation, context);
        let access = raw
            .downcast_ref::<RankedAccessOp>()
            .ok_or_else(|| reject("semantic read site is not the original ranked access"))?;
        if access.kind(context) != Some(AccessKindAttr::Read)
            || access.view(context) != view
            || access.indices(context) != indices
            || access.atomic_ordering(context).is_some()
            || access.atomic_scope(context).is_some()
            || access.checked_success(context).is_some()
            || operation.deref(context).get_num_operands() != indices.len() + 1
        {
            return Err(reject(
                "semantic read changed its exact access, view, indices or ordering",
            ));
        }
        let definition = Operation::get_op_dyn(
            view.defining_op()
                .ok_or_else(|| reject("semantic read view has no definition"))?,
            context,
        );
        let view_op = definition
            .downcast_ref::<RankedViewOp>()
            .ok_or_else(|| reject("semantic read view is not an allocation-backed ranked view"))?;
        let scalar = typed_scalar(load.scalar)?;
        if load.allocation_origin == 0
            || view_op.allocation_origin(context) != Some(load.allocation_origin)
            || view_op.memory_space(context) != Some(MemorySpaceAttr::Global)
        {
            return Err(reject("semantic read lost its exact Global allocation"));
        }
        let volatility = match load.read_mode {
            ProductionSemanticReadModeV2::UnorderedNonVolatile => {
                SemanticReadVolatilityAttr::NonVolatile
            }
            ProductionSemanticReadModeV2::UnorderedVolatile => SemanticReadVolatilityAttr::Volatile,
        };
        let producer = SemanticTypedReadOp::new(
            context,
            load.proof_symbol(),
            scalar,
            MemorySpaceAttr::Global,
            volatility,
            SemanticReadOrderingAttr::Unordered,
            view,
            indices,
            None,
        )
        .map_err(|_| reject("exact semantic read producer failed validation"))?;
        producer.get_operation().insert_at_back(block, context);
        site.result = Some(producer.result(context));
        Ok(())
    }

    pub(super) fn resolve(
        &self,
        load: &ProductionSemanticLoadV2,
    ) -> Result<Value, ProductionRankedKernelErrorV1> {
        self.sites
            .get(&(load.block, load.operation))
            .filter(|site| site.load == load)
            .and_then(|site| site.result)
            .ok_or_else(|| reject("semantic load has no dominating exact source SSA producer"))
    }

    pub(super) fn loads(&self) -> impl Iterator<Item = &'a ProductionSemanticLoadV2> + '_ {
        self.sites.values().map(|site| site.load)
    }

    pub(super) fn work(&self) -> usize {
        self.work
    }

    pub(super) fn dependencies(&self) -> impl Iterator<Item = ((u32, u32), (u32, u32))> + '_ {
        self.sites
            .iter()
            .flat_map(|(source, read)| read.consumers.iter().map(|consumer| (*source, *consumer)))
    }

    pub(super) fn finish(&self) -> Result<(), ProductionRankedKernelErrorV1> {
        if self.sites.values().any(|site| site.result.is_none()) {
            return Err(reject(
                "semantic read roster contains an unmaterialized source site",
            ));
        }
        Ok(())
    }
}

fn reject(reason: &'static str) -> ProductionRankedKernelErrorV1 {
    ProductionRankedKernelErrorV1::Materialization(reason)
}

#[cfg(test)]
#[path = "semantic_reads_v1/tests.rs"]
mod tests;
