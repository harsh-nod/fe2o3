//! Exact, small unsigned primitive-array constants and stable scalar indexing.
//! Bytes are target memory bytes: the canonical semantic target is exclusively
//! AMDGPU (little endian). No pointer/relocation or aggregate reinterpretation.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticFieldsShapeV1;

impl Engine<'_, '_> {
    pub(super) fn primitive_array_layout(&self, ty: SemanticTypeIdV1) -> Option<(usize, usize)> {
        let array = self.types.get(ty.index() as usize)?;
        let SemanticTypeShapeV1::Array { element, length } = array.shape() else {
            return None;
        };
        let length = usize::try_from(*length).ok()?;
        if !(1..=MAX_FIELDS).contains(&length) {
            return None;
        }
        let element = self.types.get(element.index() as usize)?;
        let SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: bits @ (8 | 16 | 32 | 64 | 128),
        }) = element.shape()
        else {
            return None;
        };
        let width = usize::from(*bits) / 8;
        let element_layout = element.layout();
        let array_layout = array.layout();
        if element_layout.size_bytes() != Some(width as u64)
            || element_layout.rustc_size_bytes() != width as u64
            || array_layout.size_bytes() != Some((length * width) as u64)
            || array_layout.rustc_size_bytes() != (length * width) as u64
            || array_layout.alignment_bytes() != element_layout.alignment_bytes()
            || !matches!(array_layout.fields(), SemanticFieldsShapeV1::Array { stride_bytes, count }
                if *stride_bytes == width as u64 && *count == length as u64)
        {
            return None;
        }
        Some((length, width))
    }

    pub(super) fn primitive_array_constant(&self, ty: SemanticTypeIdV1, bytes: &[u8]) -> Value {
        let Some((length, width)) = self.primitive_array_layout(ty) else {
            return Value::Unknown;
        };
        if bytes.len() != length * width {
            return Value::Unknown;
        }
        Value::Fields(
            bytes
                .chunks_exact(width)
                .map(|chunk| {
                    let mut encoded = [0; 16];
                    encoded[..width].copy_from_slice(chunk);
                    Value::exact(u128::from_le_bytes(encoded))
                })
                .collect(),
        )
    }

    pub(super) fn exact_array_index(
        &self,
        state: &State,
        projection: SemanticProjectionKindV1,
        length: usize,
    ) -> Option<usize> {
        let index = match projection {
            SemanticProjectionKindV1::Index(local) => {
                let local = local.index() as usize;
                let ty = self.function.locals().get(local)?.ty();
                if self.escaped.get(local) != Some(&false) || is_bool(self.types, ty) {
                    return None;
                }
                let maximum = self.maximum(ty)?;
                let value = state.get(&local)?.scalar()?;
                if value.lo != value.hi || value.hi > maximum {
                    return None;
                }
                usize::try_from(value.lo).ok()?
            }
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length,
                from_end,
            } => {
                if minimum_length > length as u64 || offset >= minimum_length {
                    return None;
                }
                let offset = usize::try_from(offset).ok()?;
                if from_end {
                    length.checked_sub(offset)?
                } else {
                    offset
                }
            }
            _ => return None,
        };
        (index < length).then_some(index)
    }
}
