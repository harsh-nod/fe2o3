use std::collections::BTreeSet;
use std::fmt;
use std::str;

use crate::{
    AccessMode, AddressSpace, AssemblyConstraint, AssemblyEffect, AssemblyOperand,
    AssemblyOperandKind, AssemblyOption, AssemblySourceIdentity, Atomic, AtomicKind, Axis, Barrier,
    BarrierSemantics, BasicBlock, BinaryOp, BlockId, CastKind, ComparePredicate, Constant,
    Convergence, Fence, Function, FunctionBody, FunctionId, FunctionRole, IndexKind,
    InlineAssembly, InlineAssemblyTarget, IntegerSwitchCase, IntrinsicKind, IntrinsicOperation,
    Kernel, KernelId, LaunchDomain, LaunchExtent, MemoryAccess, MemoryOrdering, Module, ModuleId,
    Operation, OperationKind, PointerType, ScalarType, Signature, SliceType, SwitchCase,
    SynchronizationScope, TargetCapability, Terminator, Type, UnaryOp, ValueDef, ValueId,
    WaveOperation, WaveOperationKind, WaveWidth, WorkgroupBarrier, WorkgroupMemory,
    WorkgroupMemoryExtent, WorkgroupSize,
};

/// Fixed magic at the start of every canonical kernel IR module.
pub const KERNEL_IR_MAGIC_V1: [u8; 8] = *b"FE2O3KI\0";
/// The original frozen kernel IR wire version.
pub const KERNEL_IR_VERSION_V1: u16 = 1;
/// Additive synchronization, LDS, and wave-capability wire version.
pub const KERNEL_IR_VERSION_V2: u16 = 2;
/// Additive source-bound inline-assembly wire version.
pub const KERNEL_IR_VERSION_V3: u16 = 3;
/// Maximum size of one encoded kernel IR module.
pub const MAX_MODULE_BYTES_V1: usize = 16 * 1024 * 1024;
/// Maximum UTF-8 byte length of any identifier or extension component.
pub const MAX_TEXT_BYTES_V1: usize = 4096;
/// Maximum functions in one module.
pub const MAX_FUNCTIONS_V1: usize = 16 * 1024;
/// Maximum kernels in one module.
pub const MAX_KERNELS_V1: usize = 16 * 1024;
/// Maximum capabilities in one capability set.
pub const MAX_CAPABILITIES_V1: usize = 1024;
/// Maximum parameters or results in one signature.
pub const MAX_SIGNATURE_TYPES_V1: usize = 65_536;
/// Maximum SSA parameter identities in one function body.
pub const MAX_FUNCTION_PARAMETERS_V1: usize = 65_536;
/// Maximum basic blocks in one function body.
pub const MAX_BLOCKS_V1: usize = 65_536;
/// Maximum block parameters in one basic block.
pub const MAX_BLOCK_PARAMETERS_V1: usize = 65_536;
/// Maximum operations in one basic block.
pub const MAX_OPERATIONS_V1: usize = 65_536;
/// Maximum SSA results produced by one operation.
pub const MAX_OPERATION_RESULTS_V1: usize = 65_536;
/// Maximum value arguments in any argument list.
pub const MAX_VALUE_ARGUMENTS_V1: usize = 65_536;
/// Maximum operands in one V3 inline-assembly statement.
pub const MAX_ASSEMBLY_OPERANDS_V3: usize = 256;
/// Maximum cases in one switch terminator.
pub const MAX_SWITCH_CASES_V1: usize = 65_536;
/// Maximum cases in one typed V2 integer switch terminator.
pub const MAX_INTEGER_SWITCH_CASES_V2: usize = 65_536;
/// Maximum nested pointer/slice type depth.
pub const MAX_TYPE_DEPTH_V1: usize = 64;

const HEADER_BYTES: usize = 20;
const MAX_ADDRESS_SPACES: usize = 5;

/// A bounded canonical-encoding failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelIrEncodeError {
    TooLarge {
        max: usize,
    },
    LimitExceeded {
        field: &'static str,
        actual: usize,
        max: usize,
    },
    TypeNestingTooDeep {
        max: usize,
    },
    Overflow {
        field: &'static str,
    },
    UnsupportedInVersion {
        version: u16,
        feature: &'static str,
    },
    NonCanonical {
        field: &'static str,
    },
}

impl fmt::Display for KernelIrEncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(formatter, "kernel IR module exceeds {max} bytes"),
            Self::LimitExceeded { field, actual, max } => {
                write!(
                    formatter,
                    "{field} has {actual} items or bytes; maximum is {max}"
                )
            }
            Self::TypeNestingTooDeep { max } => {
                write!(formatter, "kernel IR type nesting exceeds {max}")
            }
            Self::Overflow { field } => write!(formatter, "{field} does not fit its wire field"),
            Self::UnsupportedInVersion { version, feature } => {
                write!(
                    formatter,
                    "{feature} is not representable in kernel IR V{version}"
                )
            }
            Self::NonCanonical { field } => {
                write!(formatter, "{field} is not in canonical order")
            }
        }
    }
}

impl std::error::Error for KernelIrEncodeError {}

/// A bounded kernel IR decoding failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KernelIrDecodeError {
    TooLarge {
        max: usize,
    },
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    InvalidLength {
        declared: u32,
    },
    Truncated,
    TrailingBytes,
    ReservedNonZero {
        field: &'static str,
    },
    UnknownTag {
        kind: &'static str,
        tag: u8,
    },
    InvalidUtf8 {
        field: &'static str,
    },
    LimitExceeded {
        field: &'static str,
        actual: usize,
        max: usize,
    },
    TypeNestingTooDeep {
        max: usize,
    },
    NonCanonical,
    Encode(KernelIrEncodeError),
}

impl fmt::Display for KernelIrDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(formatter, "kernel IR module exceeds {max} bytes"),
            Self::InvalidMagic => formatter.write_str("invalid kernel IR magic"),
            Self::UnknownVersion(version) => {
                write!(formatter, "unknown kernel IR version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported kernel IR flags {flags:#x}")
            }
            Self::InvalidLength { declared } => {
                write!(formatter, "invalid declared kernel IR length {declared}")
            }
            Self::Truncated => formatter.write_str("truncated kernel IR module"),
            Self::TrailingBytes => formatter.write_str("trailing kernel IR bytes"),
            Self::ReservedNonZero { field } => write!(formatter, "nonzero reserved {field}"),
            Self::UnknownTag { kind, tag } => write!(formatter, "unknown {kind} tag {tag}"),
            Self::InvalidUtf8 { field } => write!(formatter, "invalid UTF-8 in {field}"),
            Self::LimitExceeded { field, actual, max } => {
                write!(
                    formatter,
                    "{field} has {actual} items or bytes; maximum is {max}"
                )
            }
            Self::TypeNestingTooDeep { max } => {
                write!(formatter, "kernel IR type nesting exceeds {max}")
            }
            Self::NonCanonical => formatter.write_str("noncanonical kernel IR encoding"),
            Self::Encode(error) => write!(formatter, "decoded kernel IR cannot re-encode: {error}"),
        }
    }
}

impl std::error::Error for KernelIrDecodeError {}

impl From<KernelIrEncodeError> for KernelIrDecodeError {
    fn from(error: KernelIrEncodeError) -> Self {
        Self::Encode(error)
    }
}

/// Encodes a module in the bounded canonical kernel IR V1 wire format.
///
/// Vector order is preserved because block and operation order is semantic.
/// Sets are emitted in their `BTreeSet` order. This function enforces wire
/// resource bounds but does not call [`crate::verify_module`].
pub fn encode_module_v1(module: &Module) -> Result<Vec<u8>, KernelIrEncodeError> {
    encode_module(module, KERNEL_IR_VERSION_V1)
}

/// Encodes a module in the bounded canonical kernel IR V2 wire format.
pub fn encode_module_v2(module: &Module) -> Result<Vec<u8>, KernelIrEncodeError> {
    encode_module(module, KERNEL_IR_VERSION_V2)
}

/// Encodes a module in the bounded canonical kernel IR V3 wire format.
pub fn encode_module_v3(module: &Module) -> Result<Vec<u8>, KernelIrEncodeError> {
    encode_module(module, KERNEL_IR_VERSION_V3)
}

fn encode_module(module: &Module, version: u16) -> Result<Vec<u8>, KernelIrEncodeError> {
    let mut writer = Writer::new(version);
    writer.bytes(&KERNEL_IR_MAGIC_V1)?;
    writer.u16(version)?;
    writer.u16(0)?;
    writer.u32(0)?;
    writer.u32(0)?;

    writer.text("module ID", module.id.as_str())?;
    writer.count("module functions", module.functions.len(), MAX_FUNCTIONS_V1)?;
    writer.count("module kernels", module.kernels.len(), MAX_KERNELS_V1)?;
    validate_legacy_function_roles(module, version)?;
    encode_capabilities(&mut writer, &module.required_capabilities)?;
    for function in &module.functions {
        encode_function(&mut writer, function)?;
    }
    for kernel in &module.kernels {
        encode_kernel(&mut writer, kernel)?;
    }

    let mut bytes = writer.finish();
    let length = u32::try_from(bytes.len()).map_err(|_| KernelIrEncodeError::Overflow {
        field: "module length",
    })?;
    bytes[12..16].copy_from_slice(&length.to_le_bytes());
    Ok(bytes)
}

