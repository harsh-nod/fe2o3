//! V18 typed Storage family, including the explicit V970 construction carrier.
use super::*;
use crate::{StorageCopyOverlapV1, StorageOperationV1, StorageProjectionV1};

pub(super) fn encode(
    writer: &mut Writer<'_>,
    operation: &StorageOperationV1,
) -> Result<(), KernelIrEncodeError> {
    storage_profile_v18::require(writer, "module-owned storage operation")?;
    writer.u8(40)?;
    match operation {
        StorageOperationV1::Project { base, step } => {
            writer.u8(1)?;
            writer.u32(base.0)?;
            match step {
                StorageProjectionV1::Field(index) => {
                    writer.u8(1)?;
                    writer.u32(*index)?;
                }
                StorageProjectionV1::ArrayIndex(index) => {
                    writer.u8(2)?;
                    writer.u32(index.0)?;
                }
                StorageProjectionV1::Variant { index, access } => {
                    writer.u8(3)?;
                    writer.u32(*index)?;
                    encode_memory_access(writer, *access)?;
                }
                StorageProjectionV1::VariantForWrite { index } => {
                    writer.u8(4)?;
                    writer.u32(*index)?;
                }
            }
        }
        StorageOperationV1::ReadValue { address, access } => {
            writer.u8(2)?;
            writer.u32(address.0)?;
            encode_memory_access(writer, *access)?;
        }
        StorageOperationV1::WriteValue {
            address,
            value,
            access,
        } => {
            writer.u8(3)?;
            writer.u32(address.0)?;
            writer.u32(value.0)?;
            encode_memory_access(writer, *access)?;
        }
        StorageOperationV1::CopyObject {
            source,
            destination,
            source_access,
            destination_access,
            overlap,
        } => {
            writer.u8(4)?;
            writer.u32(source.0)?;
            writer.u32(destination.0)?;
            encode_memory_access(writer, *source_access)?;
            encode_memory_access(writer, *destination_access)?;
            writer.u8(match overlap {
                StorageCopyOverlapV1::NonOverlapping => 1,
                StorageCopyOverlapV1::MayOverlap => 2,
            })?;
        }
        StorageOperationV1::SetDiscriminant {
            address,
            variant,
            access,
        } => {
            writer.u8(5)?;
            writer.u32(address.0)?;
            writer.u32(*variant)?;
            encode_memory_access(writer, *access)?;
        }
        StorageOperationV1::ReadDiscriminant { address, access } => {
            writer.u8(6)?;
            writer.u32(address.0)?;
            encode_memory_access(writer, *access)?;
        }
    }
    Ok(())
}

pub(super) fn decode(
    reader: &mut Reader<'_, '_>,
) -> Result<StorageOperationV1, KernelIrDecodeError> {
    Ok(match reader.u8()? {
        1 => StorageOperationV1::Project {
            base: ValueId(reader.u32()?),
            step: match reader.u8()? {
                1 => StorageProjectionV1::Field(reader.u32()?),
                2 => StorageProjectionV1::ArrayIndex(ValueId(reader.u32()?)),
                3 => StorageProjectionV1::Variant {
                    index: reader.u32()?,
                    access: decode_memory_access(reader)?,
                },
                4 => StorageProjectionV1::VariantForWrite {
                    index: reader.u32()?,
                },
                tag => {
                    return Err(KernelIrDecodeError::UnknownTag {
                        kind: "storage projection",
                        tag,
                    });
                }
            },
        },
        2 => StorageOperationV1::ReadValue {
            address: ValueId(reader.u32()?),
            access: decode_memory_access(reader)?,
        },
        3 => StorageOperationV1::WriteValue {
            address: ValueId(reader.u32()?),
            value: ValueId(reader.u32()?),
            access: decode_memory_access(reader)?,
        },
        4 => StorageOperationV1::CopyObject {
            source: ValueId(reader.u32()?),
            destination: ValueId(reader.u32()?),
            source_access: decode_memory_access(reader)?,
            destination_access: decode_memory_access(reader)?,
            overlap: match reader.u8()? {
                1 => StorageCopyOverlapV1::NonOverlapping,
                2 => StorageCopyOverlapV1::MayOverlap,
                tag => {
                    return Err(KernelIrDecodeError::UnknownTag {
                        kind: "storage copy overlap",
                        tag,
                    });
                }
            },
        },
        5 => StorageOperationV1::SetDiscriminant {
            address: ValueId(reader.u32()?),
            variant: reader.u32()?,
            access: decode_memory_access(reader)?,
        },
        6 => StorageOperationV1::ReadDiscriminant {
            address: ValueId(reader.u32()?),
            access: decode_memory_access(reader)?,
        },
        tag => {
            return Err(KernelIrDecodeError::UnknownTag {
                kind: "storage operation",
                tag,
            });
        }
    })
}
