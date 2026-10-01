include!("production_source_object_lifetimes_v40.rs");

/// Original object category. A category is not allocation or lifetime authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceObjectClassV39 {
    /// Original declaration-local storage, with a logical static generation.
    Local,
    /// A value snapshot at an original operand, call result or return.
    Snapshot,
    /// An original dereference target, not an inferred private local.
    Reference,
    /// An original root entry value.
    EntryValue,
    /// An original helper argument.
    InlineArgument,
}

/// Endpoint position in the complete original typed-storage operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceObjectEndpointRoleV39 {
    /// Base of a typed projection.
    ProjectionBase,
    /// Result of a typed projection.
    ProjectionResult,
    /// Typed value read.
    Read,
    /// Typed value write.
    Write,
    /// Source of a snapshot copy.
    CopySource,
    /// Destination of a snapshot copy.
    CopyDestination,
    /// Original or transfer-related discriminant read.
    DiscriminantRead,
    /// Original or transfer-related discriminant write.
    DiscriminantWrite,
}

/// Exact archived origin of a typed tag operation, not a tag-validity permit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceObjectTagOriginV39 {
    /// An explicit original source statement at the retained occurrence site.
    Statement(fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1),
    /// An original aggregate construction at the retained occurrence site.
    Aggregate(fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1),
    /// A physical component of an original entry value.
    EntryComponent,
    /// A generated source value transfer, not an original Discriminant read.
    Transfer,
    /// The original anchor ordinal supplying the copied tag.
    CopiedTag(usize),
}

/// Borrowed complete original object row and its exact relocated operation.
/// This names source recipes only: initialization, validity, active views and
/// dynamic source/physical lifetimes still require independent interpretation.
pub struct ProductionSourceObjectRecipeV39<'view, 'source> {
    owner: &'view ProductionSourceCorrespondenceV18<'source>,
    anchors: &'view ScopedMemoryAnchorsV29,
    object: SourcePhysicalObjectV18<'view>,
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    required: usize,
}

/// Borrowed original endpoint. The source path and logical storage generation
/// are retained independently of any chosen physical backing representative.
pub struct ProductionSourceObjectEndpointV39<'view, 'source> {
    owner: &'view ProductionSourceCorrespondenceV18<'source>,
    anchors: &'view ScopedMemoryAnchorsV29,
    endpoint: &'view ScopedObjectEndpointV29,
    role: ProductionSourceObjectEndpointRoleV39,
    required: usize,
}

fn source_object_endpoint_check_v39(
    owner: &ProductionSourceCorrespondenceV18<'_>,
    required: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    owner.retain_query((|| {
        owner.query(budget)?;
        if budget.storage() < required {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    })())
}

fn source_object_endpoint_headers_v39() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<ProductionSourceObjectRecipeV39<'_, '_>>()?,
        h::<Option<ProductionSourceObjectRecipeV39<'_, '_>>>()?,
        h::<ProductionSourceObjectEndpointV39<'_, '_>>()?,
        h::<SourcePhysicalObjectV18<'_>>()?,
        h::<&ScopedMemoryAnchorsV29>()?,
        h::<&ScopedMemoryAnchorV29>()?,
        h::<&ScopedObjectEndpointV29>()?,
        h::<TileAttachmentKeyV29>()?,
        h::<&[SourceAttachmentV18]>()?,
        h::<ProductionSourceOperationV18>()?,
        h::<ProductionSourceObjectClassV39>()?,
        h::<ProductionSourceObjectEndpointRoleV39>()?,
        h::<Option<ProductionSourceObjectTagOriginV39>>()?,
        h::<ScopedObjectIdentityV29>()?,
        h::<ScopedObjectSourceV29>()?,
        h::<ScopedObjectComponentV29>()?,
        h::<&[ScopedObjectComponentV29]>()?,
        h::<()>()?,
        h::<usize>()?,
        h::<Option<(usize, SemanticLocalIdV1, u32)>>()?,
        h::<
            Option<(
                ExecutionSiteV29,
                ExecutionOperandV29,
                SemanticLocalIdV1,
                u32,
            )>,
        >()?,
        h::<(SemanticTypeIdV1, SemanticTypeIdV1)>()?,
        h::<(
            fe2o3_kernel_ir::StorageLayoutIdV1,
            fe2o3_kernel_ir::StorageLayoutIdV1,
        )>()?,
        h::<(SemanticProjectionV1, Option<(usize, Option<SsaValueV1>)>)>()?,
        h::<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>()?,
        h::<(ScopedObjectOperationV29, Option<ValueId>)>()?,
        h::<[usize; 8]>()?,
    ])
}

