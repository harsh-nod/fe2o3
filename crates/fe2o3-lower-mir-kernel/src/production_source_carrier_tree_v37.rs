// Original aggregate fields stay structured. A carrier tree is an emission
// locator, not an ABI reconstruction or a source/target value relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceSsaComponentV37 {
    ty: SemanticTypeIdV1,
    physical: SourceSsaPhysicalV36,
}

struct SourceCarrierFrameV37<'a> {
    binding: &'a SemanticValueBindingV1,
    ty: SemanticTypeIdV1,
    destination: Option<usize>,
}

fn source_carrier_field_type_v37(
    shape: &SemanticTypeShapeV1,
    count: usize,
    field: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SemanticTypeIdV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    match shape {
        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields)
            if fields.fields().len() == count =>
        {
            fields
                .fields()
                .get(field)
                .copied()
                .ok_or_else(source_typed_endpoint_error_v36)
        }
        SemanticTypeShapeV1::Array { element, length }
            if usize::try_from(*length).ok() == Some(count) && field < count =>
        {
            Ok(*element)
        }
        _ => Err(source_typed_endpoint_error_v36()),
    }
}

fn retain_source_carrier_tree_v37(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    ty: SemanticTypeIdV1,
    binding: &SemanticValueBindingV1,
    carriers: &mut Vec<SourceSsaComponentV37>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceSsaPhysicalV36, ProductionSemanticKirErrorV1> {
    let mut pending = emission_vec_v1(1, budget)
        .inspect_err(|error| source_reference_record_failure_v29(references.plan, error))?;
    pending.push(SourceCarrierFrameV37 {
        binding,
        ty,
        destination: None,
    });
    let result = (|| {
        let mut root = SourceSsaPhysicalV36::Unmodeled;
        while let Some(frame) = pending.pop() {
            budget.charge_work(6)?;
            let physical = if let SemanticValueBindingV1::Aggregate(fields) = frame.binding {
                let shape = instances
                    .owner()
                    .source_semantic()
                    .types()
                    .get(frame.ty.index() as usize)
                    .ok_or_else(source_typed_endpoint_error_v36)?
                    .shape();
                let count = match shape {
                    SemanticTypeShapeV1::Tuple(source) | SemanticTypeShapeV1::Aggregate(source) => {
                        source.fields().len()
                    }
                    SemanticTypeShapeV1::Array { length, .. } => {
                        usize::try_from(*length).map_err(|_| ArgumentResourceV1::Arithmetic)?
                    }
                    _ => return Err(source_typed_endpoint_error_v36()),
                };
                if count != fields.len() {
                    return Err(source_typed_endpoint_error_v36());
                }
                let start = carriers.len();
                for field in 0..count {
                    let ty = source_carrier_field_type_v37(shape, count, field, budget)?;
                    emission_push_v1(
                        carriers,
                        SourceSsaComponentV37 {
                            ty,
                            physical: SourceSsaPhysicalV36::Unmodeled,
                        },
                        budget,
                    )?;
                }
                // Slots for immediate fields are contiguous; child subtrees are
                // appended later and never alter the source field ordinals.
                for (field, binding) in fields.iter().enumerate().rev() {
                    budget.charge_work(3)?;
                    let destination = start
                        .checked_add(field)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    let ty = carriers
                        .get(destination)
                        .ok_or_else(source_typed_endpoint_error_v36)?
                        .ty;
                    emission_push_v1(
                        &mut pending,
                        SourceCarrierFrameV37 {
                            binding,
                            ty,
                            destination: Some(destination),
                        },
                        budget,
                    )?;
                }
                SourceSsaPhysicalV36::Aggregate {
                    start,
                    length: count,
                }
            } else {
                retain_source_carrier_leaf_v37(
                    instances,
                    references,
                    frame.ty,
                    frame.binding,
                    budget,
                )?
            };
            match frame.destination {
                None => root = physical,
                Some(destination) => {
                    carriers
                        .get_mut(destination)
                        .ok_or_else(source_typed_endpoint_error_v36)?
                        .physical = physical
                }
            }
        }
        Ok(root)
    })()
    .inspect_err(|error| source_reference_record_failure_v29(references.plan, error));
    let backing = argument_product_v1(pending.capacity(), size_of::<SourceCarrierFrameV37<'_>>());
    drop(pending);
    let released = backing
        .map_err(ProductionSemanticKirErrorV1::from)
        .and_then(|backing| budget.release_storage(backing).map_err(Into::into))
        .inspect_err(|error| source_reference_record_failure_v29(references.plan, error));
    match result {
        Ok(value) => released.map(|()| value),
        Err(error) => {
            let _ = released;
            Err(error)
        }
    }
}

fn source_carrier_definition_v37(
    owner: &ProductionSourceCorrespondenceV18<'_>,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    physical: &SourceSsaPhysicalV36,
    carriers: &[SourceSsaComponentV37],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<usize>> {
    budget.charge_work(4)?;
    match *physical {
        SourceSsaPhysicalV36::Unmodeled => owner
            .source
            .missing("original SSA binding has no typed carrier"),
        SourceSsaPhysicalV36::Unit => Ok(None),
        SourceSsaPhysicalV36::Aggregate { start, length } => {
            let end = start
                .checked_add(length)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            if carriers.get(start..end).is_none() {
                return owner
                    .source
                    .missing("original aggregate SSA carrier range differs");
            }
            Ok(None)
        }
        SourceSsaPhysicalV36::Value { value, ty, .. } => {
            let index = owner
                .inventory
                .definition_index_for_value(function, value, budget)
                .map_err(|error| {
                    ProductionSourceOwnedViewErrorV18::from(
                        fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
                    )
                })?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original typed SSA canonical value is absent",
                ))?;
            if !ty.matches(owner.inventory.definitions()[index].ty) {
                return owner
                    .source
                    .missing("original typed SSA canonical type differs");
            }
            Ok(Some(index))
        }
    }
}

/// Physical shape of one retained source binding, without a semantic interpretation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceSsaCarrierShapeV37 {
    /// A source Unit binding with no physical canonical definition.
    Unit,
    /// One exact canonical definition; its type is available from the endpoint.
    Value,
    /// Ordered original aggregate fields, including Unit and nested aggregates.
    Aggregate {
        /// Number of immediate original fields, not the flattened leaf count.
        components: usize,
    },
}

