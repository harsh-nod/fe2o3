#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

use std::{error::Error, fmt, panic::AssertUnwindSafe};

use dialect_gpu::{HierarchyAttr, HierarchyIdOp};
use dialect_kernel::{AlgorithmOp, IterationDomainAttr};
use fe2o3_kernel_ir::{
    DiagnosticCode, KERNEL_IR_VERSION_V1, KERNEL_IR_VERSION_V2, KERNEL_IR_VERSION_V3,
    KERNEL_IR_VERSION_V4, KERNEL_IR_VERSION_V5, KernelIrDecodeError, KernelIrEncodeError,
    MAX_KERNELS_V1, MAX_MODULE_BYTES_V1, Module, decode_module_v5, encode_module_v1,
    encode_module_v2, encode_module_v3, encode_module_v4, encode_module_v5, verify_module,
};
use pliron::{
    attribute::{Attribute, AttributeDict},
    builtin::{
        attributes::{BytesAttr, IdentifierAttr, StringAttr},
        op_interfaces::{ATTR_KEY_SYM_NAME, SingleBlockRegionInterface},
        ops::ModuleOp,
    },
    context::Context,
    dialect::DialectName,
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
};

/// Fixed Pliron symbol used for every bridge envelope.
///
/// KIR module identity is never derived from this presentation-only symbol.
pub const BRIDGE_MODULE_SYMBOL: &str = "fe2o3_kir_bridge";

/// Attribute key for the fixed bridge schema marker.
pub const SCHEMA_ATTR_KEY: &str = "fe2o3_kir_bridge_schema_v1";

/// Attribute key for unchanged canonical KIR bytes.
pub const CANONICAL_BYTES_ATTR_KEY: &str = "fe2o3_kir_bridge_canonical_bytes_v1";

/// Attribute key for the redundant little-endian KIR wire-version discriminant.
pub const WIRE_VERSION_ATTR_KEY: &str = "fe2o3_kir_bridge_wire_version_v1";

/// Attribute key for the redundant exact KIR module identity.
pub const MODULE_IDENTITY_ATTR_KEY: &str = "fe2o3_kir_bridge_module_identity_v1";

/// Fixed schema bytes for the bridge envelope format.
pub const BRIDGE_SCHEMA_V1: [u8; 8] = *b"F2KPB\0\0\x01";

/// Maximum canonical KIR bytes accepted by any bridge configuration.
pub const HARD_MAX_CANONICAL_BYTES: usize = MAX_MODULE_BYTES_V1;

/// Maximum top-level shell operations accepted by any bridge configuration.
///
/// The projection contains exactly two operations per canonical KIR kernel.
pub const HARD_MAX_SHELL_OPERATIONS: usize = MAX_KERNELS_V1 * 2;

/// A frozen KIR wire-version discriminant.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum KirVersion {
    /// Frozen KIR V1.
    V1 = KERNEL_IR_VERSION_V1,
    /// Frozen KIR V2.
    V2 = KERNEL_IR_VERSION_V2,
    /// Frozen KIR V3.
    V3 = KERNEL_IR_VERSION_V3,
    /// Frozen KIR V4.
    V4 = KERNEL_IR_VERSION_V4,
    /// Frozen KIR V5.
    V5 = KERNEL_IR_VERSION_V5,
}

impl KirVersion {
    /// Returns the exact `u16` discriminant stored in canonical KIR bytes.
    pub const fn wire_value(self) -> u16 {
        self as u16
    }

    /// Recognizes an exact frozen V1-V5 wire discriminant.
    pub const fn from_wire_value(value: u16) -> Option<Self> {
        match value {
            KERNEL_IR_VERSION_V1 => Some(Self::V1),
            KERNEL_IR_VERSION_V2 => Some(Self::V2),
            KERNEL_IR_VERSION_V3 => Some(Self::V3),
            KERNEL_IR_VERSION_V4 => Some(Self::V4),
            KERNEL_IR_VERSION_V5 => Some(Self::V5),
            _ => None,
        }
    }
}