/// Decodes one bounded canonical kernel IR V1 module.
///
/// All lengths and counts are checked before allocation, set order must be
/// canonical, and successful decoding includes an exact re-encoding check.
/// Decoding only establishes wire well-formedness; callers must separately
/// invoke [`crate::verify_module`] before trusting semantic IR invariants.
pub fn decode_module_v1(bytes: &[u8]) -> Result<Module, KernelIrDecodeError> {
    decode_module(bytes, KERNEL_IR_VERSION_V1, false)
}

/// Decodes canonical V1 or V2 bytes using the latest bounded reader.
///
/// Accepting V1 here is intentional: consumers can migrate to the V2 reader
/// before producers begin emitting the additive V2 operation set.
pub fn decode_module_v2(bytes: &[u8]) -> Result<Module, KernelIrDecodeError> {
    decode_module(bytes, KERNEL_IR_VERSION_V2, true)
}

/// Decodes canonical V1, V2, or V3 bytes using the latest bounded reader.
pub fn decode_module_v3(bytes: &[u8]) -> Result<Module, KernelIrDecodeError> {
    decode_module(bytes, KERNEL_IR_VERSION_V3, true)
}

fn decode_module(
    bytes: &[u8],
    maximum_version: u16,
    accept_older: bool,
) -> Result<Module, KernelIrDecodeError> {
    if bytes.len() > MAX_MODULE_BYTES_V1 {
        return Err(KernelIrDecodeError::TooLarge {
            max: MAX_MODULE_BYTES_V1,
        });
    }
    let mut reader = Reader::new(bytes);
    if reader.fixed::<8>()? != KERNEL_IR_MAGIC_V1 {
        return Err(KernelIrDecodeError::InvalidMagic);
    }
    let version = reader.u16()?;
    if version > maximum_version || (!accept_older && version != maximum_version) || version == 0 {
        return Err(KernelIrDecodeError::UnknownVersion(version));
    }
    reader.version = version;
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(KernelIrDecodeError::UnsupportedFlags(flags));
    }
    let declared = reader.u32()?;
    if declared < HEADER_BYTES as u32 {
        return Err(KernelIrDecodeError::InvalidLength { declared });
    }
    let declared =
        usize::try_from(declared).map_err(|_| KernelIrDecodeError::InvalidLength { declared })?;
    if declared > bytes.len() {
        return Err(KernelIrDecodeError::Truncated);
    }
    if declared < bytes.len() {
        return Err(KernelIrDecodeError::TrailingBytes);
    }
    reader.reserved_u32("module header")?;

    let id = ModuleId::new(reader.text("module ID")?);
    let function_count = reader.count("module functions", MAX_FUNCTIONS_V1)?;
    let kernel_count = reader.count("module kernels", MAX_KERNELS_V1)?;
    let required_capabilities = decode_capabilities(&mut reader)?;
    let mut functions = Vec::with_capacity(function_count);
    for _ in 0..function_count {
        functions.push(decode_function(&mut reader)?);
    }
    let mut kernels = Vec::with_capacity(kernel_count);
    for _ in 0..kernel_count {
        kernels.push(decode_kernel(&mut reader)?);
    }
    if !reader.is_finished() {
        return Err(KernelIrDecodeError::TrailingBytes);
    }
    let mut module = Module {
        id,
        functions,
        kernels,
        required_capabilities,
    };
    restore_legacy_function_roles(&mut module);
    if encode_module(&module, version)? != bytes {
        return Err(KernelIrDecodeError::NonCanonical);
    }
    Ok(module)
}

fn encode_function(writer: &mut Writer, function: &Function) -> Result<(), KernelIrEncodeError> {
    writer.text("function ID", function.id.as_str())?;
    encode_signature(writer, &function.signature)?;
    match &function.body {
        None => writer.u8(0)?,
        Some(body) => {
            writer.u8(1)?;
            encode_function_body(writer, body)?;
        }
    }
    encode_capabilities(writer, &function.required_capabilities)
}

fn decode_function(reader: &mut Reader<'_>) -> Result<Function, KernelIrDecodeError> {
    let id = FunctionId::new(reader.text("function ID")?);
    let signature = decode_signature(reader)?;
    let body = if reader.option("function body")? {
        Some(decode_function_body(reader)?)
    } else {
        None
    };
    let required_capabilities = decode_capabilities(reader)?;
    Ok(Function {
        id,
        signature,
        role: if body.is_some() {
            FunctionRole::InternalHelper
        } else {
            FunctionRole::ExternalImport
        },
        body,
        required_capabilities,
    })
}

fn validate_legacy_function_roles(
    module: &Module,
    version: u16,
) -> Result<(), KernelIrEncodeError> {
    let entries = module
        .kernels
        .iter()
        .map(|kernel| kernel.entry.clone())
        .collect::<BTreeSet<_>>();
    for function in &module.functions {
        let representable = match function.role {
            FunctionRole::KernelEntry => function.body.is_some() && entries.contains(&function.id),
            FunctionRole::InternalHelper => {
                function.body.is_some() && !entries.contains(&function.id)
            }
            FunctionRole::ExternalImport => function.body.is_none(),
            FunctionRole::DeviceFfiExport => {
                return Err(KernelIrEncodeError::UnsupportedInVersion {
                    version,
                    feature: "device-FFI export function roles",
                });
            }
        };
        if !representable {
            return Err(KernelIrEncodeError::NonCanonical {
                field: "function role does not match the V1/V2 body and kernel records",
            });
        }
    }
    Ok(())
}

fn restore_legacy_function_roles(module: &mut Module) {
    let entries = module
        .kernels
        .iter()
        .map(|kernel| kernel.entry.clone())
        .collect::<BTreeSet<_>>();
    for function in &mut module.functions {
        function.role = if entries.contains(&function.id) {
            FunctionRole::KernelEntry
        } else if function.body.is_some() {
            FunctionRole::InternalHelper
        } else {
            FunctionRole::ExternalImport
        };
    }
}

fn encode_signature(writer: &mut Writer, signature: &Signature) -> Result<(), KernelIrEncodeError> {
    writer.count(
        "signature parameters",
        signature.parameters.len(),
        MAX_SIGNATURE_TYPES_V1,
    )?;
    for ty in &signature.parameters {
        encode_type(writer, ty, 0)?;
    }
    writer.count(
        "signature results",
        signature.results.len(),
        MAX_SIGNATURE_TYPES_V1,
    )?;
    for ty in &signature.results {
        encode_type(writer, ty, 0)?;
    }
    Ok(())
}

fn decode_signature(reader: &mut Reader<'_>) -> Result<Signature, KernelIrDecodeError> {
    let parameter_count = reader.count("signature parameters", MAX_SIGNATURE_TYPES_V1)?;
    let mut parameters = Vec::with_capacity(parameter_count);
    for _ in 0..parameter_count {
        parameters.push(decode_type(reader, 0)?);
    }
    let result_count = reader.count("signature results", MAX_SIGNATURE_TYPES_V1)?;
    let mut results = Vec::with_capacity(result_count);
    for _ in 0..result_count {
        results.push(decode_type(reader, 0)?);
    }
    Ok(Signature::new(parameters, results))
}

fn encode_function_body(
    writer: &mut Writer,
    body: &FunctionBody,
) -> Result<(), KernelIrEncodeError> {
    encode_values(
        writer,
        "function parameters",
        &body.parameters,
        MAX_FUNCTION_PARAMETERS_V1,
    )?;
    writer.count("function blocks", body.blocks.len(), MAX_BLOCKS_V1)?;
    for block in &body.blocks {
        encode_block(writer, block)?;
    }
    Ok(())
}

fn decode_function_body(reader: &mut Reader<'_>) -> Result<FunctionBody, KernelIrDecodeError> {
    let parameters = decode_values(reader, "function parameters", MAX_FUNCTION_PARAMETERS_V1)?;
    let block_count = reader.count("function blocks", MAX_BLOCKS_V1)?;
    let mut blocks = Vec::with_capacity(block_count);
    for _ in 0..block_count {
        blocks.push(decode_block(reader)?);
    }
    Ok(FunctionBody { parameters, blocks })
}

fn encode_kernel(writer: &mut Writer, kernel: &Kernel) -> Result<(), KernelIrEncodeError> {
    writer.text("kernel ID", kernel.id.as_str())?;
    writer.text("kernel entry", kernel.entry.as_str())?;
    encode_launch_domain(writer, &kernel.domain)?;
    match kernel.workgroup_size {
        None => writer.u8(0)?,
        Some(size) => {
            writer.u8(1)?;
            writer.u32(size.x)?;
            writer.u32(size.y)?;
            writer.u32(size.z)?;
        }
    }
    encode_capabilities(writer, &kernel.required_capabilities)
}

