// Pending source/output correspondence only. A native graph/epoch and complete
// bounds, alias, initialization and concurrency proofs remain independent.
use fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1;
include!("production_source_global_guard_v85.rs");
include!("production_source_global_guard_v86.rs");
include!("production_optimized_source_global_native_v18.rs");
include!("production_source_native_writes_v88.rs");
include!("production_source_predicated_roles_v89.rs");
include!("production_optimized_source_global_read_conditions_v18.rs");
#[cfg(test)]
include!("production_optimized_source_global_access_queries_v18_tests.rs");
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GlobalSourceLogicalEndpointV18 {
    access: SliceAccess,
    root: SliceDefinition,
    index: SliceDefinition,
    data: SliceOperation,
    length: SliceOperation,
    address: SliceOperation,
    guard: GlobalSourceGuardV85,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GlobalSourceAccessOriginV18 {
    Assertion(ProductionSliceAccessSiteV1),
    Issued {
        instance: usize,
        definition: SsaValueV1,
    },
    SourceWriteV89 {
        instance: usize,
        anchor: usize,
    },
}

// Issued rows cannot escape their constructor until complete original/output
// pointer transport replay has established the exact Global issuer. This is a
// representation gate only; native domains independently rejoin that origin.
fn global_source_memory_space_v26(
    origin: &GlobalSourceAccessOriginV18,
    space: AddressSpace,
) -> bool {
    space == AddressSpace::Global
        || (space == AddressSpace::Generic
            && matches!(origin, GlobalSourceAccessOriginV18::Issued { .. }))
}

#[cfg(test)]
include!("production_source_generic_global_domains_v26_tests.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GlobalSourceAccessEndpointV18 {
    logical: GlobalSourceLogicalEndpointV18,
    // The checked address operand may be an Index representation of logical.index.
    address_index: SliceDefinition,
    // Exact GEP result, distinct from the actual access after issued transport.
    formation_pointer: ValueId,
    pointer: ValueId,
    value: ValueId,
    memory: MemoryAccess,
    scalar: ScalarType,
    writing: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GlobalSourceAccessPairV18 {
    instance: usize,
    origin: GlobalSourceAccessOriginV18,
    input: GlobalSourceAccessEndpointV18,
    output: GlobalSourceAccessEndpointV18,
}

// Private lexical input for a future native-family join, never a completed
// memory-family token. The underlying descriptor scope owns every row.
pub(super) struct PendingGlobalSourceAccessesV18<'s> {
    roles: &'s CheckedDescriptorSourceRolesV18<'s>,
}

impl PendingGlobalSourceAccessesV18<'_> {
    pub(super) fn original(&self) -> &ProductionSourceCorrespondenceV18<'_> {
        self.roles.original
    }

    pub(super) fn optimized(&self) -> &ProductionOptimizedSourceCorrespondenceV18<'_> {
        self.roles.optimized
    }

    pub(super) fn root(&self) -> usize {
        self.roles.root
    }

    pub(super) fn operation_count(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.roles.check(budget)?;
        Ok(self.roles.rows.len())
    }

    fn access(
        &self,
        operation: SliceOperation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&GlobalSourceAccessPairV18>> {
        self.roles.check(budget)?;
        self.roles.original.retain_query((|| {
            let index = descriptor_role_index_v18(self.roles.rows, operation, budget)?;
            budget.charge_work(48)?;
            let row = self
                .roles
                .rows
                .get(index)
                .filter(|row| row.output == operation)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "pending global access changed actual occurrence",
                ))?;
            if row.write_recipe_pending
                || !matches!(
                    row.role,
                    Some(DescriptorSourceRoleV18::Read | DescriptorSourceRoleV18::Write)
                )
            {
                return Ok(None);
            }
            let Some(pair) = row.global.as_ref() else {
                return Ok(None);
            };
            if row.input != Some(pair.input.logical.access.operation)
                || row.instance != Some(pair.instance)
                || pair.output.logical.access.operation != row.output
                || pair.output.writing != matches!(row.role, Some(DescriptorSourceRoleV18::Write))
            {
                return self
                    .roles
                    .original
                    .source
                    .missing("pending global access lost its checked source role");
            }
            Ok(Some(pair))
        })())
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    pub(super) fn with_pending_global_accesses_v18<'work, T, E>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &PendingGlobalSourceAccessesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_descriptor_source_roles_namespace_v18(
            optimized,
            root,
            None,
            budget,
            |roles, budget| consume(&PendingGlobalSourceAccessesV18 { roles }, budget),
        )
    }
}

