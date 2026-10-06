/// Original static borrow site, not a dynamic scope or execution epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceExecutionBorrowSiteV163 {
    /// Root-relative original call instance.
    pub instance: usize,
    /// Original semantic MIR block.
    pub block: SemanticBlockIdV1,
    /// Original statement ordinal within the block.
    pub statement: usize,
}

/// Inert coordinates copied from an original authenticated execution borrow.
/// These do not prove currentness, exclusivity, convergence, or source values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceExecutionBorrowCoordinatesV163 {
    /// Exact source loan kind, never inferred from the canonical carrier.
    pub kind: SemanticBorrowKindV1,
    /// The original borrow statement that introduced this reference.
    pub site: ProductionSourceExecutionBorrowSiteV163,
    /// The actual parent borrow occurrence for a reborrow, if present.
    pub parent: Option<ProductionSourceExecutionBorrowSiteV163>,
    /// Local read by the borrow statement; a reborrow reads its parent reference.
    pub source_local: SemanticLocalIdV1,
    /// Local defined by the introducing borrow statement.
    pub destination_local: SemanticLocalIdV1,
    /// Exact archived referent type, not a machine pointer pointee inference.
    pub referent_type: SemanticTypeIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceSsaExecutionBorrowV163 {
    value: ValueId,
    role: ExecutionRoleV15,
    owner: ProductionSourceExecutionOwnerV199,
    coordinates: ProductionSourceExecutionBorrowCoordinatesV163,
}

fn source_execution_borrow_site_v163(
    site: SemanticExecutionBorrowOccurrenceV29,
) -> ProductionSourceExecutionBorrowSiteV163 {
    ProductionSourceExecutionBorrowSiteV163 {
        instance: site.instance.index(),
        block: site.block,
        statement: site.statement,
    }
}

fn retain_source_execution_borrow_v163(
    instances: &ExecutionInstancesV29<'_>,
    ty: SemanticTypeIdV1,
    borrow: &SemanticExecutionBorrowBindingV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceSsaExecutionBorrowV163, ProductionSemanticKirErrorV1> {
    budget.charge_work(16)?;
    let semantic = instances.owner().source_semantic();
    borrow
        .check_type(semantic.types(), ty)
        .map_err(source_reference_error_v29)?;
    let instance = instances
        .instance(borrow.occurrence.instance)
        .ok_or_else(source_typed_endpoint_error_v36)?;
    let function = semantic
        .functions()
        .get(instance.function().index() as usize)
        .ok_or_else(source_typed_endpoint_error_v36)?;
    let statement = function
        .blocks()
        .get(borrow.occurrence.block.index() as usize)
        .and_then(|block| block.statements().get(borrow.occurrence.statement))
        .ok_or_else(source_typed_endpoint_error_v36)?;
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Err(source_typed_endpoint_error_v36());
    };
    let SemanticRvalueKindV1::Borrow { kind, place } = assignment.value().kind() else {
        return Err(source_typed_endpoint_error_v36());
    };
    let projected = match borrow.parent {
        None => place.projections().is_empty(),
        Some(parent) => {
            budget.charge_work(4)?;
            instances.instance(parent.instance).is_some()
                && matches!(place.projections(), [projection]
                    if projection.kind() == SemanticProjectionKindV1::Dereference
                        && projection.result_type() == borrow.borrowed.semantic_type())
        }
    };
    if !projected
        || *kind != borrow.kind
        || assignment.destination().ty() != ty
        || assignment.destination().local() != borrow.destination_local
        || !assignment.destination().projections().is_empty()
        || assignment.value().result_type() != ty
        || place.local() != borrow.source_local
        || place.ty() != borrow.borrowed.semantic_type()
    {
        return Err(source_typed_endpoint_error_v36());
    }
    Ok(SourceSsaExecutionBorrowV163 {
        value: borrow.borrowed.identity.value,
        role: semantic_execution_kir_role_v29(borrow.borrowed.role)
            .map_err(source_reference_error_v29)?,
        owner: retain_source_execution_owner_v199(
            instances,
            borrow.borrowed.semantic_type(),
            &borrow.borrowed,
            budget,
        )?,
        coordinates: ProductionSourceExecutionBorrowCoordinatesV163 {
            kind: borrow.kind,
            site: source_execution_borrow_site_v163(borrow.occurrence),
            parent: borrow.parent.map(source_execution_borrow_site_v163),
            source_local: borrow.source_local,
            destination_local: borrow.destination_local,
            referent_type: borrow.borrowed.semantic_type(),
        },
    })
}

impl ProductionSourceSsaEndpointV36<'_, '_> {
    /// Rejoins this exact execution-reference carrier with its original borrow
    /// occurrence. A generic pointer/reference query intentionally returns None.
    pub fn execution_borrow_v163(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceExecutionBorrowCoordinatesV163>> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(2)?;
            match self.physical {
                SourceSsaPhysicalV36::ExecutionBorrow(borrow) => Ok(Some(borrow.coordinates)),
                SourceSsaPhysicalV36::Value { .. }
                | SourceSsaPhysicalV36::Execution(_)
                | SourceSsaPhysicalV36::Witness(_)
                | SourceSsaPhysicalV36::Unit => Ok(None),
                _ => self
                    .owner
                    .source
                    .missing("execution borrow query requires an exact leaf"),
            }
        })())
    }
}

fn source_execution_borrow_headers_v163() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionSemanticKirErrorV1>>())?,
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<SourceSsaExecutionBorrowV163>()?,
        h::<ProductionSourceExecutionBorrowSiteV163>()?,
        h::<ProductionSourceExecutionBorrowCoordinatesV163>()?,
        h::<Option<ProductionSourceExecutionBorrowCoordinatesV163>>()?,
        h::<&SemanticExecutionBorrowBindingV29>()?,
        argument_product_v1(8, h::<&()>()?)?,
    ])
}
