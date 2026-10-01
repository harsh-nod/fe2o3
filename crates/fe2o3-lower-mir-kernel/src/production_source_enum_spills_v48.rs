// Retained compiler spill origins are emission locators, never source objects.
#[derive(Debug)]
#[cfg_attr(test, derive(Clone))]
struct SourceEnumSpillRowV48 {
    instance: usize,
    origin: ExecutionEnumSpillV48,
}

/// Original nominal field coordinates of compiler-created payload storage.
/// This record alone grants no initializedness, lifetime or reference authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceEnumSpillOriginV48 {
    pub instance: usize,
    pub local: u32,
    pub source_type: SemanticTypeIdV1,
    pub variant: u32,
    pub field: u32,
    pub field_type: SemanticTypeIdV1,
    pub component: usize,
}

pub struct ProductionSourceEnumSpillV48<'a, 'source> {
    correspondence: &'a ProductionSourceCorrespondenceV18<'source>,
    root: usize,
    ordinal: usize,
}

fn source_enum_spill_headers_v48() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionSemanticKirErrorV1>>())?,
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<Vec<SourceEnumSpillRowV48>>()?,
        h::<SourceEnumSpillRowV48>()?,
        h::<ExecutionEnumSpillV48>()?,
        h::<ProductionSourceEnumSpillOriginV48>()?,
        h::<ProductionSourceEnumSpillV48<'_, '_>>()?,
        h::<&[SourceEnumSpillRowV48]>()?,
        h::<std::slice::Iter<'_, SourceEnumSpillRowV48>>()?,
        h::<(&ExecutionEnumSpillV48, &ExecutionEnumSpillV48)>()?,
        h::<(&Type, &Type)>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>()?,
        h::<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>()?,
        h::<[usize; 6]>()?,
    ])
}

fn retain_source_enum_spill_v48(
    instance: usize,
    origin: &ExecutionEnumSpillV48,
    rows: &mut Vec<SourceEnumSpillRowV48>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    if rows.len() == rows.capacity() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    rows.push(SourceEnumSpillRowV48 {
        instance,
        origin: ExecutionEnumSpillV48 {
            local: origin.local,
            source_type: origin.source_type,
            variant: origin.variant,
            field: origin.field,
            field_type: origin.field_type,
            component: origin.component,
            emitted_block: origin.emitted_block,
            emitted_operation: origin.emitted_operation,
            pointer: origin.pointer,
            element: emission_binding_clone_type_v1(&origin.element, budget)?,
            alignment: origin.alignment,
        },
    });
    Ok(())
}

