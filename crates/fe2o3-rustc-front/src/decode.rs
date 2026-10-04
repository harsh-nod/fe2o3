use std::str;

use crate::encode::{
    FRONTEND_UNIT_MAGIC_V1, FRONTEND_UNIT_VERSION_V1, HEADER_BYTES_V1, encode_frontend_unit_v1,
};
use crate::{
    BasicBlockV1, BlockIdV1, DecodeError, FrontendUnitV1, FunctionIdentityV1, FunctionRoleV1,
    MAX_BLOCKS_PER_FUNCTION_V1, MAX_FUNCTION_NAME_BYTES_V1, MAX_FUNCTIONS_V1,
    MAX_PARAMETERS_PER_FUNCTION_V1, MAX_SUCCESSORS_PER_BLOCK_V1, MAX_TOTAL_BLOCKS_V1,
    MAX_UNIT_BYTES_V1, MonomorphizedFunctionV1, SourceFileIdentityV1, SourceLocationV1,
    StableTypeIdentityV1, TypedSignatureV1,
};

pub fn decode_frontend_unit_v1(bytes: &[u8]) -> Result<FrontendUnitV1, DecodeError> {
    if bytes.len() > MAX_UNIT_BYTES_V1 {
        return Err(DecodeError::TooLarge {
            max: MAX_UNIT_BYTES_V1,
        });
    }
    let mut reader = Reader::new(bytes);
    if reader.fixed::<8>()? != FRONTEND_UNIT_MAGIC_V1 {
        return Err(DecodeError::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != FRONTEND_UNIT_VERSION_V1 {
        return Err(DecodeError::UnknownVersion(version));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(DecodeError::UnsupportedFlags(flags));
    }
    let declared = reader.u32()?;
    if declared < HEADER_BYTES_V1 as u32 {
        return Err(DecodeError::InvalidLength { declared });
    }
    let declared =
        usize::try_from(declared).map_err(|_| DecodeError::InvalidLength { declared })?;
    if declared > bytes.len() {
        return Err(DecodeError::Truncated);
    }
    if declared < bytes.len() {
        return Err(DecodeError::TrailingBytes);
    }
    let function_count = reader.count_u32("frontend functions", MAX_FUNCTIONS_V1)?;
    reader.reserved_u32("frontend header")?;

    let mut functions = Vec::with_capacity(function_count);
    let mut total_blocks = 0_usize;
    for _ in 0..function_count {
        functions.push(decode_function(&mut reader, &mut total_blocks)?);
    }
    if !reader.is_finished() {
        return Err(DecodeError::TrailingBytes);
    }
    let unit = FrontendUnitV1::new(functions)?;
    if encode_frontend_unit_v1(&unit)? != bytes {
        return Err(DecodeError::NonCanonical);
    }
    Ok(unit)
}

fn decode_function(
    reader: &mut Reader<'_>,
    total_blocks: &mut usize,
) -> Result<MonomorphizedFunctionV1, DecodeError> {
    let identity = FunctionIdentityV1::new(reader.fixed::<32>()?)?;
    let role = match reader.u8()? {
        1 => FunctionRoleV1::Kernel,
        2 => FunctionRoleV1::Helper,
        tag => {
            return Err(DecodeError::UnknownTag {
                kind: "function role",
                tag: u16::from(tag),
            });
        }
    };
    reader.reserved_u8("function header")?;
    reader.reserved_u16("function header")?;
    let diagnostic_name = reader.text("function diagnostic name", MAX_FUNCTION_NAME_BYTES_V1)?;
    let location = reader.location()?;
    let parameter_count =
        reader.count_u16("function parameters", MAX_PARAMETERS_PER_FUNCTION_V1)?;
    reader.reserved_u16("function signature")?;
    let return_type = StableTypeIdentityV1::new(reader.fixed::<32>()?)?;
    let mut parameters = Vec::with_capacity(parameter_count);
    for _ in 0..parameter_count {
        parameters.push(StableTypeIdentityV1::new(reader.fixed::<32>()?)?);
    }
    let signature = TypedSignatureV1::new(parameters, return_type)?;
    let entry_block = BlockIdV1::new(reader.u32()?);
    let block_count = reader.count_u32("function CFG blocks", MAX_BLOCKS_PER_FUNCTION_V1)?;
    *total_blocks = total_blocks
        .checked_add(block_count)
        .ok_or(DecodeError::CountOutOfRange {
            field: "frontend CFG blocks",
            count: u64::MAX,
            max: MAX_TOTAL_BLOCKS_V1,
        })?;
    if *total_blocks > MAX_TOTAL_BLOCKS_V1 {
        return Err(DecodeError::CountOutOfRange {
            field: "frontend CFG blocks",
            count: *total_blocks as u64,
            max: MAX_TOTAL_BLOCKS_V1,
        });
    }
    let mut blocks = Vec::with_capacity(block_count);
    for _ in 0..block_count {
        blocks.push(decode_block(reader)?);
    }
    Ok(MonomorphizedFunctionV1::new(
        identity,
        role,
        diagnostic_name,
        location,
        signature,
        entry_block,
        blocks,
    )?)
}

fn decode_block(reader: &mut Reader<'_>) -> Result<BasicBlockV1, DecodeError> {
    let id = BlockIdV1::new(reader.u32()?);
    let location = reader.location()?;
    let successor_count = reader.count_u16("CFG block successors", MAX_SUCCESSORS_PER_BLOCK_V1)?;
    reader.reserved_u16("CFG block")?;
    let mut successors = Vec::with_capacity(successor_count);
    for _ in 0..successor_count {
        successors.push(BlockIdV1::new(reader.u32()?));
    }
    Ok(BasicBlockV1::new(id, location, successors)?)
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(DecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(DecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        self.take(N)?.try_into().map_err(|_| DecodeError::Truncated)
    }

    fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, DecodeError> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn reserved_u8(&mut self, field: &'static str) -> Result<(), DecodeError> {
        if self.u8()? != 0 {
            return Err(DecodeError::NonzeroReserved { field });
        }
        Ok(())
    }

    fn reserved_u16(&mut self, field: &'static str) -> Result<(), DecodeError> {
        if self.u16()? != 0 {
            return Err(DecodeError::NonzeroReserved { field });
        }
        Ok(())
    }

    fn reserved_u32(&mut self, field: &'static str) -> Result<(), DecodeError> {
        if self.u32()? != 0 {
            return Err(DecodeError::NonzeroReserved { field });
        }
        Ok(())
    }

    fn count_u16(&mut self, field: &'static str, max: usize) -> Result<usize, DecodeError> {
        let count = usize::from(self.u16()?);
        if count > max {
            return Err(DecodeError::CountOutOfRange {
                field,
                count: count as u64,
                max,
            });
        }
        Ok(count)
    }

    fn count_u32(&mut self, field: &'static str, max: usize) -> Result<usize, DecodeError> {
        let raw = self.u32()?;
        let count = usize::try_from(raw).map_err(|_| DecodeError::CountOutOfRange {
            field,
            count: u64::from(raw),
            max,
        })?;
        if count > max {
            return Err(DecodeError::CountOutOfRange {
                field,
                count: u64::from(raw),
                max,
            });
        }
        Ok(count)
    }

    fn text(&mut self, field: &'static str, max: usize) -> Result<String, DecodeError> {
        let length = usize::from(self.u16()?);
        if length > max {
            return Err(DecodeError::CountOutOfRange {
                field,
                count: length as u64,
                max,
            });
        }
        let bytes = self.take(length)?;
        str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| DecodeError::InvalidUtf8 { field })
    }

    fn location(&mut self) -> Result<SourceLocationV1, DecodeError> {
        Ok(SourceLocationV1::new(
            SourceFileIdentityV1::new(self.fixed::<32>()?)?,
            self.u32()?,
            self.u32()?,
        )?)
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
