use crate::{
    FrontendUnitV1, FunctionRoleV1, MAX_UNIT_BYTES_V1, MonomorphizedFunctionV1, ValidationError,
};

pub const FRONTEND_UNIT_MAGIC_V1: [u8; 8] = *b"FE2O3RF\0";
pub const FRONTEND_UNIT_VERSION_V1: u16 = 1;
pub(crate) const HEADER_BYTES_V1: usize = 24;

pub fn encode_frontend_unit_v1(unit: &FrontendUnitV1) -> Result<Vec<u8>, ValidationError> {
    let mut writer = Writer::new();
    writer.bytes(&FRONTEND_UNIT_MAGIC_V1);
    writer.u16(FRONTEND_UNIT_VERSION_V1);
    writer.u16(0);
    writer.u32(0);
    writer.u32(
        u32::try_from(unit.functions().len()).map_err(|_| ValidationError::Overflow {
            field: "function count",
        })?,
    );
    writer.u32(0);
    debug_assert_eq!(writer.bytes.len(), HEADER_BYTES_V1);

    for function in unit.functions() {
        encode_function(&mut writer, function)?;
    }
    if writer.bytes.len() > MAX_UNIT_BYTES_V1 {
        return Err(ValidationError::EncodedUnitTooLarge {
            max: MAX_UNIT_BYTES_V1,
        });
    }
    let length = u32::try_from(writer.bytes.len()).map_err(|_| ValidationError::Overflow {
        field: "frontend unit length",
    })?;
    writer.bytes[12..16].copy_from_slice(&length.to_le_bytes());
    Ok(writer.bytes)
}

pub(crate) fn validate_encoded_size(unit: &FrontendUnitV1) -> Result<(), ValidationError> {
    encode_frontend_unit_v1(unit).map(|_| ())
}

fn encode_function(
    writer: &mut Writer,
    function: &MonomorphizedFunctionV1,
) -> Result<(), ValidationError> {
    writer.bytes(function.identity().as_bytes());
    writer.u8(match function.role() {
        FunctionRoleV1::Kernel => 1,
        FunctionRoleV1::Helper => 2,
    });
    writer.u8(0);
    writer.u16(0);
    writer.text(function.diagnostic_name())?;
    writer.location(function.location());
    writer.u16(
        u16::try_from(function.signature().parameters().len()).map_err(|_| {
            ValidationError::Overflow {
                field: "function parameter count",
            }
        })?,
    );
    writer.u16(0);
    writer.bytes(function.signature().return_type().as_bytes());
    for parameter in function.signature().parameters() {
        writer.bytes(parameter.as_bytes());
    }
    writer.u32(function.entry_block().get());
    writer.u32(
        u32::try_from(function.blocks().len()).map_err(|_| ValidationError::Overflow {
            field: "CFG block count",
        })?,
    );
    for block in function.blocks() {
        writer.u32(block.id().get());
        writer.location(block.location());
        writer.u16(u16::try_from(block.successors().len()).map_err(|_| {
            ValidationError::Overflow {
                field: "CFG successor count",
            }
        })?);
        writer.u16(0);
        for successor in block.successors() {
            writer.u32(successor.get());
        }
    }
    Ok(())
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn text(&mut self, value: &str) -> Result<(), ValidationError> {
        self.u16(
            u16::try_from(value.len()).map_err(|_| ValidationError::Overflow {
                field: "function diagnostic name length",
            })?,
        );
        self.bytes(value.as_bytes());
        Ok(())
    }

    fn location(&mut self, value: crate::SourceLocationV1) {
        self.bytes(value.file().as_bytes());
        self.u32(value.line());
        self.u32(value.column());
    }
}
