/// Physical operation class whose original source obligations survive optimization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionCanonicalPrivateOperationKindV1 {
    /// A retained private allocation, not a promotion or storage-elision grant.
    Allocation,
    /// A fixed-element private address computation.
    Address,
    /// An ordinary private read.
    Load,
    /// An ordinary private write.
    Store,
    /// An actual call, including separately checked assertion trap calls.
    Call,
}
fn cpc_kind_v1(kind: &OperationKind) -> Option<ProductionCanonicalPrivateOperationKindV1> {
    use ProductionCanonicalPrivateOperationKindV1 as K;
    match kind {
        OperationKind::Alloca { .. } => Some(K::Allocation),
        OperationKind::GetElementPointer { .. } => Some(K::Address),
        OperationKind::Load { .. } => Some(K::Load),
        OperationKind::Store { .. } => Some(K::Store),
        OperationKind::Call { .. } => Some(K::Call),
        _ => None,
    }
}

/// Complete original root-qualified alias, not a transferable source proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCanonicalPrivateSourceAliasV1 {
    operation: usize,
    association: usize,
    span: usize,
}
impl ProductionCanonicalPrivateSourceAliasV1 {
    /// Physical occurrence ordinal in the enclosing private-operation roster.
    pub const fn operation(&self) -> usize {
        self.operation
    }
    /// Original root-qualified source association, retained even when removed.
    pub const fn association(&self) -> usize {
        self.association
    }
    /// Original complete source-span ordinal, not a final graph coordinate.
    pub const fn span(&self) -> usize {
        self.span
    }
}