fn decode_kernel(reader: &mut Reader<'_>) -> Result<Kernel, KernelIrDecodeError> {
    let id = KernelId::new(reader.text("kernel ID")?);
    let entry = FunctionId::new(reader.text("kernel entry")?);
    let domain = decode_launch_domain(reader)?;
    let workgroup_size = if reader.option("workgroup size")? {
        Some(WorkgroupSize::new(
            reader.u32()?,
            reader.u32()?,
            reader.u32()?,
        ))
    } else {
        None
    };
    let required_capabilities = decode_capabilities(reader)?;
    Ok(Kernel {
        id,
        entry,
        domain,
        workgroup_size,
        required_capabilities,
    })
}

fn encode_block(writer: &mut Writer, block: &BasicBlock) -> Result<(), KernelIrEncodeError> {
    writer.u32(block.id.0)?;
    writer.count(
        "block parameters",
        block.parameters.len(),
        MAX_BLOCK_PARAMETERS_V1,
    )?;
    for parameter in &block.parameters {
        encode_value_def(writer, parameter)?;
    }
    writer.count(
        "block operations",
        block.operations.len(),
        MAX_OPERATIONS_V1,
    )?;
    for operation in &block.operations {
        encode_operation(writer, operation)?;
    }
    match &block.terminator {
        None => writer.u8(0)?,
        Some(terminator) => {
            writer.u8(1)?;
            encode_terminator(writer, terminator)?;
        }
    }
    Ok(())
}

fn decode_block(reader: &mut Reader<'_>) -> Result<BasicBlock, KernelIrDecodeError> {
    let id = BlockId(reader.u32()?);
    let parameter_count = reader.count("block parameters", MAX_BLOCK_PARAMETERS_V1)?;
    let mut parameters = Vec::with_capacity(parameter_count);
    for _ in 0..parameter_count {
        parameters.push(decode_value_def(reader)?);
    }
    let operation_count = reader.count("block operations", MAX_OPERATIONS_V1)?;
    let mut operations = Vec::with_capacity(operation_count);
    for _ in 0..operation_count {
        operations.push(decode_operation(reader)?);
    }
    let terminator = if reader.option("block terminator")? {
        Some(decode_terminator(reader)?)
    } else {
        None
    };
    Ok(BasicBlock {
        id,
        parameters,
        operations,
        terminator,
    })
}

fn encode_operation(writer: &mut Writer, operation: &Operation) -> Result<(), KernelIrEncodeError> {
    writer.count(
        "operation results",
        operation.results.len(),
        MAX_OPERATION_RESULTS_V1,
    )?;
    for result in &operation.results {
        encode_value_def(writer, result)?;
    }
    encode_operation_kind(writer, &operation.kind)
}

fn decode_operation(reader: &mut Reader<'_>) -> Result<Operation, KernelIrDecodeError> {
    let result_count = reader.count("operation results", MAX_OPERATION_RESULTS_V1)?;
    let mut results = Vec::with_capacity(result_count);
    for _ in 0..result_count {
        results.push(decode_value_def(reader)?);
    }
    Ok(Operation::new(results, decode_operation_kind(reader)?))
}

fn encode_operation_kind(
    writer: &mut Writer,
    operation: &OperationKind,
) -> Result<(), KernelIrEncodeError> {
    match operation {
        OperationKind::Constant(value) => {
            writer.u8(1)?;
            encode_constant(writer, value)?;
        }
        OperationKind::Intrinsic(intrinsic) => {
            writer.u8(2)?;
            encode_intrinsic(writer, intrinsic)?;
        }
        OperationKind::MemoryIntrinsic(_) => {
            return Err(KernelIrEncodeError::UnsupportedInVersion {
                version: writer.version,
                feature: "semantic memory intrinsic",
            });
        }
        OperationKind::Unary { op, operand } => {
            writer.u8(3)?;
            writer.u8(unary_op_tag(*op))?;
            writer.u32(operand.0)?;
        }
        OperationKind::Binary { op, lhs, rhs } => {
            writer.u8(4)?;
            writer.u8(binary_op_tag(*op))?;
            writer.u32(lhs.0)?;
            writer.u32(rhs.0)?;
        }
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } => {
            writer.u8(5)?;
            writer.u8(compare_predicate_tag(*predicate))?;
            writer.u32(lhs.0)?;
            writer.u32(rhs.0)?;
        }
        OperationKind::Cast { kind, value, to } => {
            writer.u8(6)?;
            writer.u8(cast_kind_tag(*kind))?;
            writer.u32(value.0)?;
            encode_type(writer, to, 0)?;
        }
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } => {
            writer.u8(7)?;
            writer.u32(condition.0)?;
            writer.u32(true_value.0)?;
            writer.u32(false_value.0)?;
        }
        OperationKind::Call { callee, arguments } => {
            writer.u8(8)?;
            writer.text("call callee", callee.as_str())?;
            encode_values(writer, "call arguments", arguments, MAX_VALUE_ARGUMENTS_V1)?;
        }
        OperationKind::Alloca {
            element,
            count,
            address_space,
            alignment,
        } => {
            writer.u8(9)?;
            encode_type(writer, element, 0)?;
            encode_optional_value(writer, *count)?;
            writer.u8(address_space_tag(*address_space))?;
            writer.u32(*alignment)?;
        }
        OperationKind::SliceLength { slice } => {
            writer.u8(10)?;
            writer.u32(slice.0)?;
        }
        OperationKind::SliceData { slice } => {
            writer.u8(11)?;
            writer.u32(slice.0)?;
        }
        OperationKind::GetElementPointer { base, offset } => {
            writer.u8(12)?;
            writer.u32(base.0)?;
            writer.u32(offset.0)?;
        }
        OperationKind::Load { pointer, access } => {
            writer.u8(13)?;
            writer.u32(pointer.0)?;
            encode_memory_access(writer, *access)?;
        }
        OperationKind::Store {
            pointer,
            value,
            access,
        } => {
            writer.u8(14)?;
            writer.u32(pointer.0)?;
            writer.u32(value.0)?;
            encode_memory_access(writer, *access)?;
        }
        OperationKind::Barrier(barrier) => {
            writer.u8(15)?;
            encode_barrier(writer, barrier)?;
        }
        OperationKind::Atomic(atomic) => {
            writer.u8(16)?;
            encode_atomic(writer, atomic)?;
        }
        OperationKind::Fence(fence) => {
            require_v2(writer, "memory fence")?;
            writer.u8(17)?;
            encode_fence(writer, fence)?;
        }
        OperationKind::WorkgroupBarrier(barrier) => {
            require_v2(writer, "convergent workgroup barrier")?;
            writer.u8(18)?;
            encode_workgroup_barrier(writer, barrier)?;
        }
        OperationKind::WorkgroupMemory(memory) => {
            require_v2(writer, "explicit workgroup memory")?;
            writer.u8(19)?;
            encode_workgroup_memory(writer, memory)?;
        }
        OperationKind::Wave(wave) => {
            require_v2(writer, "physical wave operation")?;
            writer.u8(20)?;
            encode_wave_operation(writer, wave)?;
        }
        OperationKind::Matrix(_) => {
            return Err(KernelIrEncodeError::UnsupportedInVersion {
                version: writer.version,
                feature: "matrix operation",
            });
        }
        OperationKind::InlineAssembly(assembly) => {
            require_v3(writer, "source-bound inline assembly")?;
            writer.u8(21)?;
            encode_inline_assembly(writer, assembly)?;
        }
    }
    Ok(())
}

