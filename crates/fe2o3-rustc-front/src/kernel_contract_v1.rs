//! Canonical frontend wire record for launch and unsafe-assembly declarations.

use std::fmt;

pub const FRONTEND_KERNEL_CONTRACT_MAGIC_V1: [u8; 8] = *b"FE2O3KF\0";
pub const FRONTEND_KERNEL_CONTRACT_VERSION_V1: u16 = 1;
pub const MAX_FRONTEND_KERNEL_CONTRACT_BYTES_V1: usize = 64;

pub const KERNEL_FRONTEND_REGISTRATION_PREFIX_V1: &str = "__fe2o3_kernel_frontend_contract_v1_";
pub const KERNEL_FRONTEND_REGISTRATION_MAGIC_V1: u64 = u64::from_le_bytes(*b"FE2O3KFA");
pub const KERNEL_FRONTEND_REGISTRATION_VERSION_V1: u16 = 1;
pub const KERNEL_FRONTEND_REGISTRATION_KIND_V1: u16 = 1;

const HEADER_BYTES_V1: usize = 20;
const FLAG_LAUNCH: u16 = 0x0001;
const FLAG_UNSAFE_ASSEMBLY: u16 = 0x0002;
const LAUNCH_FLAG_REQUIRED: u16 = 0x0001;
const LAUNCH_FLAG_MAXIMUM: u16 = 0x0002;
const LAUNCH_FLAG_OCCUPANCY: u16 = 0x0004;
const MAX_WORKGROUP_THREADS_V1: u64 = 1_024;
const MAX_RESIDENT_WORKGROUPS_PER_COMPUTE_UNIT_V1: u16 = 64;

pub const ASSEMBLY_OPERAND_SGPR_V1: u16 = 0x0001;
pub const ASSEMBLY_OPERAND_VGPR_V1: u16 = 0x0002;
pub const ASSEMBLY_OPERAND_IMMEDIATE_V1: u16 = 0x0004;
pub const ASSEMBLY_OPERAND_ADDRESS_V1: u16 = 0x0008;
const ASSEMBLY_OPERANDS_V1: u16 = 0x000f;

pub const ASSEMBLY_OPTION_NOMEM_V1: u16 = 0x0001;
pub const ASSEMBLY_OPTION_READONLY_V1: u16 = 0x0002;
pub const ASSEMBLY_OPTION_PURE_V1: u16 = 0x0004;
pub const ASSEMBLY_OPTION_PRESERVES_FLAGS_V1: u16 = 0x0008;
pub const ASSEMBLY_OPTION_NOSTACK_V1: u16 = 0x0010;
const ASSEMBLY_OPTIONS_V1: u16 = 0x001f;

pub const ASSEMBLY_EFFECT_READ_GLOBAL_V1: u16 = 0x0001;
pub const ASSEMBLY_EFFECT_WRITE_GLOBAL_V1: u16 = 0x0002;
pub const ASSEMBLY_EFFECT_READ_WORKGROUP_V1: u16 = 0x0004;
pub const ASSEMBLY_EFFECT_WRITE_WORKGROUP_V1: u16 = 0x0008;
pub const ASSEMBLY_EFFECT_ATOMIC_V1: u16 = 0x0010;
pub const ASSEMBLY_EFFECT_BARRIER_V1: u16 = 0x0020;
pub const ASSEMBLY_EFFECT_CONTROL_FLOW_V1: u16 = 0x0040;
const ASSEMBLY_EFFECTS_V1: u16 = 0x007f;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelFrontendContractValidationErrorV1 {
    Empty,
    ZeroWorkgroupDimension,
    WorkgroupVolumeTooLarge(u64),
    RequiredExceedsMaximum,
    OccupancyRequiresMaximum,
    InvalidOccupancy(u16),
    EmptyAssemblyOperands,
    UnsupportedAssemblyOperands(u16),
    UnsupportedAssemblyOptions(u16),
    UnsupportedAssemblyEffects(u16),
    ConflictingAssemblyOptions,
    AssemblyEffectsConflictWithOptions,
}