impl fmt::Display for KirVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "V{}", self.wire_value())
    }
}

/// Resource governed by [`BridgeLimits`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitResource {
    /// Canonical KIR payload bytes.
    CanonicalBytes,
    /// Top-level Pliron shell operations.
    ShellOperations,
}

/// Invalid bridge-limit configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitError {
    /// A limit was configured as zero.
    Zero(LimitResource),
    /// A limit exceeded the immutable implementation hard cap.
    AboveHardCap(LimitResource),
}

impl fmt::Display for LimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero(resource) => write!(formatter, "{resource:?} limit must be nonzero"),
            Self::AboveHardCap(resource) => {
                write!(formatter, "{resource:?} limit exceeds the bridge hard cap")
            }
        }
    }
}

impl Error for LimitError {}

/// Caller-configurable resource limits bounded by immutable hard caps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BridgeLimits {
    max_canonical_bytes: usize,
    max_shell_operations: usize,
}

impl BridgeLimits {
    /// Creates nonzero limits that do not exceed the implementation hard caps.
    pub const fn new(
        max_canonical_bytes: usize,
        max_shell_operations: usize,
    ) -> Result<Self, LimitError> {
        if max_canonical_bytes == 0 {
            return Err(LimitError::Zero(LimitResource::CanonicalBytes));
        }
        if max_canonical_bytes > HARD_MAX_CANONICAL_BYTES {
            return Err(LimitError::AboveHardCap(LimitResource::CanonicalBytes));
        }
        if max_shell_operations == 0 {
            return Err(LimitError::Zero(LimitResource::ShellOperations));
        }
        if max_shell_operations > HARD_MAX_SHELL_OPERATIONS {
            return Err(LimitError::AboveHardCap(LimitResource::ShellOperations));
        }
        Ok(Self {
            max_canonical_bytes,
            max_shell_operations,
        })
    }

    /// Returns the configured canonical-byte limit.
    pub const fn max_canonical_bytes(self) -> usize {
        self.max_canonical_bytes
    }

    /// Returns the configured shell-operation limit.
    pub const fn max_shell_operations(self) -> usize {
        self.max_shell_operations
    }
}

impl Default for BridgeLimits {
    fn default() -> Self {
        Self {
            max_canonical_bytes: HARD_MAX_CANONICAL_BYTES,
            max_shell_operations: HARD_MAX_SHELL_OPERATIONS,
        }
    }
}

/// One bounded summary of a semantic KIR verification failure.
///
/// Diagnostic messages are intentionally not retained at this boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticKirError {
    diagnostic_count: usize,
    first_code: Option<DiagnosticCode>,
}

impl SemanticKirError {
    /// Returns the number of deterministic KIR diagnostics.
    pub const fn diagnostic_count(self) -> usize {
        self.diagnostic_count
    }

    /// Returns the first deterministic diagnostic code, if one was reported.
    pub const fn first_code(self) -> Option<DiagnosticCode> {
        self.first_code
    }
}

impl fmt::Display for SemanticKirError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "KIR semantic verification failed with {} diagnostic(s)",
            self.diagnostic_count
        )
    }
}

impl Error for SemanticKirError {}

/// A bridge metadata field with a fixed meaning and type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataField {
    /// Fixed bridge schema marker.
    Schema,
    /// Unchanged canonical KIR bytes.
    CanonicalBytes,
    /// Redundant KIR wire version.
    WireVersion,
    /// Redundant KIR module identity.
    ModuleIdentity,
    /// Fixed presentation-only Pliron module symbol.
    ModuleSymbol,
}

/// A concrete shell operation required by the deterministic projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellOperationKind {
    /// `kernel.algorithm_root` with the exact KIR launch rank.
    KernelAlgorithm,
    /// `gpu.hierarchy_id` with the `Grid` discriminant.
    GpuGridHierarchy,
}