fn decode_operation_kind(reader: &mut Reader<'_>) -> Result<OperationKind, KernelIrDecodeError> {
    Ok(match reader.u8()? {
        1 => OperationKind::Constant(decode_constant(reader)?),
        2 => OperationKind::Intrinsic(decode_intrinsic(reader)?),
        3 => OperationKind::Unary {
            op: decode_unary_op(reader.u8()?)?,
            operand: ValueId(reader.u32()?),
        },
        4 => OperationKind::Binary {
            op: decode_binary_op(reader.u8()?)?,
            lhs: ValueId(reader.u32()?),
            rhs: ValueId(reader.u32()?),
        },
        5 => OperationKind::Compare {
            predicate: decode_compare_predicate(reader.u8()?)?,
            lhs: ValueId(reader.u32()?),
            rhs: ValueId(reader.u32()?),
        },
        6 => OperationKind::Cast {
            kind: decode_cast_kind(reader.u8()?)?,
            value: ValueId(reader.u32()?),
            to: decode_type(reader, 0)?,
        },
        7 => OperationKind::Select {
            condition: ValueId(reader.u32()?),
            true_value: ValueId(reader.u32()?),
            false_value: ValueId(reader.u32()?),
        },
        8 => OperationKind::Call {
            callee: FunctionId::new(reader.text("call callee")?),
            arguments: decode_values(reader, "call arguments", MAX_VALUE_ARGUMENTS_V1)?,
        },
        9 => OperationKind::Alloca {
            element: decode_type(reader, 0)?,
            count: decode_optional_value(reader, "alloca count")?,
            address_space: decode_address_space(reader.u8()?)?,
            alignment: reader.u32()?,
        },
        10 => OperationKind::SliceLength {
            slice: ValueId(reader.u32()?),
        },
        11 => OperationKind::SliceData {
            slice: ValueId(reader.u32()?),
        },
        12 => OperationKind::GetElementPointer {
            base: ValueId(reader.u32()?),
            offset: ValueId(reader.u32()?),
        },
        13 => OperationKind::Load {
            pointer: ValueId(reader.u32()?),
            access: decode_memory_access(reader)?,
        },
        14 => OperationKind::Store {
            pointer: ValueId(reader.u32()?),
            value: ValueId(reader.u32()?),
            access: decode_memory_access(reader)?,
        },
        15 => OperationKind::Barrier(decode_barrier(reader)?),
        16 => OperationKind::Atomic(decode_atomic(reader)?),
        17 if reader.version >= KERNEL_IR_VERSION_V2 => OperationKind::Fence(decode_fence(reader)?),
        18 if reader.version >= KERNEL_IR_VERSION_V2 => {
            OperationKind::WorkgroupBarrier(decode_workgroup_barrier(reader)?)
        }
        19 if reader.version >= KERNEL_IR_VERSION_V2 => {
            OperationKind::WorkgroupMemory(decode_workgroup_memory(reader)?)
        }
        20 if reader.version >= KERNEL_IR_VERSION_V2 => {
            OperationKind::Wave(decode_wave_operation(reader)?)
        }
        21 if reader.version >= KERNEL_IR_VERSION_V3 => {
            OperationKind::InlineAssembly(decode_inline_assembly(reader)?)
        }
        tag => {
            return Err(KernelIrDecodeError::UnknownTag {
                kind: "operation",
                tag,
            });
        }
    })
}

fn encode_inline_assembly(
    writer: &mut Writer,
    assembly: &InlineAssembly,
) -> Result<(), KernelIrEncodeError> {
    writer.u8(inline_assembly_target_tag(assembly.target))?;
    writer.bytes(&assembly.source.frontend_unit)?;
    writer.bytes(&assembly.source.function)?;
    writer.bytes(&assembly.source.contract)?;
    writer.bytes(&assembly.source.statement)?;
    writer.text("inline assembly mnemonic", &assembly.mnemonic)?;
    writer.count(
        "inline assembly operands",
        assembly.operands.len(),
        MAX_ASSEMBLY_OPERANDS_V3,
    )?;
    for operand in &assembly.operands {
        writer.u8(assembly_constraint_tag(operand.constraint))?;
        match operand.kind {
            AssemblyOperandKind::Input(value) => {
                writer.u8(1)?;
                writer.u32(value.0)?;
            }
            AssemblyOperandKind::Output { result_index } => {
                writer.u8(2)?;
                writer.u32(result_index)?;
            }
            AssemblyOperandKind::InOut {
                input,
                result_index,
            } => {
                writer.u8(3)?;
                writer.u32(input.0)?;
                writer.u32(result_index)?;
            }
            AssemblyOperandKind::ImmediateI32(value) => {
                writer.u8(4)?;
                writer.bytes(&value.to_le_bytes())?;
            }
        }
    }
    writer.count("inline assembly options", assembly.options.len(), 5)?;
    for option in &assembly.options {
        writer.u8(assembly_option_tag(*option))?;
    }
    writer.count(
        "inline assembly declared effects",
        assembly.declared_effects.len(),
        7,
    )?;
    for effect in &assembly.declared_effects {
        writer.u8(assembly_effect_tag(*effect))?;
    }
    Ok(())
}

fn decode_inline_assembly(reader: &mut Reader<'_>) -> Result<InlineAssembly, KernelIrDecodeError> {
    let target = decode_inline_assembly_target(reader.u8()?)?;
    let source = AssemblySourceIdentity::new(
        reader.fixed()?,
        reader.fixed()?,
        reader.fixed()?,
        reader.fixed()?,
    );
    let mnemonic = reader.text("inline assembly mnemonic")?;
    let operand_count = reader.count("inline assembly operands", MAX_ASSEMBLY_OPERANDS_V3)?;
    let mut operands = Vec::with_capacity(operand_count);
    for _ in 0..operand_count {
        let constraint = decode_assembly_constraint(reader.u8()?)?;
        let kind = match reader.u8()? {
            1 => AssemblyOperandKind::Input(ValueId(reader.u32()?)),
            2 => AssemblyOperandKind::Output {
                result_index: reader.u32()?,
            },
            3 => AssemblyOperandKind::InOut {
                input: ValueId(reader.u32()?),
                result_index: reader.u32()?,
            },
            4 => AssemblyOperandKind::ImmediateI32(i32::from_le_bytes(reader.fixed()?)),
            tag => {
                return Err(KernelIrDecodeError::UnknownTag {
                    kind: "inline assembly operand role",
                    tag,
                });
            }
        };
        operands.push(AssemblyOperand { kind, constraint });
    }
    let option_count = reader.count("inline assembly options", 5)?;
    let mut options = BTreeSet::new();
    let mut previous_option = None;
    for _ in 0..option_count {
        let option = decode_assembly_option(reader.u8()?)?;
        if previous_option.is_some_and(|previous| previous >= option) {
            return Err(KernelIrDecodeError::NonCanonical);
        }
        previous_option = Some(option);
        options.insert(option);
    }
    let effect_count = reader.count("inline assembly declared effects", 7)?;
    let mut declared_effects = BTreeSet::new();
    let mut previous_effect = None;
    for _ in 0..effect_count {
        let effect = decode_assembly_effect(reader.u8()?)?;
        if previous_effect.is_some_and(|previous| previous >= effect) {
            return Err(KernelIrDecodeError::NonCanonical);
        }
        previous_effect = Some(effect);
        declared_effects.insert(effect);
    }
    Ok(InlineAssembly {
        target,
        source,
        mnemonic,
        operands,
        options,
        declared_effects,
    })
}

fn encode_value_def(writer: &mut Writer, value: &ValueDef) -> Result<(), KernelIrEncodeError> {
    writer.u32(value.id.0)?;
    encode_type(writer, &value.ty, 0)
}

fn decode_value_def(reader: &mut Reader<'_>) -> Result<ValueDef, KernelIrDecodeError> {
    Ok(ValueDef::new(
        ValueId(reader.u32()?),
        decode_type(reader, 0)?,
    ))
}

fn encode_values(
    writer: &mut Writer,
    field: &'static str,
    values: &[ValueId],
    max: usize,
) -> Result<(), KernelIrEncodeError> {
    writer.count(field, values.len(), max)?;
    for value in values {
        writer.u32(value.0)?;
    }
    Ok(())
}

fn decode_values(
    reader: &mut Reader<'_>,
    field: &'static str,
    max: usize,
) -> Result<Vec<ValueId>, KernelIrDecodeError> {
    let count = reader.count(field, max)?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(ValueId(reader.u32()?));
    }
    Ok(values)
}

fn encode_optional_value(
    writer: &mut Writer,
    value: Option<ValueId>,
) -> Result<(), KernelIrEncodeError> {
    match value {
        None => writer.u8(0),
        Some(value) => {
            writer.u8(1)?;
            writer.u32(value.0)
        }
    }
}

fn decode_optional_value(
    reader: &mut Reader<'_>,
    field: &'static str,
) -> Result<Option<ValueId>, KernelIrDecodeError> {
    Ok(if reader.option(field)? {
        Some(ValueId(reader.u32()?))
    } else {
        None
    })
}

fn encode_type(writer: &mut Writer, ty: &Type, depth: usize) -> Result<(), KernelIrEncodeError> {
    if depth > MAX_TYPE_DEPTH_V1 {
        return Err(KernelIrEncodeError::TypeNestingTooDeep {
            max: MAX_TYPE_DEPTH_V1,
        });
    }
    match ty {
        Type::Unit => writer.u8(1)?,
        Type::Scalar(scalar) => {
            writer.u8(2)?;
            writer.u8(scalar_type_tag(*scalar))?;
        }
        Type::Pointer(pointer) => {
            writer.u8(3)?;
            writer.u8(address_space_tag(pointer.address_space))?;
            writer.u8(access_mode_tag(pointer.access))?;
            encode_type(writer, &pointer.pointee, depth + 1)?;
        }
        Type::Slice(slice) => {
            writer.u8(4)?;
            writer.u8(address_space_tag(slice.address_space))?;
            writer.u8(access_mode_tag(slice.access))?;
            encode_type(writer, &slice.element, depth + 1)?;
        }
    }
    Ok(())
}

fn decode_type(reader: &mut Reader<'_>, depth: usize) -> Result<Type, KernelIrDecodeError> {
    if depth > MAX_TYPE_DEPTH_V1 {
        return Err(KernelIrDecodeError::TypeNestingTooDeep {
            max: MAX_TYPE_DEPTH_V1,
        });
    }
    Ok(match reader.u8()? {
        1 => Type::Unit,
        2 => Type::Scalar(decode_scalar_type(reader.u8()?)?),
        3 => {
            let address_space = decode_address_space(reader.u8()?)?;
            let access = decode_access_mode(reader.u8()?)?;
            let pointee = decode_type(reader, depth + 1)?;
            Type::Pointer(PointerType::new(pointee, address_space, access))
        }
        4 => {
            let address_space = decode_address_space(reader.u8()?)?;
            let access = decode_access_mode(reader.u8()?)?;
            let element = decode_type(reader, depth + 1)?;
            Type::Slice(SliceType::new(element, address_space, access))
        }
        tag => return Err(KernelIrDecodeError::UnknownTag { kind: "type", tag }),
    })
}

