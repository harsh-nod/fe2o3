// Exact source SSA carriers retained before the original emission archive dies.
// These locators do not establish pointer validity, aliasing, or heap equality.
include!("production_source_carrier_tree_v37.rs");
include!("production_source_reference_endpoints_v38.rs");
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceSsaElementV36 {
    Scalar(ScalarType),
    Vector(fe2o3_kernel_ir::FixedVectorTypeV12),
    Storage(fe2o3_kernel_ir::StorageLayoutIdV1),
}

impl SourceSsaElementV36 {
    fn from_type(ty: &Type) -> Option<Self> {
        match ty {
            Type::Scalar(scalar) => Some(Self::Scalar(*scalar)),
            Type::Vector(vector) => Some(Self::Vector(*vector)),
            Type::StorageObject(layout) => Some(Self::Storage(*layout)),
            _ => None,
        }
    }

    fn matches(self, ty: &Type) -> bool {
        Self::from_type(ty) == Some(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceSsaCarrierTypeV36 {
    Scalar(ScalarType),
    Vector(fe2o3_kernel_ir::FixedVectorTypeV12),
    Pointer {
        element: SourceSsaElementV36,
        space: fe2o3_kernel_ir::AddressSpace,
        access: fe2o3_kernel_ir::AccessMode,
    },
    Slice {
        element: SourceSsaElementV36,
        space: fe2o3_kernel_ir::AddressSpace,
        access: fe2o3_kernel_ir::AccessMode,
    },
}

impl SourceSsaCarrierTypeV36 {
    fn from_type(ty: &Type) -> Option<Self> {
        match ty {
            Type::Scalar(scalar) => Some(Self::Scalar(*scalar)),
            Type::Vector(vector) => Some(Self::Vector(*vector)),
            Type::Pointer(pointer) => Some(Self::Pointer {
                element: SourceSsaElementV36::from_type(&pointer.pointee)?,
                space: pointer.address_space,
                access: pointer.access,
            }),
            Type::Slice(slice) => Some(Self::Slice {
                element: SourceSsaElementV36::from_type(&slice.element)?,
                space: slice.address_space,
                access: slice.access,
            }),
            _ => None,
        }
    }

    fn matches(self, ty: &Type) -> bool {
        match (self, ty) {
            (Self::Scalar(expected), Type::Scalar(actual)) => expected == *actual,
            (Self::Vector(expected), Type::Vector(actual)) => expected == *actual,
            (
                Self::Pointer {
                    element,
                    space,
                    access,
                },
                Type::Pointer(actual),
            ) => {
                element.matches(&actual.pointee)
                    && space == actual.address_space
                    && access == actual.access
            }
            (
                Self::Slice {
                    element,
                    space,
                    access,
                },
                Type::Slice(actual),
            ) => {
                element.matches(&actual.element)
                    && space == actual.address_space
                    && access == actual.access
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceSsaLoanV36 {
    loan: usize,
    site: SourceReferenceSiteV29,
    origin: usize,
    origin_instance: ProductionCallInstanceIdV1,
    origin_local: SemanticLocalIdV1,
    origin_generation: u32,
    origin_function: SemanticFunctionIdV1,
    origin_type: SemanticTypeIdV1,
    carrier: ProductionSourceReferenceCarrierV38,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceSsaPhysicalV36 {
    Unit,
    Aggregate {
        start: usize,
        length: usize,
    },
    Enum {
        discriminant: usize,
        start: usize,
        length: usize,
        known_variant: Option<u32>,
    },
    EnumVariant {
        variant: u32,
        start: usize,
        length: usize,
    },
    Value {
        value: ValueId,
        ty: SourceSsaCarrierTypeV36,
        loan: Option<SourceSsaLoanV36>,
    },
    Unmodeled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceSsaEndpointRowV36 {
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    physical: SourceSsaPhysicalV36,
}

fn source_typed_endpoint_error_v36() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("original typed SSA endpoint differs from its source owner")
}

fn source_ssa_record_local_v36(
    definitions: &mut [Option<SemanticLocalIdV1>],
    original: SsaValueV1,
    variable: fe2o3_mir_model::SsaVariableIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(3)?;
    let SsaValueV1::Definition(definition) = original else {
        return Err(source_typed_endpoint_error_v36());
    };
    let slot = definitions
        .get_mut(definition.get() as usize)
        .ok_or_else(source_typed_endpoint_error_v36)?;
    if slot
        .replace(SemanticLocalIdV1::from_index(variable.get()))
        .is_some()
    {
        return Err(source_typed_endpoint_error_v36());
    }
    Ok(())
}

fn source_ssa_definition_locals_v36(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<Option<SemanticLocalIdV1>>, ProductionSemanticKirErrorV1> {
    budget.charge_work(3)?;
    let original = instances
        .instance(instance)
        .ok_or_else(source_typed_endpoint_error_v36)?;
    let plan = original.ssa().plan();
    let rows = instances
        .owner()
        .occurrences_v1()
        .and_then(|rows| rows.function(original.function()))
        .ok_or_else(source_typed_endpoint_error_v36)?;
    let mut definitions = emission_vec_v1(plan.definition_count(), budget)?;
    budget.charge_work(plan.definition_count())?;
    definitions.resize(plan.definition_count(), None);
    for entry in rows.entry_definitions() {
        budget.charge_work(1)?;
        if let Some(value) = entry.value() {
            source_ssa_record_local_v36(&mut definitions, value, entry.variable(), budget)?;
        }
    }
    for event in rows.events() {
        budget.charge_work(3)?;
        if let Some(fe2o3_mir_model::SsaResolvedEventV1::Define { variable, value }) =
            event.resolved()
        {
            if !event.is_reachable() || !event.is_promoted() {
                return Err(source_typed_endpoint_error_v36());
            }
            source_ssa_record_local_v36(&mut definitions, value, variable, budget)?;
        }
    }
    for edge in rows.edge_definitions() {
        budget.charge_work(3)?;
        if let Some(value) = edge.value() {
            if !edge.is_reachable() || !edge.is_promoted() {
                return Err(source_typed_endpoint_error_v36());
            }
            source_ssa_record_local_v36(&mut definitions, value, edge.variable(), budget)?;
        }
    }
    budget.charge_work(definitions.len())?;
    if definitions.iter().any(Option::is_none) {
        return Err(source_typed_endpoint_error_v36());
    }
    Ok(definitions)
}

fn retain_source_typed_endpoint_v36(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    references: &SourceReferenceEmissionV29<'_, '_>,
    definitions: &[Option<SemanticLocalIdV1>],
    original: SsaValueV1,
    binding: &SemanticValueBindingV1,
    carriers: &mut Vec<SourceSsaComponentV37>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceSsaEndpointRowV36, ProductionSemanticKirErrorV1> {
    budget.charge_work(7)?;
    let declaration = instances
        .instance(instance)
        .ok_or_else(source_typed_endpoint_error_v36)?;
    let local = match original {
        SsaValueV1::Definition(value) => definitions
            .get(value.get() as usize)
            .copied()
            .flatten()
            .ok_or_else(source_typed_endpoint_error_v36)?,
        SsaValueV1::BlockArgument { block, variable } => {
            let variables = declaration
                .ssa()
                .plan()
                .transport_variables(block)
                .ok_or_else(source_typed_endpoint_error_v36)?;
            budget.charge_work(variables.len().checked_ilog2().unwrap_or(0) as usize + 2)?;
            if variables.binary_search(&variable).is_err() {
                return Err(source_typed_endpoint_error_v36());
            }
            SemanticLocalIdV1::from_index(variable.get())
        }
    };
    let ty = declaration
        .declaration()
        .locals()
        .get(local.index() as usize)
        .ok_or_else(source_typed_endpoint_error_v36)?
        .ty();
    let physical =
        retain_source_carrier_tree_v37(instances, references, ty, binding, carriers, budget)?;
    Ok(SourceSsaEndpointRowV36 {
        local,
        ty,
        physical,
    })
}

fn retain_source_carrier_leaf_v37(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    ty: SemanticTypeIdV1,
    binding: &SemanticValueBindingV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceSsaPhysicalV36, ProductionSemanticKirErrorV1> {
    let shape = instances
        .owner()
        .source_semantic()
        .types()
        .get(ty.index() as usize)
        .ok_or_else(source_typed_endpoint_error_v36)?
        .shape();
    let physical = match (binding, shape) {
        (SemanticValueBindingV1::Unit, SemanticTypeShapeV1::Unit) => SourceSsaPhysicalV36::Unit,
        (SemanticValueBindingV1::Value { id, ty: actual }, _) => {
            // This records the original emitted carrier, not an interpretation
            // of its source type. Descriptor wrappers require a separate
            // authenticated source ABI recipe before a consumer uses Slice.
            match SourceSsaCarrierTypeV36::from_type(actual) {
                Some(ty) => SourceSsaPhysicalV36::Value {
                    value: *id,
                    ty,
                    loan: None,
                },
                _ => SourceSsaPhysicalV36::Unmodeled,
            }
        }
        (
            SemanticValueBindingV1::SourceReference(reference),
            SemanticTypeShapeV1::Pointer(pointer),
        ) if pointer.kind() == SemanticPointerKindV1::Reference
            && pointer.metadata() == SemanticPointerMetadataV1::None =>
        {
            // A scalar stable-referent payload is a distinct locator class,
            // never a pointer value just because there is one carrier.
            let carrier = match reference.values.as_slice() {
                [value] => SourceSsaCarrierTypeV36::from_type(&value.ty),
                _ => None,
            };
            match (reference.origin, carrier) {
                (
                    SourceReferenceBindingOriginV29::SingleLoan(loan),
                    Some(
                        carrier_type @ (SourceSsaCarrierTypeV36::Pointer { .. }
                        | SourceSsaCarrierTypeV36::Scalar(_)),
                    ),
                ) => {
                    references.check(budget)?;
                    if !std::ptr::eq(references.plan.instances, instances)
                        || reference.source_type != ty
                    {
                        return Err(source_typed_endpoint_error_v36());
                    }
                    source_reference_validate_binding_v29(references.plan, reference, budget)?;
                    budget.charge_work(3)?;
                    let record = references
                        .plan
                        .loans
                        .get(loan)
                        .ok_or_else(source_typed_endpoint_error_v36)?;
                    let origin = references
                        .plan
                        .origins
                        .get(record.origin)
                        .ok_or_else(source_typed_endpoint_error_v36)?;
                    let carrier = match carrier_type {
                        SourceSsaCarrierTypeV36::Pointer { .. } => {
                            ProductionSourceReferenceCarrierV38::MemoryPointer
                        }
                        SourceSsaCarrierTypeV36::Scalar(_)
                            if record.representation
                                == SourceReferenceRepresentationV29::StableReferent
                                && record.kind == SemanticBorrowKindV1::Shared
                                && pointer.mutability() == SemanticMutabilityV1::Immutable
                                && origin.projections.is_empty()
                                && origin.ty == pointer.pointee() =>
                        {
                            ProductionSourceReferenceCarrierV38::StableScalar
                        }
                        _ => return Ok(SourceSsaPhysicalV36::Unmodeled),
                    };
                    budget.charge_work(3)?;
                    let origin_function = instances
                        .instance(origin.instance)
                        .ok_or_else(source_typed_endpoint_error_v36)?
                        .function();
                    SourceSsaPhysicalV36::Value {
                        value: reference.values[0].id,
                        ty: carrier_type,
                        loan: Some(SourceSsaLoanV36 {
                            loan,
                            site: record.site,
                            origin: record.origin,
                            origin_instance: origin.instance,
                            origin_local: origin.local,
                            origin_generation: origin.generation,
                            origin_function,
                            origin_type: origin.ty,
                            carrier,
                        }),
                    }
                }
                _ => SourceSsaPhysicalV36::Unmodeled,
            }
        }
        _ => SourceSsaPhysicalV36::Unmodeled,
    };
    Ok(physical)
}

/// Borrowed original-emission locator. Pointer validity and the source/target
/// heap relation must be supplied by the consuming semantic interpreter.
/// A physical carrier shape does not classify the original source type.
pub struct ProductionSourceSsaEndpointV36<'a, 'source> {
    owner: &'a ProductionSourceCorrespondenceV18<'source>,
    row: &'a SourceSsaRowV30,
    function: SemanticFunctionIdV1,
    coordinate: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    source_type: SemanticTypeIdV1,
    physical: &'a SourceSsaPhysicalV36,
    carriers: &'a [SourceSsaComponentV37],
    definition: Option<usize>,
}

impl ProductionSourceSsaEndpointV36<'_, '_> {
    /// Returns the original function containing this source SSA value.
    pub fn source_function(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<SemanticFunctionIdV1> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            Ok(self.function)
        })())
    }
    /// Returns the original local represented by this definition or block argument.
    pub fn source_local(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<SemanticLocalIdV1> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            Ok(self.row.typed.local)
        })())
    }
    /// Returns this endpoint's original source type without asserting a value relation.
    pub fn source_type(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<SemanticTypeIdV1> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            Ok(self.source_type)
        })())
    }
    /// Returns the exact original canonical definition, or `None` for Unit.
    /// Structured endpoints require a component query and are refused here.
    pub fn original_definition(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<usize>> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            if matches!(
                self.physical,
                SourceSsaPhysicalV36::Aggregate { .. }
                    | SourceSsaPhysicalV36::Enum { .. }
                    | SourceSsaPhysicalV36::EnumVariant { .. }
            ) {
                return self
                    .owner
                    .source
                    .missing("aggregate SSA carrier has multiple components");
            }
            Ok(self.definition)
        })())
    }
    /// Borrows the original canonical carrier type, or returns `None` for Unit.
    pub fn physical_type(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<&Type>> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            if matches!(
                self.physical,
                SourceSsaPhysicalV36::Aggregate { .. }
                    | SourceSsaPhysicalV36::Enum { .. }
                    | SourceSsaPhysicalV36::EnumVariant { .. }
            ) {
                return self
                    .owner
                    .source
                    .missing("aggregate SSA carrier has multiple components");
            }
            Ok(self
                .definition
                .map(|index| self.owner.inventory.definitions()[index].ty))
        })())
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Rejoins an original SSA value to its retained source and canonical carrier.
    /// This locator does not establish pointer validity or source/target equality.
    pub fn ssa_typed_endpoint_v36(
        &self,
        root: usize,
        instance: usize,
        original: SsaValueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceSsaEndpointV36<'_, '_>> {
        self.retain_query((|| {
            self.query(budget)?;
            let owner = self.source.root_row(root)?;
            let results =
                owner
                    .rvalue_results
                    .as_ref()
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "original typed SSA roster is absent",
                    ))?;
            let source = self.source.source_ssa(budget)?;
            budget.charge_work(8)?;
            if results.source.semantic != *source.source_semantic_sha256()
                || results.source.ssa != source.identity()
                || results.source.root != owner.coordinates.root
                || results.ledger != budget.work_ledger_identity_v1()
                || budget.storage() < results.storage
            {
                return self.source.missing("original typed SSA owner differs");
            }
            let (function, _) = self.source.instance(root, instance, budget)?;
            if !self.source.instance_active(root, instance, budget)? {
                return self
                    .source
                    .missing("original typed SSA instance is inactive");
            }
            budget.charge_work(argument_product_v1(
                results.values.len().checked_ilog2().unwrap_or(0) as usize + 2,
                16,
            )?)?;
            let at = results
                .values
                .binary_search_by_key(&(instance, original), |row| (row.instance, row.original))
                .map_err(|_| {
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "original typed SSA endpoint is missing",
                    )
                })?;
            let row = &results.values[at];
            let declaration = source
                .source_semantic()
                .functions()
                .get(function.index() as usize)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original typed SSA function is absent",
                ))?;
            budget.charge_work(3)?;
            if declaration
                .locals()
                .get(row.typed.local.index() as usize)
                .map(|local| local.ty())
                != Some(row.typed.ty)
            {
                return self.source.missing("original typed SSA local type differs");
            }
            let coordinate = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(owner.function_ordinal)
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            let definition = source_carrier_definition_v37(
                self,
                coordinate,
                &row.typed.physical,
                &results.carriers,
                budget,
            )?;
            Ok(ProductionSourceSsaEndpointV36 {
                owner: self,
                row,
                function,
                coordinate,
                source_type: row.typed.ty,
                physical: &row.typed.physical,
                carriers: &results.carriers,
                definition,
            })
        })())
    }
}