/// Fail-closed bridge errors.
#[derive(Debug)]
pub enum BridgeError {
    /// Caller configuration is invalid.
    InvalidLimits(LimitError),
    /// Canonical bytes exceed the active caller limit.
    CanonicalBytesLimit {
        /// Actual payload length.
        actual: usize,
        /// Active maximum payload length.
        max: usize,
    },
    /// The projected shell operation count exceeds the active caller limit.
    ShellOperationsLimit {
        /// Actual or required operation count.
        actual: usize,
        /// Active maximum operation count.
        max: usize,
    },
    /// Canonical KIR encoding failed without producing a bridge envelope.
    Encode(KernelIrEncodeError),
    /// Canonical KIR decoding or canonicality checking failed.
    Decode(KernelIrDecodeError),
    /// Canonical wire bytes decoded but failed semantic KIR verification.
    InvalidKir(SemanticKirError),
    /// Explicit `dialect-kernel` registration failed closed.
    KernelRegistration(dialect_kernel::RegistrationError),
    /// Explicit `dialect-gpu` registration failed closed.
    GpuRegistration(dialect_gpu::RegistrationError),
    /// Recovering without the canonical payload would require lossy conversion.
    LossyConversion {
        /// Required field that was absent.
        missing: MetadataField,
    },
    /// A required bridge field was absent.
    MissingMetadata(MetadataField),
    /// A field existed under the right key with the wrong Pliron attribute type.
    MetadataTypeConfusion(MetadataField),
    /// Redundant metadata disagreed with the canonical record or schema.
    MetadataConflict(MetadataField),
    /// Metadata carried an unknown KIR version discriminant.
    UnknownVersion(u16),
    /// The envelope had extra metadata or an unexpected metadata cardinality.
    UnexpectedMetadata,
    /// Pliron structure or local verification was malformed.
    MalformedShell,
    /// The shell had a duplicate, omitted, or extra operation.
    ShellOperationCount {
        /// Exact operation count derived from canonical KIR.
        expected: usize,
        /// Observed top-level operation count.
        actual: usize,
    },
    /// A shell operation had the wrong concrete type, order, or discriminant.
    ShellOperationConflict {
        /// Zero-based top-level operation index.
        index: usize,
        /// Required operation kind at that index.
        expected: ShellOperationKind,
    },
    /// A caller-supplied expected canonical record was substituted.
    RecordSubstitution,
    /// Bounded count arithmetic overflowed.
    ArithmeticOverflow,
}

impl fmt::Display for BridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits(error) => error.fmt(formatter),
            Self::CanonicalBytesLimit { actual, max } => {
                write!(
                    formatter,
                    "canonical KIR has {actual} bytes; maximum is {max}"
                )
            }
            Self::ShellOperationsLimit { actual, max } => {
                write!(formatter, "shell has {actual} operations; maximum is {max}")
            }
            Self::Encode(error) => write!(formatter, "canonical KIR encoding failed: {error}"),
            Self::Decode(error) => write!(formatter, "canonical KIR decoding failed: {error}"),
            Self::InvalidKir(error) => error.fmt(formatter),
            Self::KernelRegistration(error) => {
                write!(formatter, "kernel dialect registration failed: {error}")
            }
            Self::GpuRegistration(error) => {
                write!(formatter, "GPU dialect registration failed: {error}")
            }
            Self::LossyConversion { missing } => {
                write!(formatter, "lossless recovery requires {missing:?}")
            }
            Self::MissingMetadata(field) => write!(formatter, "missing {field:?} metadata"),
            Self::MetadataTypeConfusion(field) => {
                write!(formatter, "{field:?} metadata has the wrong attribute type")
            }
            Self::MetadataConflict(field) => {
                write!(formatter, "{field:?} metadata conflicts with canonical KIR")
            }
            Self::UnknownVersion(version) => write!(formatter, "unknown KIR version {version}"),
            Self::UnexpectedMetadata => formatter.write_str("unexpected bridge metadata"),
            Self::MalformedShell => formatter.write_str("malformed Pliron bridge shell"),
            Self::ShellOperationCount { expected, actual } => write!(
                formatter,
                "shell operation count is {actual}; canonical projection requires {expected}"
            ),
            Self::ShellOperationConflict { index, expected } => write!(
                formatter,
                "shell operation {index} conflicts with required {expected:?} projection"
            ),
            Self::RecordSubstitution => {
                formatter.write_str("recovered canonical KIR is not the expected record")
            }
            Self::ArithmeticOverflow => formatter.write_str("bridge count arithmetic overflow"),
        }
    }
}

