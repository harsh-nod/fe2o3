#[derive(Clone, Copy)]
enum KirBridgeTypeProfileV12<'input> {
    Legacy,
    V12,
    V18(storage_v18::ProfileV18<'input>),
}

impl KirBridgeTypeProfileV12<'_> {
    fn preserved_kind(
        self,
        kind: &OperationKind,
    ) -> Result<PreservedOperationKindAttr, KirBridgeErrorV1> {
        match self {
            Self::V18(_) => storage_v18::preserved_kind(kind),
            Self::Legacy | Self::V12 => preserved_operation_kind(kind),
        }
    }

    fn remap_preserved(
        self,
        kind: &OperationKind,
        values: Vec<ValueId>,
    ) -> Result<OperationKind, KirBridgeErrorV1> {
        match self {
            Self::V18(_) => storage_v18::remap(kind, values),
            Self::Legacy | Self::V12 => remap_preserved_operation(kind, values),
        }
    }

    fn validate_module(self, module: &Module) -> Result<(), KirBridgeErrorV1> {
        if let Self::V18(profile) = self {
            profile.validate_module(module)?;
        } else if !module.storage_layouts.is_empty() {
            return Err(KirBridgeErrorV1::UnsupportedType);
        }
        Ok(())
    }

    fn preflight_type(self, ty: &Type) -> Result<(), KirBridgeErrorV1> {
        match self {
            Self::Legacy => preflight_type(ty),
            Self::V18(profile) => profile.preflight_type(ty),
            Self::V12 => match ty {
                Type::Execution(_) | Type::StorageObject(_) => {
                    Err(KirBridgeErrorV1::UnsupportedType)
                }
                Type::Vector(vector) => vector
                    .validate()
                    .map_err(|_| KirBridgeErrorV1::UnsupportedType),
                Type::Pointer(pointer) => self.preflight_type(&pointer.pointee),
                Type::Slice(slice) => self.preflight_type(&slice.element),
                Type::Unit | Type::Scalar(_) => Ok(()),
            },
        }
    }

    fn preflight_operation(
        self,
        operation: &KirOperation,
        coordinate: KirBridgeCoordinateV1,
    ) -> Result<(), KirBridgeErrorV1> {
        if let Self::V18(profile) = self {
            return profile.preflight_operation(operation, coordinate);
        }
        if matches!(self, Self::V12) {
            match &operation.kind {
                OperationKind::VerificationContract(_) | OperationKind::VectorLayoutConvert(_) => {
                    return Ok(());
                }
                OperationKind::VectorLoad(load) => {
                    return load
                        .access
                        .vector
                        .validate()
                        .map_err(|_| KirBridgeErrorV1::UnsupportedType);
                }
                OperationKind::VectorStore(store) => {
                    return store
                        .access
                        .vector
                        .validate()
                        .map_err(|_| KirBridgeErrorV1::UnsupportedType);
                }
                _ => {}
            }
        }
        preflight_operation(operation, coordinate)
    }

    fn to_pliron(self, context: &Context, ty: &Type) -> Result<TypeHandle, KirBridgeErrorV1> {
        if let Self::V18(profile) = self {
            return profile.to_pliron(context, ty);
        }
        if matches!(self, Self::Legacy) {
            return type_to_pliron(context, ty);
        }
        Ok(match ty {
            Type::Execution(_) | Type::StorageObject(_) => {
                return Err(KirBridgeErrorV1::UnsupportedType);
            }
            Type::Vector(vector) => {
                vector
                    .validate()
                    .map_err(|_| KirBridgeErrorV1::UnsupportedType)?;
                PlironFixedVectorTypeV12::try_get(
                    context,
                    type_to_pliron(context, &Type::Scalar(vector.element))?,
                    vector.lanes,
                    match vector.layout {
                        fe2o3_kernel_ir::VectorLayoutV12::Contiguous => {
                            dialect_gpu::vector_v12::VectorLayoutAttrV12::CONTIGUOUS
                        }
                        fe2o3_kernel_ir::VectorLayoutV12::Interleaved { factor } => {
                            dialect_gpu::vector_v12::VectorLayoutAttrV12::interleaved(factor)
                        }
                    },
                )
                .ok_or(KirBridgeErrorV1::UnsupportedType)?
                .into()
            }
            Type::Pointer(pointer) => PlironPointerType::get(
                context,
                self.to_pliron(context, &pointer.pointee)?,
                address_space_to_pliron(pointer.address_space)?,
                access_mode_to_pliron(pointer.access),
            )
            .into(),
            Type::Slice(slice) => PlironSliceType::get(
                context,
                self.to_pliron(context, &slice.element)?,
                address_space_to_pliron(slice.address_space)?,
                access_mode_to_pliron(slice.access),
            )
            .into(),
            Type::Unit | Type::Scalar(_) => type_to_pliron(context, ty)?,
        })
    }

    fn decode_type(self, context: &Context, ty: TypeHandle) -> Result<Type, KirBridgeErrorV1> {
        if matches!(self, Self::Legacy) {
            return type_from_pliron(context, ty);
        }
        self.decode_type_depth(context, ty, 0)
    }

    fn decode_type_depth(
        self,
        context: &Context,
        ty: TypeHandle,
        depth: usize,
    ) -> Result<Type, KirBridgeErrorV1> {
        if let Self::V18(profile) = self {
            return profile.decode_type(context, ty, depth);
        }
        if depth > fe2o3_kernel_ir::MAX_TYPE_DEPTH_V1 {
            return Err(KirBridgeErrorV1::UnsupportedType);
        }
        let raw = ty.deref(context);
        if let Some(vector) = raw.downcast_ref::<PlironFixedVectorTypeV12>() {
            let Type::Scalar(element) = type_from_pliron(context, vector.element())? else {
                return Err(KirBridgeErrorV1::UnsupportedType);
            };
            let descriptor = fe2o3_kernel_ir::FixedVectorTypeV12::new(
                element,
                vector.lanes(),
                match vector.layout().interleave_factor() {
                    None => fe2o3_kernel_ir::VectorLayoutV12::Contiguous,
                    Some(factor) => fe2o3_kernel_ir::VectorLayoutV12::Interleaved { factor },
                },
            );
            descriptor
                .validate()
                .map_err(|_| KirBridgeErrorV1::UnsupportedType)?;
            return Ok(Type::Vector(descriptor));
        }
        if let Some(pointer) = raw.downcast_ref::<PlironPointerType>() {
            return Ok(Type::pointer(
                self.decode_type_depth(context, pointer.pointee(), depth + 1)?,
                address_space_from_pliron(pointer.address_space()),
                access_mode_from_pliron(pointer.access()),
            ));
        }
        if let Some(slice) = raw.downcast_ref::<PlironSliceType>() {
            return Ok(Type::slice(
                self.decode_type_depth(context, slice.element(), depth + 1)?,
                address_space_from_pliron(slice.address_space()),
                access_mode_from_pliron(slice.access()),
            ));
        }
        type_from_pliron(context, ty)
    }
}