fn source_typed_endpoint_headers_v36() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionSemanticKirErrorV1>>())?,
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<SourceSsaElementV36>()?,
        h::<SourceSsaCarrierTypeV36>()?,
        h::<SourceSsaLoanV36>()?,
        source_reference_endpoint_headers_v38()?,
        h::<SourceSsaPhysicalV36>()?,
        h::<SourceSsaEndpointRowV36>()?,
        source_carrier_tree_headers_v37()?,
        h::<ProductionSourceSsaEndpointV36<'_, '_>>()?,
        h::<Vec<Option<SemanticLocalIdV1>>>()?,
        h::<&mut [Option<SemanticLocalIdV1>]>()?,
        h::<&[Option<SemanticLocalIdV1>]>()?,
        h::<Option<SemanticLocalIdV1>>()?,
        h::<&SourceReferenceEmissionV29<'_, '_>>()?,
        h::<&SemanticSourceReferenceBindingV29>()?,
        h::<&SourceReferenceLoanV29>()?,
        h::<&SourceReferenceOriginV29>()?,
        h::<&fe2o3_mir_model::SsaConstructionPlanV1>()?,
        h::<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>>()?,
        h::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEntryDefinitionOccurrenceV1>>(
        )?,
        h::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>>()?,
        h::<std::slice::Iter<'_, fe2o3_pliron::ProductionSemanticSsaEdgeDefinitionOccurrenceV1>>()?,
        h::<std::slice::Iter<'_, Option<SemanticLocalIdV1>>>()?,
        h::<&SemanticTypeShapeV1>()?,
        h::<Option<SourceSsaCarrierTypeV36>>()?,
        h::<(
            SourceReferenceBindingOriginV29,
            Option<SourceSsaCarrierTypeV36>,
        )>()?,
        h::<(Option<SourceSsaCarrierTypeV36>, &SemanticTypeShapeV1)>()?,
        h::<(bool, Option<SourceSsaCarrierTypeV36>)>()?,
        h::<(&SemanticValueBindingV1, &SemanticTypeShapeV1)>()?,
        h::<SemanticFunctionIdV1>()?,
        h::<SemanticLocalIdV1>()?,
        h::<SemanticTypeIdV1>()?,
        argument_product_v1(12, h::<usize>()?)?,
    ])
}
