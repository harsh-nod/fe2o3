use std::collections::BTreeSet;

use rmpv::{ValueRef, decode::read_value_ref_with_max_depth};

use crate::{
    InspectionError, MAX_MESSAGEPACK_BLOB_BYTES, MAX_MESSAGEPACK_COLLECTION_ITEMS,
    MAX_MESSAGEPACK_DEPTH, MAX_MESSAGEPACK_NODES, MAX_MESSAGEPACK_STRING_BYTES,
    MAX_MESSAGEPACK_TOTAL_ITEMS, MessagePackLimit,
};

// rmpv decrements once for each value, once for each collection body, and up
// to twice more for a terminal string or extension payload.
const RMPV_MAX_DEPTH: usize = MAX_MESSAGEPACK_DEPTH * 2 + 3;

pub(crate) fn decode_bounded(bytes: &[u8]) -> Result<ValueRef<'_>, InspectionError> {
    preflight(bytes)?;

    let mut remaining = bytes;
    let value = read_value_ref_with_max_depth(&mut remaining, RMPV_MAX_DEPTH)
        .map_err(|_| InspectionError::MalformedMessagePack)?;
    if !remaining.is_empty() {
        return Err(InspectionError::TrailingMessagePack);
    }
    validate_value_tree(&value)?;
    Ok(value)
}

fn preflight(bytes: &[u8]) -> Result<(), InspectionError> {
    let mut cursor = 0usize;
    let mut pending = [0usize; MAX_MESSAGEPACK_DEPTH + 1];
    let mut stack_len = 1usize;
    pending[0] = 1;
    let mut nodes = 0usize;
    let mut total_items = 0usize;

    while stack_len != 0 {
        while stack_len != 0 && pending[stack_len - 1] == 0 {
            stack_len -= 1;
        }
        if stack_len == 0 {
            break;
        }

        pending[stack_len - 1] -= 1;
        let marker = take_u8(bytes, &mut cursor)?;
        nodes = nodes
            .checked_add(1)
            .ok_or(InspectionError::MessagePackLimit(MessagePackLimit::Nodes))?;
        if nodes > MAX_MESSAGEPACK_NODES {
            return Err(InspectionError::MessagePackLimit(MessagePackLimit::Nodes));
        }

        match marker {
            0x00..=0x7f | 0xc0 | 0xc2 | 0xc3 | 0xe0..=0xff => {}
            0x80..=0x8f => push_collection(
                usize::from(marker & 0x0f),
                true,
                &mut pending,
                &mut stack_len,
                &mut total_items,
            )?,
            0x90..=0x9f => push_collection(
                usize::from(marker & 0x0f),
                false,
                &mut pending,
                &mut stack_len,
                &mut total_items,
            )?,
            0xa0..=0xbf => {
                skip_string(bytes, &mut cursor, usize::from(marker & 0x1f))?;
            }
            0xc1 => return Err(InspectionError::MalformedMessagePack),
            0xc4 => {
                let length = usize::from(take_u8(bytes, &mut cursor)?);
                skip_blob(bytes, &mut cursor, length)?;
            }
            0xc5 => {
                let length = usize::from(take_u16(bytes, &mut cursor)?);
                skip_blob(bytes, &mut cursor, length)?;
            }
            0xc6 => {
                let length = usize_from_u32(take_u32(bytes, &mut cursor)?)?;
                skip_blob(bytes, &mut cursor, length)?;
            }
            0xc7 => {
                let length = usize::from(take_u8(bytes, &mut cursor)?);
                skip_extension(bytes, &mut cursor, length)?;
            }
            0xc8 => {
                let length = usize::from(take_u16(bytes, &mut cursor)?);
                skip_extension(bytes, &mut cursor, length)?;
            }
            0xc9 => {
                let length = usize_from_u32(take_u32(bytes, &mut cursor)?)?;
                skip_extension(bytes, &mut cursor, length)?;
            }
            0xca => skip(bytes, &mut cursor, 4)?,
            0xcb => skip(bytes, &mut cursor, 8)?,
            0xcc | 0xd0 => skip(bytes, &mut cursor, 1)?,
            0xcd | 0xd1 => skip(bytes, &mut cursor, 2)?,
            0xce | 0xd2 => skip(bytes, &mut cursor, 4)?,
            0xcf | 0xd3 => skip(bytes, &mut cursor, 8)?,
            0xd4 => skip_extension(bytes, &mut cursor, 1)?,
            0xd5 => skip_extension(bytes, &mut cursor, 2)?,
            0xd6 => skip_extension(bytes, &mut cursor, 4)?,
            0xd7 => skip_extension(bytes, &mut cursor, 8)?,
            0xd8 => skip_extension(bytes, &mut cursor, 16)?,
            0xd9 => {
                let length = usize::from(take_u8(bytes, &mut cursor)?);
                skip_string(bytes, &mut cursor, length)?;
            }
            0xda => {
                let length = usize::from(take_u16(bytes, &mut cursor)?);
                skip_string(bytes, &mut cursor, length)?;
            }
            0xdb => {
                let length = usize_from_u32(take_u32(bytes, &mut cursor)?)?;
                skip_string(bytes, &mut cursor, length)?;
            }
            0xdc => {
                let length = usize::from(take_u16(bytes, &mut cursor)?);
                push_collection(
                    length,
                    false,
                    &mut pending,
                    &mut stack_len,
                    &mut total_items,
                )?;
            }
            0xdd => {
                let length = usize_from_u32(take_u32(bytes, &mut cursor)?)?;
                push_collection(
                    length,
                    false,
                    &mut pending,
                    &mut stack_len,
                    &mut total_items,
                )?;
            }
            0xde => {
                let length = usize::from(take_u16(bytes, &mut cursor)?);
                push_collection(length, true, &mut pending, &mut stack_len, &mut total_items)?;
            }
            0xdf => {
                let length = usize_from_u32(take_u32(bytes, &mut cursor)?)?;
                push_collection(length, true, &mut pending, &mut stack_len, &mut total_items)?;
            }
        }
    }

    if cursor != bytes.len() {
        return Err(InspectionError::TrailingMessagePack);
    }
    Ok(())
}