fn global_source_guard_definition_v18(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    coordinate: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<SliceDefinition> {
    let row = source_block_row_v18(inventory, coordinate, budget)?;
    budget.charge_work(8)?;
    let value = match row.block.terminator.as_ref() {
        Some(Terminator::ConditionalBranch { condition, .. }) => *condition,
        Some(Terminator::Switch { selector, .. } | Terminator::IntegerSwitch { selector, .. }) => {
            *selector
        }
        _ => {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "pending global guard kind",
            ));
        }
    };
    Ok(inventory
        .definition_for_value(coordinate.function, value, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "pending global guard definition",
        ))?
        .coordinate)
}

fn global_source_endpoint_v18(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    logical: GlobalSourceLogicalEndpointV18,
    origin: &GlobalSourceAccessOriginV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<GlobalSourceAccessEndpointV18>> {
    // V18 source recipes remain CFG-only until an issued-write constructor
    // authenticates every explicit-predicate input/output field.
    let guard = logical.guard.require_cfg_v26()?;
    budget.charge_work(32 + 4)?;
    let row = source_operation_row_v18(inventory, logical.access.operation, budget)?;
    let (pointer, value, memory, scalar, writing) = match &row.operation.kind {
        OperationKind::Load { pointer, access } => {
            let [result] = row.operation.results.as_slice() else {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "pending global read result census",
                ));
            };
            let Some(scalar) = result.ty.as_scalar() else {
                return Ok(None);
            };
            (*pointer, result.id, *access, scalar, false)
        }
        OperationKind::Store {
            pointer,
            value,
            access,
        } if row.operation.results.is_empty() => {
            let value_type = inventory
                .definition_for_value(logical.access.operation.block.function, *value, budget)
                .map_err(source_pointer_inventory_error_v18)?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "pending global store value definition",
                ))?;
            let Some(scalar) = value_type.ty.as_scalar() else {
                return Ok(None);
            };
            (*pointer, *value, *access, scalar, true)
        }
        _ => {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "pending global access opcode",
            ));
        }
    };
    if !global_source_memory_space_v26(origin, memory.address_space) || memory.volatile {
        return Ok(None);
    }
    if logical.access.effect != 0
        || row.effects.len() != 1
        || guard.edge.source.function != logical.access.operation.block.function
    {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "pending global access effect or guard owner",
        ));
    }
    budget.charge_work(8 + 4)?;
    let address = source_operation_row_v18(inventory, logical.address, budget)?;
    let OperationKind::GetElementPointer { offset, .. } = &address.operation.kind else {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "pending global checked address is not a GEP",
        ));
    };
    let formation_pointer = match address.operation.results.as_slice() {
        [result] => result.id,
        _ => {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "pending global address formation result census",
            ));
        }
    };
    // Issued rows become visible only after complete original/output typed
    // pointer transport replay. Assertion rows retain their direct-pointer rule.
    if matches!(origin, GlobalSourceAccessOriginV18::Assertion(_)) && formation_pointer != pointer {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "pending global assertion changed direct address pointer",
        ));
    }
    let address_index = inventory
        .definition_for_value(logical.access.operation.block.function, *offset, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "pending global exact address index definition",
        ))?
        .coordinate;
    Ok(Some(GlobalSourceAccessEndpointV18 {
        logical,
        address_index,
        formation_pointer,
        pointer,
        value,
        memory,
        scalar,
        writing,
    }))
}

fn install_global_source_access_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    rows: &mut [DescriptorSourceRoleRowV18],
    instance: usize,
    origin: GlobalSourceAccessOriginV18,
    before: GlobalSourceLogicalEndpointV18,
    after: GlobalSourceLogicalEndpointV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let Some(input) = global_source_endpoint_v18(original.inventory, before, &origin, budget)?
    else {
        return Ok(());
    };
    let Some(output) =
        global_source_endpoint_v18(optimized.output_inventory(budget)?, after, &origin, budget)?
    else {
        return Ok(());
    };
    // Includes the complete fixed-shape pair comparison on an already-filled
    // row; the pair contains only bounded coordinate/value/enum scalars.
    budget.charge_work(160)?;
    if input.scalar != output.scalar
        || input.memory != output.memory
        || input.writing != output.writing
    {
        return original
            .source
            .missing("pending global source/output access differs");
    }
    let pair = GlobalSourceAccessPairV18 {
        instance,
        origin,
        input,
        output,
    };
    let index = descriptor_role_index_v18(rows, after.access.operation, budget)?;
    let row = rows
        .get_mut(index)
        .filter(|row| row.output == after.access.operation)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "pending global output row missing",
        ))?;
    if row.global.is_some_and(|existing| existing != pair) {
        return original
            .source
            .missing("pending global access has conflicting checked recipes");
    }
    row.global = Some(pair);
    Ok(())
}