fn encode_intrinsic(
    writer: &mut Writer,
    intrinsic: &IntrinsicOperation,
) -> Result<(), KernelIrEncodeError> {
    match intrinsic.kind {
        IntrinsicKind::InvocationIndex { kind, axis } => {
            writer.u8(1)?;
            writer.u8(index_kind_tag(kind))?;
            writer.u8(axis_tag(axis))?;
        }
        IntrinsicKind::LaunchExtent { axis } => {
            writer.u8(2)?;
            writer.u8(axis_tag(axis))?;
        }
    }
    encode_type(writer, &intrinsic.result_type, 0)
}

fn decode_intrinsic(reader: &mut Reader<'_>) -> Result<IntrinsicOperation, KernelIrDecodeError> {
    let kind = match reader.u8()? {
        1 => IntrinsicKind::InvocationIndex {
            kind: decode_index_kind(reader.u8()?)?,
            axis: decode_axis(reader.u8()?)?,
        },
        2 => IntrinsicKind::LaunchExtent {
            axis: decode_axis(reader.u8()?)?,
        },
        tag => {
            return Err(KernelIrDecodeError::UnknownTag {
                kind: "intrinsic",
                tag,
            });
        }
    };
    Ok(IntrinsicOperation::new(kind, decode_type(reader, 0)?))
}

fn encode_constant(writer: &mut Writer, value: &Constant) -> Result<(), KernelIrEncodeError> {
    match value {
        Constant::Bool(value) => {
            writer.u8(1)?;
            writer.u8(u8::from(*value))?;
        }
        Constant::I8(value) => {
            writer.u8(2)?;
            writer.u8(*value as u8)?;
        }
        Constant::I16(value) => {
            writer.u8(3)?;
            writer.bytes(&value.to_le_bytes())?;
        }
        Constant::I32(value) => {
            writer.u8(4)?;
            writer.bytes(&value.to_le_bytes())?;
        }
        Constant::I64(value) => {
            writer.u8(5)?;
            writer.bytes(&value.to_le_bytes())?;
        }
        Constant::U8(value) => {
            writer.u8(6)?;
            writer.u8(*value)?;
        }
        Constant::U16(value) => {
            writer.u8(7)?;
            writer.u16(*value)?;
        }
        Constant::U32(value) => {
            writer.u8(8)?;
            writer.u32(*value)?;
        }
        Constant::U64(value) => {
            writer.u8(9)?;
            writer.u64(*value)?;
        }
        Constant::Index(value) => {
            writer.u8(10)?;
            writer.u64(*value)?;
        }
        Constant::F16Bits(value) => {
            writer.u8(11)?;
            writer.u16(*value)?;
        }
        Constant::Bf16Bits(value) => {
            writer.u8(12)?;
            writer.u16(*value)?;
        }
        Constant::F32Bits(value) => {
            writer.u8(13)?;
            writer.u32(*value)?;
        }
        Constant::F64Bits(value) => {
            writer.u8(14)?;
            writer.u64(*value)?;
        }
    }
    Ok(())
}

fn decode_constant(reader: &mut Reader<'_>) -> Result<Constant, KernelIrDecodeError> {
    Ok(match reader.u8()? {
        1 => Constant::Bool(reader.boolean("boolean constant")?),
        2 => Constant::I8(reader.u8()? as i8),
        3 => Constant::I16(i16::from_le_bytes(reader.fixed::<2>()?)),
        4 => Constant::I32(i32::from_le_bytes(reader.fixed::<4>()?)),
        5 => Constant::I64(i64::from_le_bytes(reader.fixed::<8>()?)),
        6 => Constant::U8(reader.u8()?),
        7 => Constant::U16(reader.u16()?),
        8 => Constant::U32(reader.u32()?),
        9 => Constant::U64(reader.u64()?),
        10 => Constant::Index(reader.u64()?),
        11 => Constant::F16Bits(reader.u16()?),
        12 => Constant::Bf16Bits(reader.u16()?),
        13 => Constant::F32Bits(reader.u32()?),
        14 => Constant::F64Bits(reader.u64()?),
        tag => {
            return Err(KernelIrDecodeError::UnknownTag {
                kind: "constant",
                tag,
            });
        }
    })
}

fn encode_memory_access(
    writer: &mut Writer,
    access: MemoryAccess,
) -> Result<(), KernelIrEncodeError> {
    writer.u8(address_space_tag(access.address_space))?;
    writer.u32(access.alignment)?;
    writer.u8(u8::from(access.volatile))
}

fn decode_memory_access(reader: &mut Reader<'_>) -> Result<MemoryAccess, KernelIrDecodeError> {
    Ok(MemoryAccess {
        address_space: decode_address_space(reader.u8()?)?,
        alignment: reader.u32()?,
        volatile: reader.boolean("volatile memory access")?,
    })
}

fn encode_barrier(writer: &mut Writer, barrier: &Barrier) -> Result<(), KernelIrEncodeError> {
    writer.u8(scope_tag(barrier.execution_scope))?;
    writer.u8(scope_tag(barrier.memory_scope))?;
    writer.u8(ordering_tag(barrier.semantics.ordering))?;
    encode_address_spaces(writer, &barrier.semantics.address_spaces)
}

fn decode_barrier(reader: &mut Reader<'_>) -> Result<Barrier, KernelIrDecodeError> {
    Ok(Barrier {
        execution_scope: decode_scope(reader.u8()?)?,
        memory_scope: decode_scope(reader.u8()?)?,
        semantics: BarrierSemantics {
            ordering: decode_ordering(reader.u8()?)?,
            address_spaces: decode_address_spaces(reader)?,
        },
    })
}

fn encode_fence(writer: &mut Writer, fence: &Fence) -> Result<(), KernelIrEncodeError> {
    writer.u8(scope_tag(fence.memory_scope))?;
    encode_barrier_semantics(writer, &fence.semantics)
}

fn decode_fence(reader: &mut Reader<'_>) -> Result<Fence, KernelIrDecodeError> {
    Ok(Fence {
        memory_scope: decode_scope(reader.u8()?)?,
        semantics: decode_barrier_semantics(reader)?,
    })
}

fn encode_workgroup_barrier(
    writer: &mut Writer,
    barrier: &WorkgroupBarrier,
) -> Result<(), KernelIrEncodeError> {
    writer.u8(scope_tag(barrier.memory_scope))?;
    encode_barrier_semantics(writer, &barrier.semantics)?;
    encode_convergence(writer, barrier.convergence)
}

fn decode_workgroup_barrier(
    reader: &mut Reader<'_>,
) -> Result<WorkgroupBarrier, KernelIrDecodeError> {
    Ok(WorkgroupBarrier {
        memory_scope: decode_scope(reader.u8()?)?,
        semantics: decode_barrier_semantics(reader)?,
        convergence: decode_convergence(reader)?,
    })
}

fn encode_convergence(
    writer: &mut Writer,
    convergence: Convergence,
) -> Result<(), KernelIrEncodeError> {
    match convergence {
        Convergence::Uniform { scope } => {
            writer.u8(1)?;
            writer.u8(scope_tag(scope))
        }
    }
}

fn decode_convergence(reader: &mut Reader<'_>) -> Result<Convergence, KernelIrDecodeError> {
    match reader.u8()? {
        1 => Ok(Convergence::uniform(decode_scope(reader.u8()?)?)),
        tag => Err(KernelIrDecodeError::UnknownTag {
            kind: "convergence",
            tag,
        }),
    }
}

fn encode_workgroup_memory(
    writer: &mut Writer,
    memory: &WorkgroupMemory,
) -> Result<(), KernelIrEncodeError> {
    encode_type(writer, &memory.element, 0)?;
    match memory.extent {
        WorkgroupMemoryExtent::Static(elements) => {
            writer.u8(1)?;
            writer.u32(elements)?;
        }
        WorkgroupMemoryExtent::Dynamic => writer.u8(2)?,
        WorkgroupMemoryExtent::DynamicAtLeast(_) => {
            return Err(KernelIrEncodeError::UnsupportedInVersion {
                version: writer.version,
                feature: "authenticated dynamic workgroup-memory extent",
            });
        }
    }
    writer.u32(memory.alignment)
}

fn decode_workgroup_memory(
    reader: &mut Reader<'_>,
) -> Result<WorkgroupMemory, KernelIrDecodeError> {
    let element = decode_type(reader, 0)?;
    let extent = match reader.u8()? {
        1 => WorkgroupMemoryExtent::Static(reader.u32()?),
        2 => WorkgroupMemoryExtent::Dynamic,
        tag => {
            return Err(KernelIrDecodeError::UnknownTag {
                kind: "workgroup memory extent",
                tag,
            });
        }
    };
    Ok(WorkgroupMemory {
        element,
        extent,
        alignment: reader.u32()?,
    })
}