impl Error for BridgeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidLimits(error) => Some(error),
            Self::Encode(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::InvalidKir(error) => Some(error),
            Self::KernelRegistration(error) => Some(error),
            Self::GpuRegistration(error) => Some(error),
            _ => None,
        }
    }
}

impl From<LimitError> for BridgeError {
    fn from(error: LimitError) -> Self {
        Self::InvalidLimits(error)
    }
}

/// Validated canonical KIR together with its exact original bytes.
///
/// The decoded module is an inspection view. [`Self::canonical_bytes`] remains
/// the durable identity-bearing representation returned by recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalKirRecord {
    version: KirVersion,
    canonical_bytes: Vec<u8>,
    module: Module,
}

impl CanonicalKirRecord {
    /// Validates bounded canonical V1-V5 bytes and KIR semantic invariants.
    pub fn parse(bytes: &[u8], limits: BridgeLimits) -> Result<Self, BridgeError> {
        check_canonical_byte_limit(bytes.len(), limits)?;
        let module = decode_module_v5(bytes).map_err(BridgeError::Decode)?;
        let version = wire_version(bytes)?;
        verify_semantic_kir(&module)?;
        check_projection_count(module.kernels.len(), limits)?;
        Ok(Self {
            version,
            canonical_bytes: bytes.to_vec(),
            module,
        })
    }

    /// Encodes and validates a KIR module at one exact frozen wire version.
    pub fn from_module(
        module: &Module,
        version: KirVersion,
        limits: BridgeLimits,
    ) -> Result<Self, BridgeError> {
        verify_semantic_kir(module)?;
        check_projection_count(module.kernels.len(), limits)?;
        let bytes = match version {
            KirVersion::V1 => encode_module_v1(module),
            KirVersion::V2 => encode_module_v2(module),
            KirVersion::V3 => encode_module_v3(module),
            KirVersion::V4 => encode_module_v4(module),
            KirVersion::V5 => encode_module_v5(module),
        }
        .map_err(BridgeError::Encode)?;
        Self::parse(&bytes, limits)
    }

    /// Returns the exact frozen wire-version discriminant.
    pub const fn version(&self) -> KirVersion {
        self.version
    }

    /// Returns the exact canonical bytes supplied to or produced by the KIR codec.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the exact canonical KIR module identity.
    pub fn module_identity(&self) -> &str {
        self.module.id.as_str()
    }

    /// Returns the decoded, semantically verified KIR inspection view.
    pub const fn module(&self) -> &Module {
        &self.module
    }

    /// Creates a detached Pliron envelope and deterministic shell projection.
    pub fn project_to_pliron(
        &self,
        context: &mut Context,
        limits: BridgeLimits,
    ) -> Result<ModuleOp, BridgeError> {
        check_canonical_byte_limit(self.canonical_bytes.len(), limits)?;
        check_projection_count(self.module.kernels.len(), limits)?;
        register_shells(context)?;

        let symbol: Identifier = BRIDGE_MODULE_SYMBOL
            .try_into()
            .expect("fixed bridge module symbol is a valid Pliron identifier");
        let shell = ModuleOp::new(context, symbol);
        install_metadata(context, &shell, self);

        for kernel in &self.module.kernels {
            let rank = u32::from(kernel.domain.rank());
            let algorithm =
                AlgorithmOp::new(context, rank).map_err(|_| BridgeError::MalformedShell)?;
            shell.append_operation(context, algorithm.get_operation(), 0);

            let hierarchy = HierarchyIdOp::new(context, HierarchyAttr::Grid);
            shell.append_operation(context, hierarchy.get_operation(), 0);
        }
        Ok(shell)
    }
}

