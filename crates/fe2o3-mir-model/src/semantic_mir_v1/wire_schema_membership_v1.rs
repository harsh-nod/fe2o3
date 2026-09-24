//! Exact sibling-schema membership and versioned direct-call bytes.
//! These private helpers preserve the original validation and encoding order.

use super::*;

impl SemanticMirWireVersionV1 {
    /// Exact membership, not a numeric version threshold: 31/32 are frozen
    /// diagnostic siblings and 34 is the noncolliding standalone grammar.
    pub(super) const fn scalar_authoring_intrinsic_tag(self) -> Option<u8> {
        match self {
            Self::V31 | Self::V32 => Some(87),
            Self::V34 => Some(91),
            _ => None,
        }
    }
}

pub(super) fn validate_request_schema(
    request: &InertSemanticMirRequestV1,
    wire_version: SemanticMirWireVersionV1,
) -> Result<(), SemanticMirErrorV1> {
    if wire_version != SemanticMirWireVersionV1::V29
        && saturating_integer_v30::contains_inert_execution(request)
    {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V29,
        });
    }
    if wire_version != SemanticMirWireVersionV1::V36 && complete_body_v36::uses_v36(request) {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V36,
        });
    }
    // These schemas are siblings, not ordinal-compatible extensions.
    if wire_version != SemanticMirWireVersionV1::V31 && gfx942_ordered_region_v31::uses_v31(request)
    {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V31,
        });
    }
    if wire_version != SemanticMirWireVersionV1::V32
        && gfx942_ordered_program_v32::uses_v32(request)
    {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V32,
        });
    }
    if wire_version.scalar_authoring_intrinsic_tag().is_none()
        && gfx942_inline_v30::uses_scalar_authoring(request)
    {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V34,
        });
    }
    if !matches!(
        wire_version,
        SemanticMirWireVersionV1::V30 | SemanticMirWireVersionV1::V33
    ) && saturating_integer_v30::uses_saturating_integer(request)
    {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V30,
        });
    }
    if wire_version != SemanticMirWireVersionV1::V33
        && wave64_shuffle_v33::uses_wave64_shuffle(request)
    {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V33,
        });
    }
    Ok(())
}

pub(super) fn encode_direct_call(
    writer: &mut CanonicalWriterV1,
    call: &SemanticDirectCallV1,
    wire_version: SemanticMirWireVersionV1,
) -> Result<(), SemanticMirErrorV1> {
    if call.complete_body_source_vnext.is_some() {
        if wire_version != SemanticMirWireVersionV1::V36 {
            return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: wire_version,
                required: SemanticMirWireVersionV1::V36,
            });
        }
        if call.inline_assembly_source_v30.is_some()
            || call.ordered_region_source_v31.is_some()
            || call.ordered_program_source_v32.is_some()
        {
            return Err(SemanticMirErrorV1::InvalidCompleteBodyV36);
        }
    }
    if call.ordered_program_source_v32.is_some()
        && (call.inline_assembly_source_v30.is_some() || call.ordered_region_source_v31.is_some())
    {
        return Err(SemanticMirErrorV1::InvalidOrderedProgramV32);
    }
    if call.inline_assembly_source_v30.is_some() && call.ordered_region_source_v31.is_some() {
        return Err(SemanticMirErrorV1::InvalidOrderedRegionV31);
    }
    if call.inline_assembly_source_v30.is_some()
        && wire_version.scalar_authoring_intrinsic_tag().is_none()
    {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V34,
        });
    }
    if call.ordered_region_source_v31.is_some() && wire_version != SemanticMirWireVersionV1::V31 {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V31,
        });
    }
    if call.ordered_program_source_v32.is_some() && wire_version != SemanticMirWireVersionV1::V32 {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V32,
        });
    }
    writer.u8(2)?;
    writer.u32(call.callee.0)?;
    encode_operands(writer, &call.arguments)?;
    writer.count(call.variadic_argument_abis.len())?;
    for argument_abi in &call.variadic_argument_abis {
        encode_abi_value(writer, argument_abi)?;
    }
    match &call.destination {
        Some(destination) => {
            writer.u8(1)?;
            encode_place(writer, &destination.place)?;
            encode_edge(writer, destination.edge)?;
        }
        None => writer.u8(0)?,
    }
    encode_unwind(writer, call.unwind)?;
    if wire_version.scalar_authoring_intrinsic_tag().is_some() {
        match call.inline_assembly_source_v30 {
            Some(source) => {
                writer.u8(1)?;
                writer.identity(source.frontend_unit())?;
                writer.identity(*source.function().as_bytes())?;
                writer.identity(source.contract())?;
                writer.identity(source.statement())?;
            }
            None => writer.u8(0)?,
        }
    }
    if wire_version == SemanticMirWireVersionV1::V31 {
        match call.ordered_region_source_v31 {
            Some(source) => {
                writer.u8(1)?;
                writer.identity(source.frontend_unit())?;
                writer.identity(*source.function().as_bytes())?;
                writer.identity(source.contract())?;
                writer.identity(source.statement())?;
            }
            None => writer.u8(0)?,
        }
    }
    if wire_version == SemanticMirWireVersionV1::V32 {
        match call.ordered_program_source_v32 {
            Some(source) => {
                writer.u8(1)?;
                writer.identity(source.frontend_unit())?;
                writer.identity(*source.function().as_bytes())?;
                writer.identity(source.contract())?;
                writer.identity(source.statement())?;
            }
            None => writer.u8(0)?,
        }
    }
    if wire_version == SemanticMirWireVersionV1::V36 {
        complete_body_v36::encode_source(writer, call.complete_body_source_vnext)?;
    }
    Ok(())
}