impl fmt::Display for KernelFrontendContractValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("kernel frontend contract is empty"),
            Self::ZeroWorkgroupDimension => {
                formatter.write_str("workgroup dimensions must be nonzero")
            }
            Self::WorkgroupVolumeTooLarge(actual) => {
                write!(formatter, "workgroup volume {actual} exceeds 1024")
            }
            Self::RequiredExceedsMaximum => {
                formatter.write_str("required workgroup dimensions exceed maximum dimensions")
            }
            Self::OccupancyRequiresMaximum => formatter
                .write_str("minimum resident workgroups requires maximum workgroup dimensions"),
            Self::InvalidOccupancy(actual) => write!(
                formatter,
                "minimum resident workgroups {actual} is outside 1..=64"
            ),
            Self::EmptyAssemblyOperands => {
                formatter.write_str("unsafe assembly operand set is empty")
            }
            Self::UnsupportedAssemblyOperands(bits) => {
                write!(formatter, "unsupported unsafe assembly operands {bits:#x}")
            }
            Self::UnsupportedAssemblyOptions(bits) => {
                write!(formatter, "unsupported unsafe assembly options {bits:#x}")
            }
            Self::UnsupportedAssemblyEffects(bits) => {
                write!(formatter, "unsupported unsafe assembly effects {bits:#x}")
            }
            Self::ConflictingAssemblyOptions => {
                formatter.write_str("unsafe assembly options conflict")
            }
            Self::AssemblyEffectsConflictWithOptions => {
                formatter.write_str("unsafe assembly effects conflict with options")
            }
        }
    }
}