/// Validates canonical bytes and creates their Pliron bridge envelope.
pub fn import_canonical(
    context: &mut Context,
    bytes: &[u8],
    limits: BridgeLimits,
) -> Result<ModuleOp, BridgeError> {
    CanonicalKirRecord::parse(bytes, limits)?.project_to_pliron(context, limits)
}

/// Recovers exact canonical KIR bytes from a bridge envelope.
///
/// This function catches malformed Pliron traversal failures and returns a
/// fail-closed error. It never reconstructs KIR from shell presentation.
pub fn recover_canonical(
    context: &Context,
    shell: &ModuleOp,
    limits: BridgeLimits,
) -> Result<CanonicalKirRecord, BridgeError> {
    std::panic::catch_unwind(AssertUnwindSafe(|| {
        recover_canonical_inner(context, shell, limits)
    }))
    .unwrap_or(Err(BridgeError::MalformedShell))
}

/// Recovers a bridge envelope and rejects substitution of an expected record.
pub fn recover_exact(
    context: &Context,
    shell: &ModuleOp,
    expected: &CanonicalKirRecord,
    limits: BridgeLimits,
) -> Result<CanonicalKirRecord, BridgeError> {
    let recovered = recover_canonical(context, shell, limits)?;
    if recovered.version != expected.version
        || recovered.module.id != expected.module.id
        || recovered.canonical_bytes != expected.canonical_bytes
    {
        return Err(BridgeError::RecordSubstitution);
    }
    Ok(recovered)
}

fn verify_semantic_kir(module: &Module) -> Result<(), BridgeError> {
    verify_module(module).map_err(|errors| {
        BridgeError::InvalidKir(SemanticKirError {
            diagnostic_count: errors.diagnostics().len(),
            first_code: errors
                .diagnostics()
                .first()
                .map(|diagnostic| diagnostic.code),
        })
    })
}

fn check_canonical_byte_limit(length: usize, limits: BridgeLimits) -> Result<(), BridgeError> {
    if length > limits.max_canonical_bytes {
        return Err(BridgeError::CanonicalBytesLimit {
            actual: length,
            max: limits.max_canonical_bytes,
        });
    }
    Ok(())
}

fn projected_operation_count(kernel_count: usize) -> Result<usize, BridgeError> {
    kernel_count
        .checked_mul(2)
        .ok_or(BridgeError::ArithmeticOverflow)
}

fn check_projection_count(kernel_count: usize, limits: BridgeLimits) -> Result<usize, BridgeError> {
    let count = projected_operation_count(kernel_count)?;
    if count > limits.max_shell_operations {
        return Err(BridgeError::ShellOperationsLimit {
            actual: count,
            max: limits.max_shell_operations,
        });
    }
    Ok(count)
}

fn wire_version(bytes: &[u8]) -> Result<KirVersion, BridgeError> {
    let raw = bytes
        .get(8..10)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(BridgeError::Decode(KernelIrDecodeError::Truncated))?;
    KirVersion::from_wire_value(raw).ok_or(BridgeError::UnknownVersion(raw))
}

fn register_shells(context: &mut Context) -> Result<(), BridgeError> {
    let kernel_name = DialectName::try_new(dialect_kernel::DIALECT_NAME)
        .expect("fixed kernel dialect name is valid");
    dialect_kernel::register_dialect(context, &kernel_name)
        .map_err(BridgeError::KernelRegistration)?;
    dialect_gpu::register_dialect(context).map_err(BridgeError::GpuRegistration)?;
    Ok(())
}