fn push_collection(
    items: usize,
    is_map: bool,
    pending: &mut [usize; MAX_MESSAGEPACK_DEPTH + 1],
    stack_len: &mut usize,
    total_items: &mut usize,
) -> Result<(), InspectionError> {
    if items > MAX_MESSAGEPACK_COLLECTION_ITEMS {
        return Err(InspectionError::MessagePackLimit(
            MessagePackLimit::CollectionItems,
        ));
    }
    *total_items = total_items
        .checked_add(items)
        .ok_or(InspectionError::MessagePackLimit(
            MessagePackLimit::TotalCollectionItems,
        ))?;
    if *total_items > MAX_MESSAGEPACK_TOTAL_ITEMS {
        return Err(InspectionError::MessagePackLimit(
            MessagePackLimit::TotalCollectionItems,
        ));
    }
    let children = if is_map {
        items
            .checked_mul(2)
            .ok_or(InspectionError::MessagePackLimit(
                MessagePackLimit::CollectionItems,
            ))?
    } else {
        items
    };
    if children == 0 {
        return Ok(());
    }
    if *stack_len > MAX_MESSAGEPACK_DEPTH {
        return Err(InspectionError::MessagePackLimit(MessagePackLimit::Depth));
    }
    pending[*stack_len] = children;
    *stack_len += 1;
    Ok(())
}

fn skip_string(bytes: &[u8], cursor: &mut usize, length: usize) -> Result<(), InspectionError> {
    if length > MAX_MESSAGEPACK_STRING_BYTES {
        return Err(InspectionError::MessagePackLimit(
            MessagePackLimit::StringBytes,
        ));
    }
    skip(bytes, cursor, length)
}

fn skip_blob(bytes: &[u8], cursor: &mut usize, length: usize) -> Result<(), InspectionError> {
    if length > MAX_MESSAGEPACK_BLOB_BYTES {
        return Err(InspectionError::MessagePackLimit(
            MessagePackLimit::BlobBytes,
        ));
    }
    skip(bytes, cursor, length)
}

fn skip_extension(bytes: &[u8], cursor: &mut usize, length: usize) -> Result<(), InspectionError> {
    if length > MAX_MESSAGEPACK_BLOB_BYTES {
        return Err(InspectionError::MessagePackLimit(
            MessagePackLimit::BlobBytes,
        ));
    }
    skip(bytes, cursor, 1)?;
    skip(bytes, cursor, length)
}

fn take_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8, InspectionError> {
    let value = *bytes
        .get(*cursor)
        .ok_or(InspectionError::MalformedMessagePack)?;
    *cursor += 1;
    Ok(value)
}

fn take_u16(bytes: &[u8], cursor: &mut usize) -> Result<u16, InspectionError> {
    let value = take_array::<2>(bytes, cursor)?;
    Ok(u16::from_be_bytes(value))
}

fn take_u32(bytes: &[u8], cursor: &mut usize) -> Result<u32, InspectionError> {
    let value = take_array::<4>(bytes, cursor)?;
    Ok(u32::from_be_bytes(value))
}

fn take_array<const N: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], InspectionError> {
    let end = cursor
        .checked_add(N)
        .ok_or(InspectionError::MalformedMessagePack)?;
    let value: [u8; N] = bytes
        .get(*cursor..end)
        .ok_or(InspectionError::MalformedMessagePack)?
        .try_into()
        .map_err(|_| InspectionError::MalformedMessagePack)?;
    *cursor = end;
    Ok(value)
}

fn skip(bytes: &[u8], cursor: &mut usize, length: usize) -> Result<(), InspectionError> {
    let end = cursor
        .checked_add(length)
        .ok_or(InspectionError::MalformedMessagePack)?;
    if end > bytes.len() {
        return Err(InspectionError::MalformedMessagePack);
    }
    *cursor = end;
    Ok(())
}

