use std::fmt::{self, Write};

use crate::{MirAddressSpace, MirLayout, MirPointerWidth};

const MAGIC: &[u8; 8] = b"F2MMEMOP";
const VERSION: u16 = 1;
const FLAGS: u16 = 0;

pub const MAX_MEMORY_OPERATION_WIRE_BYTES: usize = 128;

/// An operation-local variable for allocation provenance.
///
/// Equal region IDs require equal allocation provenance. Distinct IDs make no
/// equality claim, which permits a generic copy contract whose operands may or
/// may not share an allocation at runtime. IDs are normalized from zero so
/// equivalent contracts have one wire representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirProvenanceRegion(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOperationProvenance {
    Allocation(MirProvenanceRegion),
    /// A non-Rust memory location, such as a device register. Only volatile
    /// accesses may use this provenance contract.
    ExposedAddress,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirPointerOperandContract {
    pub address_space: MirAddressSpace,
    pub provenance: MirOperationProvenance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirMemoryPermission {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirMemoryAccessContract {
    pub pointer: MirPointerOperandContract,
    pub permission: MirMemoryPermission,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirPointerDistanceUnit {
    Elements,
    Bytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirPointerDistanceResult {
    Signed,
    /// The pointer operand must not precede the origin operand.
    Unsigned,
}

/// Semantic preconditions for `offset_from` and its byte/unsigned variants.
///
/// The pointer and origin operands must retain the same allocation provenance.
/// Element distance additionally requires their byte difference to be exactly
/// divisible by the non-zero pointee size. The result must fit the signed or
/// unsigned pointer-sized result selected by `result`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirPointerDistanceContract {
    pub pointee_layout: MirLayout,
    pub pointer_width: MirPointerWidth,
    pub unit: MirPointerDistanceUnit,
    pub result: MirPointerDistanceResult,
    pub pointer: MirPointerOperandContract,
    pub origin: MirPointerOperandContract,
}

/// Semantic preconditions for a volatile load or store.
///
/// The layout supplies the exact access width and alignment. Allocation
/// provenance requires a live, dereferenceable allocation; exposed-address
/// provenance requires an external, non-trapping location. Validation does not
/// grant either fact: they remain obligations of the unsafe source operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirVolatileAccessContract {
    pub pointee_layout: MirLayout,
    pub pointer_width: MirPointerWidth,
    pub access: MirMemoryAccessContract,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirElementCount {
    Constant(u64),
    /// The runtime count carries the precondition that `count * size` fits the
    /// signed pointer-offset range. Lowering must not replace it with byte count.
    Runtime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOverlapContract {
    NonOverlapping,
    /// Representable so malformed imported semantics fail during validation.
    MayOverlap,
}

/// Semantic preconditions for `copy_nonoverlapping`.
///
/// Source and destination retain separate address spaces and allocation
/// provenance. Equal provenance regions require one allocation; distinct
/// provenance variables leave that relationship unconstrained. In either case,
/// the ranges must be disjoint. The destination range must be writable, the
/// source range readable, and the byte extent is always
/// `element_count * element_layout.size`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirCopyNonOverlappingContract {
    pub element_layout: MirLayout,
    pub pointer_width: MirPointerWidth,
    pub element_count: MirElementCount,
    pub source: MirMemoryAccessContract,
    pub destination: MirMemoryAccessContract,
    pub overlap: MirOverlapContract,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirSemanticMemoryOperation {
    PointerDistance(MirPointerDistanceContract),
    VolatileLoad(MirVolatileAccessContract),
    VolatileStore(MirVolatileAccessContract),
    CopyNonOverlapping(MirCopyNonOverlappingContract),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMemoryContractValidationError {
    path: String,
    reason: String,
}

impl MirMemoryContractValidationError {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    fn new(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for MirMemoryContractValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for MirMemoryContractValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirMemoryContractDecodeError {
    InputTooLarge,
    UnexpectedEnd,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    UnknownTag { field: &'static str, tag: u8 },
    TrailingBytes,
    NonCanonical,
    Validation(MirMemoryContractValidationError),
}

impl fmt::Display for MirMemoryContractDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge => {
                formatter.write_str("memory operation wire input exceeds its bound")
            }
            Self::UnexpectedEnd => {
                formatter.write_str("memory operation wire input ended unexpectedly")
            }
            Self::InvalidMagic => formatter.write_str("invalid memory operation wire magic"),
            Self::UnknownVersion(version) => {
                write!(formatter, "unknown memory operation version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported memory operation flags {flags:#06x}")
            }
            Self::UnknownTag { field, tag } => write!(formatter, "unknown {field} tag {tag}"),
            Self::TrailingBytes => formatter.write_str("trailing memory operation wire bytes"),
            Self::NonCanonical => {
                formatter.write_str("memory operation wire input is not canonical")
            }
            Self::Validation(error) => write!(formatter, "invalid memory operation: {error}"),
        }
    }
}

impl std::error::Error for MirMemoryContractDecodeError {}

impl From<MirMemoryContractValidationError> for MirMemoryContractDecodeError {
    fn from(value: MirMemoryContractValidationError) -> Self {
        Self::Validation(value)
    }
}

impl MirSemanticMemoryOperation {
    pub fn validate(&self) -> Result<(), MirMemoryContractValidationError> {
        match self {
            Self::PointerDistance(contract) => validate_pointer_distance(contract),
            Self::VolatileLoad(contract) => {
                validate_volatile(contract, MirMemoryPermission::Read, "volatile_load")
            }
            Self::VolatileStore(contract) => {
                validate_volatile(contract, MirMemoryPermission::Write, "volatile_store")
            }
            Self::CopyNonOverlapping(contract) => validate_copy(contract),
        }
    }

    pub fn canonical_text(&self) -> Result<String, MirMemoryContractValidationError> {
        self.validate()?;
        let mut output = String::from("mir.memory.v1:");
        match self {
            Self::PointerDistance(contract) => write_pointer_distance(&mut output, contract),
            Self::VolatileLoad(contract) => write_volatile(&mut output, "volatile-load", contract),
            Self::VolatileStore(contract) => {
                write_volatile(&mut output, "volatile-store", contract)
            }
            Self::CopyNonOverlapping(contract) => write_copy(&mut output, contract),
        }
        Ok(output)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, MirMemoryContractValidationError> {
        self.validate()?;
        let mut writer = Writer::new();
        writer.bytes.extend_from_slice(MAGIC);
        writer.u16(VERSION);
        writer.u16(FLAGS);
        match self {
            Self::PointerDistance(contract) => {
                writer.u8(1);
                encode_layout(&mut writer, contract.pointee_layout);
                writer.u8(contract.pointer_width.0);
                writer.u8(match contract.unit {
                    MirPointerDistanceUnit::Elements => 1,
                    MirPointerDistanceUnit::Bytes => 2,
                });
                writer.u8(match contract.result {
                    MirPointerDistanceResult::Signed => 1,
                    MirPointerDistanceResult::Unsigned => 2,
                });
                encode_pointer(&mut writer, contract.pointer);
                encode_pointer(&mut writer, contract.origin);
            }
            Self::VolatileLoad(contract) | Self::VolatileStore(contract) => {
                writer.u8(if matches!(self, Self::VolatileLoad(_)) {
                    2
                } else {
                    3
                });
                encode_layout(&mut writer, contract.pointee_layout);
                writer.u8(contract.pointer_width.0);
                encode_access(&mut writer, contract.access);
            }
            Self::CopyNonOverlapping(contract) => {
                writer.u8(4);
                encode_layout(&mut writer, contract.element_layout);
                writer.u8(contract.pointer_width.0);
                match contract.element_count {
                    MirElementCount::Constant(count) => {
                        writer.u8(1);
                        writer.u64(count);
                    }
                    MirElementCount::Runtime => writer.u8(2),
                }
                encode_access(&mut writer, contract.source);
                encode_access(&mut writer, contract.destination);
                writer.u8(match contract.overlap {
                    MirOverlapContract::NonOverlapping => 1,
                    MirOverlapContract::MayOverlap => 2,
                });
            }
        }
        debug_assert!(writer.bytes.len() <= MAX_MEMORY_OPERATION_WIRE_BYTES);
        Ok(writer.bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MirMemoryContractDecodeError> {
        if bytes.len() > MAX_MEMORY_OPERATION_WIRE_BYTES {
            return Err(MirMemoryContractDecodeError::InputTooLarge);
        }
        let mut reader = Reader::new(bytes);
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(MirMemoryContractDecodeError::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != VERSION {
            return Err(MirMemoryContractDecodeError::UnknownVersion(version));
        }
        let flags = reader.u16()?;
        if flags != FLAGS {
            return Err(MirMemoryContractDecodeError::UnsupportedFlags(flags));
        }
        let operation = match reader.u8()? {
            1 => Self::PointerDistance(MirPointerDistanceContract {
                pointee_layout: decode_layout(&mut reader)?,
                pointer_width: MirPointerWidth(reader.u8()?),
                unit: match reader.u8()? {
                    1 => MirPointerDistanceUnit::Elements,
                    2 => MirPointerDistanceUnit::Bytes,
                    tag => {
                        return Err(MirMemoryContractDecodeError::UnknownTag {
                            field: "pointer distance unit",
                            tag,
                        });
                    }
                },
                result: match reader.u8()? {
                    1 => MirPointerDistanceResult::Signed,
                    2 => MirPointerDistanceResult::Unsigned,
                    tag => {
                        return Err(MirMemoryContractDecodeError::UnknownTag {
                            field: "pointer distance result",
                            tag,
                        });
                    }
                },
                pointer: decode_pointer(&mut reader)?,
                origin: decode_pointer(&mut reader)?,
            }),
            tag @ (2 | 3) => {
                let contract = MirVolatileAccessContract {
                    pointee_layout: decode_layout(&mut reader)?,
                    pointer_width: MirPointerWidth(reader.u8()?),
                    access: decode_access(&mut reader)?,
                };
                if tag == 2 {
                    Self::VolatileLoad(contract)
                } else {
                    Self::VolatileStore(contract)
                }
            }
            4 => Self::CopyNonOverlapping(MirCopyNonOverlappingContract {
                element_layout: decode_layout(&mut reader)?,
                pointer_width: MirPointerWidth(reader.u8()?),
                element_count: match reader.u8()? {
                    1 => MirElementCount::Constant(reader.u64()?),
                    2 => MirElementCount::Runtime,
                    tag => {
                        return Err(MirMemoryContractDecodeError::UnknownTag {
                            field: "element count",
                            tag,
                        });
                    }
                },
                source: decode_access(&mut reader)?,
                destination: decode_access(&mut reader)?,
                overlap: match reader.u8()? {
                    1 => MirOverlapContract::NonOverlapping,
                    2 => MirOverlapContract::MayOverlap,
                    tag => {
                        return Err(MirMemoryContractDecodeError::UnknownTag {
                            field: "overlap contract",
                            tag,
                        });
                    }
                },
            }),
            tag => {
                return Err(MirMemoryContractDecodeError::UnknownTag {
                    field: "memory operation",
                    tag,
                });
            }
        };
        if !reader.is_empty() {
            return Err(MirMemoryContractDecodeError::TrailingBytes);
        }
        operation.validate()?;
        if operation.to_bytes()? != bytes {
            return Err(MirMemoryContractDecodeError::NonCanonical);
        }
        Ok(operation)
    }
}

impl MirCopyNonOverlappingContract {
    /// Returns the exact byte extent for a constant element count.
    pub fn constant_byte_count(&self) -> Result<Option<u128>, MirMemoryContractValidationError> {
        validate_copy(self)?;
        let Some(size) = self.element_layout.size else {
            unreachable!("validated copy layouts are sized")
        };
        Ok(match self.element_count {
            MirElementCount::Constant(count) => Some(u128::from(count) * u128::from(size)),
            MirElementCount::Runtime => None,
        })
    }

    /// Maximum runtime element count allowed by the signed pointer-offset range.
    pub fn maximum_element_count(&self) -> Result<u64, MirMemoryContractValidationError> {
        validate_copy(self)?;
        let Some(size) = self.element_layout.size else {
            unreachable!("validated copy layouts are sized")
        };
        if size == 0 {
            return Ok(u64::MAX);
        }
        let maximum = signed_pointer_max(self.pointer_width)? / u128::from(size);
        Ok(u64::try_from(maximum).unwrap_or(u64::MAX))
    }
}

fn validate_pointer_distance(
    contract: &MirPointerDistanceContract,
) -> Result<(), MirMemoryContractValidationError> {
    validate_layout(
        contract.pointee_layout,
        contract.pointer_width,
        "pointer_distance.pointee_layout",
    )?;
    if contract.unit == MirPointerDistanceUnit::Elements && contract.pointee_layout.size == Some(0)
    {
        return Err(MirMemoryContractValidationError::new(
            "pointer_distance.pointee_layout.size",
            "element distance requires a non-zero-sized pointee",
        ));
    }
    if contract.pointer.address_space != contract.origin.address_space {
        return Err(MirMemoryContractValidationError::new(
            "pointer_distance.origin.address_space",
            "pointer distance operands must use the same address space",
        ));
    }
    require_allocation_region(
        contract.pointer,
        MirProvenanceRegion(0),
        "pointer_distance.pointer.provenance",
    )?;
    require_allocation_region(
        contract.origin,
        MirProvenanceRegion(0),
        "pointer_distance.origin.provenance",
    )
}

fn validate_volatile(
    contract: &MirVolatileAccessContract,
    permission: MirMemoryPermission,
    path: &str,
) -> Result<(), MirMemoryContractValidationError> {
    validate_layout(
        contract.pointee_layout,
        contract.pointer_width,
        &format!("{path}.pointee_layout"),
    )?;
    if contract.access.permission != permission {
        return Err(MirMemoryContractValidationError::new(
            format!("{path}.access.permission"),
            match permission {
                MirMemoryPermission::Read => "volatile load requires read permission",
                MirMemoryPermission::Write => "volatile store requires write permission",
            },
        ));
    }
    if let MirOperationProvenance::Allocation(region) = contract.access.pointer.provenance
        && region != MirProvenanceRegion(0)
    {
        return Err(MirMemoryContractValidationError::new(
            format!("{path}.access.pointer.provenance"),
            "a single allocation provenance region must be numbered zero",
        ));
    }
    Ok(())
}

fn validate_copy(
    contract: &MirCopyNonOverlappingContract,
) -> Result<(), MirMemoryContractValidationError> {
    validate_layout(
        contract.element_layout,
        contract.pointer_width,
        "copy_nonoverlapping.element_layout",
    )?;
    if contract.source.permission != MirMemoryPermission::Read {
        return Err(MirMemoryContractValidationError::new(
            "copy_nonoverlapping.source.permission",
            "copy source requires read permission",
        ));
    }
    if contract.destination.permission != MirMemoryPermission::Write {
        return Err(MirMemoryContractValidationError::new(
            "copy_nonoverlapping.destination.permission",
            "copy destination requires write permission",
        ));
    }
    require_allocation_region(
        contract.source.pointer,
        MirProvenanceRegion(0),
        "copy_nonoverlapping.source.pointer.provenance",
    )?;
    match contract.destination.pointer.provenance {
        MirOperationProvenance::Allocation(MirProvenanceRegion(0 | 1)) => {}
        MirOperationProvenance::Allocation(_) => {
            return Err(MirMemoryContractValidationError::new(
                "copy_nonoverlapping.destination.pointer.provenance",
                "destination allocation region must be zero or one",
            ));
        }
        MirOperationProvenance::ExposedAddress => {
            return Err(MirMemoryContractValidationError::new(
                "copy_nonoverlapping.destination.pointer.provenance",
                "non-volatile copy requires allocation provenance",
            ));
        }
    }
    if contract.overlap != MirOverlapContract::NonOverlapping {
        return Err(MirMemoryContractValidationError::new(
            "copy_nonoverlapping.overlap",
            "copy_nonoverlapping requires disjoint source and destination byte ranges",
        ));
    }
    if let MirElementCount::Constant(count) = contract.element_count {
        let size = contract
            .element_layout
            .size
            .expect("validated copy layout is sized");
        let bytes = u128::from(count) * u128::from(size);
        if bytes > signed_pointer_max(contract.pointer_width)? {
            return Err(MirMemoryContractValidationError::new(
                "copy_nonoverlapping.element_count",
                "element count times layout size exceeds the signed pointer-offset range",
            ));
        }
    }
    Ok(())
}

fn validate_layout(
    layout: MirLayout,
    pointer_width: MirPointerWidth,
    path: &str,
) -> Result<(), MirMemoryContractValidationError> {
    let maximum = signed_pointer_max(pointer_width)?;
    if layout.align == 0 || !layout.align.is_power_of_two() {
        return Err(MirMemoryContractValidationError::new(
            format!("{path}.align"),
            "alignment must be a nonzero power of two",
        ));
    }
    let Some(size) = layout.size else {
        return Err(MirMemoryContractValidationError::new(
            format!("{path}.size"),
            "memory operation layout must be sized",
        ));
    };
    if !size.is_multiple_of(layout.align) {
        return Err(MirMemoryContractValidationError::new(
            format!("{path}.size"),
            "layout size must be rounded up to its alignment",
        ));
    }
    if u128::from(size) > maximum {
        return Err(MirMemoryContractValidationError::new(
            format!("{path}.size"),
            "layout size exceeds the signed pointer-offset range",
        ));
    }
    Ok(())
}

fn signed_pointer_max(width: MirPointerWidth) -> Result<u128, MirMemoryContractValidationError> {
    match width.0 {
        4 => Ok(u128::from(i32::MAX as u32)),
        8 => Ok(u128::from(i64::MAX as u64)),
        16 => Ok(i128::MAX as u128),
        _ => Err(MirMemoryContractValidationError::new(
            "pointer_width",
            "pointer width must be 4, 8, or 16 bytes",
        )),
    }
}

fn require_allocation_region(
    pointer: MirPointerOperandContract,
    expected: MirProvenanceRegion,
    path: &str,
) -> Result<(), MirMemoryContractValidationError> {
    match pointer.provenance {
        MirOperationProvenance::Allocation(region) if region == expected => Ok(()),
        MirOperationProvenance::Allocation(_) => Err(MirMemoryContractValidationError::new(
            path,
            format!("allocation provenance region must be {}", expected.0),
        )),
        MirOperationProvenance::ExposedAddress => Err(MirMemoryContractValidationError::new(
            path,
            "operation requires allocation provenance",
        )),
    }
}

fn write_pointer_distance(output: &mut String, contract: &MirPointerDistanceContract) {
    output.push_str("pointer-distance(");
    write_layout(output, contract.pointee_layout);
    write!(
        output,
        ";pointer-width={};unit={};result={};pointer=",
        contract.pointer_width.0,
        match contract.unit {
            MirPointerDistanceUnit::Elements => "elements",
            MirPointerDistanceUnit::Bytes => "bytes",
        },
        match contract.result {
            MirPointerDistanceResult::Signed => "signed",
            MirPointerDistanceResult::Unsigned => "unsigned",
        }
    )
    .expect("writing to a String cannot fail");
    write_pointer(output, contract.pointer);
    output.push_str(";origin=");
    write_pointer(output, contract.origin);
    output.push(')');
}

fn write_volatile(output: &mut String, name: &str, contract: &MirVolatileAccessContract) {
    write!(output, "{name}(").expect("writing to a String cannot fail");
    write_layout(output, contract.pointee_layout);
    write!(
        output,
        ";pointer-width={};access=",
        contract.pointer_width.0
    )
    .expect("writing to a String cannot fail");
    write_access(output, contract.access);
    output.push(')');
}

fn write_copy(output: &mut String, contract: &MirCopyNonOverlappingContract) {
    output.push_str("copy-nonoverlapping(");
    write_layout(output, contract.element_layout);
    write!(output, ";pointer-width={};count=", contract.pointer_width.0)
        .expect("writing to a String cannot fail");
    match contract.element_count {
        MirElementCount::Constant(count) => {
            write!(output, "constant({count})").expect("writing to a String cannot fail");
        }
        MirElementCount::Runtime => output.push_str("runtime"),
    }
    output.push_str(";source=");
    write_access(output, contract.source);
    output.push_str(";destination=");
    write_access(output, contract.destination);
    output.push_str(";overlap=nonoverlapping)");
}

fn write_layout(output: &mut String, layout: MirLayout) {
    write!(
        output,
        "layout(size={};align={})",
        layout
            .size
            .expect("validated memory operation layout is sized"),
        layout.align
    )
    .expect("writing to a String cannot fail");
}

fn write_pointer(output: &mut String, pointer: MirPointerOperandContract) {
    write!(
        output,
        "ptr(addrspace={};provenance=",
        pointer.address_space.0
    )
    .expect("writing to a String cannot fail");
    match pointer.provenance {
        MirOperationProvenance::Allocation(region) => {
            write!(output, "allocation({})", region.0).expect("writing to a String cannot fail");
        }
        MirOperationProvenance::ExposedAddress => output.push_str("exposed-address"),
    }
    output.push(')');
}

fn write_access(output: &mut String, access: MirMemoryAccessContract) {
    output.push_str("access(permission=");
    output.push_str(match access.permission {
        MirMemoryPermission::Read => "read",
        MirMemoryPermission::Write => "write",
    });
    output.push_str(";pointer=");
    write_pointer(output, access.pointer);
    output.push(')');
}

fn encode_layout(writer: &mut Writer, layout: MirLayout) {
    writer.u64(
        layout
            .size
            .expect("validated memory operation layout is sized"),
    );
    writer.u64(layout.align);
}

fn decode_layout(reader: &mut Reader<'_>) -> Result<MirLayout, MirMemoryContractDecodeError> {
    Ok(MirLayout::sized(reader.u64()?, reader.u64()?))
}

fn encode_pointer(writer: &mut Writer, pointer: MirPointerOperandContract) {
    writer.u32(pointer.address_space.0);
    match pointer.provenance {
        MirOperationProvenance::Allocation(region) => {
            writer.u8(1);
            writer.u32(region.0);
        }
        MirOperationProvenance::ExposedAddress => writer.u8(2),
    }
}

fn decode_pointer(
    reader: &mut Reader<'_>,
) -> Result<MirPointerOperandContract, MirMemoryContractDecodeError> {
    let address_space = MirAddressSpace(reader.u32()?);
    let provenance = match reader.u8()? {
        1 => MirOperationProvenance::Allocation(MirProvenanceRegion(reader.u32()?)),
        2 => MirOperationProvenance::ExposedAddress,
        tag => {
            return Err(MirMemoryContractDecodeError::UnknownTag {
                field: "operation provenance",
                tag,
            });
        }
    };
    Ok(MirPointerOperandContract {
        address_space,
        provenance,
    })
}

fn encode_access(writer: &mut Writer, access: MirMemoryAccessContract) {
    encode_pointer(writer, access.pointer);
    writer.u8(match access.permission {
        MirMemoryPermission::Read => 1,
        MirMemoryPermission::Write => 2,
    });
}

fn decode_access(
    reader: &mut Reader<'_>,
) -> Result<MirMemoryAccessContract, MirMemoryContractDecodeError> {
    let pointer = decode_pointer(reader)?;
    let permission = match reader.u8()? {
        1 => MirMemoryPermission::Read,
        2 => MirMemoryPermission::Write,
        tag => {
            return Err(MirMemoryContractDecodeError::UnknownTag {
                field: "memory permission",
                tag,
            });
        }
    };
    Ok(MirMemoryAccessContract {
        pointer,
        permission,
    })
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(MAX_MEMORY_OPERATION_WIRE_BYTES),
        }
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

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], MirMemoryContractDecodeError> {
        if self.remaining.len() < count {
            return Err(MirMemoryContractDecodeError::UnexpectedEnd);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, MirMemoryContractDecodeError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, MirMemoryContractDecodeError> {
        let bytes: [u8; 2] = self
            .take(2)?
            .try_into()
            .expect("reader returned the requested length");
        Ok(u16::from_le_bytes(bytes))
    }

    fn u32(&mut self) -> Result<u32, MirMemoryContractDecodeError> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .expect("reader returned the requested length");
        Ok(u32::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, MirMemoryContractDecodeError> {
        let bytes: [u8; 8] = self
            .take(8)?
            .try_into()
            .expect("reader returned the requested length");
        Ok(u64::from_le_bytes(bytes))
    }

    fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }
}