fn metadata_key(value: &'static str) -> Identifier {
    value
        .try_into()
        .expect("fixed bridge metadata key is a valid Pliron identifier")
}

fn install_metadata(context: &Context, shell: &ModuleOp, record: &CanonicalKirRecord) {
    let binding = shell.get_operation();
    let mut operation = binding.deref_mut(context);
    operation.attributes.set(
        metadata_key(SCHEMA_ATTR_KEY),
        BytesAttr::new(BRIDGE_SCHEMA_V1.to_vec()),
    );
    operation.attributes.set(
        metadata_key(CANONICAL_BYTES_ATTR_KEY),
        BytesAttr::new(record.canonical_bytes.clone()),
    );
    operation.attributes.set(
        metadata_key(WIRE_VERSION_ATTR_KEY),
        BytesAttr::new(record.version.wire_value().to_le_bytes().to_vec()),
    );
    operation.attributes.set(
        metadata_key(MODULE_IDENTITY_ATTR_KEY),
        StringAttr::new(record.module.id.as_str().to_owned()),
    );
}

fn recover_canonical_inner(
    context: &Context,
    shell: &ModuleOp,
    limits: BridgeLimits,
) -> Result<CanonicalKirRecord, BridgeError> {
    if !Operation::is_op::<ModuleOp>(shell.get_operation(), context) {
        return Err(BridgeError::MalformedShell);
    }

    let binding = shell.get_operation();
    let operation = binding.deref(context);
    let schema = required_metadata::<BytesAttr>(
        &operation.attributes,
        SCHEMA_ATTR_KEY,
        MetadataField::Schema,
    )?;
    let canonical = required_canonical_bytes(&operation.attributes)?;
    let version = required_metadata::<BytesAttr>(
        &operation.attributes,
        WIRE_VERSION_ATTR_KEY,
        MetadataField::WireVersion,
    )?;
    let identity = required_metadata::<StringAttr>(
        &operation.attributes,
        MODULE_IDENTITY_ATTR_KEY,
        MetadataField::ModuleIdentity,
    )?;
    let symbol = required_symbol(&operation.attributes)?;

    if operation.attributes.0.len() != 5 {
        return Err(BridgeError::UnexpectedMetadata);
    }
    if schema.as_ref().as_slice() != BRIDGE_SCHEMA_V1 {
        return Err(BridgeError::MetadataConflict(MetadataField::Schema));
    }
    if symbol.as_ref() != BRIDGE_MODULE_SYMBOL {
        return Err(BridgeError::MetadataConflict(MetadataField::ModuleSymbol));
    }
    check_canonical_byte_limit(canonical.as_ref().len(), limits)?;

    let stored_version = decode_stored_version(version.as_ref())?;
    let record = CanonicalKirRecord::parse(canonical.as_ref(), limits)?;
    if stored_version != record.version {
        return Err(BridgeError::MetadataConflict(MetadataField::WireVersion));
    }
    if identity.as_str() != record.module.id.as_str() {
        return Err(BridgeError::MetadataConflict(MetadataField::ModuleIdentity));
    }
    drop(operation);

    preflight_and_verify_shell(context, shell, &record, limits)?;
    Ok(record)
}

fn required_canonical_bytes(attributes: &AttributeDict) -> Result<&BytesAttr, BridgeError> {
    let key = metadata_key(CANONICAL_BYTES_ATTR_KEY);
    let Some(attribute) = attributes.0.get(&key) else {
        return Err(BridgeError::LossyConversion {
            missing: MetadataField::CanonicalBytes,
        });
    };
    attribute
        .downcast_ref::<BytesAttr>()
        .ok_or(BridgeError::MetadataTypeConfusion(
            MetadataField::CanonicalBytes,
        ))
}