fn global_source_headers_v18() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<PendingGlobalSourceAccessesV18<'_>>()?,
        h::<&PendingGlobalSourceAccessesV18<'_>>()?,
        h::<&CheckedDescriptorSourceRolesV18<'_>>()?,
        h::<&mut ArgumentBudgetV1<'_>>()?,
        h::<(
            &PendingGlobalSourceAccessesV18<'_>,
            SliceOperation,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(
            &PendingGlobalSourceAccessesV18<'_>,
            &SliceOperation,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(&&DescriptorSourceRoleRowV18, &SliceOperation)>()?,
        h::<(&&mut DescriptorSourceRoleRowV18, &SliceOperation)>()?,
        h::<(
            &ProductionSourceCorrespondenceV18<'_>,
            &ProductionOptimizedSourceCorrespondenceV18<'_>,
            &mut [DescriptorSourceRoleRowV18],
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(
            &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
            GlobalSourceLogicalEndpointV18,
            &GlobalSourceAccessOriginV18,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(&GlobalSourceAccessOriginV18, AddressSpace, bool)>()?,
        h::<(
            &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
            fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<Option<&GlobalSourceAccessPairV18>>()?,
        h::<&GlobalSourceAccessPairV18>()?,
        h::<&DescriptorSourceRoleRowV18>()?,
        h::<Option<&DescriptorSourceRoleRowV18>>()?,
        h::<&mut DescriptorSourceRoleRowV18>()?,
        h::<Option<&mut DescriptorSourceRoleRowV18>>()?,
        h::<Option<GlobalSourceAccessPairV18>>()?,
        argument_product_v1(2, h::<GlobalSourceAccessPairV18>()?)?,
        argument_product_v1(3, h::<GlobalSourceLogicalEndpointV18>()?)?,
        argument_product_v1(3, h::<GlobalSourceAccessEndpointV18>()?)?,
        argument_product_v1(2, h::<Option<GlobalSourceAccessEndpointV18>>()?)?,
        h::<GlobalSourceAccessOriginV18>()?,
        h::<GlobalSourceGuardV85>()?,
        h::<GlobalSourceCfgGuardV85>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>>()?,
        h::<&Option<Terminator>>()?,
        h::<Option<&Terminator>>()?,
        h::<&Terminator>()?,
        h::<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>()?,
        h::<fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1>()?,
        h::<SliceDefinition>()?,
        h::<Result<u32, std::num::TryFromIntError>>()?,
        h::<u32>()?,
        h::<&CanonicalKirOperationRefV1<'_>>()?,
        // Address lookup and exact offset definition coexist with the access row.
        h::<&CanonicalKirOperationRefV1<'_>>()?,
        h::<&OperationKind>()?,
        h::<&ValueId>()?,
        h::<SliceDefinition>()?,
        h::<Option<&CanonicalKirDefinitionRefV1<'_>>>()?,
        h::<&CanonicalKirDefinitionRefV1<'_>>()?,
        size_of::<
            Result<
                Option<&CanonicalKirDefinitionRefV1<'_>>,
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
            >,
        >(),
        h::<&CanonicalKirDefinitionRefV1<'_>>()?,
        h::<Option<&CanonicalKirDefinitionRefV1<'_>>>()?,
        size_of::<
            Result<
                Option<&CanonicalKirDefinitionRefV1<'_>>,
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
            >,
        >(),
        h::<&fe2o3_kernel_ir::Operation>()?,
        h::<&OperationKind>()?,
        h::<&[fe2o3_kernel_ir::ValueDef]>()?,
        h::<&fe2o3_kernel_ir::ValueDef>()?,
        h::<&Type>()?,
        argument_product_v1(2, h::<&ValueId>()?)?,
        h::<&MemoryAccess>()?,
        h::<Option<ScalarType>>()?,
        h::<(ValueId, ValueId, MemoryAccess, ScalarType, bool)>()?,
        argument_product_v1(4, h::<ValueId>()?)?,
        h::<&fe2o3_kernel_ir::ValueDef>()?,
        h::<MemoryAccess>()?,
        h::<ScalarType>()?,
        h::<bool>()?,
        argument_product_v1(2, h::<usize>()?)?,
        h::<()>()?,
    ])
}
