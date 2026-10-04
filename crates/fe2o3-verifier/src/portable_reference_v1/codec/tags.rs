//! Explicit wire constants; Rust enum reordering cannot change the V1 frame.
use super::*;

pub(super) const SCALARS: &[(u8, ReferenceScalarTypeV1)] = &[
    (0, ReferenceScalarTypeV1::Bool),
    (1, ReferenceScalarTypeV1::U8),
    (2, ReferenceScalarTypeV1::U16),
    (3, ReferenceScalarTypeV1::U32),
    (4, ReferenceScalarTypeV1::U64),
    (5, ReferenceScalarTypeV1::Usize),
    (6, ReferenceScalarTypeV1::I8),
    (7, ReferenceScalarTypeV1::I16),
    (8, ReferenceScalarTypeV1::I32),
    (9, ReferenceScalarTypeV1::I64),
    (10, ReferenceScalarTypeV1::Isize),
    (11, ReferenceScalarTypeV1::F32),
    (12, ReferenceScalarTypeV1::F64),
];
pub(super) const BINARY: &[(u8, ReferenceBinaryOpV1)] = &[
    (0, ReferenceBinaryOpV1::Add),
    (1, ReferenceBinaryOpV1::Subtract),
    (2, ReferenceBinaryOpV1::Multiply),
    (3, ReferenceBinaryOpV1::Divide),
    (4, ReferenceBinaryOpV1::Remainder),
    (5, ReferenceBinaryOpV1::BitXor),
    (6, ReferenceBinaryOpV1::BitAnd),
    (7, ReferenceBinaryOpV1::BitOr),
    (8, ReferenceBinaryOpV1::ShiftLeft),
    (9, ReferenceBinaryOpV1::ShiftRight),
    (10, ReferenceBinaryOpV1::Equal),
    (11, ReferenceBinaryOpV1::LessThan),
    (12, ReferenceBinaryOpV1::LessEqual),
    (13, ReferenceBinaryOpV1::NotEqual),
    (14, ReferenceBinaryOpV1::GreaterEqual),
    (15, ReferenceBinaryOpV1::GreaterThan),
];
pub(super) const UNARY: &[(u8, ReferenceUnaryOpV1)] = &[
    (0, ReferenceUnaryOpV1::Not),
    (1, ReferenceUnaryOpV1::Negate),
];
pub(super) const CASTS: &[(u8, ReferenceCastKindV1)] = &[
    (0, ReferenceCastKindV1::Integer),
    (1, ReferenceCastKindV1::IntegerToFloat),
    (2, ReferenceCastKindV1::FloatToFloat),
    (3, ReferenceCastKindV1::FloatToIntegerSaturating),
];
pub(super) fn tag<T: PartialEq>(table: &[(u8, T)], value: &T) -> Result<u8, Error> {
    table
        .iter()
        .find_map(|(tag, item)| (item == value).then_some(*tag))
        .ok_or(Error::Wire("unsupported enum"))
}