impl ProductionSourceCorrespondenceV18<'_> {
    /// Queries one original anchor without rescanning the source graph.
    /// Non-object anchors return None. An object is joined to its complete
    /// relocated operation, operand uses and result by the existing mapper.
    /// Fixed query storage remains charged until the enclosing source phase
    /// settles it; drop every returned borrow before releasing that credit.
    pub fn memory_object_recipe_v39(
        &self,
        root: usize,
        instance: usize,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceObjectRecipeV39<'_, '_>>> {
        self.retain_query((|| {
            self.query(budget)?;
            budget.reserve_storage(source_object_endpoint_headers_v39()?)?;
            let anchors = self
                .source
                .sidecar(root, instance, budget)?
                .scoped_memory_anchors
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "object endpoint census absent",
                ))?;
            let anchor =
                anchors
                    .rows
                    .get(ordinal)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "object endpoint anchor ordinal",
                    ))?;
            budget.charge_work(2)?;
            if !matches!(anchor.kind, ScopedMemoryAnchorKindV29::Object(_)) {
                return Ok(None);
            }
            let key = TileAttachmentKeyV29 {
                root,
                family: TileAttachmentFamilyV29::MemoryAnchor,
                instance,
                row: ordinal,
                field: TileAttachmentFieldV29::MemoryPosition,
                component: 0,
                part: 0,
            };
            let [row] = self.attachment_range(key, budget)? else {
                return self.source.missing("object endpoint position census");
            };
            let ProductionSourceOperationV18::Operation(coordinate) =
                self.mapped_source_operation(row.location, budget)?
            else {
                return self
                    .source
                    .missing("object endpoint is not an original operation");
            };
            let object =
                self.retained_object_payload_at_v29(root, instance, ordinal, coordinate, budget)?;
            Ok(Some(ProductionSourceObjectRecipeV39 {
                owner: self,
                anchors,
                object,
                coordinate,
                required: budget.storage(),
            }))
        })())
    }
}

impl<'view, 'source> ProductionSourceObjectRecipeV39<'view, 'source> {
    /// Exact original tag-operation purpose; non-tag operations return None.
    /// Transfer and component rows must not be relabeled as source reads.
    pub fn tag_origin(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<ProductionSourceObjectTagOriginV39>> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        let origin = match self.object.source.role {
            ScopedObjectRoleV29::ReadDiscriminant { origin, .. }
            | ScopedObjectRoleV29::SetDiscriminant { origin, .. } => origin,
            ScopedObjectRoleV29::Project { .. }
            | ScopedObjectRoleV29::ReadValue { .. }
            | ScopedObjectRoleV29::WriteValue { .. }
            | ScopedObjectRoleV29::CopyObject { .. } => return Ok(None),
        };
        Ok(Some(match origin {
            ScopedObjectTagOriginV29::Statement(site) => {
                ProductionSourceObjectTagOriginV39::Statement(site)
            }
            ScopedObjectTagOriginV29::Aggregate(site) => {
                ProductionSourceObjectTagOriginV39::Aggregate(site)
            }
            ScopedObjectTagOriginV29::EntryComponent => {
                ProductionSourceObjectTagOriginV39::EntryComponent
            }
            ScopedObjectTagOriginV29::Transfer => ProductionSourceObjectTagOriginV39::Transfer,
            ScopedObjectTagOriginV29::CopiedTag { anchor } => {
                ProductionSourceObjectTagOriginV39::CopiedTag(anchor)
            }
        }))
    }

    /// Exact original canonical coordinate after relocation, not a selected op.
    pub fn original_operation(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok(self.coordinate)
    }

    /// Complete relocated Storage operation and optional result value.
    pub fn physical_operation(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(fe2o3_kernel_ir::StorageOperationV1, Option<ValueId>)> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok((self.object.actual.operation, self.object.actual.result))
    }

    /// Number of original endpoints; projections and copies retain both sides.
    pub fn endpoint_count(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<usize> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok(match self.object.source.role {
            ScopedObjectRoleV29::Project { .. } | ScopedObjectRoleV29::CopyObject { .. } => 2,
            ScopedObjectRoleV29::ReadValue { .. }
            | ScopedObjectRoleV29::WriteValue { .. }
            | ScopedObjectRoleV29::ReadDiscriminant { .. }
            | ScopedObjectRoleV29::SetDiscriminant { .. } => 1,
        })
    }

    /// Returns one source endpoint in operand order; invalid ordinals refuse.
    pub fn endpoint(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceObjectEndpointV39<'view, 'source>> {
        use ProductionSourceObjectEndpointRoleV39 as Role;
        self.owner.retain_query((|| {
            source_object_endpoint_check_v39(self.owner, self.required, budget)?;
            let (endpoint, role) = match (&self.object.source.role, ordinal) {
                (ScopedObjectRoleV29::Project { source, .. }, 0) => (source, Role::ProjectionBase),
                (ScopedObjectRoleV29::Project { projected, .. }, 1) => {
                    (projected, Role::ProjectionResult)
                }
                (ScopedObjectRoleV29::ReadValue { source, .. }, 0) => (source, Role::Read),
                (ScopedObjectRoleV29::WriteValue { destination, .. }, 0) => {
                    (destination, Role::Write)
                }
                (ScopedObjectRoleV29::CopyObject { source, .. }, 0) => (source, Role::CopySource),
                (ScopedObjectRoleV29::CopyObject { destination, .. }, 1) => {
                    (destination, Role::CopyDestination)
                }
                (ScopedObjectRoleV29::ReadDiscriminant { source, .. }, 0) => {
                    (source, Role::DiscriminantRead)
                }
                (ScopedObjectRoleV29::SetDiscriminant { destination, .. }, 0) => {
                    (destination, Role::DiscriminantWrite)
                }
                _ => return self.owner.source.missing("object endpoint ordinal"),
            };
            Ok(ProductionSourceObjectEndpointV39 {
                owner: self.owner,
                anchors: self.anchors,
                endpoint,
                role,
                required: self.required,
            })
        })())
    }
}

impl ProductionSourceObjectEndpointV39<'_, '_> {
    /// Exact endpoint role in the retained original operation.
    pub fn role(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceObjectEndpointRoleV39> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok(self.role)
    }