impl std::error::Error for KernelFrontendContractValidationErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelFrontendContractDecodeErrorV1 {
    TooLarge,
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    InvalidLength(u32),
    TrailingBytes,
    NonzeroReserved(&'static str),
    UnknownTag { kind: &'static str, tag: u16 },
    NonCanonical,
    Validation(KernelFrontendContractValidationErrorV1),
}

impl fmt::Display for KernelFrontendContractDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("kernel frontend contract exceeds 64 bytes"),
            Self::Truncated => formatter.write_str("kernel frontend contract is truncated"),
            Self::InvalidMagic => formatter.write_str("kernel frontend contract magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "unsupported kernel frontend contract version {version}"
                )
            }
            Self::UnsupportedFlags(flags) => {
                write!(
                    formatter,
                    "unsupported kernel frontend contract flags {flags:#x}"
                )
            }
            Self::InvalidLength(length) => {
                write!(
                    formatter,
                    "invalid kernel frontend contract length {length}"
                )
            }
            Self::TrailingBytes => {
                formatter.write_str("kernel frontend contract contains trailing bytes")
            }
            Self::NonzeroReserved(field) => write!(formatter, "{field} reserved field is nonzero"),
            Self::UnknownTag { kind, tag } => write!(formatter, "unknown {kind} tag {tag}"),
            Self::NonCanonical => formatter.write_str("kernel frontend contract is not canonical"),
            Self::Validation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for KernelFrontendContractDecodeErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<KernelFrontendContractValidationErrorV1> for KernelFrontendContractDecodeErrorV1 {
    fn from(value: KernelFrontendContractValidationErrorV1) -> Self {
        Self::Validation(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrontendWorkgroupDimensionsV1([u32; 3]);

impl FrontendWorkgroupDimensionsV1 {
    pub fn new(value: [u32; 3]) -> Result<Self, KernelFrontendContractValidationErrorV1> {
        if value.contains(&0) {
            return Err(KernelFrontendContractValidationErrorV1::ZeroWorkgroupDimension);
        }
        let volume = value
            .into_iter()
            .try_fold(1_u64, |volume, component| {
                volume.checked_mul(u64::from(component))
            })
            .ok_or(KernelFrontendContractValidationErrorV1::WorkgroupVolumeTooLarge(u64::MAX))?;
        if volume > MAX_WORKGROUP_THREADS_V1 {
            return Err(KernelFrontendContractValidationErrorV1::WorkgroupVolumeTooLarge(volume));
        }
        Ok(Self(value))
    }

    pub const fn as_array(self) -> [u32; 3] {
        self.0
    }

    fn contains(self, required: Self) -> bool {
        self.0
            .into_iter()
            .zip(required.0)
            .all(|(maximum, required)| required <= maximum)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrontendLaunchBoundsV1 {
    required: Option<FrontendWorkgroupDimensionsV1>,
    maximum: Option<FrontendWorkgroupDimensionsV1>,
    min_workgroups_per_compute_unit: Option<u16>,
}

impl FrontendLaunchBoundsV1 {
    pub fn new(
        required: Option<FrontendWorkgroupDimensionsV1>,
        maximum: Option<FrontendWorkgroupDimensionsV1>,
        min_workgroups_per_compute_unit: Option<u16>,
    ) -> Result<Self, KernelFrontendContractValidationErrorV1> {
        if required.is_none() && maximum.is_none() {
            return Err(KernelFrontendContractValidationErrorV1::Empty);
        }
        if let (Some(required), Some(maximum)) = (required, maximum)
            && !maximum.contains(required)
        {
            return Err(KernelFrontendContractValidationErrorV1::RequiredExceedsMaximum);
        }
        if let Some(actual) = min_workgroups_per_compute_unit {
            if maximum.is_none() {
                return Err(KernelFrontendContractValidationErrorV1::OccupancyRequiresMaximum);
            }
            if actual == 0 || actual > MAX_RESIDENT_WORKGROUPS_PER_COMPUTE_UNIT_V1 {
                return Err(KernelFrontendContractValidationErrorV1::InvalidOccupancy(
                    actual,
                ));
            }
        }
        Ok(Self {
            required,
            maximum,
            min_workgroups_per_compute_unit,
        })
    }

    pub const fn required(self) -> Option<FrontendWorkgroupDimensionsV1> {
        self.required
    }

    pub const fn maximum(self) -> Option<FrontendWorkgroupDimensionsV1> {
        self.maximum
    }

    pub const fn min_workgroups_per_compute_unit(self) -> Option<u16> {
        self.min_workgroups_per_compute_unit
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum FrontendUnsafeAssemblyTargetV1 {
    AmdGpuGfx942 = 1,
}

impl FrontendUnsafeAssemblyTargetV1 {
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::AmdGpuGfx942 => "gfx942",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrontendUnsafeAssemblyDeclarationV1 {
    target: FrontendUnsafeAssemblyTargetV1,
    operand_bits: u16,
    option_bits: u16,
    effect_bits: u16,
}

impl FrontendUnsafeAssemblyDeclarationV1 {
    pub fn new(
        target: FrontendUnsafeAssemblyTargetV1,
        operand_bits: u16,
        option_bits: u16,
        effect_bits: u16,
    ) -> Result<Self, KernelFrontendContractValidationErrorV1> {
        if operand_bits & !ASSEMBLY_OPERANDS_V1 != 0 {
            return Err(
                KernelFrontendContractValidationErrorV1::UnsupportedAssemblyOperands(operand_bits),
            );
        }
        if operand_bits == 0 {
            return Err(KernelFrontendContractValidationErrorV1::EmptyAssemblyOperands);
        }
        if option_bits & !ASSEMBLY_OPTIONS_V1 != 0 {
            return Err(
                KernelFrontendContractValidationErrorV1::UnsupportedAssemblyOptions(option_bits),
            );
        }
        if effect_bits & !ASSEMBLY_EFFECTS_V1 != 0 {
            return Err(
                KernelFrontendContractValidationErrorV1::UnsupportedAssemblyEffects(effect_bits),
            );
        }
        if option_bits & ASSEMBLY_OPTION_NOMEM_V1 != 0
            && option_bits & ASSEMBLY_OPTION_READONLY_V1 != 0
            || option_bits & ASSEMBLY_OPTION_PURE_V1 != 0
                && option_bits & (ASSEMBLY_OPTION_NOMEM_V1 | ASSEMBLY_OPTION_READONLY_V1) == 0
        {
            return Err(KernelFrontendContractValidationErrorV1::ConflictingAssemblyOptions);
        }

        let memory = ASSEMBLY_EFFECT_READ_GLOBAL_V1
            | ASSEMBLY_EFFECT_WRITE_GLOBAL_V1
            | ASSEMBLY_EFFECT_READ_WORKGROUP_V1
            | ASSEMBLY_EFFECT_WRITE_WORKGROUP_V1
            | ASSEMBLY_EFFECT_ATOMIC_V1
            | ASSEMBLY_EFFECT_BARRIER_V1;
        let writes = ASSEMBLY_EFFECT_WRITE_GLOBAL_V1
            | ASSEMBLY_EFFECT_WRITE_WORKGROUP_V1
            | ASSEMBLY_EFFECT_ATOMIC_V1;
        if option_bits & ASSEMBLY_OPTION_NOMEM_V1 != 0 && effect_bits & memory != 0
            || option_bits & ASSEMBLY_OPTION_READONLY_V1 != 0 && effect_bits & writes != 0
            || option_bits & ASSEMBLY_OPTION_PURE_V1 != 0
                && effect_bits & ASSEMBLY_EFFECT_CONTROL_FLOW_V1 != 0
            || effect_bits == 0 && option_bits & ASSEMBLY_OPTION_NOMEM_V1 == 0
        {
            return Err(
                KernelFrontendContractValidationErrorV1::AssemblyEffectsConflictWithOptions,
            );
        }
        Ok(Self {
            target,
            operand_bits,
            option_bits,
            effect_bits,
        })
    }

    pub const fn target(self) -> FrontendUnsafeAssemblyTargetV1 {
        self.target
    }

    pub const fn operand_bits(self) -> u16 {
        self.operand_bits
    }

    pub const fn option_bits(self) -> u16 {
        self.option_bits
    }

    pub const fn effect_bits(self) -> u16 {
        self.effect_bits
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelFrontendContractV1 {
    launch: Option<FrontendLaunchBoundsV1>,
    unsafe_assembly: Option<FrontendUnsafeAssemblyDeclarationV1>,
}

impl KernelFrontendContractV1 {
    pub fn new(
        launch: Option<FrontendLaunchBoundsV1>,
        unsafe_assembly: Option<FrontendUnsafeAssemblyDeclarationV1>,
    ) -> Result<Self, KernelFrontendContractValidationErrorV1> {
        if launch.is_none() && unsafe_assembly.is_none() {
            return Err(KernelFrontendContractValidationErrorV1::Empty);
        }
        Ok(Self {
            launch,
            unsafe_assembly,
        })
    }

    pub const fn launch(self) -> Option<FrontendLaunchBoundsV1> {
        self.launch
    }

    pub const fn unsafe_assembly(self) -> Option<FrontendUnsafeAssemblyDeclarationV1> {
        self.unsafe_assembly
    }
}

pub fn encode_kernel_frontend_contract_v1(contract: KernelFrontendContractV1) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(MAX_FRONTEND_KERNEL_CONTRACT_BYTES_V1);
    bytes.extend_from_slice(&FRONTEND_KERNEL_CONTRACT_MAGIC_V1);
    push_u16(&mut bytes, FRONTEND_KERNEL_CONTRACT_VERSION_V1);
    let flags = (u16::from(contract.launch.is_some()) * FLAG_LAUNCH)
        | (u16::from(contract.unsafe_assembly.is_some()) * FLAG_UNSAFE_ASSEMBLY);
    push_u16(&mut bytes, flags);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);

    if let Some(launch) = contract.launch {
        let launch_flags = (u16::from(launch.required.is_some()) * LAUNCH_FLAG_REQUIRED)
            | (u16::from(launch.maximum.is_some()) * LAUNCH_FLAG_MAXIMUM)
            | (u16::from(launch.min_workgroups_per_compute_unit.is_some()) * LAUNCH_FLAG_OCCUPANCY);
        push_u16(&mut bytes, launch_flags);
        push_u16(&mut bytes, 0);
        push_dimensions(&mut bytes, launch.required);
        push_dimensions(&mut bytes, launch.maximum);
        push_u16(
            &mut bytes,
            launch.min_workgroups_per_compute_unit.unwrap_or(0),
        );
        push_u16(&mut bytes, 0);
    }
    if let Some(assembly) = contract.unsafe_assembly {
        push_u16(&mut bytes, assembly.target as u16);
        push_u16(&mut bytes, assembly.operand_bits);
        push_u16(&mut bytes, assembly.option_bits);
        push_u16(&mut bytes, assembly.effect_bits);
        push_u32(&mut bytes, 0);
    }
    let length = u32::try_from(bytes.len()).expect("V1 kernel contract is bounded below u32");
    bytes[12..16].copy_from_slice(&length.to_le_bytes());
    bytes
}

pub fn decode_kernel_frontend_contract_v1(
    bytes: &[u8],
) -> Result<KernelFrontendContractV1, KernelFrontendContractDecodeErrorV1> {
    if bytes.len() > MAX_FRONTEND_KERNEL_CONTRACT_BYTES_V1 {
        return Err(KernelFrontendContractDecodeErrorV1::TooLarge);
    }
    let mut reader = Reader::new(bytes);
    if reader.fixed::<8>()? != FRONTEND_KERNEL_CONTRACT_MAGIC_V1 {
        return Err(KernelFrontendContractDecodeErrorV1::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != FRONTEND_KERNEL_CONTRACT_VERSION_V1 {
        return Err(KernelFrontendContractDecodeErrorV1::UnknownVersion(version));
    }
    let flags = reader.u16()?;
    if flags == 0 || flags & !(FLAG_LAUNCH | FLAG_UNSAFE_ASSEMBLY) != 0 {
        return Err(KernelFrontendContractDecodeErrorV1::UnsupportedFlags(flags));
    }
    let declared = reader.u32()?;
    if declared < HEADER_BYTES_V1 as u32 {
        return Err(KernelFrontendContractDecodeErrorV1::InvalidLength(declared));
    }
    let declared = usize::try_from(declared)
        .map_err(|_| KernelFrontendContractDecodeErrorV1::InvalidLength(declared))?;
    if declared > bytes.len() {
        return Err(KernelFrontendContractDecodeErrorV1::Truncated);
    }
    if declared < bytes.len() {
        return Err(KernelFrontendContractDecodeErrorV1::TrailingBytes);
    }
    reader.reserved_u32("kernel contract header")?;

    let launch = if flags & FLAG_LAUNCH != 0 {
        Some(decode_launch(&mut reader)?)
    } else {
        None
    };
    let unsafe_assembly = if flags & FLAG_UNSAFE_ASSEMBLY != 0 {
        Some(decode_assembly(&mut reader)?)
    } else {
        None
    };
    if !reader.finished() {
        return Err(KernelFrontendContractDecodeErrorV1::TrailingBytes);
    }
    let contract = KernelFrontendContractV1::new(launch, unsafe_assembly)?;
    if encode_kernel_frontend_contract_v1(contract) != bytes {
        return Err(KernelFrontendContractDecodeErrorV1::NonCanonical);
    }
    Ok(contract)
}

fn decode_launch(
    reader: &mut Reader<'_>,
) -> Result<FrontendLaunchBoundsV1, KernelFrontendContractDecodeErrorV1> {
    let flags = reader.u16()?;
    if flags == 0
        || flags & !(LAUNCH_FLAG_REQUIRED | LAUNCH_FLAG_MAXIMUM | LAUNCH_FLAG_OCCUPANCY) != 0
    {
        return Err(KernelFrontendContractDecodeErrorV1::UnsupportedFlags(flags));
    }
    reader.reserved_u16("launch contract")?;
    let required_raw = reader.dimensions()?;
    let maximum_raw = reader.dimensions()?;
    let occupancy_raw = reader.u16()?;
    reader.reserved_u16("launch contract")?;
    let required =
        decode_optional_dimensions(flags, LAUNCH_FLAG_REQUIRED, required_raw, "required")?;
    let maximum = decode_optional_dimensions(flags, LAUNCH_FLAG_MAXIMUM, maximum_raw, "maximum")?;
    let occupancy = if flags & LAUNCH_FLAG_OCCUPANCY != 0 {
        Some(occupancy_raw)
    } else if occupancy_raw == 0 {
        None
    } else {
        return Err(KernelFrontendContractDecodeErrorV1::NonzeroReserved(
            "absent occupancy",
        ));
    };
    FrontendLaunchBoundsV1::new(required, maximum, occupancy).map_err(Into::into)
}

fn decode_optional_dimensions(
    flags: u16,
    flag: u16,
    value: [u32; 3],
    field: &'static str,
) -> Result<Option<FrontendWorkgroupDimensionsV1>, KernelFrontendContractDecodeErrorV1> {
    if flags & flag != 0 {
        return Ok(Some(FrontendWorkgroupDimensionsV1::new(value)?));
    }
    if value != [0; 3] {
        return Err(KernelFrontendContractDecodeErrorV1::NonzeroReserved(field));
    }
    Ok(None)
}

fn decode_assembly(
    reader: &mut Reader<'_>,
) -> Result<FrontendUnsafeAssemblyDeclarationV1, KernelFrontendContractDecodeErrorV1> {
    let target = match reader.u16()? {
        1 => FrontendUnsafeAssemblyTargetV1::AmdGpuGfx942,
        tag => {
            return Err(KernelFrontendContractDecodeErrorV1::UnknownTag {
                kind: "unsafe assembly target",
                tag,
            });
        }
    };
    let operands = reader.u16()?;
    let options = reader.u16()?;
    let effects = reader.u16()?;
    reader.reserved_u32("unsafe assembly contract")?;
    FrontendUnsafeAssemblyDeclarationV1::new(target, operands, options, effects).map_err(Into::into)
}

fn push_dimensions(bytes: &mut Vec<u8>, value: Option<FrontendWorkgroupDimensionsV1>) {
    for component in value.map_or([0; 3], FrontendWorkgroupDimensionsV1::as_array) {
        push_u32(bytes, component);
    }
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], KernelFrontendContractDecodeErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(KernelFrontendContractDecodeErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(KernelFrontendContractDecodeErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], KernelFrontendContractDecodeErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| KernelFrontendContractDecodeErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, KernelFrontendContractDecodeErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, KernelFrontendContractDecodeErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn dimensions(&mut self) -> Result<[u32; 3], KernelFrontendContractDecodeErrorV1> {
        Ok([self.u32()?, self.u32()?, self.u32()?])
    }

    fn reserved_u16(
        &mut self,
        field: &'static str,
    ) -> Result<(), KernelFrontendContractDecodeErrorV1> {
        if self.u16()? != 0 {
            return Err(KernelFrontendContractDecodeErrorV1::NonzeroReserved(field));
        }
        Ok(())
    }

    fn reserved_u32(
        &mut self,
        field: &'static str,
    ) -> Result<(), KernelFrontendContractDecodeErrorV1> {
        if self.u32()? != 0 {
            return Err(KernelFrontendContractDecodeErrorV1::NonzeroReserved(field));
        }
        Ok(())
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
