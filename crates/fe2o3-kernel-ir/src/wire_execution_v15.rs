use super::*;
use crate::{ExecutionOperationV15, ExecutionRoleV15, SemanticExecutionInstancePayloadV3};

#[cfg(test)]
#[path = "wire_execution_v15_tests.rs"]
mod tests;

fn require_v15(writer: &Writer<'_>) -> Result<(), KernelIrEncodeError> {
    if writer.version == KERNEL_IR_VERSION_V15 {
        Ok(())
    } else {
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: writer.version,
            feature: "execution lifecycle",
        })
    }
}

pub(super) fn encode_role(
    writer: &mut Writer<'_>,
    role: ExecutionRoleV15,
) -> Result<(), KernelIrEncodeError> {
    require_v15(writer)?;
    role.validate()
        .map_err(|_| KernelIrEncodeError::NonCanonical {
            field: "execution role geometry",
        })?;
    match role {
        ExecutionRoleV15::Context => writer.u8(9)?,
        ExecutionRoleV15::Workgroup => writer.u8(10)?,
        ExecutionRoleV15::MaskedTileU32 { lanes, elements }
        | ExecutionRoleV15::LaneFragmentU32 { lanes, elements } => {
            writer.u8(if matches!(role, ExecutionRoleV15::MaskedTileU32 { .. }) {
                11
            } else {
                12
            })?;
            writer.u16(lanes)?;
            writer.u16(elements)?;
        }
    }
    Ok(())
}

pub(super) fn decode_role(
    reader: &mut Reader<'_, '_>,
    tag: u8,
) -> Result<ExecutionRoleV15, KernelIrDecodeError> {
    let role = match tag {
        9 => ExecutionRoleV15::Context,
        10 => ExecutionRoleV15::Workgroup,
        11 => ExecutionRoleV15::MaskedTileU32 {
            lanes: reader.u16()?,
            elements: reader.u16()?,
        },
        12 => ExecutionRoleV15::LaneFragmentU32 {
            lanes: reader.u16()?,
            elements: reader.u16()?,
        },
        _ => {
            return Err(KernelIrDecodeError::UnknownTag {
                kind: "execution role",
                tag,
            });
        }
    };
    role.validate()
        .map_err(|_| KernelIrDecodeError::InvalidSemanticOperationInstance)?;
    Ok(role)
}

pub(super) fn encode_operation(
    writer: &mut Writer<'_>,
    operation: &ExecutionOperationV15,
) -> Result<(), KernelIrEncodeError> {
    require_v15(writer)?;
    // The raw enum can be malformed. Admit its full roster before descriptor validation.
    let operands = match operation {
        ExecutionOperationV15::ScopeEnd { discarded, .. } => discarded.len().checked_add(1),
        ExecutionOperationV15::ContextIssue => Some(0),
        ExecutionOperationV15::MaskedTileLoadU32 { .. } => Some(3),
        _ => Some(1),
    }
    .ok_or(KernelIrEncodeError::Overflow {
        field: "execution operands",
    })?;
    if operands > MAX_VALUE_ARGUMENTS_V1 {
        return Err(KernelIrEncodeError::LimitExceeded {
            field: "execution operands",
            actual: operands,
            max: MAX_VALUE_ARGUMENTS_V1,
        });
    }
    writer.charge_work(operands)?;
    let instance_id =
        operation
            .semantic_instance_id_v3()
            .map_err(|_| KernelIrEncodeError::NonCanonical {
                field: "execution payload",
            })?;
    let tag = match operation {
        ExecutionOperationV15::ContextIssue => 32,
        ExecutionOperationV15::WorkgroupDerive { .. } => 33,
        ExecutionOperationV15::ScopeEnd { .. } => 34,
        ExecutionOperationV15::MaskedTileLoadU32 { .. } => 35,
        ExecutionOperationV15::TileIntoFragmentU32 { .. } => 36,
        ExecutionOperationV15::FragmentIntoPartsU32 { .. } => 37,
    };
    writer.u8(tag)?;
    writer.charge_work(semantic_operation_instance_encoding_work_v1(instance_id))?;
    let instance =
        if writer.counts_only() {
            writer.peak_auxiliary_bytes = writer.peak_auxiliary_bytes.max(
                semantic_operation_instance_encoding_scratch_bytes_v1(instance_id),
            );
            None
        } else {
            Some(encode_semantic_operation_instance_id(instance_id))
        };
    let length = instance.as_ref().map_or_else(
        || semantic_operation_instance_encoded_len_v1(instance_id),
        Vec::len,
    );
    writer.count(
        "semantic operation instance",
        length,
        crate::SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1
            + MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1,
    )?;
    if let Some(bytes) = instance {
        writer.bytes(&bytes)?;
    } else {
        writer.count_bytes(length)?;
    }
    operation.try_visit_operands_v1(|value| writer.u32(value.0))
}

