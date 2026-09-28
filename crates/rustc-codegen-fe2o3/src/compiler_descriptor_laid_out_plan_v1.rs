//! Allocation-free capture inside the existing rustc/collection accounting domain.
//! This does not admit compiler-laid-out arguments to any descriptor or host ABI.

use super::{
    AccessMode, CompilerDescriptorError, DescriptorArgumentKindV1 as Kind, RustcAbiClassV1,
    ScalarTypeV1, TypedDescriptorArgumentV1 as Argument, TypedDescriptorRootV1 as Root,
};
use fe2o3_artifacts::{
    MAX_ABI_BYTES, MAX_ABI_FIELDS, PointerWidth, RustScalarElementTypeV1 as Scalar,
    RustSourceTypeShapeV1 as Shape,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Extent {
    pub(super) bytes: u32,
    pub(super) alignment: u32,
}

fn mismatch(reason: &'static str) -> CompilerDescriptorError {
    CompilerDescriptorError::ProductionDescriptorMismatch(reason)
}

fn scalar_matches(actual: Scalar, expected: ScalarTypeV1) -> bool {
    matches!(
        actual,
        Scalar::I8
            | Scalar::U8
            | Scalar::I16
            | Scalar::U16
            | Scalar::I32
            | Scalar::U32
            | Scalar::I64
            | Scalar::U64
            | Scalar::F32
            | Scalar::F64
    ) && super::descriptor_scalar(actual) == expected
}

fn physical(argument: &Argument) -> Result<(u64, u32), CompilerDescriptorError> {
    let (size, alignment, class, components, access_ok) = match argument.kind {
        Kind::CompilerLaidOutUsize | Kind::CompilerLaidOutIsize => {
            if argument.layout.is_some() {
                return Err(mismatch(
                    "nominal pointer-sized argument has no fixed source layout",
                ));
            }
            (
                8,
                8,
                RustcAbiClassV1::Scalar,
                0,
                argument.access == AccessMode::ByValue,
            )
        }
        Kind::Scalar(scalar) => (
            u64::from(scalar.size_bytes()),
            u32::from(scalar.alignment_bytes()),
            RustcAbiClassV1::Scalar,
            1,
            argument.access == AccessMode::ByValue,
        ),
        Kind::SharedSlice(_) => (
            16,
            8,
            RustcAbiClassV1::ScalarPair,
            2,
            argument.access == AccessMode::ReadOnly,
        ),
        Kind::DisjointSlice(_) => (
            16,
            8,
            RustcAbiClassV1::ScalarPair,
            2,
            matches!(
                argument.access,
                AccessMode::WriteOnly | AccessMode::ReadWrite
            ),
        ),
        Kind::GlobalMutPointer(_) => (
            8,
            8,
            RustcAbiClassV1::Scalar,
            1,
            argument.access == AccessMode::ReadWrite,
        ),
        Kind::CompilerLaidOutByValue => {
            return Err(mismatch("aggregate has no scalar packing plan"));
        }
    };
    if !access_ok
        || argument.source_size != size
        || argument.source_alignment != alignment
        || argument.rustc_abi_class != class
    {
        return Err(mismatch("whole-root physical argument layout"));
    }
    if components != 0 {
        let layout = argument
            .layout
            .as_ref()
            .ok_or_else(|| mismatch("whole-root Rust layout evidence"))?;
        let shape_ok = match (argument.kind, layout.rust_type().source_type()) {
            (Kind::Scalar(expected), Shape::Scalar { scalar }) => scalar_matches(scalar, expected),
            (Kind::SharedSlice(expected), Shape::SharedSlice { element })
            | (Kind::DisjointSlice(expected), Shape::DisjointSlice { element, .. }) => {
                scalar_matches(element, expected)
            }
            (Kind::GlobalMutPointer(expected), Shape::GlobalMutPointer { pointee }) => {
                scalar_matches(pointee, expected)
            }
            _ => false,
        };
        // RustLayoutEvidenceV1's checked constructor already validates its exact components.
        if !shape_ok
            || layout.size() != size
            || layout.abi_alignment() != alignment
            || layout.abi_class() != class
            || layout.pointer_width() != PointerWidth::Bits64
            || layout.components().len() != components
        {
            return Err(mismatch("whole-root source and physical layout join"));
        }
    }
    Ok((size, alignment))
}

#[derive(Default)]
struct Cursor {
    end: u64,
    alignment: u32,
}

impl Cursor {
    fn advance(&mut self, size: u64, alignment: u32) -> Result<u32, CompilerDescriptorError> {
        if !alignment.is_power_of_two() || alignment > 8 || size == 0 || size > 16 {
            return Err(mismatch("bounded scalar packing dimensions"));
        }
        let mask = u64::from(alignment) - 1;
        let offset = self
            .end
            .checked_add(mask)
            .map(|value| value & !mask)
            .ok_or_else(|| mismatch("scalar packing offset arithmetic"))?;
        let end = offset
            .checked_add(size)
            .ok_or_else(|| mismatch("scalar packing extent arithmetic"))?;
        if end > MAX_ABI_BYTES {
            return Err(mismatch("scalar packing explicit byte limit"));
        }
        self.end = end;
        self.alignment = self.alignment.max(alignment);
        u32::try_from(offset).map_err(|_| mismatch("scalar packing offset width"))
    }

    fn finish(self) -> Result<Extent, CompilerDescriptorError> {
        let mask = u64::from(self.alignment.max(1)) - 1;
        let end = self
            .end
            .checked_add(mask)
            .map(|value| value & !mask)
            .ok_or_else(|| mismatch("scalar packing final alignment"))?;
        let bytes = u32::try_from(end).map_err(|_| mismatch("scalar packing extent width"))?;
        if end > MAX_ABI_BYTES
            || bytes
                .checked_add(256)
                .is_none_or(|size| size > fe2o3_kernel_descriptor::MAX_KERNARG_SEGMENT_BYTES)
        {
            return Err(mismatch("scalar packing whole segment limit"));
        }
        Ok(Extent {
            bytes,
            alignment: self.alignment.max(1),
        })
    }
}

fn inspect(
    arguments: &[Argument],
    check_offsets: bool,
) -> Result<Option<Extent>, CompilerDescriptorError> {
    if arguments.len() > MAX_ABI_FIELDS {
        return Err(mismatch("scalar packing actual argument count"));
    }
    let mut nominal = false;
    let mut aggregate = false;
    for argument in arguments {
        nominal |= matches!(
            argument.kind,
            Kind::CompilerLaidOutUsize | Kind::CompilerLaidOutIsize
        );
        aggregate |= argument.kind == Kind::CompilerLaidOutByValue;
    }
    if !nominal || aggregate {
        return Ok(None);
    }
    let mut cursor = Cursor::default();
    for argument in arguments {
        let (size, alignment) = physical(argument)?;
        let offset = cursor.advance(size, alignment)?;
        if check_offsets && argument.offset != offset {
            return Err(mismatch("retained whole-root argument offset"));
        }
    }
    cursor.finish().map(Some)
}

pub(super) fn capture(
    arguments: &mut [Argument],
) -> Result<Option<Extent>, CompilerDescriptorError> {
    let Some(extent) = inspect(arguments, false)? else {
        return Ok(None);
    };
    // Complete validation precedes mutation; this repeats only checked scalar arithmetic.
    let mut cursor = Cursor::default();
    for argument in arguments {
        argument.offset = cursor.advance(argument.source_size, argument.source_alignment)?;
    }
    Ok(Some(extent))
}

pub(super) fn check(root: &Root) -> Result<(), CompilerDescriptorError> {
    if let Some(extent) = inspect(root.arguments.as_slice(), true)?
        && (root.explicit_argument_bytes != extent.bytes
            || root.kernarg_alignment_bytes != extent.alignment)
    {
        return Err(mismatch("retained whole-root packing extent"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "compiler_descriptor_laid_out_plan_v1_tests.rs"]
mod tests;
