// Original geometry constructors and source subobjects; initialized-byte queries are deferred.
include!("production_source_storage_geometry_v29.rs");

fn scalar_kind(scalar: SemanticScalarTypeV1) -> Result<ScalarType, Error> {
    match lower_scalar_kind(scalar)? {
        Type::Scalar(scalar) => Ok(scalar),
        _ => Err(error("source storage scalar lowering changed kind")),
    }
}

fn primitive_scalar(primitive: SemanticBackendPrimitiveV1) -> Result<ScalarType, Error> {
    match primitive {
        SemanticBackendPrimitiveV1::Integer { signed, bits, .. } => {
            scalar_kind(SemanticScalarTypeV1::Integer { signed, bits })
        }
        SemanticBackendPrimitiveV1::Float { bits, .. } => {
            scalar_kind(SemanticScalarTypeV1::Float { bits })
        }
        SemanticBackendPrimitiveV1::Pointer { .. } => {
            Err(error("source storage pointer bits are not a scalar value"))
        }
    }
}

fn physical_alignment(alignment: u64) -> Result<u32, Error> {
    let alignment = u32::try_from(alignment).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    if !alignment.is_power_of_two() {
        return Err(error("source storage alignment is invalid"));
    }
    Ok(alignment)
}

fn exact_box<T>(rows: Vec<T>) -> Result<Box<[T]>, Error> {
    // With equal length/capacity this conversion cannot request replacement
    // backing. Larger allocator capacity must not hide a shrinking allocation.
    if rows.len() != rows.capacity() {
        return Err(ArgumentResourceV1::Allocation.into());
    }
    Ok(rows.into_boxed_slice())
}