fn source_enum_spills_equal_v48(
    left: &[SourceEnumSpillRowV48],
    right: &[SourceEnumSpillRowV48],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if left.len() != right.len() {
        return Ok(false);
    }
    for (a, b) in left.iter().zip(right) {
        budget.charge_work(12)?;
        let (x, y) = (&a.origin, &b.origin);
        if a.instance != b.instance
            || x.local != y.local
            || x.source_type != y.source_type
            || x.variant != y.variant
            || x.field != y.field
            || x.field_type != y.field_type
            || x.component != y.component
            || x.emitted_block != y.emitted_block
            || x.emitted_operation != y.emitted_operation
            || x.pointer != y.pointer
            || x.alignment != y.alignment
            || !enum_spill_types_equal_v48(&x.element, &y.element, budget)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

impl<'source> ProductionSourceCorrespondenceV18<'source> {
    fn enum_spill_rows_v48(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[SourceEnumSpillRowV48]> {
        self.query(budget)?;
        let owner = self.source.root_row(root)?;
        let results =
            owner
                .rvalue_results
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "compiler enum spill roster is absent",
                ))?;
        budget.charge_work(8)?;
        let source = self.source.source_ssa(budget)?;
        if results.source.semantic != *source.source_semantic_sha256()
            || results.source.ssa != source.identity()
            || results.source.root != owner.coordinates.root
            || results.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < results.storage
        {
            return self.source.missing("compiler enum spill owner differs");
        }
        Ok(&results.enum_spills)
    }

    pub fn enum_spill_count_v48(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.retain_query(self.enum_spill_rows_v48(root, budget).map(<[_]>::len))
    }

    pub fn enum_spill_v48(
        &self,
        root: usize,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceEnumSpillV48<'_, 'source>> {
        let result = (|| {
            self.enum_spill_rows_v48(root, budget)?.get(ordinal).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("compiler enum spill ordinal is absent"),
            )?;
            Ok(ProductionSourceEnumSpillV48 {
                correspondence: self,
                root,
                ordinal,
            })
        })();
        self.retain_query(result)
    }
}

impl ProductionSourceEnumSpillV48<'_, '_> {
    fn row(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&SourceEnumSpillRowV48> {
        let result = (|| {
            let row = self
                .correspondence
                .enum_spill_rows_v48(self.root, budget)?
                .get(self.ordinal)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "compiler enum spill ordinal is absent",
                ))?;
            if !self
                .correspondence
                .source
                .instance_active(self.root, row.instance, budget)?
            {
                return self
                    .correspondence
                    .source
                    .missing("compiler enum spill instance is inactive");
            }
            Ok(row)
        })();
        self.correspondence.retain_query(result)
    }

    pub fn origin(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceEnumSpillOriginV48> {
        let row = self.row(budget)?;
        Ok(ProductionSourceEnumSpillOriginV48 {
            instance: row.instance,
            local: row.origin.local,
            source_type: row.origin.source_type,
            variant: row.origin.variant,
            field: row.origin.field,
            field_type: row.origin.field_type,
            component: row.origin.component,
        })
    }

    /// Exact current-inventory Alloca definition and operation, not a source
    /// frame descriptor or permission to read an initialized payload.
    pub fn allocation(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(usize, usize)> {
        let result = (|| {
            let row = self.row(budget)?;
            let root = self.correspondence.source.root_row(self.root)?;
            let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(root.function_ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            let inventory = self.correspondence.inventory;
            let definition = inventory
                .definition_index_for_value(function, row.origin.pointer, budget)
                .map_err(|error| {
                    ProductionSourceOwnedViewErrorV18::from(
                        fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
                    )
                })?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "compiler enum spill definition is absent",
                ))?;
            let value = &inventory.definitions()[definition];
            let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
                operation,
                result: 0,
            } = value.coordinate
            else {
                return self
                    .correspondence
                    .source
                    .missing("compiler enum spill is not an operation result");
            };
            budget.charge_work(
                inventory.operations().len().checked_ilog2().unwrap_or(0) as usize + 2,
            )?;
            let at = inventory
                .operations()
                .binary_search_by_key(&operation, |row| row.coordinate)
                .map_err(|_| {
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "compiler enum spill operation is absent",
                    )
                })?;
            let actual = &inventory.operations()[at];
            let OperationKind::Alloca {
                element,
                count: None,
                address_space: AddressSpace::Private,
                alignment,
            } = &actual.operation.kind
            else {
                return self
                    .correspondence
                    .source
                    .missing("compiler enum spill allocation changed");
            };
            let Type::Pointer(pointer) = value.ty else {
                return self
                    .correspondence
                    .source
                    .missing("compiler enum spill pointer type changed");
            };
            budget.charge_work(8)?;
            let definition_end = definition
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            if actual.results != (definition..definition_end)
                || *alignment != row.origin.alignment
                || pointer.address_space != AddressSpace::Private
                || pointer.access != AccessMode::ReadWrite
                || !call_types_equal_v1(element, &row.origin.element, budget)?
                || !call_types_equal_v1(&pointer.pointee, element, budget)?
            {
                return self
                    .correspondence
                    .source
                    .missing("compiler enum spill geometry changed");
            }
            Ok((definition, at))
        })();
        self.correspondence.retain_query(result)
    }
}
