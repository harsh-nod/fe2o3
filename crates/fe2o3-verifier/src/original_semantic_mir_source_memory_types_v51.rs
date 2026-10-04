//! Original typed memory shapes. Padding has no value or initializedness claim.
use super::super::super::{ScalarV30, Shape, Type, TypeId};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBackendPrimitiveV1 as Primitive, SemanticBackendReprV1 as Repr,
    SemanticBackendScalarV1 as BackendScalar, SemanticFieldsShapeV1 as Fields,
    SemanticMutabilityV1 as Mutability, SemanticPointerKindV1 as PointerKind,
    SemanticPointerMetadataV1 as Metadata,
};

const MAX_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Unsupported,
    Unit,
    Scalar(ScalarV30),
    Pointer {
        pointee: TypeId,
        reference: bool,
        mutable: bool,
    },
    Fields {
        first: usize,
        count: usize,
    },
    Array {
        element: TypeId,
        count: u64,
        stride: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Field {
    ty: TypeId,
    offset: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Layout {
    kind: Kind,
    bytes: u64,
    alignment: u64,
    depth: usize,
    copy: bool,
    validity: [Option<Validity>; 2],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Validity {
    offset: u64,
    bytes: u64,
    start: u128,
    end: u128,
    pointer: bool,
}

fn scalar_validity(scalar: BackendScalar, offset: u64) -> Option<Validity> {
    let range = scalar.valid_range()?;
    let (bytes, pointer) = match scalar.primitive() {
        Primitive::Integer {
            bits: 8 | 16 | 32 | 64,
            ..
        } => (scalar.primitive().size_bytes()?, false),
        Primitive::Float {
            bits: bits @ (32 | 64),
            ..
        } if range.start() == 0 && range.end() == (1u128 << bits) - 1 => {
            (scalar.primitive().size_bytes()?, false)
        }
        Primitive::Pointer {
            address_space: 0,
            size_bytes: 8,
            ..
        } if range.start() <= 1 && range.end() == u64::MAX.into() => (8, true),
        _ => return None,
    };
    Some(Validity {
        offset,
        bytes,
        start: range.start(),
        end: range.end(),
        pointer,
    })
}

fn validity(repr: &Repr) -> Option<[Option<Validity>; 2]> {
    match *repr {
        Repr::Memory { sized: true } => Some([None, None]),
        Repr::Scalar(value) => Some([Some(scalar_validity(value, 0)?), None]),
        Repr::ScalarPair { first, second } => {
            let size = first.primitive().size_bytes()?;
            let align = second.primitive().alignment_bytes();
            if align == 0 {
                return None;
            }
            let offset = size.checked_add(align - 1)? / align * align;
            Some([
                Some(scalar_validity(first, 0)?),
                Some(scalar_validity(second, offset)?),
            ])
        }
        _ => None,
    }
}

#[derive(Clone, Copy)]
struct Frame {
    ty: usize,
    next: usize,
    depth: usize,
    copy: bool,
    supported: bool,
}

pub(super) struct SourceMemoryTypesV51 {
    layouts: Vec<Layout>,
    fields: Vec<Field>,
}

fn mismatch() -> Error {
    Error::Statement("original typed memory layout differs from its source declaration")
}

fn total(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| Resource::Arithmetic.into())
}

impl SourceMemoryTypesV51 {
    pub(super) fn derive(
        types: &[Type],
        abi: &source_abi::SourceAbi,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let mut field_count = 0;
        for ty in types {
            out.budget.charge_work(1)?;
            if let Shape::Tuple(fields) | Shape::Aggregate(fields) = ty.shape() {
                field_count = total(field_count, fields.fields().len())?;
            }
        }
        let mut layouts = vector(types.len(), out)?;
        let mut fields = vector(field_count, out)?;
        for (index, declaration) in types.iter().enumerate() {
            out.budget.charge_work(4)?;
            let ty = TypeId::from_index(u32::try_from(index).map_err(|_| Resource::Arithmetic)?);
            let layout = declaration.layout();
            let mut row = Layout {
                kind: Kind::Unsupported,
                bytes: layout.size_bytes().unwrap_or(0),
                alignment: layout.alignment_bytes(),
                depth: 0,
                copy: false,
                validity: [None, None],
            };
            // Nominal descriptor/witness contracts are not structural field lists.
            if layout.is_uninhabited()
                || layout.size_bytes().is_none()
                || abi.slice(ty, out)?.is_some()
                || abi.witness(ty, out)?.is_some()
            {
                layouts.push(row);
                continue;
            }
            let Some(validity) = validity(layout.backend_repr()) else {
                layouts.push(row);
                continue;
            };
            if validity.iter().flatten().any(|valid| {
                valid
                    .offset
                    .checked_add(valid.bytes)
                    .is_none_or(|end| end > row.bytes)
            }) {
                layouts.push(row);
                continue;
            }
            row.validity = validity;
            row.kind = match declaration.shape() {
                Shape::Unit if row.bytes == 0 => Kind::Unit,
                Shape::Scalar(_) => match ScalarV30::from_source(types, ty) {
                    Ok(scalar)
                        if row.bytes
                            == if scalar == ScalarV30::Bool {
                                1
                            } else {
                                u64::from(scalar.width() / 8)
                            } =>
                    {
                        Kind::Scalar(scalar)
                    }
                    _ => Kind::Unsupported,
                },
                Shape::Pointer(pointer)
                    if pointer.metadata() == Metadata::None
                        && pointer.address_space() == 0
                        && pointer.pointer_width_bits() == 64
                        && row.bytes == 8 =>
                {
                    Kind::Pointer {
                        pointee: pointer.pointee(),
                        reference: pointer.kind() == PointerKind::Reference,
                        mutable: pointer.mutability() == Mutability::Mutable,
                    }
                }
                Shape::Tuple(children) | Shape::Aggregate(children) => {
                    let offsets = layout
                        .fields()
                        .source_order_offsets_bytes()
                        .ok_or_else(mismatch)?;
                    if offsets.len() != children.fields().len() {
                        return Err(mismatch());
                    }
                    let first = fields.len();
                    let mut aligned = true;
                    for (&child_type, &offset) in children.fields().iter().zip(offsets) {
                        out.budget.charge_work(5)?;
                        let child = types
                            .get(child_type.index() as usize)
                            .ok_or_else(mismatch)?;
                        let Some(bytes) = child.layout().size_bytes() else {
                            return Err(mismatch());
                        };
                        if offset.checked_add(bytes).ok_or(Resource::Arithmetic)? > row.bytes {
                            return Err(mismatch());
                        }
                        // Byte initializedness cannot encode a moved ZST field.
                        // Until a path-definedness carrier is retained, do not
                        // turn an empty byte range into a complete value.
                        aligned &= bytes > 0
                            && offset % child.layout().alignment_bytes() == 0
                            && row.alignment % child.layout().alignment_bytes() == 0;
                        if fields.len() == fields.capacity() {
                            return Err(Resource::Accounting.into());
                        }
                        fields.push(Field {
                            ty: child_type,
                            offset,
                        });
                    }
                    let memory_order = layout
                        .fields()
                        .memory_order_source_indices()
                        .ok_or_else(mismatch)?;
                    if memory_order.len() != children.fields().len() {
                        return Err(mismatch());
                    }
                    let mut previous_end = 0;
                    for &ordinal in memory_order {
                        out.budget.charge_work(4)?;
                        let field = fields.get(first + ordinal as usize).ok_or_else(mismatch)?;
                        let bytes = types[field.ty.index() as usize]
                            .layout()
                            .size_bytes()
                            .ok_or_else(mismatch)?;
                        aligned &= field.offset >= previous_end;
                        previous_end = field
                            .offset
                            .checked_add(bytes)
                            .ok_or(Resource::Arithmetic)?;
                    }
                    if !aligned {
                        Kind::Unsupported
                    } else if fields.len() == first {
                        if row.bytes == 0 {
                            Kind::Unit
                        } else {
                            Kind::Unsupported
                        }
                    } else {
                        Kind::Fields {
                            first,
                            count: fields.len() - first,
                        }
                    }
                }
                Shape::Array { element, length } => {
                    let Fields::Array {
                        stride_bytes,
                        count,
                    } = layout.fields()
                    else {
                        return Err(mismatch());
                    };
                    let child = types.get(element.index() as usize).ok_or_else(mismatch)?;
                    let child_bytes = child.layout().size_bytes().ok_or_else(mismatch)?;
                    if count != length
                        || *stride_bytes != child_bytes
                        || length
                            .checked_mul(*stride_bytes)
                            .ok_or(Resource::Arithmetic)?
                            != row.bytes
                        || row.alignment % child.layout().alignment_bytes() != 0
                    {
                        return Err(mismatch());
                    }
                    if *length == 0 {
                        Kind::Unit
                    } else if child_bytes == 0 {
                        Kind::Unsupported
                    } else {
                        Kind::Array {
                            element: *element,
                            count: *length,
                            stride: *stride_bytes,
                        }
                    }
                }
                _ => Kind::Unsupported,
            };
            layouts.push(row);
        }

        // One paid iterative postorder. Arrays retain one element edge rather
        // than expanding by their length; reference cycles remain unsupported.
        let mut colors = vector(types.len(), out)?;
        colors.resize(types.len(), 0_u8);
        let mut stack = vector(types.len().min(MAX_DEPTH), out)?;
        out.budget.charge_work(types.len())?;
        for root in 0..layouts.len() {
            out.budget.charge_work(1)?;
            if colors[root] != 0 {
                continue;
            }
            colors[root] = 1;
            stack.push(Frame {
                ty: root,
                next: 0,
                depth: 1,
                copy: true,
                supported: true,
            });
            while let Some(frame) = stack.last().copied() {
                out.budget.charge_work(5)?;
                let child = match layouts[frame.ty].kind {
                    Kind::Fields { first, count } if frame.next < count => {
                        Some(fields[first + frame.next].ty)
                    }
                    Kind::Array { element, .. } if frame.next == 0 => Some(element),
                    Kind::Pointer {
                        pointee,
                        reference: true,
                        ..
                    } if frame.next == 0 => Some(pointee),
                    _ => None,
                };
                if let Some(child) = child {
                    let child = child.index() as usize;
                    let color = *colors.get(child).ok_or_else(mismatch)?;
                    if color == 0 && stack.len() < MAX_DEPTH {
                        if stack.len() == stack.capacity() {
                            return Err(Resource::Accounting.into());
                        }
                        colors[child] = 1;
                        stack.push(Frame {
                            ty: child,
                            next: 0,
                            depth: 1,
                            copy: true,
                            supported: true,
                        });
                        continue;
                    }
                    let current = stack.last_mut().ok_or_else(mismatch)?;
                    current.next += 1;
                    if color != 2 || layouts[child].depth == 0 {
                        current.supported = false;
                    } else {
                        current.depth = current.depth.max(total(layouts[child].depth, 1)?);
                        if !matches!(layouts[frame.ty].kind, Kind::Pointer { .. }) {
                            current.copy &= layouts[child].copy;
                        }
                    }
                    continue;
                }
                let row = &mut layouts[frame.ty];
                row.depth = if frame.supported
                    && frame.depth <= MAX_DEPTH
                    && !matches!(row.kind, Kind::Unsupported | Kind::Unit)
                {
                    frame.depth
                } else {
                    0
                };
                row.copy = row.depth > 0
                    && match row.kind {
                        Kind::Pointer {
                            reference: true,
                            mutable: true,
                            ..
                        } => false,
                        _ => frame.copy,
                    };
                colors[frame.ty] = 2;
                stack.pop();
            }
        }
        let scratch = total(
            colors
                .capacity()
                .checked_mul(size_of::<u8>())
                .ok_or(Resource::Arithmetic)?,
            stack
                .capacity()
                .checked_mul(size_of::<Frame>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        drop(colors);
        drop(stack);
        out.budget.release_storage(scratch)?;
        Ok(Self { layouts, fields })
    }

    pub(super) fn layout(
        &self,
        ty: TypeId,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<(u64, u64, bool)>> {
        out.budget.charge_work(3)?;
        let row = self.layouts.get(ty.index() as usize).ok_or_else(mismatch)?;
        Ok((row.depth > 0 && row.bytes > 0).then_some((row.bytes, row.alignment, row.copy)))
    }

    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.reserve_storage(headers())?;
        write!(out, "spec fn invocation_source_memory_layout_v51(ty: int) -> Option<InvocationSourceMemoryLayoutV51> {{\n")
            .map_err(|_| out.error())?;
        for (ty, row) in self.layouts.iter().enumerate() {
            out.budget.charge_work(2)?;
            if row.depth == 0 {
                continue;
            }
            write!(out, " if ty == {ty}int {{ Some(InvocationSourceMemoryLayoutV51 {{ width: {}int, alignment: {}int, depth: {}nat, copy: {}, kind: ", row.bytes, row.alignment, row.depth, row.copy)
                .map_err(|_| out.error())?;
            match row.kind {
                Kind::Unit => write!(out, "InvocationSourceMemoryKindV51::Unit"),
                Kind::Scalar(scalar) => write!(out, "InvocationSourceMemoryKindV51::Scalar({}int)", scalar.width()),
                Kind::Pointer { pointee, reference, mutable } => write!(out,
                    "InvocationSourceMemoryKindV51::Pointer {{ pointee: {}int, reference: {reference}, mutable: {mutable} }}", pointee.index()),
                Kind::Fields { first, count } => {
                    write!(out, "InvocationSourceMemoryKindV51::Fields(seq![").map_err(|_| out.error())?;
                    for field in &self.fields[first..first + count] {
                        out.budget.charge_work(1)?;
                        write!(out, "InvocationSourceMemoryFieldV51 {{ source_type: {}int, offset: {}int }},", field.ty.index(), field.offset).map_err(|_| out.error())?;
                    }
                    write!(out, "])")
                }
                Kind::Array { element, count, stride } => write!(out,
                    "InvocationSourceMemoryKindV51::Array {{ element: {}int, count: {count}int, stride: {stride}int }}", element.index()),
                Kind::Unsupported => return Err(Resource::Accounting.into()),
            }.map_err(|_| out.error())?;
            write!(out, ", validity: seq![").map_err(|_| out.error())?;
            for valid in row.validity.iter().flatten() {
                out.budget.charge_work(1)?;
                write!(out, "InvocationSourceMemoryValidityV51 {{ offset: {}int, width: {}int, start: {}int, end: {}int, pointer: {} }},", valid.offset, valid.bytes, valid.start, valid.end, valid.pointer)
                    .map_err(|_| out.error())?;
            }
            write!(out, "] }}) }} else").map_err(|_| out.error())?;
        }
        write!(out, " {{ None }}\n}}\n").map_err(|_| out.error())
    }
}

fn headers() -> usize {
    size_of::<SourceMemoryTypesV51>()
        + 2 * size_of::<Vec<Layout>>()
        + 2 * size_of::<Vec<Field>>()
        + size_of::<Vec<u8>>()
        + size_of::<Vec<Frame>>()
        + 2 * size_of::<Frame>()
        + size_of::<Layout>()
        + size_of::<Option<TypeId>>()
        + size_of::<Result<Option<(u64, u64, bool)>>>()
        + 23 * size_of::<usize>()
        + 14 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_memory_types_v51_tests.rs"]
mod tests;

#[test]
fn float_memory_validity_requires_the_complete_exact_bit_domain() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticScalarValidityRangeV1 as Range;
    for bits in [32u16, 64] {
        let maximum = (1u128 << bits) - 1;
        let primitive = Primitive::float(bits, u64::from(bits / 8));
        let scalar = BackendScalar::initialized(primitive, Range::new(0, maximum));
        let valid = scalar_validity(scalar, 3).unwrap();
        assert_eq!(
            valid,
            Validity {
                offset: 3,
                bytes: u64::from(bits / 8),
                start: 0,
                end: maximum,
                pointer: false
            }
        );
        for (start, end) in [(1, maximum), (0, maximum - 1)] {
            assert!(
                scalar_validity(
                    BackendScalar::initialized(primitive, Range::new(start, end)),
                    0
                )
                .is_none()
            );
        }
    }
    for bits in [16u16, 128] {
        let maximum = u128::MAX >> (128 - bits);
        assert!(
            scalar_validity(
                BackendScalar::initialized(
                    Primitive::float(bits, u64::from(bits / 8)),
                    Range::new(0, maximum)
                ),
                0
            )
            .is_none()
        );
    }
}
