//! Flat, lossless rows. Structural validity is checked once on the owning Module.
use super::*;
use crate::storage_layout_v1::*;

const MAX_ROWS: usize = MAX_MODULE_BYTES_V1 / 13;
const MAX_FIELDS: usize = MAX_MODULE_BYTES_V1 / 12;
const MAX_VARIANTS: usize = MAX_MODULE_BYTES_V1 / 22;

fn wide(writer: &mut Writer<'_>, value: u128) -> Result<(), KernelIrEncodeError> {
    if writer.counts_only() {
        writer.count_bytes(16)
    } else {
        writer.bytes(&value.to_le_bytes())
    }
}

fn field(writer: &mut Writer<'_>, value: StorageFieldV1) -> Result<(), KernelIrEncodeError> {
    writer.u64(value.offset)?;
    writer.u32(value.layout.0)
}

fn read_field(reader: &mut Reader<'_, '_>) -> Result<StorageFieldV1, KernelIrDecodeError> {
    Ok(StorageFieldV1 {
        offset: reader.u64()?,
        layout: StorageLayoutIdV1(reader.u32()?),
    })
}

fn read_count(
    reader: &mut Reader<'_, '_>,
    name: &'static str,
    maximum: usize,
    minimum_bytes: usize,
) -> Result<usize, KernelIrDecodeError> {
    let count = reader.count(name, maximum)?;
    reader.charge_work(1)?;
    let minimum = count
        .checked_mul(minimum_bytes)
        .ok_or(KernelIrDecodeError::Truncated)?;
    if minimum > reader.bytes.len().saturating_sub(reader.offset) {
        return Err(KernelIrDecodeError::Truncated);
    }
    Ok(count)
}

fn fields(writer: &mut Writer<'_>, values: &[StorageFieldV1]) -> Result<(), KernelIrEncodeError> {
    writer.count("storage fields", values.len(), MAX_FIELDS)?;
    for value in values {
        field(writer, *value)?;
    }
    Ok(())
}

fn read_fields(reader: &mut Reader<'_, '_>) -> Result<Box<[StorageFieldV1]>, KernelIrDecodeError> {
    let count = read_count(reader, "storage fields", MAX_FIELDS, 12)?;
    let mut values = reader.vector(count)?;
    for _ in 0..count {
        values.push(read_field(reader)?);
    }
    Ok(values.into_boxed_slice())
}

pub(super) fn encode(
    writer: &mut Writer<'_>,
    rows: &[StorageLayoutV1],
) -> Result<(), KernelIrEncodeError> {
    storage_profile_v18::require(writer, "module storage layouts")?;
    writer.count("storage rows", rows.len(), MAX_ROWS)?;
    for row in rows {
        writer.charge_work(1)?;
        writer.u64(row.size)?;
        writer.u32(row.alignment)?;
        match &row.kind {
            StorageLayoutKindV1::Scalar(value) => {
                writer.u8(1)?;
                writer.u8(scalar_type_tag(*value))?;
            }
            StorageLayoutKindV1::Vector(value) => {
                writer.u8(2)?;
                encode_fixed_vector_type_v12(writer, *value)?;
            }
            StorageLayoutKindV1::Pointer(value) => {
                writer.u8(3)?;
                writer.u32(value.pointee.0)?;
                writer.u8(address_space_tag(value.value_space))?;
                writer.u8(address_space_tag(value.encoded_space))?;
                encode_access_mode(writer, value.access)?;
                writer.u16(value.stored_bits)?;
            }
            StorageLayoutKindV1::Record(values) => {
                writer.u8(4)?;
                fields(writer, values)?;
            }
            StorageLayoutKindV1::Union(values) => {
                writer.u8(5)?;
                fields(writer, values)?;
            }
            StorageLayoutKindV1::Array {
                element,
                length,
                stride,
            } => {
                writer.u8(6)?;
                writer.u32(element.0)?;
                writer.u64(*length)?;
                writer.u64(*stride)?;
            }
            StorageLayoutKindV1::Slice {
                element,
                value_space,
                access,
                data,
                length,
            } => {
                writer.u8(7)?;
                writer.u32(element.0)?;
                writer.u8(address_space_tag(*value_space))?;
                encode_access_mode(writer, *access)?;
                field(writer, *data)?;
                field(writer, *length)?;
            }
            StorageLayoutKindV1::Variants { encoding, variants } => {
                writer.u8(8)?;
                match encoding {
                    StorageVariantEncodingV1::Direct { tag } => {
                        writer.u8(1)?;
                        field(writer, *tag)?;
                    }
                    StorageVariantEncodingV1::Niche {
                        tag,
                        untagged_variant,
                        first_niche_variant,
                        last_niche_variant,
                        niche_start,
                    } => {
                        writer.u8(2)?;
                        field(writer, *tag)?;
                        writer.u32(*untagged_variant)?;
                        writer.u32(*first_niche_variant)?;
                        writer.u32(*last_niche_variant)?;
                        wide(writer, *niche_start)?;
                    }
                }
                writer.count("storage variants", variants.len(), MAX_VARIANTS)?;
                for variant in variants {
                    wide(writer, variant.discriminant)?;
                    match variant.direct_tag_bits {
                        None => writer.u8(0)?,
                        Some(value) => {
                            writer.u8(1)?;
                            wide(writer, value)?;
                        }
                    }
                    writer.u8(u8::from(variant.uninhabited))?;
                    writer.u32(variant.layout.0)?;
                }
            }
        }
    }
    Ok(())
}