struct CpcOriginalOperationV1 {
    coordinate: CsOperationV1,
    kind: ProductionCanonicalPrivateOperationKindV1,
    aliases: std::ops::Range<usize>,
}
struct CpcOriginsV1<'s, 'm> {
    source: &'s ProductionCanonicalRankedMetadataV1<'m>,
    operations: Vec<CpcOriginalOperationV1>,
    aliases: Vec<ProductionCanonicalPrivateSourceAliasV1>,
    calls: Vec<ProductionCanonicalPrivateCallSiteV1>,
    index: CpcCallIndexV1,
}
impl<'s, 'm> CpcOriginsV1<'s, 'm> {
    fn derive(
        source: &'s ProductionCanonicalRankedMetadataV1<'m>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<Self> {
        source.guard.query(budget)?;
        let mut count = 0usize;
        let mut aliases = 0usize;
        for (ordinal, operation) in source.inventory.operations().iter().enumerate() {
            budget.charge_work(2)?;
            if cpc_kind_v1(&operation.operation.kind).is_some() {
                count = argument_sum_v1(&[count, 1])?;
                aliases =
                    argument_sum_v1(&[aliases, source.source.operation_origins[ordinal].len()])?;
            }
        }
        budget.reserve_storage(std::mem::size_of::<Self>())?;
        let mut result = Self {
            source,
            operations: cs_vec_v1(count, budget)?,
            aliases: cs_vec_v1(aliases, budget)?,
            calls: cs_vec_v1(source.calls.call_count(), budget)?,
            index: CpcCallIndexV1::new(source, count, budget)?,
        };
        for (ordinal, operation) in source.inventory.operations().iter().enumerate() {
            budget.charge_work(2)?;
            let Some(kind) = cpc_kind_v1(&operation.operation.kind) else {
                continue;
            };
            let start = result.aliases.len();
            budget.charge_work(1)?;
            result.index.operations[ordinal] = Some(result.operations.len());
            for origin in &source.source.origins[source.source.operation_origins[ordinal].clone()] {
                cs_push_v1(
                    &mut result.aliases,
                    ProductionCanonicalPrivateSourceAliasV1 {
                        operation: result.operations.len(),
                        association: origin.association,
                        span: origin.span,
                    },
                    budget,
                )?;
            }
            cs_push_v1(
                &mut result.operations,
                CpcOriginalOperationV1 {
                    coordinate: operation.coordinate,
                    kind,
                    aliases: start..result.aliases.len(),
                },
                budget,
            )?;
        }
        cpc_original_calls_v1(&mut result, budget)?;
        result.check(source, budget)?;
        Ok(result)
    }
    fn check(
        &self,
        source: &ProductionCanonicalRankedMetadataV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<()> {
        source.guard.query(budget)?;
        budget.charge_work(1)?;
        if !std::ptr::eq(self.source, source) {
            return Err(ProductionCanonicalScalarSourceErrorV1::InputCustody);
        }
        self.index.check_associations(source, budget)?;
        budget.charge_work(1)?;
        if self.index.operations.len() != source.inventory.operations().len() {
            return Err(cs_invalid_v1("complete private operation inverse"));
        }
        let mut physical = 0;
        let mut alias = 0;
        for (ordinal, operation) in source.inventory.operations().iter().enumerate() {
            budget.charge_work(3)?;
            let expected = cpc_kind_v1(&operation.operation.kind).map(|_| physical);
            if self.index.operations[ordinal] != expected {
                return Err(cs_invalid_v1("private operation inverse identity"));
            }
            let Some(kind) = cpc_kind_v1(&operation.operation.kind) else {
                continue;
            };
            let row = self
                .operations
                .get(physical)
                .ok_or_else(|| cs_invalid_v1("missing private source operation"))?;
            let originals =
                &source.source.origins[source.source.operation_origins[ordinal].clone()];
            if row.coordinate != operation.coordinate
                || row.kind != kind
                || row.aliases.start != alias
                || row.aliases.len() != originals.len()
            {
                return Err(cs_invalid_v1("private source operation/alias roster"));
            }
            for original in originals {
                budget.charge_work(4)?;
                let expected = ProductionCanonicalPrivateSourceAliasV1 {
                    operation: physical,
                    association: original.association,
                    span: original.span,
                };
                if self.aliases.get(alias) != Some(&expected) {
                    return Err(cs_invalid_v1("complete private source alias identity"));
                }
                alias = argument_sum_v1(&[alias, 1])?;
            }
            physical = argument_sum_v1(&[physical, 1])?;
        }
        if physical != self.operations.len() || alias != self.aliases.len() {
            return Err(cs_invalid_v1("extra private source operations/aliases"));
        }
        cpc_check_original_calls_v1(self, budget)
    }
}

/// One original private/call occurrence and its exact current disposition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCanonicalPrivateOperationV1 {
    original: CsOperationV1,
    kind: ProductionCanonicalPrivateOperationKindV1,
    current: Option<CsOperationV1>,
    removed: Option<ProductionCanonicalScalarAssertionStepV1>,
}
impl ProductionCanonicalPrivateOperationV1 {
    /// Exact original N coordinate.
    pub const fn original(&self) -> CsOperationV1 {
        self.original
    }
    /// Original operation class; removal never upgrades its source obligations.
    pub const fn kind(&self) -> ProductionCanonicalPrivateOperationKindV1 {
        self.kind
    }
    /// Exact final F occurrence when retained.
    pub const fn current(&self) -> Option<CsOperationV1> {
        self.current
    }
    /// First checked unreachable-removal event; not inferred from final absence.
    pub const fn removal(&self) -> Option<ProductionCanonicalScalarAssertionStepV1> {
        self.removed
    }
}

// Only this module constructs this capability from complete checked final lineage.
// The sibling lifetime reader can inspect it but cannot attach caller-supplied rows.
pub(super) struct CpcSiteViewV1<'s, 'm, 'g> {
    source: &'s ProductionCanonicalRankedMetadataV1<'m>,
    output: &'s CanonicalKirInventoryV1<'g>,
    originals: Vec<Option<usize>>,
}
impl<'s, 'm, 'g> CpcSiteViewV1<'s, 'm, 'g> {
    fn derive(
        source: &'s ProductionCanonicalRankedMetadataV1<'m>,
        output: &'s CanonicalKirInventoryV1<'g>,
        lineage: &CsLineageV1,
        transport: &CpcTransportV1<'_, '_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<Self> {
        transport.origins.check(source, budget)?;
        transport.check_current(output, lineage, budget)?;
        budget.reserve_storage(std::mem::size_of::<Self>())?;
        let mut originals = cs_vec_v1(output.operations().len(), budget)?;
        for (operation, origin) in output.operations().iter().zip(&lineage.operations) {
            budget.charge_work(2)?;
            let original = match origin {
                ProductionCanonicalScalarOperationOriginV1::Original(coordinate) => {
                    Some(cs_operation_v1(source.inventory, *coordinate, budget)?)
                }
                ProductionCanonicalScalarOperationOriginV1::SynthesizedConstant { .. } => {
                    if !matches!(operation.operation.kind, OperationKind::Constant(_)) {
                        return Err(cs_invalid_v1("synthesized private source site"));
                    }
                    None
                }
            };
            cs_push_v1(&mut originals, original, budget)?;
        }
        if originals.len() != output.operations().len() {
            return Err(cs_invalid_v1("complete final source-site roster"));
        }
        Ok(Self {
            source,
            output,
            originals,
        })
    }
    pub(super) fn source(&self) -> &ProductionCanonicalRankedMetadataV1<'m> {
        self.source
    }
    pub(super) fn inventory(&self) -> &CanonicalKirInventoryV1<'g> {
        self.output
    }
    pub(super) fn original_operation(&self, ordinal: usize) -> Option<usize> {
        self.originals.get(ordinal).copied().flatten()
    }
}
