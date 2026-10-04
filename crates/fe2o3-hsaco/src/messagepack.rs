use std::collections::BTreeSet;

use crate::{
    InspectionError, MAX_MESSAGEPACK_BLOB_BYTES, MAX_MESSAGEPACK_COLLECTION_ITEMS,
    MAX_MESSAGEPACK_DEPTH, MAX_MESSAGEPACK_NODES, MAX_MESSAGEPACK_STRING_BYTES,
    MAX_MESSAGEPACK_TOTAL_ITEMS, MessagePackLimit,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StringRef<'a>(&'a [u8]);

impl<'a> StringRef<'a> {
    pub(crate) fn as_str(self) -> Option<&'a str> {
        core::str::from_utf8(self.0).ok()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Integer {
    Unsigned(u64),
    Signed(i64),
}

impl Integer {
    const fn as_u64(self) -> Option<u64> {
        match self {
            Self::Unsigned(value) => Some(value),
            Self::Signed(value) if value >= 0 => Some(value as u64),
            Self::Signed(_) => None,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum ValueRef<'a> {
    Nil,
    Boolean(bool),
    Integer(Integer),
    F32,
    F64,
    String(StringRef<'a>),
    Binary,
    Array(Vec<ValueRef<'a>>),
    Map(Vec<(ValueRef<'a>, ValueRef<'a>)>),
    Ext,
}

impl ValueRef<'_> {
    pub(crate) const fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Integer(value) => value.as_u64(),
            _ => None,
        }
    }
}

pub(crate) fn decode_bounded(bytes: &[u8]) -> Result<ValueRef<'_>, InspectionError> {
    preflight(bytes)?;

    let mut cursor = 0usize;
    let value = decode_value(bytes, &mut cursor, 0)?;
    if cursor != bytes.len() {
        return Err(InspectionError::TrailingMessagePack);
    }
    validate_value_tree(&value)?;
    Ok(value)
}

fn decode_value<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    depth: usize,
) -> Result<ValueRef<'a>, InspectionError> {
    if depth > MAX_MESSAGEPACK_DEPTH {
        return Err(InspectionError::MessagePackLimit(MessagePackLimit::Depth));
    }
    let marker = take_u8(bytes, cursor)?;
    match marker {
        0x00..=0x7f => Ok(ValueRef::Integer(Integer::Unsigned(u64::from(marker)))),
        0x80..=0x8f => decode_map(bytes, cursor, depth, usize::from(marker & 0x0f)),
        0x90..=0x9f => decode_array(bytes, cursor, depth, usize::from(marker & 0x0f)),
        0xa0..=0xbf => decode_string(bytes, cursor, usize::from(marker & 0x1f)),
        0xc0 => Ok(ValueRef::Nil),
        0xc1 => Err(InspectionError::MalformedMessagePack),
        0xc2 => Ok(ValueRef::Boolean(false)),
        0xc3 => Ok(ValueRef::Boolean(true)),
        0xc4 => {
            let length = usize::from(take_u8(bytes, cursor)?);
            take_slice(bytes, cursor, length)?;
            Ok(ValueRef::Binary)
        }
        0xc5 => {
            let length = usize::from(take_u16(bytes, cursor)?);
            take_slice(bytes, cursor, length)?;
            Ok(ValueRef::Binary)
        }
        0xc6 => {
            let length = usize_from_u32(take_u32(bytes, cursor)?)?;
            take_slice(bytes, cursor, length)?;
            Ok(ValueRef::Binary)
        }
        0xc7 => {
            let length = usize::from(take_u8(bytes, cursor)?);
            decode_extension(bytes, cursor, length)
        }
        0xc8 => {
            let length = usize::from(take_u16(bytes, cursor)?);
            decode_extension(bytes, cursor, length)
        }
        0xc9 => {
            let length = usize_from_u32(take_u32(bytes, cursor)?)?;
            decode_extension(bytes, cursor, length)
        }
        0xca => {
            take_u32(bytes, cursor)?;
            Ok(ValueRef::F32)
        }
        0xcb => {
            take_u64(bytes, cursor)?;
            Ok(ValueRef::F64)
        }
        0xcc => Ok(ValueRef::Integer(Integer::Unsigned(u64::from(take_u8(
            bytes, cursor,
        )?)))),
        0xcd => Ok(ValueRef::Integer(Integer::Unsigned(u64::from(take_u16(
            bytes, cursor,
        )?)))),
        0xce => Ok(ValueRef::Integer(Integer::Unsigned(u64::from(take_u32(
            bytes, cursor,
        )?)))),
        0xcf => Ok(ValueRef::Integer(Integer::Unsigned(take_u64(
            bytes, cursor,
        )?))),
        0xd0 => Ok(ValueRef::Integer(Integer::Signed(i64::from(
            take_u8(bytes, cursor)? as i8,
        )))),
        0xd1 => Ok(ValueRef::Integer(Integer::Signed(i64::from(
            take_u16(bytes, cursor)? as i16,
        )))),
        0xd2 => Ok(ValueRef::Integer(Integer::Signed(i64::from(
            take_u32(bytes, cursor)? as i32,
        )))),
        0xd3 => Ok(ValueRef::Integer(Integer::Signed(
            take_u64(bytes, cursor)? as i64
        ))),
        0xd4 => decode_extension(bytes, cursor, 1),
        0xd5 => decode_extension(bytes, cursor, 2),
        0xd6 => decode_extension(bytes, cursor, 4),
        0xd7 => decode_extension(bytes, cursor, 8),
        0xd8 => decode_extension(bytes, cursor, 16),
        0xd9 => {
            let length = usize::from(take_u8(bytes, cursor)?);
            decode_string(bytes, cursor, length)
        }
        0xda => {
            let length = usize::from(take_u16(bytes, cursor)?);
            decode_string(bytes, cursor, length)
        }
        0xdb => {
            let length = usize_from_u32(take_u32(bytes, cursor)?)?;
            decode_string(bytes, cursor, length)
        }
        0xdc => {
            let length = usize::from(take_u16(bytes, cursor)?);
            decode_array(bytes, cursor, depth, length)
        }
        0xdd => {
            let length = usize_from_u32(take_u32(bytes, cursor)?)?;
            decode_array(bytes, cursor, depth, length)
        }
        0xde => {
            let length = usize::from(take_u16(bytes, cursor)?);
            decode_map(bytes, cursor, depth, length)
        }
        0xdf => {
            let length = usize_from_u32(take_u32(bytes, cursor)?)?;
            decode_map(bytes, cursor, depth, length)
        }
        0xe0..=0xff => Ok(ValueRef::Integer(Integer::Signed(i64::from(marker as i8)))),
    }
}