impl<'a, 'source> ProductionSourceSsaEndpointV36<'a, 'source> {
    /// Reports the retained physical shape, without classifying its source semantics.
    pub fn carrier_shape(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceSsaCarrierShapeV37> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(1)?;
            match *self.physical {
                SourceSsaPhysicalV36::Unit => Ok(ProductionSourceSsaCarrierShapeV37::Unit),
                SourceSsaPhysicalV36::Value { .. } => Ok(ProductionSourceSsaCarrierShapeV37::Value),
                SourceSsaPhysicalV36::Aggregate { length, .. } => {
                    Ok(ProductionSourceSsaCarrierShapeV37::Aggregate { components: length })
                }
                SourceSsaPhysicalV36::Unmodeled => {
                    self.owner.source.missing("unmodeled SSA carrier shape")
                }
            }
        })())
    }

    /// Borrows an immediate original aggregate field. Source-local identity is
    /// retained; `source_type` now identifies the selected original field type.
    pub fn component(
        &self,
        field: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceSsaEndpointV36<'a, 'source>> {
        self.owner.retain_query((|| {
            self.owner.query(budget)?;
            budget.charge_work(6)?;
            let SourceSsaPhysicalV36::Aggregate { start, length } = *self.physical else {
                return self
                    .owner
                    .source
                    .missing("SSA carrier has no aggregate components");
            };
            if field >= length {
                return self
                    .owner
                    .source
                    .missing("SSA aggregate component is out of range");
            }
            let at = start
                .checked_add(field)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let row = self
                .carriers
                .get(at)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "SSA aggregate component is absent",
                ))?;
            let definition = source_carrier_definition_v37(
                self.owner,
                self.coordinate,
                &row.physical,
                self.carriers,
                budget,
            )?;
            Ok(ProductionSourceSsaEndpointV36 {
                owner: self.owner,
                row: self.row,
                function: self.function,
                coordinate: self.coordinate,
                source_type: row.ty,
                physical: &row.physical,
                carriers: self.carriers,
                definition,
            })
        })())
    }
}

fn source_carrier_tree_headers_v37() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionSemanticKirErrorV1>>())?,
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<SourceSsaComponentV37>()?,
        h::<SourceCarrierFrameV37<'_>>()?,
        h::<ProductionSourceSsaCarrierShapeV37>()?,
        h::<Vec<SourceSsaComponentV37>>()?,
        h::<&mut Vec<SourceSsaComponentV37>>()?,
        h::<&[SourceSsaComponentV37]>()?,
        h::<Vec<SourceCarrierFrameV37<'_>>>()?,
        h::<Option<SourceCarrierFrameV37<'_>>>()?,
        h::<&SemanticTypeShapeV1>()?,
        h::<&Vec<SemanticValueBindingV1>>()?,
        h::<std::ops::Range<usize>>()?,
        h::<std::iter::Rev<std::iter::Enumerate<std::slice::Iter<'_, SemanticValueBindingV1>>>>()?,
        h::<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>()?,
        h::<fe2o3_kernel_ir::FixedVectorTypeV12>()?,
        h::<(
            &ExecutionInstancesV29<'_>,
            &SourceReferenceEmissionV29<'_, '_>,
            &mut Vec<SourceCarrierFrameV37<'_>>,
            &mut Vec<SourceSsaComponentV37>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        argument_product_v1(10, h::<usize>()?)?,
    ])
}