fn encode_wave_operation(
    writer: &mut Writer,
    wave: &WaveOperation,
) -> Result<(), KernelIrEncodeError> {
    writer.u8(wave_width_tag(wave.width))?;
    writer.u32(wave.active_lanes)?;
    encode_convergence(writer, wave.convergence)?;
    match wave.kind {
        WaveOperationKind::LaneId => writer.u8(1),
        WaveOperationKind::Ballot { predicate } => {
            writer.u8(2)?;
            writer.u32(predicate.0)
        }
        WaveOperationKind::Any { predicate } => {
            writer.u8(3)?;
            writer.u32(predicate.0)
        }
        WaveOperationKind::All { predicate } => {
            writer.u8(4)?;
            writer.u32(predicate.0)
        }
        WaveOperationKind::ShuffleIndex {
            value,
            source_lane,
            tile_width,
        } => {
            writer.u8(5)?;
            writer.u32(value.0)?;
            writer.u32(source_lane.0)?;
            writer.u32(tile_width)
        }
    }
}

fn decode_wave_operation(reader: &mut Reader<'_>) -> Result<WaveOperation, KernelIrDecodeError> {
    let width = decode_wave_width(reader.u8()?)?;
    let active_lanes = reader.u32()?;
    let convergence = decode_convergence(reader)?;
    let kind = match reader.u8()? {
        1 => WaveOperationKind::LaneId,
        2 => WaveOperationKind::Ballot {
            predicate: ValueId(reader.u32()?),
        },
        3 => WaveOperationKind::Any {
            predicate: ValueId(reader.u32()?),
        },
        4 => WaveOperationKind::All {
            predicate: ValueId(reader.u32()?),
        },
        5 => WaveOperationKind::ShuffleIndex {
            value: ValueId(reader.u32()?),
            source_lane: ValueId(reader.u32()?),
            tile_width: reader.u32()?,
        },
        tag => {
            return Err(KernelIrDecodeError::UnknownTag {
                kind: "wave operation",
                tag,
            });
        }
    };
    Ok(WaveOperation {
        kind,
        width,
        active_lanes,
        convergence,
    })
}

fn encode_barrier_semantics(
    writer: &mut Writer,
    semantics: &BarrierSemantics,
) -> Result<(), KernelIrEncodeError> {
    writer.u8(ordering_tag(semantics.ordering))?;
    encode_address_spaces(writer, &semantics.address_spaces)
}

fn decode_barrier_semantics(
    reader: &mut Reader<'_>,
) -> Result<BarrierSemantics, KernelIrDecodeError> {
    Ok(BarrierSemantics {
        ordering: decode_ordering(reader.u8()?)?,
        address_spaces: decode_address_spaces(reader)?,
    })
}

fn encode_atomic(writer: &mut Writer, atomic: &Atomic) -> Result<(), KernelIrEncodeError> {
    writer.u8(atomic_kind_tag(atomic.kind))?;
    writer.u32(atomic.pointer.0)?;
    encode_optional_value(writer, atomic.value)?;
    encode_optional_value(writer, atomic.compare)?;
    encode_memory_access(writer, atomic.access)?;
    writer.u8(scope_tag(atomic.scope))?;
    writer.u8(ordering_tag(atomic.ordering))?;
    match atomic.failure_ordering {
        None => writer.u8(0),
        Some(ordering) => {
            writer.u8(1)?;
            writer.u8(ordering_tag(ordering))
        }
    }
}

fn decode_atomic(reader: &mut Reader<'_>) -> Result<Atomic, KernelIrDecodeError> {
    Ok(Atomic {
        kind: decode_atomic_kind(reader.u8()?)?,
        pointer: ValueId(reader.u32()?),
        value: decode_optional_value(reader, "atomic value")?,
        compare: decode_optional_value(reader, "atomic comparison")?,
        access: decode_memory_access(reader)?,
        scope: decode_scope(reader.u8()?)?,
        ordering: decode_ordering(reader.u8()?)?,
        failure_ordering: if reader.option("atomic failure ordering")? {
            Some(decode_ordering(reader.u8()?)?)
        } else {
            None
        },
    })
}

fn encode_terminator(
    writer: &mut Writer,
    terminator: &Terminator,
) -> Result<(), KernelIrEncodeError> {
    match terminator {
        Terminator::Branch { target, arguments } => {
            writer.u8(1)?;
            writer.u32(target.0)?;
            encode_values(
                writer,
                "branch arguments",
                arguments,
                MAX_VALUE_ARGUMENTS_V1,
            )?;
        }
        Terminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => {
            writer.u8(2)?;
            writer.u32(condition.0)?;
            writer.u32(then_target.0)?;
            encode_values(
                writer,
                "conditional branch then arguments",
                then_arguments,
                MAX_VALUE_ARGUMENTS_V1,
            )?;
            writer.u32(else_target.0)?;
            encode_values(
                writer,
                "conditional branch else arguments",
                else_arguments,
                MAX_VALUE_ARGUMENTS_V1,
            )?;
        }
        Terminator::Switch {
            selector,
            cases,
            default_target,
            default_arguments,
        } => {
            writer.u8(3)?;
            writer.u32(selector.0)?;
            writer.count("switch cases", cases.len(), MAX_SWITCH_CASES_V1)?;
            for case in cases {
                writer.u64(case.value)?;
                writer.u32(case.target.0)?;
                encode_values(
                    writer,
                    "switch case arguments",
                    &case.arguments,
                    MAX_VALUE_ARGUMENTS_V1,
                )?;
            }
            writer.u32(default_target.0)?;
            encode_values(
                writer,
                "switch default arguments",
                default_arguments,
                MAX_VALUE_ARGUMENTS_V1,
            )?;
        }
        Terminator::IntegerSwitch {
            selector,
            cases,
            default_target,
            default_arguments,
        } => {
            require_v2(writer, "typed integer switch terminator")?;
            check_limit(
                "integer switch cases",
                cases.len(),
                MAX_INTEGER_SWITCH_CASES_V2,
            )?;
            if cases.windows(2).any(|pair| pair[0].value >= pair[1].value) {
                return Err(KernelIrEncodeError::NonCanonical {
                    field: "integer switch cases",
                });
            }

            writer.u8(6)?;
            writer.u32(selector.0)?;
            writer.count(
                "integer switch cases",
                cases.len(),
                MAX_INTEGER_SWITCH_CASES_V2,
            )?;
            for case in cases {
                encode_constant(writer, &case.value)?;
                writer.u32(case.target.0)?;
                encode_values(
                    writer,
                    "integer switch case arguments",
                    &case.arguments,
                    MAX_VALUE_ARGUMENTS_V1,
                )?;
            }
            writer.u32(default_target.0)?;
            encode_values(
                writer,
                "integer switch default arguments",
                default_arguments,
                MAX_VALUE_ARGUMENTS_V1,
            )?;
        }
        Terminator::Return { values } => {
            writer.u8(4)?;
            encode_values(writer, "return values", values, MAX_VALUE_ARGUMENTS_V1)?;
        }
        Terminator::Unreachable => writer.u8(5)?,
    }
    Ok(())
}

fn decode_terminator(reader: &mut Reader<'_>) -> Result<Terminator, KernelIrDecodeError> {
    Ok(match reader.u8()? {
        1 => Terminator::Branch {
            target: BlockId(reader.u32()?),
            arguments: decode_values(reader, "branch arguments", MAX_VALUE_ARGUMENTS_V1)?,
        },
        2 => Terminator::ConditionalBranch {
            condition: ValueId(reader.u32()?),
            then_target: BlockId(reader.u32()?),
            then_arguments: decode_values(
                reader,
                "conditional branch then arguments",
                MAX_VALUE_ARGUMENTS_V1,
            )?,
            else_target: BlockId(reader.u32()?),
            else_arguments: decode_values(
                reader,
                "conditional branch else arguments",
                MAX_VALUE_ARGUMENTS_V1,
            )?,
        },
        3 => {
            let selector = ValueId(reader.u32()?);
            let case_count = reader.count("switch cases", MAX_SWITCH_CASES_V1)?;
            let mut cases = Vec::with_capacity(case_count);
            for _ in 0..case_count {
                cases.push(SwitchCase {
                    value: reader.u64()?,
                    target: BlockId(reader.u32()?),
                    arguments: decode_values(
                        reader,
                        "switch case arguments",
                        MAX_VALUE_ARGUMENTS_V1,
                    )?,
                });
            }
            Terminator::Switch {
                selector,
                cases,
                default_target: BlockId(reader.u32()?),
                default_arguments: decode_values(
                    reader,
                    "switch default arguments",
                    MAX_VALUE_ARGUMENTS_V1,
                )?,
            }
        }
        4 => Terminator::Return {
            values: decode_values(reader, "return values", MAX_VALUE_ARGUMENTS_V1)?,
        },
        5 => Terminator::Unreachable,
        6 if reader.version >= KERNEL_IR_VERSION_V2 => {
            let selector = ValueId(reader.u32()?);
            let case_count = reader.count("integer switch cases", MAX_INTEGER_SWITCH_CASES_V2)?;
            let mut cases: Vec<IntegerSwitchCase> = Vec::with_capacity(case_count);
            for _ in 0..case_count {
                let value = decode_constant(reader)?;
                if cases.last().is_some_and(|previous| previous.value >= value) {
                    return Err(KernelIrDecodeError::NonCanonical);
                }
                cases.push(IntegerSwitchCase {
                    value,
                    target: BlockId(reader.u32()?),
                    arguments: decode_values(
                        reader,
                        "integer switch case arguments",
                        MAX_VALUE_ARGUMENTS_V1,
                    )?,
                });
            }
            Terminator::IntegerSwitch {
                selector,
                cases,
                default_target: BlockId(reader.u32()?),
                default_arguments: decode_values(
                    reader,
                    "integer switch default arguments",
                    MAX_VALUE_ARGUMENTS_V1,
                )?,
            }
        }
        tag => {
            return Err(KernelIrDecodeError::UnknownTag {
                kind: "terminator",
                tag,
            });
        }
    })
}

