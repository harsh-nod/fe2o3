/// Original producer of a retained execution value. These coordinates do not
/// establish a current dynamic loan, scope epoch, or source/target refinement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceExecutionIdentityV199 {
    /// Root-relative original call instance containing the producer.
    pub instance: usize,
    /// Original block whose call terminator produced the value.
    pub block: SemanticBlockIdV1,
    /// Original destination local at the producer, not a later moved-to local.
    pub destination: SemanticLocalIdV1,
    /// Exact nominal source type of the produced value.
    pub source_type: SemanticTypeIdV1,
    /// Original canonical value, resolved only against the retained owner.
    pub value: ValueId,
}

/// Static identity links retained from the original execution binding.
/// Context, workgroup, and payload links are distinct from dynamic authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceExecutionOwnerV199 {
    /// Exact original nominal role, including tile geometry when applicable.
    pub role: SemanticExecutionRoleV29,
    /// Original producer of this value, before moves and borrows.
    pub identity: ProductionSourceExecutionIdentityV199,
    /// Context producer from which this execution value descends.
    pub context: ProductionSourceExecutionIdentityV199,
    /// Workgroup producer for a workgroup or payload; absent for Context.
    pub workgroup: Option<ProductionSourceExecutionIdentityV199>,
}

fn retain_source_execution_identity_v199(
    instances: &ExecutionInstancesV29<'_>,
    identity: SemanticExecutionIdentityV29,
    role: SemanticExecutionRoleV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ProductionSourceExecutionIdentityV199, ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    let semantic = instances.owner().source_semantic();
    identity
        .check_type(semantic.types(), role)
        .map_err(source_reference_error_v29)?;
    let instance = instances
        .instance(identity.producer.caller)
        .ok_or_else(source_typed_endpoint_error_v36)?;
    let function = semantic
        .functions()
        .get(instance.function().index() as usize)
        .ok_or_else(source_typed_endpoint_error_v36)?;
    let block = function
        .blocks()
        .get(identity.producer.block.index() as usize)
        .ok_or_else(source_typed_endpoint_error_v36)?;
    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
        return Err(source_typed_endpoint_error_v36());
    };
    let destination = call
        .destination()
        .ok_or_else(source_typed_endpoint_error_v36)?
        .place();
    if !destination.projections().is_empty() || destination.ty() != identity.semantic_type {
        return Err(source_typed_endpoint_error_v36());
    }
    Ok(ProductionSourceExecutionIdentityV199 {
        instance: identity.producer.caller.index(),
        block: identity.producer.block,
        destination: destination.local(),
        source_type: identity.semantic_type,
        value: identity.value,
    })
}

fn retain_source_execution_owner_v199(
    instances: &ExecutionInstancesV29<'_>,
    ty: SemanticTypeIdV1,
    binding: &SemanticExecutionBindingV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ProductionSourceExecutionOwnerV199, ProductionSemanticKirErrorV1> {
    budget.charge_work(5)?;
    binding
        .check_type(instances.owner().source_semantic().types(), ty)
        .map_err(source_reference_error_v29)?;
    let identity =
        retain_source_execution_identity_v199(instances, binding.identity, binding.role, budget)?;
    let context = retain_source_execution_identity_v199(
        instances,
        binding.context,
        SemanticExecutionRoleV29::KernelContext,
        budget,
    )?;
    let workgroup = binding
        .workgroup
        .map(|identity| {
            retain_source_execution_identity_v199(
                instances,
                identity,
                SemanticExecutionRoleV29::Workgroup,
                budget,
            )
        })
        .transpose()?;
    let shape = match binding.role {
        SemanticExecutionRoleV29::KernelContext => identity == context && workgroup.is_none(),
        SemanticExecutionRoleV29::Workgroup => workgroup == Some(identity),
        SemanticExecutionRoleV29::MaskedTileU32 { .. }
        | SemanticExecutionRoleV29::LaneFragmentU32 { .. } => workgroup.is_some(),
    };
    if !shape {
        return Err(source_typed_endpoint_error_v36());
    }
    Ok(ProductionSourceExecutionOwnerV199 {
        role: binding.role,
        identity,
        context,
        workgroup,
    })
}

impl ProductionSourceSsaEndpointV36<'_, '_> {
    /// Returns original producer links for an owned or borrowed execution value.
    /// This does not assert dynamic loan currentness or target scope authority.
    pub fn execution_owner_v199(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceExecutionOwnerV199>> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(2)?;
            match self.physical {
                SourceSsaPhysicalV36::Execution(owner) => Ok(Some(*owner)),
                SourceSsaPhysicalV36::ExecutionBorrow(borrow) => Ok(Some(borrow.owner)),
                SourceSsaPhysicalV36::Value { .. }
                | SourceSsaPhysicalV36::Witness(_)
                | SourceSsaPhysicalV36::Unit => Ok(None),
                _ => self
                    .owner
                    .source
                    .missing("execution owner query requires an exact leaf"),
            }
        })())
    }
}

fn source_execution_owner_headers_v199() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionSemanticKirErrorV1>>())?,
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<ProductionSourceExecutionIdentityV199>()?,
        h::<ProductionSourceExecutionOwnerV199>()?,
        h::<Option<ProductionSourceExecutionIdentityV199>>()?,
        h::<Option<ProductionSourceExecutionOwnerV199>>()?,
        argument_product_v1(8, h::<&()>()?)?,
    ])
}