pub(super) fn decode_operation(
    reader: &mut Reader<'_, '_>,
    tag: u8,
) -> Result<ExecutionOperationV15, KernelIrDecodeError> {
    let length = reader.count(
        "semantic operation instance",
        crate::SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1
            + MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1,
    )?;
    let work = length.checked_mul(4).ok_or(KernelIrDecodeError::Resource(
        crate::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
    ))?;
    reader.charge_work(work)?;
    let instance = decode_semantic_operation_instance_id(reader.take(length)?)
        .map_err(|_| KernelIrDecodeError::InvalidSemanticOperationInstance)?;
    let SemanticOperationInstancePayloadV1::Execution(payload) = instance.payload() else {
        return Err(KernelIrDecodeError::InvalidSemanticOperationInstance);
    };
    let operation = match (tag, payload) {
        (32, SemanticExecutionInstancePayloadV3::ContextIssue) => {
            ExecutionOperationV15::ContextIssue
        }
        (33, SemanticExecutionInstancePayloadV3::WorkgroupDerive) => {
            ExecutionOperationV15::WorkgroupDerive {
                context: ValueId(reader.u32()?),
            }
        }
        (34, SemanticExecutionInstancePayloadV3::ScopeEnd { discard_count }) => {
            let count = usize::try_from(discard_count)
                .map_err(|_| KernelIrDecodeError::InvalidSemanticOperationInstance)?;
            if count > MAX_VALUE_ARGUMENTS_V1 - 1 {
                return Err(KernelIrDecodeError::InvalidSemanticOperationInstance);
            }
            let operands = count.checked_add(1).ok_or(KernelIrDecodeError::Resource(
                crate::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
            ))?;
            let bytes = operands.checked_mul(std::mem::size_of::<u32>()).ok_or(
                KernelIrDecodeError::Resource(
                    crate::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
                ),
            )?;
            // Check the complete encoded roster before allocating its retained tail.
            reader.charge_work(operands)?;
            let encoded = reader.take(bytes)?;
            let workgroup = ValueId(u32::from_le_bytes(encoded[..4].try_into().unwrap()));
            let mut discarded = reader.vector(count)?;
            let mut previous = None;
            for chunk in encoded[4..].chunks_exact(4) {
                let value = ValueId(u32::from_le_bytes(chunk.try_into().unwrap()));
                if previous.is_some_and(|previous| previous >= value) {
                    return Err(KernelIrDecodeError::InvalidSemanticOperationInstance);
                }
                discarded.push(value);
                previous = Some(value);
            }
            ExecutionOperationV15::ScopeEnd {
                workgroup,
                discarded,
            }
        }
        (35, SemanticExecutionInstancePayloadV3::MaskedTileLoadU32 { lanes, elements }) => {
            ExecutionOperationV15::MaskedTileLoadU32 {
                workgroup: ValueId(reader.u32()?),
                input: ValueId(reader.u32()?),
                base: ValueId(reader.u32()?),
                lanes,
                elements,
            }
        }
        (36, SemanticExecutionInstancePayloadV3::TileIntoFragmentU32 { lanes, elements }) => {
            ExecutionOperationV15::TileIntoFragmentU32 {
                tile: ValueId(reader.u32()?),
                lanes,
                elements,
            }
        }
        (37, SemanticExecutionInstancePayloadV3::FragmentIntoPartsU32 { lanes, elements }) => {
            ExecutionOperationV15::FragmentIntoPartsU32 {
                fragment: ValueId(reader.u32()?),
                lanes,
                elements,
            }
        }
        _ => return Err(KernelIrDecodeError::InvalidSemanticOperationInstance),
    };
    let payload_work = match &operation {
        ExecutionOperationV15::ScopeEnd { discarded, .. } => discarded.len() + 1,
        _ => 1,
    };
    reader.charge_work(payload_work)?;
    operation
        .validate_payload()
        .map_err(|_| KernelIrDecodeError::InvalidSemanticOperationInstance)?;
    Ok(operation)
}