fn encode_launch_domain(
    writer: &mut Writer,
    domain: &LaunchDomain,
) -> Result<(), KernelIrEncodeError> {
    match domain {
        LaunchDomain::D1 { x } => {
            writer.u8(1)?;
            encode_launch_extent(writer, *x)?;
        }
        LaunchDomain::D2 { x, y } => {
            writer.u8(2)?;
            encode_launch_extent(writer, *x)?;
            encode_launch_extent(writer, *y)?;
        }
        LaunchDomain::D3 { x, y, z } => {
            writer.u8(3)?;
            encode_launch_extent(writer, *x)?;
            encode_launch_extent(writer, *y)?;
            encode_launch_extent(writer, *z)?;
        }
    }
    Ok(())
}

fn decode_launch_domain(reader: &mut Reader<'_>) -> Result<LaunchDomain, KernelIrDecodeError> {
    Ok(match reader.u8()? {
        1 => LaunchDomain::D1 {
            x: decode_launch_extent(reader)?,
        },
        2 => LaunchDomain::D2 {
            x: decode_launch_extent(reader)?,
            y: decode_launch_extent(reader)?,
        },
        3 => LaunchDomain::D3 {
            x: decode_launch_extent(reader)?,
            y: decode_launch_extent(reader)?,
            z: decode_launch_extent(reader)?,
        },
        tag => {
            return Err(KernelIrDecodeError::UnknownTag {
                kind: "launch domain",
                tag,
            });
        }
    })
}

fn encode_launch_extent(
    writer: &mut Writer,
    extent: LaunchExtent,
) -> Result<(), KernelIrEncodeError> {
    match extent {
        LaunchExtent::Dynamic => writer.u8(1),
        LaunchExtent::Static(value) => {
            writer.u8(2)?;
            writer.u32(value)
        }
    }
}

fn decode_launch_extent(reader: &mut Reader<'_>) -> Result<LaunchExtent, KernelIrDecodeError> {
    match reader.u8()? {
        1 => Ok(LaunchExtent::Dynamic),
        2 => Ok(LaunchExtent::Static(reader.u32()?)),
        tag => Err(KernelIrDecodeError::UnknownTag {
            kind: "launch extent",
            tag,
        }),
    }
}

fn encode_capabilities(
    writer: &mut Writer,
    capabilities: &BTreeSet<TargetCapability>,
) -> Result<(), KernelIrEncodeError> {
    writer.count("capabilities", capabilities.len(), MAX_CAPABILITIES_V1)?;
    for capability in capabilities {
        match capability {
            TargetCapability::Float16 => writer.u8(1)?,
            TargetCapability::BFloat16 => writer.u8(2)?,
            TargetCapability::Float64 => writer.u8(3)?,
            TargetCapability::Int64 => writer.u8(4)?,
            TargetCapability::Subgroups => writer.u8(5)?,
            TargetCapability::SubgroupSize(size) => {
                writer.u8(6)?;
                writer.u32(*size)?;
            }
            TargetCapability::WorkgroupMemory => writer.u8(7)?,
            TargetCapability::WorkgroupBarrier => writer.u8(8)?,
            TargetCapability::Atomic {
                width_bits,
                address_space,
                max_scope,
            } => {
                writer.u8(9)?;
                writer.u16(*width_bits)?;
                writer.u8(address_space_tag(*address_space))?;
                writer.u8(scope_tag(*max_scope))?;
            }
            TargetCapability::DynamicWorkgroupMemory => writer.u8(10)?,
            TargetCapability::Extension { namespace, name } => {
                writer.u8(11)?;
                writer.text("capability extension namespace", namespace)?;
                writer.text("capability extension name", name)?;
            }
            TargetCapability::WaveWidth(width) => {
                require_v2(writer, "exact wave-width capability")?;
                writer.u8(12)?;
                writer.u8(wave_width_tag(*width))?;
            }
        }
    }
    Ok(())
}

fn decode_capabilities(
    reader: &mut Reader<'_>,
) -> Result<BTreeSet<TargetCapability>, KernelIrDecodeError> {
    let count = reader.count("capabilities", MAX_CAPABILITIES_V1)?;
    let mut capabilities = BTreeSet::new();
    let mut previous: Option<TargetCapability> = None;
    for _ in 0..count {
        let capability = match reader.u8()? {
            1 => TargetCapability::Float16,
            2 => TargetCapability::BFloat16,
            3 => TargetCapability::Float64,
            4 => TargetCapability::Int64,
            5 => TargetCapability::Subgroups,
            6 => TargetCapability::SubgroupSize(reader.u32()?),
            7 => TargetCapability::WorkgroupMemory,
            8 => TargetCapability::WorkgroupBarrier,
            9 => TargetCapability::Atomic {
                width_bits: reader.u16()?,
                address_space: decode_address_space(reader.u8()?)?,
                max_scope: decode_scope(reader.u8()?)?,
            },
            10 => TargetCapability::DynamicWorkgroupMemory,
            11 => TargetCapability::Extension {
                namespace: reader.text("capability extension namespace")?,
                name: reader.text("capability extension name")?,
            },
            12 if reader.version >= KERNEL_IR_VERSION_V2 => {
                TargetCapability::WaveWidth(decode_wave_width(reader.u8()?)?)
            }
            tag => {
                return Err(KernelIrDecodeError::UnknownTag {
                    kind: "target capability",
                    tag,
                });
            }
        };
        if previous.as_ref().is_some_and(|item| item >= &capability) {
            return Err(KernelIrDecodeError::NonCanonical);
        }
        previous = Some(capability.clone());
        capabilities.insert(capability);
    }
    Ok(capabilities)
}

fn encode_address_spaces(
    writer: &mut Writer,
    address_spaces: &BTreeSet<AddressSpace>,
) -> Result<(), KernelIrEncodeError> {
    writer.count(
        "barrier address spaces",
        address_spaces.len(),
        MAX_ADDRESS_SPACES,
    )?;
    for address_space in address_spaces {
        writer.u8(address_space_tag(*address_space))?;
    }
    Ok(())
}

fn decode_address_spaces(
    reader: &mut Reader<'_>,
) -> Result<BTreeSet<AddressSpace>, KernelIrDecodeError> {
    let count = reader.count("barrier address spaces", MAX_ADDRESS_SPACES)?;
    let mut address_spaces = BTreeSet::new();
    let mut previous = None;
    for _ in 0..count {
        let address_space = decode_address_space(reader.u8()?)?;
        if previous.is_some_and(|item| item >= address_space) {
            return Err(KernelIrDecodeError::NonCanonical);
        }
        previous = Some(address_space);
        address_spaces.insert(address_space);
    }
    Ok(address_spaces)
}

macro_rules! enum_codec {
    ($tag_fn:ident, $decode_fn:ident, $ty:ty, $kind:literal, {$($value:path => $tag:literal),+ $(,)?}) => {
        const fn $tag_fn(value: $ty) -> u8 {
            match value { $($value => $tag),+ }
        }

        fn $decode_fn(tag: u8) -> Result<$ty, KernelIrDecodeError> {
            match tag {
                $($tag => Ok($value)),+,
                tag => Err(KernelIrDecodeError::UnknownTag { kind: $kind, tag }),
            }
        }
    };
}

