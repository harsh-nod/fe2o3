// Slot facts share the old accepted set without constructing query-side pointer Types.

trait PrivateArrayChargeV1 {
    type Error;

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error>;
}

struct PrivateArrayNoWorkV1;

impl PrivateArrayChargeV1 for PrivateArrayNoWorkV1 {
    type Error = std::convert::Infallible;

    fn charge_private_array_work(&mut self, _amount: usize) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrivateRetainedElementFactsV1 {
    Scalar(ScalarType),
    ThinPointer {
        element: ScalarType,
        space: AddressSpace,
        access: AccessMode,
    },
}

impl PrivateRetainedElementFactsV1 {
    fn into_owned_type(self) -> Type {
        match self {
            Self::Scalar(scalar) => Type::Scalar(scalar),
            Self::ThinPointer {
                element,
                space,
                access,
            } => Type::pointer(Type::Scalar(element), space, access),
        }
    }

    fn matches_borrowed<W: PrivateArrayChargeV1>(
        self,
        actual: &Type,
        work: &mut W,
    ) -> Result<bool, W::Error> {
        work.charge_private_array_work(1)?;
        match (self, actual) {
            (Self::Scalar(expected), Type::Scalar(actual)) => {
                work.charge_private_array_work(1)?;
                Ok(expected == *actual)
            }
            (
                Self::ThinPointer {
                    element,
                    space,
                    access,
                },
                Type::Pointer(pointer),
            ) => {
                // Pointee scalar, address space, and access are independent comparisons.
                work.charge_private_array_work(3)?;
                Ok(pointer.pointee.as_ref() == &Type::Scalar(element)
                    && pointer.address_space == space
                    && pointer.access == access)
            }
            _ => Ok(false),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrivateRetainedSlotFactsV1 {
    element: PrivateRetainedElementFactsV1,
    size: u64,
    alignment: u32,
}

fn private_scalar_declaration_fact_v1<W: PrivateArrayChargeV1>(
    declaration: &SemanticTypeDeclV1,
    work: &mut W,
) -> Result<Option<ScalarType>, W::Error> {
    work.charge_private_array_work(1)?;
    let scalar = match declaration.shape() {
        SemanticTypeShapeV1::Scalar(scalar) => *scalar,
        SemanticTypeShapeV1::ValidityScalar(validity) => validity.scalar(),
        _ => return Ok(None),
    };
    work.charge_private_array_work(1)?;
    // lower_scalar_kind has only static Unsupported errors. Unlike lower_scalar_type,
    // it cannot format a non-scalar declaration or allocate that diagnostic.
    Ok(match lower_scalar_kind(scalar) {
        Ok(Type::Scalar(scalar)) => Some(scalar),
        Ok(_) | Err(_) => None,
    })
}

fn private_memory_scalar_fact_v1<W: PrivateArrayChargeV1>(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    work: &mut W,
) -> Result<Option<ScalarType>, W::Error> {
    work.charge_private_array_work(1)?;
    let Some(declaration) = types.get(ty.index() as usize) else {
        return Ok(None);
    };
    // The old direct scalar probe precedes any pointee inhabitedness check.
    if let Some(scalar) = private_scalar_declaration_fact_v1(declaration, work)? {
        return Ok(Some(scalar));
    }

    let mut current = ty;
    for _ in 0..MAX_SSA_VALUE_COMPONENTS_V1 {
        work.charge_private_array_work(1)?;
        let Some(declaration) = types.get(current.index() as usize) else {
            return Ok(None);
        };
        if let Some(scalar) = private_scalar_declaration_fact_v1(declaration, work)? {
            return Ok(Some(scalar));
        }
        work.charge_private_array_work(1)?;
        let SemanticTypeShapeV1::Aggregate(aggregate) = declaration.shape() else {
            return Ok(None);
        };
        work.charge_private_array_work(1)?;
        let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
            return Ok(None);
        };
        // Inhabited wrapper, one field, one zero offset, and no padding.
        work.charge_private_array_work(5)?;
        if declaration.layout().is_uninhabited()
            || aggregate.fields().len() != 1
            || layout.field_offsets().len() != 1
            || layout.field_offsets()[0] != 0
            || !layout.padding().is_empty()
        {
            return Ok(None);
        }
        work.charge_private_array_work(2)?;
        let field_ty = aggregate.fields()[0];
        let Some(field) = types.get(field_ty.index() as usize) else {
            return Ok(None);
        };
        // The old wrapper step checks the field before advancing to its scalar probe.
        work.charge_private_array_work(3)?;
        if field.layout().is_uninhabited()
            || declaration.layout().size_bytes() != field.layout().size_bytes()
            || declaration.layout().alignment_bytes() != field.layout().alignment_bytes()
        {
            return Ok(None);
        }
        current = field_ty;
    }
    Ok(None)
}

fn private_retained_slot_facts_v1<W: PrivateArrayChargeV1>(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    work: &mut W,
) -> Result<Option<PrivateRetainedSlotFactsV1>, W::Error> {
    work.charge_private_array_work(1)?;
    let Some(declaration) = types.get(ty.index() as usize) else {
        return Ok(None);
    };
    let layout = declaration.layout();
    work.charge_private_array_work(1)?;
    if layout.is_uninhabited() {
        return Ok(None);
    }
    work.charge_private_array_work(1)?;
    let (element, size) = match declaration.shape() {
        SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
            let Some(scalar) = private_scalar_declaration_fact_v1(declaration, work)? else {
                return Ok(None);
            };
            work.charge_private_array_work(1)?;
            let size = match scalar {
                ScalarType::Bool => 1,
                ScalarType::Index => return Ok(None),
                scalar => {
                    let Some(bits) = scalar.bit_width() else {
                        return Ok(None);
                    };
                    u64::from(bits / 8)
                }
            };
            (PrivateRetainedElementFactsV1::Scalar(scalar), size)
        }
        SemanticTypeShapeV1::Pointer(pointer) => {
            work.charge_private_array_work(2)?;
            if pointer.metadata() != SemanticPointerMetadataV1::None
                || !matches!(pointer.pointer_width_bits(), 32 | 64)
            {
                return Ok(None);
            }
            let Some(element) = private_memory_scalar_fact_v1(types, pointer.pointee(), work)?
            else {
                return Ok(None);
            };
            work.charge_private_array_work(1)?;
            let Ok(space) = lower_address_space(pointer.address_space()) else {
                return Ok(None);
            };
            work.charge_private_array_work(1)?;
            let access = match pointer.mutability() {
                SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
                SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
            };
            work.charge_private_array_work(1)?;
            (
                PrivateRetainedElementFactsV1::ThinPointer {
                    element,
                    space,
                    access,
                },
                u64::from(pointer.pointer_width_bits() / 8),
            )
        }
        _ => return Ok(None),
    };
    work.charge_private_array_work(1)?;
    let Ok(alignment) = u32::try_from(layout.alignment_bytes()) else {
        return Ok(None);
    };
    work.charge_private_array_work(4)?;
    if layout.size_bytes() != Some(size)
        || alignment == 0
        || !alignment.is_power_of_two()
        || u64::from(alignment) > size
    {
        return Ok(None);
    }
    // Only Scalar/Pointer facts are constructible here; both satisfy Type::is_storable.
    Ok(Some(PrivateRetainedSlotFactsV1 {
        element,
        size,
        alignment,
    }))
}

// Replacement body only; keep the existing public/private boundary and caller errors.
fn retained_local_slot_type_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<(Type, u32)> {
    let facts = match private_retained_slot_facts_v1(types, ty, &mut PrivateArrayNoWorkV1) {
        Ok(facts) => facts?,
        Err(never) => match never {},
    };
    Some((facts.element.into_owned_type(), facts.alignment))
}
