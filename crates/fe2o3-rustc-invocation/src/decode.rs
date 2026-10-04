use std::str;

use crate::DecodeError;
use crate::encode::{
    HEADER_BYTES, INVOCATION_DESCRIPTOR_MAGIC, INVOCATION_DESCRIPTOR_VERSION, encode_descriptor_v1,
};
use crate::model::{
    AmdTargetIdTextV1, BackendToolsV1, CargoIdentityV1, CargoPackageV1, CargoTargetKindV1,
    CargoTargetV1, CompileEnvironmentEntryV1, CrateTypeV1, DeviceConfigurationV1, EditionV1,
    MAX_ARGUMENT_BYTES, MAX_COMPILE_ENVIRONMENT_ENTRIES, MAX_CRATE_TYPES, MAX_DESCRIPTOR_BYTES,
    MAX_ENVIRONMENT_VALUE_BYTES, MAX_FEATURES, MAX_NAME_BYTES, MAX_PATH_BYTES, MAX_RUSTC_ARGUMENTS,
    MAX_TEXT_BYTES, OutputDomainV1, RustcIdentityV1, RustcInvocationDescriptorV1, RustcUnitV1,
    TestStateV1, ToolIdentityV1, VerificationModeV1,
};

/// Decodes and validates canonical V1 descriptor bytes.
///
/// Length and count bounds are checked before allocation. Successful decoding
/// includes an exact re-encoding check.
pub fn decode_descriptor_v1(bytes: &[u8]) -> Result<RustcInvocationDescriptorV1, DecodeError> {
    if bytes.len() > MAX_DESCRIPTOR_BYTES {
        return Err(DecodeError::TooLarge {
            max: MAX_DESCRIPTOR_BYTES,
        });
    }

    let mut reader = Reader::new(bytes);
    if reader.fixed::<8>()? != INVOCATION_DESCRIPTOR_MAGIC {
        return Err(DecodeError::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != INVOCATION_DESCRIPTOR_VERSION {
        return Err(DecodeError::UnknownVersion(version));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(DecodeError::UnsupportedFlags(flags));
    }
    let declared_len = reader.u32()?;
    if declared_len < HEADER_BYTES as u32 {
        return Err(DecodeError::InvalidLength {
            declared: declared_len,
        });
    }
    let declared_len_usize =
        usize::try_from(declared_len).map_err(|_| DecodeError::InvalidLength {
            declared: declared_len,
        })?;
    if declared_len_usize > bytes.len() {
        return Err(DecodeError::Truncated);
    }
    if declared_len_usize < bytes.len() {
        return Err(DecodeError::TrailingBytes);
    }
    reader.reserved_u32("descriptor header")?;

    let cargo_executable = parse_tool(&mut reader)?;
    let package = CargoPackageV1::new(
        reader.name("Cargo package name")?,
        reader.text("Cargo package version")?,
        reader.path("Cargo manifest path")?,
    )?;

    let target_name = reader.name("Cargo target name")?;
    let target_kind = parse_cargo_target_kind(reader.u16()?)?;
    let edition = parse_edition(reader.u16()?)?;
    reader.reserved_u16("Cargo target flags")?;
    let crate_type_count = reader.count_u16("crate types", MAX_CRATE_TYPES)?;
    let feature_count = reader.count_u16("Cargo features", MAX_FEATURES)?;
    reader.reserved_u32("Cargo target")?;
    let source_path = reader.path("Cargo target source path")?;
    let mut crate_types = Vec::with_capacity(crate_type_count);
    for _ in 0..crate_type_count {
        crate_types.push(parse_crate_type(reader.u16()?)?);
    }
    let mut features = Vec::with_capacity(feature_count);
    for _ in 0..feature_count {
        features.push(reader.name("Cargo feature")?);
    }
    let target = CargoTargetV1::new(
        target_name,
        target_kind,
        crate_types,
        edition,
        source_path,
        features,
    )?;
    let cargo = CargoIdentityV1::new(cargo_executable, package, target);

    let rustc_executable = parse_tool(&mut reader)?;
    let crate_name = reader.name("rustc crate name")?;
    let host_target = reader.name("rustc host target")?;
    let effective_target = reader.name("rustc effective target")?;
    let test_state = parse_test_state(reader.u8()?)?;
    reader.reserved_u8("rustc unit")?;
    reader.reserved_u16("rustc unit")?;
    let argument_count = reader.count_u32("rustc arguments", MAX_RUSTC_ARGUMENTS)?;
    let mut argv = Vec::with_capacity(argument_count);
    for _ in 0..argument_count {
        argv.push(reader.argument()?);
    }
    let rustc = RustcIdentityV1::new(
        rustc_executable,
        RustcUnitV1::new(crate_name, host_target, effective_target, test_state, argv)?,
    );

    let backend = parse_tool(&mut reader)?;
    let clang = parse_tool(&mut reader)?;
    let linker = parse_tool(&mut reader)?;
    let inspector = match reader.u8()? {
        0 => None,
        1 => Some(()),
        tag => {
            return Err(DecodeError::UnknownTag {
                kind: "inspector presence",
                tag: u16::from(tag),
            });
        }
    };
    reader.reserved_u8("inspector identity")?;
    reader.reserved_u16("inspector identity")?;
    let inspector = inspector.map(|()| parse_tool(&mut reader)).transpose()?;
    let tools = BackendToolsV1::new(backend, clang, linker, inspector);

    let amd_target = AmdTargetIdTextV1::new(reader.text("AMD target ID")?)?;
    let verification = parse_verification_mode(reader.u8()?)?;
    reader.reserved_u8("device configuration")?;
    reader.reserved_u16("device configuration")?;
    let device = DeviceConfigurationV1::new(amd_target, verification);
    let output = OutputDomainV1::new(
        reader.path("workspace root")?,
        reader.path("artifact output directory")?,
    )?;

    let environment_count =
        reader.count_u16("compile environment", MAX_COMPILE_ENVIRONMENT_ENTRIES)?;
    reader.reserved_u16("compile environment")?;
    let mut compile_environment = Vec::with_capacity(environment_count);
    for _ in 0..environment_count {
        compile_environment.push(CompileEnvironmentEntryV1::new(
            reader.name("compile environment key")?,
            reader.environment_value()?,
        )?);
    }

    if !reader.is_finished() {
        return Err(DecodeError::TrailingBytes);
    }
    let descriptor =
        RustcInvocationDescriptorV1::new(cargo, rustc, tools, device, output, compile_environment)?;
    if encode_descriptor_v1(&descriptor)? != bytes {
        return Err(DecodeError::NonCanonical);
    }
    Ok(descriptor)
}

fn parse_tool(reader: &mut Reader<'_>) -> Result<ToolIdentityV1, DecodeError> {
    Ok(ToolIdentityV1::new(
        reader.text("tool version")?,
        reader.fixed::<32>()?,
    )?)
}

fn parse_cargo_target_kind(tag: u16) -> Result<CargoTargetKindV1, DecodeError> {
    match tag {
        1 => Ok(CargoTargetKindV1::Library),
        2 => Ok(CargoTargetKindV1::Binary),
        3 => Ok(CargoTargetKindV1::Example),
        4 => Ok(CargoTargetKindV1::Test),
        5 => Ok(CargoTargetKindV1::Benchmark),
        6 => Ok(CargoTargetKindV1::BuildScript),
        7 => Ok(CargoTargetKindV1::ProcMacro),
        _ => Err(DecodeError::UnknownTag {
            kind: "Cargo target kind",
            tag,
        }),
    }
}

fn parse_crate_type(tag: u16) -> Result<CrateTypeV1, DecodeError> {
    match tag {
        1 => Ok(CrateTypeV1::Lib),
        2 => Ok(CrateTypeV1::Rlib),
        3 => Ok(CrateTypeV1::Dylib),
        4 => Ok(CrateTypeV1::Cdylib),
        5 => Ok(CrateTypeV1::Staticlib),
        6 => Ok(CrateTypeV1::ProcMacro),
        7 => Ok(CrateTypeV1::Bin),
        _ => Err(DecodeError::UnknownTag {
            kind: "crate type",
            tag,
        }),
    }
}

fn parse_edition(tag: u16) -> Result<EditionV1, DecodeError> {
    match tag {
        1 => Ok(EditionV1::Rust2015),
        2 => Ok(EditionV1::Rust2018),
        3 => Ok(EditionV1::Rust2021),
        4 => Ok(EditionV1::Rust2024),
        _ => Err(DecodeError::UnknownTag {
            kind: "Rust edition",
            tag,
        }),
    }
}

fn parse_test_state(tag: u8) -> Result<TestStateV1, DecodeError> {
    match tag {
        1 => Ok(TestStateV1::NotTest),
        2 => Ok(TestStateV1::Test),
        _ => Err(DecodeError::UnknownTag {
            kind: "rustc test state",
            tag: u16::from(tag),
        }),
    }
}

fn parse_verification_mode(tag: u8) -> Result<VerificationModeV1, DecodeError> {
    match tag {
        1 => Ok(VerificationModeV1::Disabled),
        2 => Ok(VerificationModeV1::Required),
        _ => Err(DecodeError::UnknownTag {
            kind: "verification mode",
            tag: u16::from(tag),
        }),
    }
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

    fn name(&mut self, field: &'static str) -> Result<String, DecodeError> {
        let length = usize::from(self.u16()?);
        self.string(length, MAX_NAME_BYTES, field)
    }

    fn text(&mut self, field: &'static str) -> Result<String, DecodeError> {
        let length = usize::from(self.u16()?);
        self.string(length, MAX_TEXT_BYTES, field)
    }

    fn path(&mut self, field: &'static str) -> Result<String, DecodeError> {
        let raw = self.u32()?;
        let length = usize::try_from(raw).map_err(|_| DecodeError::CountOutOfRange {
            field,
            count: u64::from(raw),
            max: MAX_PATH_BYTES,
        })?;
        self.string(length, MAX_PATH_BYTES, field)
    }

    fn argument(&mut self) -> Result<String, DecodeError> {
        let raw = self.u32()?;
        let length = usize::try_from(raw).map_err(|_| DecodeError::CountOutOfRange {
            field: "rustc argument bytes",
            count: u64::from(raw),
            max: MAX_ARGUMENT_BYTES,
        })?;
        self.string(length, MAX_ARGUMENT_BYTES, "rustc argument")
    }

    fn environment_value(&mut self) -> Result<String, DecodeError> {
        let raw = self.u32()?;
        let length = usize::try_from(raw).map_err(|_| DecodeError::CountOutOfRange {
            field: "compile environment value bytes",
            count: u64::from(raw),
            max: MAX_ENVIRONMENT_VALUE_BYTES,
        })?;
        self.string(
            length,
            MAX_ENVIRONMENT_VALUE_BYTES,
            "compile environment value",
        )
    }

    fn string(
        &mut self,
        length: usize,
        max: usize,
        field: &'static str,
    ) -> Result<String, DecodeError> {
        if length > max {
            return Err(DecodeError::CountOutOfRange {
                field,
                count: length as u64,
                max,
            });
        }
        let bytes = self.take(length)?;
        let value = str::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8 { field })?;
        Ok(value.to_owned())
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