pub(super) fn decode(
    reader: &mut Reader<'_, '_>,
) -> Result<Vec<StorageLayoutV1>, KernelIrDecodeError> {
    let count = read_count(reader, "storage rows", MAX_ROWS, 13)?;
    let mut rows = reader.vector(count)?;
    for _ in 0..count {
        reader.charge_work(1)?;
        let size = reader.u64()?;
        let alignment = reader.u32()?;
        let kind = match reader.u8()? {
            1 => StorageLayoutKindV1::Scalar(decode_scalar_type(reader.u8()?)?),
            2 => StorageLayoutKindV1::Vector(decode_fixed_vector_type_v12(reader)?),
            3 => StorageLayoutKindV1::Pointer(StoragePointerV1 {
                pointee: StorageLayoutIdV1(reader.u32()?),
                value_space: decode_address_space(reader.u8()?)?,
                encoded_space: decode_address_space(reader.u8()?)?,
                access: decode_access_mode(reader.u8()?)?,
                stored_bits: reader.u16()?,
            }),
            4 => StorageLayoutKindV1::Record(read_fields(reader)?),
            5 => StorageLayoutKindV1::Union(read_fields(reader)?),
            6 => StorageLayoutKindV1::Array {
                element: StorageLayoutIdV1(reader.u32()?),
                length: reader.u64()?,
                stride: reader.u64()?,
            },
            7 => StorageLayoutKindV1::Slice {
                element: StorageLayoutIdV1(reader.u32()?),
                value_space: decode_address_space(reader.u8()?)?,
                access: decode_access_mode(reader.u8()?)?,
                data: read_field(reader)?,
                length: read_field(reader)?,
            },
            8 => {
                let encoding = match reader.u8()? {
                    1 => StorageVariantEncodingV1::Direct {
                        tag: read_field(reader)?,
                    },
                    2 => StorageVariantEncodingV1::Niche {
                        tag: read_field(reader)?,
                        untagged_variant: reader.u32()?,
                        first_niche_variant: reader.u32()?,
                        last_niche_variant: reader.u32()?,
                        niche_start: u128::from_le_bytes(reader.fixed()?),
                    },
                    tag => {
                        return Err(KernelIrDecodeError::UnknownTag {
                            kind: "storage variant encoding",
                            tag,
                        });
                    }
                };
                let count = read_count(reader, "storage variants", MAX_VARIANTS, 22)?;
                let mut variants = reader.vector(count)?;
                for _ in 0..count {
                    let discriminant = u128::from_le_bytes(reader.fixed()?);
                    let direct_tag_bits = if reader.option("storage direct tag")? {
                        Some(u128::from_le_bytes(reader.fixed()?))
                    } else {
                        None
                    };
                    variants.push(StorageVariantV1 {
                        discriminant,
                        direct_tag_bits,
                        uninhabited: reader.boolean("storage uninhabited variant")?,
                        layout: StorageLayoutIdV1(reader.u32()?),
                    });
                }
                StorageLayoutKindV1::Variants {
                    encoding,
                    variants: variants.into_boxed_slice(),
                }
            }
            tag => {
                return Err(KernelIrDecodeError::UnknownTag {
                    kind: "storage layout",
                    tag,
                });
            }
        };
        rows.push(StorageLayoutV1 {
            size,
            alignment,
            kind,
        });
    }
    Ok(rows)
}