fn usize_from_u32(value: u32) -> Result<usize, InspectionError> {
    usize::try_from(value).map_err(|_| InspectionError::MalformedMessagePack)
}

fn validate_value_tree(value: &ValueRef<'_>) -> Result<(), InspectionError> {
    match value {
        ValueRef::String(value) => {
            if value.as_str().is_none() {
                return Err(InspectionError::InvalidUtf8String);
            }
        }
        ValueRef::Array(values) => {
            for value in values {
                validate_value_tree(value)?;
            }
        }
        ValueRef::Map(entries) => {
            let mut keys = BTreeSet::new();
            for (key, _) in entries {
                let key = value_as_str(key).ok_or(InspectionError::NonStringMapKey)?;
                if !keys.insert(key) {
                    return Err(InspectionError::DuplicateMapKey);
                }
            }
            for (_, value) in entries {
                validate_value_tree(value)?;
            }
        }
        ValueRef::Nil
        | ValueRef::Boolean(_)
        | ValueRef::Integer(_)
        | ValueRef::F32(_)
        | ValueRef::F64(_)
        | ValueRef::Binary(_)
        | ValueRef::Ext(_, _) => {}
    }
    Ok(())
}

fn value_as_str<'value>(value: &'value ValueRef<'_>) -> Option<&'value str> {
    match value {
        ValueRef::String(value) => value.as_str(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_and_decoder_agree_at_the_depth_boundary() {
        let mut accepted = vec![0x91; MAX_MESSAGEPACK_DEPTH];
        accepted.push(0xa0);
        assert!(decode_bounded(&accepted).is_ok());

        let mut rejected = vec![0x91; MAX_MESSAGEPACK_DEPTH + 1];
        rejected.push(0xa0);
        assert_eq!(
            decode_bounded(&rejected),
            Err(InspectionError::MessagePackLimit(MessagePackLimit::Depth))
        );
    }

    #[test]
    fn preflight_rejects_declared_collection_count_before_payload() {
        let count = u32::try_from(MAX_MESSAGEPACK_COLLECTION_ITEMS + 1).unwrap();
        let mut bytes = vec![0xdd];
        bytes.extend_from_slice(&count.to_be_bytes());
        assert_eq!(
            decode_bounded(&bytes),
            Err(InspectionError::MessagePackLimit(
                MessagePackLimit::CollectionItems
            ))
        );
    }

    #[test]
    fn preflight_distinguishes_trailing_data_from_truncation() {
        assert_eq!(
            decode_bounded(&[0xc0, 0xc0]),
            Err(InspectionError::TrailingMessagePack)
        );
        assert_eq!(
            decode_bounded(&[0xd9, 2, b'a']),
            Err(InspectionError::MalformedMessagePack)
        );
    }

    #[test]
    fn preflight_rejects_oversized_strings_and_blobs_from_headers() {
        let string_length = u32::try_from(MAX_MESSAGEPACK_STRING_BYTES + 1).unwrap();
        let mut string = vec![0xdb];
        string.extend_from_slice(&string_length.to_be_bytes());
        assert_eq!(
            decode_bounded(&string),
            Err(InspectionError::MessagePackLimit(
                MessagePackLimit::StringBytes
            ))
        );

        let blob_length = u32::try_from(MAX_MESSAGEPACK_BLOB_BYTES + 1).unwrap();
        let mut blob = vec![0xc6];
        blob.extend_from_slice(&blob_length.to_be_bytes());
        assert_eq!(
            decode_bounded(&blob),
            Err(InspectionError::MessagePackLimit(
                MessagePackLimit::BlobBytes
            ))
        );
    }

    #[test]
    fn preflight_rejects_total_collection_items() {
        let outer_count = 1024u16;
        let inner_count = 32u16;
        let mut bytes = vec![0xdc];
        bytes.extend_from_slice(&outer_count.to_be_bytes());
        for _ in 0..outer_count {
            bytes.push(0xdc);
            bytes.extend_from_slice(&inner_count.to_be_bytes());
            bytes.extend(std::iter::repeat_n(0xc0, usize::from(inner_count)));
        }
        assert_eq!(
            decode_bounded(&bytes),
            Err(InspectionError::MessagePackLimit(
                MessagePackLimit::TotalCollectionItems
            ))
        );
    }

    #[test]
    fn preflight_rejects_total_nodes() {
        let outer_count = 512u16;
        let map_count = 32u16;
        let mut bytes = vec![0xdc];
        bytes.extend_from_slice(&outer_count.to_be_bytes());
        for _ in 0..outer_count {
            bytes.push(0xde);
            bytes.extend_from_slice(&map_count.to_be_bytes());
            bytes.extend(std::iter::repeat_n(0xc0, 2 * usize::from(map_count)));
        }
        assert_eq!(
            decode_bounded(&bytes),
            Err(InspectionError::MessagePackLimit(MessagePackLimit::Nodes))
        );
    }
}
