/// Complete source-block coverage, authenticated by the constructor's N replay.
/// Every statement span (including zero-length spans), terminator span and
/// synthetic span was checked against the exact original block before this view
/// was sealed. This record describes that entire block, not one omitted access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceOutputBlockCoverageV1 {
    disposition: ProductionSourceOutputBlockV1,
    source_statements: usize,
    original_operations: Option<usize>,
}

impl ProductionSourceOutputBlockCoverageV1 {
    /// Original absence, checked execution, and optional O placement stay distinct.
    pub const fn disposition(self) -> ProductionSourceOutputBlockV1 {
        self.disposition
    }

    /// Every source statement is included, even if it lowered to no operations.
    pub const fn source_statements(self) -> usize {
        self.source_statements
    }

    /// Complete original N operation count, including synthetic prefix operations.
    /// Original-absent source blocks have no original operation coordinate or count.
    pub const fn original_operations(self) -> Option<usize> {
        self.original_operations
    }
}

impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    /// Queries complete replayed source coverage without creating an operation
    /// index, inferring an O ordinal, or granting physical/formal authority.
    pub fn block_coverage(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<ProductionSourceOutputBlockCoverageV1, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(4).map_err(Error::Resource)?;
        let minimum = self
            .source
            .executable_storage()
            .retained_storage()
            .checked_add(self.source.assert_origin_storage().payload_storage())
            .and_then(|n| n.checked_add(self.checked_output.storage().retained_storage()))
            .and_then(|n| n.checked_add(self.storage.retained_storage()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        if budget.storage() < minimum {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        let disposition = self.block(owner, function, block, budget)?;
        budget.charge_work(3).map_err(Error::Resource)?;
        let source = self
            .source
            .semantic_ssa()
            .source_semantic()
            .functions()
            .get(function.index() as usize)
            .and_then(|function| function.blocks().get(block.index() as usize))
            .ok_or(Error::Invalid("coverage source block is absent"))?;
        let original_operations = match disposition {
            ProductionSourceOutputBlockV1::NotMaterialized => None,
            ProductionSourceOutputBlockV1::Materialized { original, .. } => {
                budget.charge_work(5).map_err(Error::Resource)?;
                let original_block = self
                    .source
                    .executable()
                    .module()
                    .functions
                    .get(original.function.0 as usize)
                    .and_then(|function| function.body.as_ref())
                    .and_then(|body| body.blocks.get(original.block as usize))
                    .ok_or(Error::Invalid("coverage original block is absent"))?;
                if original_block.id != BlockId(block.index()) {
                    return Err(Error::Invalid(
                        "coverage original block differs from source",
                    ));
                }
                Some(original_block.operations.len())
            }
        };
        Ok(ProductionSourceOutputBlockCoverageV1 {
            disposition,
            source_statements: source.statements().len(),
            original_operations,
        })
    }
}
