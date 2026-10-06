/// Original activation location. These are static source sites, not dynamic
/// allocation incarnations and not permission to use a stale pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceObjectActivationV40 {
    /// Storage is available at entry to the original declaration instance.
    Entry,
    /// Storage is activated by this exact original StorageLive statement.
    StorageLive {
        /// Original declaration-local block ordinal.
        block: SemanticBlockIdV1,
        /// Original statement ordinal within that block.
        statement: usize,
    },
}

/// Original may-activation recipe for one logical source object generation.
/// Borrowed members preserve joined generations without equating the logical
/// label to a physical backing or to a runtime allocation generation.
pub struct ProductionSourceObjectLifetimeV40<'view, 'source> {
    owner: &'view ProductionSourceCorrespondenceV18<'source>,
    root: usize,
    instance: usize,
    local: SemanticLocalIdV1,
    generation: u32,
    ty: SemanticTypeIdV1,
    slot: usize,
    members: &'view [usize],
    activations: &'view [SourceReferenceStorageActivationV29],
    required: usize,
}

fn source_object_lifetime_headers_v40() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<ProductionSourceObjectLifetimeV40<'_, '_>>()?,
        h::<Option<ProductionSourceObjectLifetimeV40<'_, '_>>>()?,
        h::<scoped_raw_admission_v29::ObjectLifetimePartsV40<'_>>()?,
        h::<Option<scoped_raw_admission_v29::ObjectLifetimePartsV40<'_>>>()?,
        h::<(
            (usize, SemanticLocalIdV1, u32),
            scoped_raw_admission_v29::ObjectLifetimePartsV40<'_>,
        )>()?,
        h::<ProductionSourceObjectActivationV40>()?,
        h::<SourceReferenceStorageActivationV29>()?,
        h::<(u32, ProductionSourceObjectActivationV40)>()?,
        h::<(usize, usize, SemanticLocalIdV1, u32, SemanticTypeIdV1)>()?,
        h::<usize>()?,
        h::<()>()?,
        h::<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>()?,
        h::<[usize; 12]>()?,
    ])
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Number of exact original logical object lifetimes in the root roster.
    pub fn memory_object_lifetime_count_v40(
        &self,
        root: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.retain_query((|| {
            self.query(budget)?;
            budget.reserve_storage(source_object_lifetime_headers_v40()?)?;
            scoped_raw_admission_v29::object_lifetime_count_v40(self, root, budget)
        })())
    }

    /// Borrows one original roster row in sorted source identity order.
    /// This permits a single complete census, including joined generations
    /// whose atomic predecessors have no separately emitted local use.
    pub fn memory_object_lifetime_at_v40(
        &self,
        root: usize,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceObjectLifetimeV40<'_, '_>> {
        self.retain_query((|| {
            self.query(budget)?;
            budget.reserve_storage(source_object_lifetime_headers_v40()?)?;
            let ((instance, local, generation), (ty, slot, members, activations)) =
                scoped_raw_admission_v29::object_lifetime_at_v40(self, root, ordinal, budget)?;
            Ok(ProductionSourceObjectLifetimeV40 {
                owner: self,
                root,
                instance,
                local,
                generation,
                ty,
                slot,
                members,
                activations,
                required: budget.storage(),
            })
        })())
    }

    /// Looks up the original finite activation recipe in logarithmic work.
    /// Absence is explicit and cannot discharge an object lifetime obligation.
    /// Drop this borrow before releasing the enclosing source query credit.
    pub fn memory_object_lifetime_v40(
        &self,
        root: usize,
        instance: usize,
        local: SemanticLocalIdV1,
        generation: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceObjectLifetimeV40<'_, '_>>> {
        self.retain_query((|| {
            self.query(budget)?;
            budget.reserve_storage(source_object_lifetime_headers_v40()?)?;
            let Some((ty, slot, members, activations)) =
                scoped_raw_admission_v29::object_lifetime_parts_v40(
                    self, root, instance, local, generation, budget,
                )?
            else {
                return Ok(None);
            };
            Ok(Some(ProductionSourceObjectLifetimeV40 {
                owner: self,
                root,
                instance,
                local,
                generation,
                ty,
                slot,
                members,
                activations,
                required: budget.storage(),
            }))
        })())
    }
}

impl ProductionSourceObjectLifetimeV40<'_, '_> {
    /// Exact root, instance, local, logical generation and original type.
    pub fn identity(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(usize, usize, SemanticLocalIdV1, u32, SemanticTypeIdV1)> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok((
            self.root,
            self.instance,
            self.local,
            self.generation,
            self.ty,
        ))
    }

    /// Exact original physical backing operation. This is a locator only and
    /// does not establish that the original runtime object is live there.
    pub fn original_backing(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1> {
        self.owner.retain_query((|| {
            source_object_endpoint_check_v39(self.owner, self.required, budget)?;
            scoped_raw_admission_v29::source_slot_input_v18(
                self.owner, self.root, self.slot, budget,
            )
        })())
    }

    /// Number of possible original activation sites in this logical generation.
    pub fn activation_count(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok(self.members.len())
    }

    /// One exact atomic source generation and its original activation site.
    /// Repeated dynamic executions of a site require distinct runtime identities.
    pub fn activation(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(u32, ProductionSourceObjectActivationV40)> {
        self.owner.retain_query((|| {
            source_object_endpoint_check_v39(self.owner, self.required, budget)?;
            budget.charge_work(4)?;
            let row = self
                .members
                .get(ordinal)
                .and_then(|index| self.activations.get(*index))
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original object activation ordinal",
                ))?;
            if row.instance.index() != self.instance || row.local != self.local {
                return self
                    .owner
                    .source
                    .missing("original object activation identity");
            }
            let site = match row.origin {
                SourceReferenceActivationOriginV29::Entry if row.generation == 0 => {
                    ProductionSourceObjectActivationV40::Entry
                }
                SourceReferenceActivationOriginV29::StorageLive(site)
                    if site.instance == row.instance =>
                {
                    ProductionSourceObjectActivationV40::StorageLive {
                        block: site.block,
                        statement: site.statement.ok_or(
                            ProductionSourceOwnedViewErrorV18::Binding(
                                "original object activation is not a statement",
                            ),
                        )?,
                    }
                }
                _ => {
                    return self
                        .owner
                        .source
                        .missing("original object activation origin");
                }
            };
            Ok((row.generation, site))
        })())
    }
}