fn required_metadata<'a, T: Attribute + 'static>(
    attributes: &'a AttributeDict,
    key: &'static str,
    field: MetadataField,
) -> Result<&'a T, BridgeError> {
    let key = metadata_key(key);
    let Some(attribute) = attributes.0.get(&key) else {
        return Err(BridgeError::MissingMetadata(field));
    };
    attribute
        .downcast_ref::<T>()
        .ok_or(BridgeError::MetadataTypeConfusion(field))
}

fn required_symbol(attributes: &AttributeDict) -> Result<&Identifier, BridgeError> {
    let Some(attribute) = attributes.0.get(&*ATTR_KEY_SYM_NAME) else {
        return Err(BridgeError::MissingMetadata(MetadataField::ModuleSymbol));
    };
    attribute
        .downcast_ref::<IdentifierAttr>()
        .map(AsRef::as_ref)
        .ok_or(BridgeError::MetadataTypeConfusion(
            MetadataField::ModuleSymbol,
        ))
}

fn decode_stored_version(bytes: &[u8]) -> Result<KirVersion, BridgeError> {
    let raw: [u8; 2] = bytes
        .try_into()
        .map_err(|_| BridgeError::MetadataConflict(MetadataField::WireVersion))?;
    let raw = u16::from_le_bytes(raw);
    KirVersion::from_wire_value(raw).ok_or(BridgeError::UnknownVersion(raw))
}

fn preflight_and_verify_shell(
    context: &Context,
    shell: &ModuleOp,
    record: &CanonicalKirRecord,
    limits: BridgeLimits,
) -> Result<(), BridgeError> {
    let body = shell.get_body(context, 0);
    let actual = body
        .deref(context)
        .iter(context)
        .take(limits.max_shell_operations + 1)
        .count();
    if actual > limits.max_shell_operations {
        return Err(BridgeError::ShellOperationsLimit {
            actual,
            max: limits.max_shell_operations,
        });
    }
    let expected = check_projection_count(record.module.kernels.len(), limits)?;
    if actual != expected {
        return Err(BridgeError::ShellOperationCount { expected, actual });
    }

    if verify_operation(shell.get_operation(), context).is_err() {
        return Err(BridgeError::MalformedShell);
    }

    let body_ref = body.deref(context);
    let mut operations = body_ref.iter(context);
    for (kernel_index, kernel) in record.module.kernels.iter().enumerate() {
        let algorithm_index = kernel_index * 2;
        let algorithm_ptr = operations.next().ok_or(BridgeError::ShellOperationCount {
            expected,
            actual: algorithm_index,
        })?;
        let algorithm = Operation::get_op::<AlgorithmOp>(algorithm_ptr, context).ok_or(
            BridgeError::ShellOperationConflict {
                index: algorithm_index,
                expected: ShellOperationKind::KernelAlgorithm,
            },
        )?;
        let expected_rank = u32::from(kernel.domain.rank());
        let rank = algorithm
            .iteration_domain(context)
            .map(|domain: IterationDomainAttr| domain.rank());
        if rank != Some(expected_rank) {
            return Err(BridgeError::ShellOperationConflict {
                index: algorithm_index,
                expected: ShellOperationKind::KernelAlgorithm,
            });
        }

        let hierarchy_index = algorithm_index + 1;
        let hierarchy_ptr = operations.next().ok_or(BridgeError::ShellOperationCount {
            expected,
            actual: hierarchy_index,
        })?;
        let hierarchy = Operation::get_op::<HierarchyIdOp>(hierarchy_ptr, context).ok_or(
            BridgeError::ShellOperationConflict {
                index: hierarchy_index,
                expected: ShellOperationKind::GpuGridHierarchy,
            },
        )?;
        if hierarchy
            .get_attr_gpu_hierarchy_id_hierarchy(context)
            .is_none_or(|value| *value != HierarchyAttr::Grid)
        {
            return Err(BridgeError::ShellOperationConflict {
                index: hierarchy_index,
                expected: ShellOperationKind::GpuGridHierarchy,
            });
        }
    }
    Ok(())
}