    /// Nominal object category; non-local objects must not be guessed as locals.
    pub fn object_class(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceObjectClassV39> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok(match self.endpoint.object {
            ScopedObjectIdentityV29::Local { .. } => ProductionSourceObjectClassV39::Local,
            ScopedObjectIdentityV29::Snapshot { .. } => ProductionSourceObjectClassV39::Snapshot,
            ScopedObjectIdentityV29::Reference { .. } => ProductionSourceObjectClassV39::Reference,
            ScopedObjectIdentityV29::EntryValue { .. } => {
                ProductionSourceObjectClassV39::EntryValue
            }
            ScopedObjectIdentityV29::InlineArgument { .. } => {
                ProductionSourceObjectClassV39::InlineArgument
            }
        })
    }

    /// Original instance, local and logical static storage generation, if local.
    /// The generation is neither a dynamic lifetime nor a physical representative.
    pub fn local_identity(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<(usize, SemanticLocalIdV1, u32)>> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok(match self.endpoint.object {
            ScopedObjectIdentityV29::Local {
                instance,
                local,
                generation,
            } => Some((instance.index(), local, generation)),
            ScopedObjectIdentityV29::Snapshot { .. }
            | ScopedObjectIdentityV29::Reference { .. }
            | ScopedObjectIdentityV29::EntryValue { .. }
            | ScopedObjectIdentityV29::InlineArgument { .. } => None,
        })
    }

    /// Original place site, operand role, local and retained projection prefix.
    /// None denotes a non-place origin such as entry/component/snapshot data.
    pub fn original_place(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<
        Option<(
            fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
            fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
            SemanticLocalIdV1,
            u32,
        )>,
    > {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok(match self.endpoint.source {
            ScopedObjectSourceV29::Place {
                site,
                role,
                local,
                prefix,
            } => Some((site, role, local, prefix)),
            ScopedObjectSourceV29::EntryArgument { .. }
            | ScopedObjectSourceV29::ProjectionIndex(_)
            | ScopedObjectSourceV29::AggregateComponent { .. }
            | ScopedObjectSourceV29::EntryComponent { .. }
            | ScopedObjectSourceV29::OperandSnapshot { .. }
            | ScopedObjectSourceV29::CallResultSnapshot { .. }
            | ScopedObjectSourceV29::ReturnComponent { .. } => None,
        })
    }

    /// Exact original root and projected semantic types.
    pub fn source_types(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(SemanticTypeIdV1, SemanticTypeIdV1)> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok((self.endpoint.root_type, self.endpoint.projected_type))
    }

    /// Exact original owned storage schemas; these do not grant source validity.
    pub fn storage_layouts(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        fe2o3_kernel_ir::StorageLayoutIdV1,
        fe2o3_kernel_ir::StorageLayoutIdV1,
    )> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok((self.endpoint.root_schema, self.endpoint.projected_schema))
    }

    /// Complete count of retained original source projections.
    pub fn original_projection_count(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        source_object_endpoint_check_v39(self.owner, self.required, budget)?;
        Ok(self.endpoint.source_path.count)
    }

    /// One original projection and optional exact event/promoted SSA selector.
    /// A retained selector has Some((event, None)); absence is not fabricated SSA.
    pub fn original_projection(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(SemanticProjectionV1, Option<(usize, Option<SsaValueV1>)>)> {
        self.owner.retain_query((|| {
            source_object_endpoint_check_v39(self.owner, self.required, budget)?;
            let path = self
                .anchors
                .object_path(self.endpoint.source_path, budget)
                .map_err(|error| source_attachment_error_v18(error.into()))?;
            let Some(ScopedObjectComponentV29::Original {
                projection,
                selector,
            }) = path.get(ordinal)
            else {
                return self
                    .owner
                    .source
                    .missing("object endpoint original projection ordinal or kind");
            };
            Ok((
                *projection,
                selector.map(|selector| match selector {
                    ScopedMemoryOccurrenceV29::Promoted { event, definition } => {
                        (event, Some(definition))
                    }
                    ScopedMemoryOccurrenceV29::Retained { event } => (event, None),
                }),
            ))
        })())
    }
}