fn decode_array<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    depth: usize,
    length: usize,
) -> Result<ValueRef<'a>, InspectionError> {
    let mut values = Vec::with_capacity(length);
    for _ in 0..length {
        values.push(decode_value(bytes, cursor, depth + 1)?);
    }
    Ok(ValueRef::Array(values))
}

fn decode_map<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    depth: usize,
    length: usize,
) -> Result<ValueRef<'a>, InspectionError> {
    let mut values = Vec::with_capacity(length);
    for _ in 0..length {
        let key = decode_value(bytes, cursor, depth + 1)?;
        let value = decode_value(bytes, cursor, depth + 1)?;
        values.push((key, value));
    }
    Ok(ValueRef::Map(values))
}

fn decode_string<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<ValueRef<'a>, InspectionError> {
    Ok(ValueRef::String(StringRef(take_slice(
        bytes, cursor, length,
    )?)))
}

fn decode_extension<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<ValueRef<'a>, InspectionError> {
    take_u8(bytes, cursor)?;
    take_slice(bytes, cursor, length)?;
    Ok(ValueRef::Ext)
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

fn take_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64, InspectionError> {
    let value = take_array::<8>(bytes, cursor)?;
    Ok(u64::from_be_bytes(value))
}

fn take_slice<'a>(
    bytes: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], InspectionError> {
    let end = cursor
        .checked_add(length)
        .ok_or(InspectionError::MalformedMessagePack)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(InspectionError::MalformedMessagePack)?;
    *cursor = end;
    Ok(value)
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
        | ValueRef::F32
        | ValueRef::F64
        | ValueRef::Binary
        | ValueRef::Ext => {}
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