impl SourceStorageLayoutsV29<'_> {
    fn pointer_space(&self, address_space: u32) -> Result<AddressSpace, Error> {
        match self.owner.source_semantic().target().architecture() {
            // Stored pointer bits retain the captured target encoding. The
            // legacy kernel ABI mapper deliberately coalesces generic/global.
            SemanticTargetArchitectureV1::AmdGpuGfx942 => match address_space {
                0 => Ok(AddressSpace::Generic),
                1 => Ok(AddressSpace::Global),
                3 => Ok(AddressSpace::Workgroup),
                4 => Ok(AddressSpace::Constant),
                5 => Ok(AddressSpace::Private),
                _ => Err(error(
                    "source storage pointer requires an additional admitted target address-space contract",
                )),
            },
        }
    }

    fn pointer_access(pointer: &SemanticPointerTypeV1) -> AccessMode {
        // An inert representation restriction, not ownership or allocation
        // authority. The source access/loan checker still controls every use.
        match pointer.mutability() {
            SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
            SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
        }
    }

    fn slice_element(&self, pointer: &SemanticPointerTypeV1) -> Result<SemanticTypeIdV1, Error> {
        match self.declaration(pointer.pointee())?.shape() {
            SemanticTypeShapeV1::Slice { element } => Ok(*element),
            _ => Err(error(
                "source storage slice metadata has a non-slice referent",
            )),
        }
    }

    fn follow_type(
        &mut self,
        ty: SemanticTypeIdV1,
        selected: &mut [bool],
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.lease.work(1, budget)?;
        let declaration = self.declaration(ty)?;
        if declaration.layout().size_bytes().is_none() {
            return Err(error(
                "source storage cannot fabricate a sized object for an unsized source type",
            ));
        }
        match declaration.shape() {
            SemanticTypeShapeV1::Unit
            | SemanticTypeShapeV1::Never
            | SemanticTypeShapeV1::Scalar(_)
            | SemanticTypeShapeV1::ValidityScalar(_) => {}
            SemanticTypeShapeV1::Array { element, .. } => {
                self.enqueue(*element, selected, budget)?
            }
            SemanticTypeShapeV1::Tuple(aggregate)
            | SemanticTypeShapeV1::Aggregate(aggregate)
            | SemanticTypeShapeV1::Union(aggregate) => {
                for &field in aggregate.fields() {
                    self.enqueue(field, selected, budget)?;
                }
            }
            SemanticTypeShapeV1::Pointer(pointer) => match pointer.metadata() {
                SemanticPointerMetadataV1::None => {
                    self.enqueue(pointer.pointee(), selected, budget)?
                }
                SemanticPointerMetadataV1::SliceLength => {
                    self.enqueue(self.slice_element(pointer)?, selected, budget)?;
                    self.auxiliary(ty, RowRole::DescriptorData, budget)?;
                    self.auxiliary(ty, RowRole::DescriptorLength, budget)?;
                }
                SemanticPointerMetadataV1::VTable => {
                    return Err(error(
                        "source storage vtable metadata needs its admitted descriptor contract",
                    ));
                }
            },
            SemanticTypeShapeV1::Enum { variants, .. } => match declaration.layout().variants() {
                SemanticRustcVariantsV1::Multiple(_) => {
                    self.auxiliary(ty, RowRole::Tag, budget)?;
                    for (index, variant) in variants.iter().enumerate() {
                        self.auxiliary(
                            ty,
                            RowRole::Payload(
                                u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            ),
                            budget,
                        )?;
                        for &field in variant.fields().fields() {
                            self.enqueue(field, selected, budget)?;
                        }
                    }
                }
                SemanticRustcVariantsV1::Single { index } => {
                    let variant = variants
                        .get(*index as usize)
                        .ok_or_else(|| error("source storage single variant is absent"))?;
                    for &field in variant.fields().fields() {
                        self.enqueue(field, selected, budget)?;
                    }
                }
                SemanticRustcVariantsV1::Empty => {}
            },
            SemanticTypeShapeV1::Opaque
                if matches!(
                    declaration.layout().backend_repr(),
                    SemanticBackendReprV1::SimdVector { .. }
                ) => {}
            SemanticTypeShapeV1::Slice { .. }
            | SemanticTypeShapeV1::FunctionPointer { .. }
            | SemanticTypeShapeV1::Opaque => {
                return Err(error(
                    "source storage type requires additional exact physical representation facts",
                ));
            }
        }
        Ok(())
    }

    fn field_rows(
        &self,
        fields: &[SemanticTypeIdV1],
        offsets: &[u64],
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<Box<[StorageFieldV1]>, Error> {
        if fields.len() != offsets.len() {
            return Err(error(
                "source storage field offsets differ from source field roster",
            ));
        }
        let mut result = self.lease.vector(fields.len(), budget)?;
        for (&ty, &offset) in fields.iter().zip(offsets) {
            self.lease.work(1, budget)?;
            let layout = self.original_resolve(RowKey::ty(ty), resolver, budget)?;
            self.lease
                .push(&mut result, StorageFieldV1 { offset, layout }, budget)?;
        }
        exact_box(result)
    }

    fn aggregate_fields(
        &self,
        declaration: &SemanticTypeDeclV1,
        fields: &[SemanticTypeIdV1],
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<Box<[StorageFieldV1]>, Error> {
        let offsets = match declaration.layout().details() {
            SemanticTypeLayoutDetailsV1::Aggregate(layout) => layout.field_offsets(),
            SemanticTypeLayoutDetailsV1::None if fields.is_empty() => &[],
            _ => {
                return Err(error(
                    "source storage aggregate has no exact field-offset facts",
                ));
            }
        };
        self.field_rows(fields, offsets, resolver, budget)
    }

    fn union_fields(
        &self,
        declaration: &SemanticTypeDeclV1,
        fields: &[SemanticTypeIdV1],
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<Box<[StorageFieldV1]>, Error> {
        if !matches!(declaration.layout().fields(), SemanticFieldsShapeV1::Union { field_count }
            if *field_count as usize == fields.len())
            || !matches!(
                declaration.layout().details(),
                SemanticTypeLayoutDetailsV1::None
            )
        {
            return Err(error(
                "source union lacks its exact overlapping field roster",
            ));
        }
        let mut result = self.lease.vector(fields.len(), budget)?;
        for &ty in fields {
            self.lease.work(1, budget)?;
            let layout = self.original_resolve(RowKey::ty(ty), resolver, budget)?;
            self.lease
                .push(&mut result, StorageFieldV1 { offset: 0, layout }, budget)?;
        }
        exact_box(result)
    }

    fn thin_pointer_row(
        &self,
        pointer: &SemanticPointerTypeV1,
        pointee: SemanticTypeIdV1,
        size: u64,
        alignment: u64,
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutV1, Error> {
        self.lease.work(1, budget)?;
        if size.checked_mul(8) != Some(u64::from(pointer.pointer_width_bits())) {
            return Err(error(
                "source storage pointer layout disagrees with admitted pointer width",
            ));
        }
        let space = self.pointer_space(pointer.address_space())?;
        Ok(StorageLayoutV1 {
            size,
            alignment: physical_alignment(alignment)?,
            kind: StorageLayoutKindV1::Pointer(StoragePointerV1 {
                pointee: self.original_resolve(RowKey::ty(pointee), resolver, budget)?,
                value_space: space,
                encoded_space: space,
                access: Self::pointer_access(pointer),
                stored_bits: pointer.pointer_width_bits(),
            }),
        })
    }

    fn descriptor_parts(
        &self,
        ty: SemanticTypeIdV1,
    ) -> Result<(SemanticBackendPrimitiveV1, SemanticBackendPrimitiveV1, u64), Error> {
        let declaration = self.declaration(ty)?;
        let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
            return Err(error("source descriptor row has a non-pointer source type"));
        };
        if pointer.metadata() != SemanticPointerMetadataV1::SliceLength {
            return Err(error(
                "source descriptor row does not carry slice length metadata",
            ));
        }
        let SemanticBackendReprV1::ScalarPair { first, second } =
            declaration.layout().backend_repr()
        else {
            return Err(error(
                "source slice descriptor lacks exact scalar-pair representation",
            ));
        };
        let first = first.primitive();
        let second = second.primitive();
        if !matches!(first, SemanticBackendPrimitiveV1::Pointer { address_space, size_bytes, .. }
            if address_space == pointer.address_space() && size_bytes.checked_mul(8) == Some(u64::from(pointer.pointer_width_bits())))
            || !matches!(
                second,
                SemanticBackendPrimitiveV1::Integer {
                    signed: false,
                    bits: 64,
                    ..
                }
            )
        {
            return Err(error(
                "source slice descriptor data and length representation changed",
            ));
        }
        let first_size = first.size_bytes().ok_or(ArgumentResourceV1::Arithmetic)?;
        let align = second.alignment_bytes();
        let offset = first_size
            .checked_add(align - 1)
            .map(|value| value & !(align - 1))
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if let SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            ..
        } = declaration.layout().fields()
            && source_order_offsets_bytes.as_ref() != [0, offset]
        {
            return Err(error(
                "source descriptor field placement disagrees with its scalar pair",
            ));
        }
        Ok((first, second, offset))
    }

    fn descriptor_row(
        &self,
        key: RowKey,
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutV1, Error> {
        let declaration = self.declaration(key.ty)?;
        let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
            return Err(error("source descriptor key changed source type"));
        };
        let (data, length, offset) = self.descriptor_parts(key.ty)?;
        match key.role {
            RowRole::DescriptorData => self.thin_pointer_row(
                pointer,
                self.slice_element(pointer)?,
                data.size_bytes().ok_or(ArgumentResourceV1::Arithmetic)?,
                data.alignment_bytes(),
                resolver,
                budget,
            ),
            RowRole::DescriptorLength => Ok(StorageLayoutV1 {
                size: length.size_bytes().ok_or(ArgumentResourceV1::Arithmetic)?,
                alignment: physical_alignment(length.alignment_bytes())?,
                kind: StorageLayoutKindV1::Scalar(ScalarType::Index),
            }),
            RowRole::Type => Ok(StorageLayoutV1 {
                size: declaration
                    .layout()
                    .size_bytes()
                    .ok_or_else(|| error("source descriptor is unsized"))?,
                alignment: physical_alignment(declaration.layout().alignment_bytes())?,
                kind: StorageLayoutKindV1::Slice {
                    element: self.original_resolve(
                        RowKey::ty(self.slice_element(pointer)?),
                        resolver,
                        budget,
                    )?,
                    value_space: self.pointer_space(pointer.address_space())?,
                    access: Self::pointer_access(pointer),
                    data: StorageFieldV1 {
                        offset: 0,
                        layout: self.original_resolve(
                            RowKey {
                                ty: key.ty,
                                role: RowRole::DescriptorData,
                            },
                            resolver,
                            budget,
                        )?,
                    },
                    length: StorageFieldV1 {
                        offset,
                        layout: self.original_resolve(
                            RowKey {
                                ty: key.ty,
                                role: RowRole::DescriptorLength,
                            },
                            resolver,
                            budget,
                        )?,
                    },
                },
            }),
            _ => Err(error("source descriptor row key has the wrong role")),
        }
    }

    fn lower_row(&self, key: RowKey, budget: &mut Budget<'_>) -> Result<StorageLayoutV1, Error> {
        self.lower_row_resolved(key, &SourceStorageOriginalResolverV29::Original, budget)
    }

    fn lower_row_resolved(
        &self,
        key: RowKey,
        resolver: &SourceStorageOriginalResolverV29<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<StorageLayoutV1, Error> {
        self.lease.work(1, budget)?;
        let declaration = self.declaration(key.ty)?;
        match key.role {
            RowRole::Payload(_) | RowRole::Tag => {
                return self.enum_auxiliary_resolved(key, resolver, budget);
            }
            RowRole::DescriptorData | RowRole::DescriptorLength => {
                return self.descriptor_row(key, resolver, budget);
            }
            RowRole::Type => {}
        }
        let layout = declaration.layout();
        let size = layout
            .size_bytes()
            .ok_or_else(|| error("source storage object is unsized"))?;
        let alignment = physical_alignment(layout.alignment_bytes())?;
        let kind =
            match declaration.shape() {
                SemanticTypeShapeV1::Unit | SemanticTypeShapeV1::Never => {
                    StorageLayoutKindV1::Record(exact_box(self.lease.vector(0, budget)?)?)
                }
                SemanticTypeShapeV1::Scalar(scalar) => {
                    StorageLayoutKindV1::Scalar(scalar_kind(*scalar)?)
                }
                SemanticTypeShapeV1::ValidityScalar(scalar) => {
                    StorageLayoutKindV1::Scalar(scalar_kind(scalar.scalar())?)
                }
                SemanticTypeShapeV1::Pointer(pointer) => match pointer.metadata() {
                    SemanticPointerMetadataV1::None => {
                        return self.thin_pointer_row(
                            pointer,
                            pointer.pointee(),
                            size,
                            layout.alignment_bytes(),
                            resolver,
                            budget,
                        );
                    }
                    SemanticPointerMetadataV1::SliceLength => {
                        return self.descriptor_row(key, resolver, budget);
                    }
                    SemanticPointerMetadataV1::VTable => {
                        return Err(error(
                            "source storage vtable descriptor is not yet represented",
                        ));
                    }
                },
                SemanticTypeShapeV1::Array { element, length } => {
                    let SemanticFieldsShapeV1::Array {
                        stride_bytes,
                        count,
                    } = layout.fields()
                    else {
                        return Err(error("source array has no exact rustc stride"));
                    };
                    if count != length {
                        return Err(error("source array length differs from its layout"));
                    }
                    StorageLayoutKindV1::Array {
                        element: self.original_resolve(RowKey::ty(*element), resolver, budget)?,
                        length: *length,
                        stride: *stride_bytes,
                    }
                }
                SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                    StorageLayoutKindV1::Record(self.aggregate_fields(
                        declaration,
                        fields.fields(),
                        resolver,
                        budget,
                    )?)
                }
                SemanticTypeShapeV1::Union(fields) => StorageLayoutKindV1::Union(
                    self.union_fields(declaration, fields.fields(), resolver, budget)?,
                ),
                SemanticTypeShapeV1::Enum { .. } => return self.enum_row(key.ty, resolver, budget),
                SemanticTypeShapeV1::Opaque => {
                    let SemanticBackendReprV1::SimdVector { element, count } =
                        layout.backend_repr()
                    else {
                        return Err(error(
                            "opaque source storage lacks exact vector representation",
                        ));
                    };
                    let vector = FixedVectorTypeV12::new(
                        primitive_scalar(element.primitive())?,
                        u16::try_from(*count).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        VectorLayoutV12::Contiguous,
                    );
                    vector.validate().map_err(|_| {
                        error("source vector exceeds the admitted physical vector contract")
                    })?;
                    StorageLayoutKindV1::Vector(vector)
                }
                SemanticTypeShapeV1::Slice { .. } | SemanticTypeShapeV1::FunctionPointer { .. } => {
                    return Err(error(
                        "source storage type lacks a complete physical row contract",
                    ));
                }
            };
        Ok(StorageLayoutV1 {
            size,
            alignment,
            kind,
        })
    }
}