enum_codec!(address_space_tag, decode_address_space, AddressSpace, "address space", {
    AddressSpace::Private => 1,
    AddressSpace::Workgroup => 2,
    AddressSpace::Global => 3,
    AddressSpace::Constant => 4,
    AddressSpace::Generic => 5,
});
enum_codec!(access_mode_tag, decode_access_mode, AccessMode, "access mode", {
    AccessMode::ReadOnly => 1,
    AccessMode::ReadWrite => 2,
});
enum_codec!(scalar_type_tag, decode_scalar_type, ScalarType, "scalar type", {
    ScalarType::Bool => 1,
    ScalarType::I8 => 2,
    ScalarType::I16 => 3,
    ScalarType::I32 => 4,
    ScalarType::I64 => 5,
    ScalarType::U8 => 6,
    ScalarType::U16 => 7,
    ScalarType::U32 => 8,
    ScalarType::U64 => 9,
    ScalarType::Index => 10,
    ScalarType::F16 => 11,
    ScalarType::Bf16 => 12,
    ScalarType::F32 => 13,
    ScalarType::F64 => 14,
    ScalarType::I128 => 15,
    ScalarType::U128 => 16,
});
enum_codec!(axis_tag, decode_axis, Axis, "axis", {
    Axis::X => 1,
    Axis::Y => 2,
    Axis::Z => 3,
});
enum_codec!(scope_tag, decode_scope, SynchronizationScope, "synchronization scope", {
    SynchronizationScope::Invocation => 1,
    SynchronizationScope::Subgroup => 2,
    SynchronizationScope::Workgroup => 3,
    SynchronizationScope::Device => 4,
    SynchronizationScope::System => 5,
});
enum_codec!(ordering_tag, decode_ordering, MemoryOrdering, "memory ordering", {
    MemoryOrdering::Relaxed => 1,
    MemoryOrdering::Acquire => 2,
    MemoryOrdering::Release => 3,
    MemoryOrdering::AcquireRelease => 4,
    MemoryOrdering::SequentiallyConsistent => 5,
});
enum_codec!(wave_width_tag, decode_wave_width, WaveWidth, "wave width", {
    WaveWidth::Wave32 => 1,
    WaveWidth::Wave64 => 2,
});
enum_codec!(index_kind_tag, decode_index_kind, IndexKind, "index kind", {
    IndexKind::Global => 1,
    IndexKind::Workgroup => 2,
    IndexKind::Local => 3,
    IndexKind::WorkgroupSize => 4,
    IndexKind::WorkgroupCount => 5,
});
enum_codec!(unary_op_tag, decode_unary_op, UnaryOp, "unary operation", {
    UnaryOp::Negate => 1,
    UnaryOp::Not => 2,
});
enum_codec!(binary_op_tag, decode_binary_op, BinaryOp, "binary operation", {
    BinaryOp::Add => 1,
    BinaryOp::Subtract => 2,
    BinaryOp::Multiply => 3,
    BinaryOp::Divide => 4,
    BinaryOp::Remainder => 5,
    BinaryOp::BitAnd => 6,
    BinaryOp::BitOr => 7,
    BinaryOp::BitXor => 8,
    BinaryOp::ShiftLeft => 9,
    BinaryOp::ShiftRight => 10,
});
enum_codec!(compare_predicate_tag, decode_compare_predicate, ComparePredicate, "compare predicate", {
    ComparePredicate::Equal => 1,
    ComparePredicate::NotEqual => 2,
    ComparePredicate::LessThan => 3,
    ComparePredicate::LessThanOrEqual => 4,
    ComparePredicate::GreaterThan => 5,
    ComparePredicate::GreaterThanOrEqual => 6,
});
enum_codec!(cast_kind_tag, decode_cast_kind, CastKind, "cast kind", {
    CastKind::Truncate => 1,
    CastKind::ZeroExtend => 2,
    CastKind::SignExtend => 3,
    CastKind::FloatExtend => 4,
    CastKind::FloatTruncate => 5,
    CastKind::IntegerToFloat => 6,
    CastKind::FloatToInteger => 7,
    CastKind::Bitcast => 8,
});
enum_codec!(atomic_kind_tag, decode_atomic_kind, AtomicKind, "atomic kind", {
    AtomicKind::Load => 1,
    AtomicKind::Store => 2,
    AtomicKind::Exchange => 3,
    AtomicKind::CompareExchange => 4,
    AtomicKind::Add => 5,
    AtomicKind::Subtract => 6,
    AtomicKind::Min => 7,
    AtomicKind::Max => 8,
    AtomicKind::BitAnd => 9,
    AtomicKind::BitOr => 10,
    AtomicKind::BitXor => 11,
});
enum_codec!(inline_assembly_target_tag, decode_inline_assembly_target, InlineAssemblyTarget, "inline assembly target", {
    InlineAssemblyTarget::AmdGpuGfx942 => 1,
});
enum_codec!(assembly_constraint_tag, decode_assembly_constraint, AssemblyConstraint, "inline assembly constraint", {
    AssemblyConstraint::Sgpr32 => 1,
    AssemblyConstraint::Vgpr32 => 2,
    AssemblyConstraint::ImmediateI32 => 3,
});
enum_codec!(assembly_option_tag, decode_assembly_option, AssemblyOption, "inline assembly option", {
    AssemblyOption::NoMemory => 1,
    AssemblyOption::ReadOnly => 2,
    AssemblyOption::Pure => 3,
    AssemblyOption::PreservesFlags => 4,
    AssemblyOption::NoStack => 5,
});
enum_codec!(assembly_effect_tag, decode_assembly_effect, AssemblyEffect, "inline assembly effect", {
    AssemblyEffect::ReadGlobal => 1,
    AssemblyEffect::WriteGlobal => 2,
    AssemblyEffect::ReadWorkgroup => 3,
    AssemblyEffect::WriteWorkgroup => 4,
    AssemblyEffect::Atomic => 5,
    AssemblyEffect::Barrier => 6,
    AssemblyEffect::ControlFlow => 7,
});

fn require_v2(writer: &Writer, feature: &'static str) -> Result<(), KernelIrEncodeError> {
    if writer.version >= KERNEL_IR_VERSION_V2 {
        Ok(())
    } else {
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: writer.version,
            feature,
        })
    }
}

fn require_v3(writer: &Writer, feature: &'static str) -> Result<(), KernelIrEncodeError> {
    if writer.version >= KERNEL_IR_VERSION_V3 {
        Ok(())
    } else {
        Err(KernelIrEncodeError::UnsupportedInVersion {
            version: writer.version,
            feature,
        })
    }
}

struct Writer {
    bytes: Vec<u8>,
    version: u16,
}

impl Writer {
    fn new(version: u16) -> Self {
        Self {
            bytes: Vec::new(),
            version,
        }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), KernelIrEncodeError> {
        let next =
            self.bytes
                .len()
                .checked_add(value.len())
                .ok_or(KernelIrEncodeError::Overflow {
                    field: "module length",
                })?;
        if next > MAX_MODULE_BYTES_V1 {
            return Err(KernelIrEncodeError::TooLarge {
                max: MAX_MODULE_BYTES_V1,
            });
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn u8(&mut self, value: u8) -> Result<(), KernelIrEncodeError> {
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), KernelIrEncodeError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), KernelIrEncodeError> {
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), KernelIrEncodeError> {
        self.bytes(&value.to_le_bytes())
    }

    fn count(
        &mut self,
        field: &'static str,
        value: usize,
        max: usize,
    ) -> Result<(), KernelIrEncodeError> {
        check_limit(field, value, max)?;
        self.u32(u32::try_from(value).map_err(|_| KernelIrEncodeError::Overflow { field })?)
    }

    fn text(&mut self, field: &'static str, value: &str) -> Result<(), KernelIrEncodeError> {
        check_limit(field, value.len(), MAX_TEXT_BYTES_V1)?;
        self.u32(u32::try_from(value.len()).map_err(|_| KernelIrEncodeError::Overflow { field })?)?;
        self.bytes(value.as_bytes())
    }
}

fn check_limit(field: &'static str, actual: usize, max: usize) -> Result<(), KernelIrEncodeError> {
    if actual > max {
        Err(KernelIrEncodeError::LimitExceeded { field, actual, max })
    } else {
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    version: u16,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            version: 0,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], KernelIrDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(KernelIrDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(KernelIrDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], KernelIrDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| KernelIrDecodeError::Truncated)
    }

    fn u8(&mut self) -> Result<u8, KernelIrDecodeError> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, KernelIrDecodeError> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, KernelIrDecodeError> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, KernelIrDecodeError> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn reserved_u32(&mut self, field: &'static str) -> Result<(), KernelIrDecodeError> {
        if self.u32()? != 0 {
            Err(KernelIrDecodeError::ReservedNonZero { field })
        } else {
            Ok(())
        }
    }

    fn count(&mut self, field: &'static str, max: usize) -> Result<usize, KernelIrDecodeError> {
        let count = self.u32()? as usize;
        if count > max {
            Err(KernelIrDecodeError::LimitExceeded {
                field,
                actual: count,
                max,
            })
        } else {
            Ok(count)
        }
    }

    fn text(&mut self, field: &'static str) -> Result<String, KernelIrDecodeError> {
        let length = self.count(field, MAX_TEXT_BYTES_V1)?;
        let bytes = self.take(length)?;
        str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| KernelIrDecodeError::InvalidUtf8 { field })
    }

    fn option(&mut self, field: &'static str) -> Result<bool, KernelIrDecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(KernelIrDecodeError::UnknownTag { kind: field, tag }),
        }
    }

    fn boolean(&mut self, field: &'static str) -> Result<bool, KernelIrDecodeError> {
        self.option(field)
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
