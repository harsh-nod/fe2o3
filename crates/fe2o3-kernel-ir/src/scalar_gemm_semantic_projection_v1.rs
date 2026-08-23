//! Bounded, exhaustive projection of the canonical scalar GEMM V1 KIR graph.
//!
//! This module is a reviewed Rust TCB translation. It checks and records every
//! field of the closed scalar GEMM graph, but it is not a Verus proof that KIR
//! execution refines a source model.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    AccessMode, AddressSpace, Axis, BasicBlock, BinaryOp, CastKind, ComparePredicate, Constant,
    Function, FunctionBody, FunctionRole, IndexKind, IntrinsicKind, IntrinsicOperation, Kernel,
    KernelIrDecodeError, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation,
    OperationKind, ScalarGemmTargetRequirementsV1, ScalarGemmV1Error, ScalarType, Signature,
    TargetCapability, Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrErrorV5,
    VerifiedCanonicalKernelIrIdentityV5, VerifiedCanonicalKernelIrV5, WaveWidth, WorkgroupSize,
    decode_module_v5, scalar_gemm_v1_module, verify_scalar_gemm_v1_module,
};

/// Version of the closed scalar GEMM semantic-projection policy.
pub const SCALAR_GEMM_SEMANTIC_PROJECTION_POLICY_V1: u16 = 1;

/// Hard upper bound for the canonical scalar GEMM projection preimage.
pub const MAX_SCALAR_GEMM_SEMANTIC_PROJECTION_BYTES_V1: usize = 16 * 1024;

const PROJECTION_SCHEMA_V1: &[u8] = b"fe2o3.scalar-gemm.semantic-projection.v1";
const IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/SCALAR-GEMM-SEMANTIC-PROJECTION/V1\0";

/// Identity of one exact canonical scalar GEMM semantic projection.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScalarGemmSemanticProjectionIdentityV1([u8; 32]);

impl ScalarGemmSemanticProjectionIdentityV1 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Move-only owner of a checked scalar GEMM V1 projection.
///
/// The private retained V5 bytes permit full custody-boundary revalidation.
/// Constructing this owner grants no compiler, artifact, or runtime authority.
#[derive(Debug, Eq, PartialEq)]
pub struct CheckedScalarGemmSemanticProjectionV1 {
    canonical_kir_v5: Vec<u8>,
    source_kir_identity: VerifiedCanonicalKernelIrIdentityV5,
    canonical_token_preimage: Vec<u8>,
    identity: ScalarGemmSemanticProjectionIdentityV1,
}

impl CheckedScalarGemmSemanticProjectionV1 {
    /// Canonicalizes, decodes, and verifies a caller-provided decoded module as
    /// exact V5 before exhaustively projecting the scalar GEMM graph.
    pub fn from_module(module: Module) -> Result<Self, ScalarGemmSemanticProjectionErrorV1> {
        let exact = VerifiedCanonicalKernelIrV5::from_module(module)
            .map_err(ScalarGemmSemanticProjectionErrorV1::CanonicalKir)?;
        Self::from_exact_v5_owner(exact)
    }

    /// Decodes and verifies caller-provided bytes as exact canonical V5 before
    /// exhaustively projecting the scalar GEMM graph.
    pub fn from_canonical_kir_v5_bytes(
        canonical_kir_v5: Vec<u8>,
    ) -> Result<Self, ScalarGemmSemanticProjectionErrorV1> {
        let exact = VerifiedCanonicalKernelIrV5::from_canonical_bytes(canonical_kir_v5)
            .map_err(ScalarGemmSemanticProjectionErrorV1::CanonicalKir)?;
        Self::from_exact_v5_owner(exact)
    }

    pub fn canonical_kir_v5(&self) -> &[u8] {
        &self.canonical_kir_v5
    }

    pub const fn source_kir_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV5 {
        &self.source_kir_identity
    }

    /// Deterministic bounded typed-token preimage for the complete graph.
    pub fn canonical_token_preimage(&self) -> &[u8] {
        &self.canonical_token_preimage
    }

    pub const fn identity(&self) -> &ScalarGemmSemanticProjectionIdentityV1 {
        &self.identity
    }

    /// Replays exact V5 decoding, KIR verification, exhaustive projection, and
    /// both retained identities.
    pub fn revalidate(&self) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
        let rebuilt = Self::from_canonical_kir_v5_bytes(self.canonical_kir_v5.clone())?;
        if rebuilt.source_kir_identity != self.source_kir_identity {
            return Err(ScalarGemmSemanticProjectionErrorV1::SourceIdentityMismatch);
        }
        if rebuilt.canonical_token_preimage != self.canonical_token_preimage {
            return Err(ScalarGemmSemanticProjectionErrorV1::ProjectionPreimageMismatch);
        }
        if rebuilt.identity != self.identity {
            return Err(ScalarGemmSemanticProjectionErrorV1::IdentityMismatch);
        }
        Ok(())
    }

    pub fn into_canonical_token_preimage(self) -> Vec<u8> {
        self.canonical_token_preimage
    }

    /// This checked Rust translation is part of the reviewed TCB, not a Verus
    /// semantic-refinement proof.
    pub const fn is_verus_semantic_refinement_proof(&self) -> bool {
        false
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }

    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }

    fn from_exact_v5_owner(
        exact: VerifiedCanonicalKernelIrV5,
    ) -> Result<Self, ScalarGemmSemanticProjectionErrorV1> {
        exact
            .revalidate()
            .map_err(ScalarGemmSemanticProjectionErrorV1::CanonicalKir)?;
        let source_kir_identity = *exact.identity();
        let canonical_kir_v5 = exact.into_canonical_bytes();
        let module = decode_module_v5(&canonical_kir_v5)
            .map_err(ScalarGemmSemanticProjectionErrorV1::DecodeAfterValidation)?;

        let canonical_token_preimage = project_module(&module)?;
        let expected_token_preimage = project_module(&scalar_gemm_v1_module())?;
        if canonical_token_preimage != expected_token_preimage {
            return Err(ScalarGemmSemanticProjectionErrorV1::NonCanonicalProjection);
        }

        verify_scalar_gemm_v1_module(
            &module,
            ScalarGemmTargetRequirementsV1::gfx942_xnack_minus_cov6(),
        )
        .map_err(ScalarGemmSemanticProjectionErrorV1::ScalarProfile)?;

        let identity = projection_identity(&canonical_token_preimage);
        Ok(Self {
            canonical_kir_v5,
            source_kir_identity,
            canonical_token_preimage,
            identity,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScalarGemmSemanticProjectionErrorV1 {
    CanonicalKir(VerifiedCanonicalKernelIrErrorV5),
    DecodeAfterValidation(KernelIrDecodeError),
    ScalarProfile(ScalarGemmV1Error),
    UnsupportedField(&'static str),
    ProjectionTooLarge { attempted: usize, maximum: usize },
    NonCanonicalProjection,
    SourceIdentityMismatch,
    ProjectionPreimageMismatch,
    IdentityMismatch,
}

impl fmt::Display for ScalarGemmSemanticProjectionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CanonicalKir(error) => write!(formatter, "invalid canonical KIR V5: {error}"),
            Self::DecodeAfterValidation(error) => {
                write!(formatter, "validated KIR V5 no longer decodes: {error}")
            }
            Self::ScalarProfile(error) => error.fmt(formatter),
            Self::UnsupportedField(field) => {
                write!(
                    formatter,
                    "unsupported scalar GEMM projection field: {field}"
                )
            }
            Self::ProjectionTooLarge { attempted, maximum } => write!(
                formatter,
                "scalar GEMM projection is {attempted} bytes, maximum is {maximum}"
            ),
            Self::NonCanonicalProjection => formatter.write_str(
                "projected graph does not match the exhaustive canonical scalar GEMM projection",
            ),
            Self::SourceIdentityMismatch => {
                formatter.write_str("retained canonical KIR V5 identity mismatch")
            }
            Self::ProjectionPreimageMismatch => {
                formatter.write_str("retained scalar GEMM projection preimage mismatch")
            }
            Self::IdentityMismatch => {
                formatter.write_str("retained scalar GEMM projection identity mismatch")
            }
        }
    }
}

impl Error for ScalarGemmSemanticProjectionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CanonicalKir(error) => Some(error),
            Self::DecodeAfterValidation(error) => Some(error),
            Self::ScalarProfile(error) => Some(error),
            Self::UnsupportedField(_)
            | Self::ProjectionTooLarge { .. }
            | Self::NonCanonicalProjection
            | Self::SourceIdentityMismatch
            | Self::ProjectionPreimageMismatch
            | Self::IdentityMismatch => None,
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy)]
enum Token {
    Schema = 1,
    Policy = 2,
    ModuleId = 3,
    ModuleCapabilities = 4,
    Functions = 5,
    Kernels = 6,
    CapabilityKind = 7,
    CapabilityArgument = 8,
    CapabilityNamespace = 9,
    CapabilityName = 10,
    FunctionId = 11,
    FunctionRole = 12,
    SignatureParameters = 13,
    SignatureResults = 14,
    FunctionBodyPresent = 15,
    FunctionParameters = 16,
    FunctionBlocks = 17,
    FunctionCapabilities = 18,
    BlockId = 19,
    BlockParameters = 20,
    BlockOperations = 21,
    TerminatorPresent = 22,
    ValueId = 23,
    TypeKind = 24,
    ScalarType = 25,
    AddressSpace = 26,
    AccessMode = 27,
    OperationResults = 28,
    OperationKind = 29,
    ConstantKind = 30,
    ConstantValue = 31,
    IntrinsicKind = 32,
    IndexKind = 33,
    Axis = 34,
    BinaryOp = 35,
    ComparePredicate = 36,
    CastKind = 37,
    MemoryAddressSpace = 38,
    MemoryAlignment = 39,
    MemoryVolatile = 40,
    TerminatorKind = 41,
    TerminatorArguments = 42,
    ThenArguments = 43,
    ElseArguments = 44,
    KernelId = 45,
    KernelEntry = 46,
    LaunchDomain = 47,
    LaunchExtent = 48,
    LaunchStaticExtent = 49,
    WorkgroupPresent = 50,
    WorkgroupX = 51,
    WorkgroupY = 52,
    WorkgroupZ = 53,
    KernelCapabilities = 54,
}

struct ProjectionWriter {
    bytes: Vec<u8>,
}

impl ProjectionWriter {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn token(
        &mut self,
        token: Token,
        payload: &[u8],
    ) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
        let attempted = self
            .bytes
            .len()
            .checked_add(1 + size_of::<u32>())
            .and_then(|length| length.checked_add(payload.len()))
            .unwrap_or(usize::MAX);
        if attempted > MAX_SCALAR_GEMM_SEMANTIC_PROJECTION_BYTES_V1
            || payload.len() > u32::MAX as usize
        {
            return Err(ScalarGemmSemanticProjectionErrorV1::ProjectionTooLarge {
                attempted,
                maximum: MAX_SCALAR_GEMM_SEMANTIC_PROJECTION_BYTES_V1,
            });
        }
        self.bytes.push(token as u8);
        self.bytes
            .extend_from_slice(&(payload.len() as u32).to_le_bytes());
        self.bytes.extend_from_slice(payload);
        Ok(())
    }

    fn u8(&mut self, token: Token, value: u8) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
        self.token(token, &[value])
    }

    fn u16(&mut self, token: Token, value: u16) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
        self.token(token, &value.to_le_bytes())
    }

    fn u32(&mut self, token: Token, value: u32) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
        self.token(token, &value.to_le_bytes())
    }

    fn count(
        &mut self,
        token: Token,
        value: usize,
    ) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
        let value = u32::try_from(value).map_err(|_| {
            ScalarGemmSemanticProjectionErrorV1::ProjectionTooLarge {
                attempted: usize::MAX,
                maximum: MAX_SCALAR_GEMM_SEMANTIC_PROJECTION_BYTES_V1,
            }
        })?;
        self.u32(token, value)
    }

    fn boolean(
        &mut self,
        token: Token,
        value: bool,
    ) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
        self.u8(token, u8::from(value))
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

fn project_module(module: &Module) -> Result<Vec<u8>, ScalarGemmSemanticProjectionErrorV1> {
    let Module {
        id,
        functions,
        kernels,
        required_capabilities,
    } = module;
    let mut writer = ProjectionWriter::new();
    writer.token(Token::Schema, PROJECTION_SCHEMA_V1)?;
    writer.u16(Token::Policy, SCALAR_GEMM_SEMANTIC_PROJECTION_POLICY_V1)?;
    writer.token(Token::ModuleId, id.as_str().as_bytes())?;
    project_capabilities(
        &mut writer,
        Token::ModuleCapabilities,
        required_capabilities,
    )?;
    writer.count(Token::Functions, functions.len())?;
    for function in functions {
        project_function(&mut writer, function)?;
    }
    writer.count(Token::Kernels, kernels.len())?;
    for kernel in kernels {
        project_kernel(&mut writer, kernel)?;
    }
    Ok(writer.finish())
}

fn project_function(
    writer: &mut ProjectionWriter,
    function: &Function,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    let Function {
        id,
        signature,
        role,
        body,
        required_capabilities,
    } = function;
    writer.token(Token::FunctionId, id.as_str().as_bytes())?;
    project_signature(writer, signature)?;
    writer.u8(Token::FunctionRole, function_role_tag(*role))?;
    writer.boolean(Token::FunctionBodyPresent, body.is_some())?;
    if let Some(body) = body {
        project_function_body(writer, body)?;
    }
    project_capabilities(writer, Token::FunctionCapabilities, required_capabilities)
}

fn project_signature(
    writer: &mut ProjectionWriter,
    signature: &Signature,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    let Signature {
        parameters,
        results,
    } = signature;
    writer.count(Token::SignatureParameters, parameters.len())?;
    for ty in parameters {
        project_type(writer, ty)?;
    }
    writer.count(Token::SignatureResults, results.len())?;
    for ty in results {
        project_type(writer, ty)?;
    }
    Ok(())
}

fn project_function_body(
    writer: &mut ProjectionWriter,
    body: &FunctionBody,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    let FunctionBody { parameters, blocks } = body;
    project_value_ids(writer, Token::FunctionParameters, parameters)?;
    writer.count(Token::FunctionBlocks, blocks.len())?;
    for block in blocks {
        project_block(writer, block)?;
    }
    Ok(())
}

fn project_block(
    writer: &mut ProjectionWriter,
    block: &BasicBlock,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    let BasicBlock {
        id,
        parameters,
        operations,
        terminator,
    } = block;
    writer.u32(Token::BlockId, id.0)?;
    writer.count(Token::BlockParameters, parameters.len())?;
    for parameter in parameters {
        project_value_def(writer, parameter)?;
    }
    writer.count(Token::BlockOperations, operations.len())?;
    for operation in operations {
        project_operation(writer, operation)?;
    }
    writer.boolean(Token::TerminatorPresent, terminator.is_some())?;
    if let Some(terminator) = terminator {
        project_terminator(writer, terminator)?;
    }
    Ok(())
}

fn project_value_def(
    writer: &mut ProjectionWriter,
    definition: &ValueDef,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    let ValueDef { id, ty } = definition;
    writer.u32(Token::ValueId, id.0)?;
    project_type(writer, ty)
}

fn project_operation(
    writer: &mut ProjectionWriter,
    operation: &Operation,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    let Operation { results, kind } = operation;
    writer.count(Token::OperationResults, results.len())?;
    for result in results {
        project_value_def(writer, result)?;
    }
    project_operation_kind(writer, kind)
}

fn project_operation_kind(
    writer: &mut ProjectionWriter,
    kind: &OperationKind,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    match kind {
        OperationKind::Constant(constant) => {
            writer.u8(Token::OperationKind, 1)?;
            project_constant(writer, constant)
        }
        OperationKind::Intrinsic(intrinsic) => {
            writer.u8(Token::OperationKind, 2)?;
            project_intrinsic(writer, intrinsic)
        }
        OperationKind::Binary { op, lhs, rhs } => {
            let Some(operator_tag) = binary_op_tag(*op) else {
                return Err(ScalarGemmSemanticProjectionErrorV1::UnsupportedField(
                    "checked binary operation",
                ));
            };
            writer.u8(Token::OperationKind, 3)?;
            writer.u8(Token::BinaryOp, operator_tag)?;
            project_value_id(writer, *lhs)?;
            project_value_id(writer, *rhs)
        }
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } => {
            writer.u8(Token::OperationKind, 4)?;
            writer.u8(Token::ComparePredicate, compare_predicate_tag(*predicate))?;
            project_value_id(writer, *lhs)?;
            project_value_id(writer, *rhs)
        }
        OperationKind::Cast { kind, value, to } => {
            writer.u8(Token::OperationKind, 5)?;
            writer.u8(Token::CastKind, cast_kind_tag(*kind))?;
            project_value_id(writer, *value)?;
            project_type(writer, to)
        }
        OperationKind::SliceData { slice } => {
            writer.u8(Token::OperationKind, 6)?;
            project_value_id(writer, *slice)
        }
        OperationKind::GetElementPointer { base, offset } => {
            writer.u8(Token::OperationKind, 7)?;
            project_value_id(writer, *base)?;
            project_value_id(writer, *offset)
        }
        OperationKind::Load { pointer, access } => {
            writer.u8(Token::OperationKind, 8)?;
            project_value_id(writer, *pointer)?;
            project_memory_access(writer, access)
        }
        OperationKind::Store {
            pointer,
            value,
            access,
        } => {
            writer.u8(Token::OperationKind, 9)?;
            project_value_id(writer, *pointer)?;
            project_value_id(writer, *value)?;
            project_memory_access(writer, access)
        }
        OperationKind::MemoryIntrinsic(_)
        | OperationKind::Unary { .. }
        | OperationKind::Select { .. }
        | OperationKind::Call { .. }
        | OperationKind::Alloca { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::Barrier(_)
        | OperationKind::Atomic(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::WorkgroupMemory(_)
        | OperationKind::Matrix(_)
        | OperationKind::Wave(_)
        | OperationKind::InlineAssembly(_) => Err(
            ScalarGemmSemanticProjectionErrorV1::UnsupportedField("operation kind"),
        ),
    }
}

fn project_constant(
    writer: &mut ProjectionWriter,
    constant: &Constant,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    let (kind, bytes): (u8, Vec<u8>) = match constant {
        Constant::Bool(value) => (1, vec![u8::from(*value)]),
        Constant::I8(value) => (2, value.to_le_bytes().to_vec()),
        Constant::I16(value) => (3, value.to_le_bytes().to_vec()),
        Constant::I32(value) => (4, value.to_le_bytes().to_vec()),
        Constant::I64(value) => (5, value.to_le_bytes().to_vec()),
        Constant::U8(value) => (6, value.to_le_bytes().to_vec()),
        Constant::U16(value) => (7, value.to_le_bytes().to_vec()),
        Constant::U32(value) => (8, value.to_le_bytes().to_vec()),
        Constant::U64(value) => (9, value.to_le_bytes().to_vec()),
        Constant::Index(value) => (10, value.to_le_bytes().to_vec()),
        Constant::F16Bits(value) => (11, value.to_le_bytes().to_vec()),
        Constant::Bf16Bits(value) => (12, value.to_le_bytes().to_vec()),
        Constant::F32Bits(value) => (13, value.to_le_bytes().to_vec()),
        Constant::F64Bits(value) => (14, value.to_le_bytes().to_vec()),
    };
    writer.u8(Token::ConstantKind, kind)?;
    writer.token(Token::ConstantValue, &bytes)
}

fn project_intrinsic(
    writer: &mut ProjectionWriter,
    intrinsic: &IntrinsicOperation,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    let IntrinsicOperation { kind, result_type } = intrinsic;
    match kind {
        IntrinsicKind::InvocationIndex { kind, axis } => {
            writer.u8(Token::IntrinsicKind, 1)?;
            writer.u8(Token::IndexKind, index_kind_tag(*kind))?;
            writer.u8(Token::Axis, axis_tag(*axis))?;
        }
        IntrinsicKind::LaunchExtent { axis } => {
            writer.u8(Token::IntrinsicKind, 2)?;
            writer.u8(Token::Axis, axis_tag(*axis))?;
        }
    }
    project_type(writer, result_type)
}

fn project_memory_access(
    writer: &mut ProjectionWriter,
    access: &MemoryAccess,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    let MemoryAccess {
        address_space,
        alignment,
        volatile,
    } = access;
    writer.u8(Token::MemoryAddressSpace, address_space_tag(*address_space))?;
    writer.u32(Token::MemoryAlignment, *alignment)?;
    writer.boolean(Token::MemoryVolatile, *volatile)
}

fn project_terminator(
    writer: &mut ProjectionWriter,
    terminator: &Terminator,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    match terminator {
        Terminator::Branch { target, arguments } => {
            writer.u8(Token::TerminatorKind, 1)?;
            writer.u32(Token::BlockId, target.0)?;
            project_value_ids(writer, Token::TerminatorArguments, arguments)
        }
        Terminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => {
            writer.u8(Token::TerminatorKind, 2)?;
            project_value_id(writer, *condition)?;
            writer.u32(Token::BlockId, then_target.0)?;
            project_value_ids(writer, Token::ThenArguments, then_arguments)?;
            writer.u32(Token::BlockId, else_target.0)?;
            project_value_ids(writer, Token::ElseArguments, else_arguments)
        }
        Terminator::Return { values } => {
            writer.u8(Token::TerminatorKind, 3)?;
            project_value_ids(writer, Token::TerminatorArguments, values)
        }
        Terminator::Switch { .. } | Terminator::IntegerSwitch { .. } | Terminator::Unreachable => {
            Err(ScalarGemmSemanticProjectionErrorV1::UnsupportedField(
                "terminator kind",
            ))
        }
    }
}

fn project_kernel(
    writer: &mut ProjectionWriter,
    kernel: &Kernel,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    let Kernel {
        id,
        entry,
        domain,
        workgroup_size,
        required_capabilities,
    } = kernel;
    writer.token(Token::KernelId, id.as_str().as_bytes())?;
    writer.token(Token::KernelEntry, entry.as_str().as_bytes())?;
    project_launch_domain(writer, domain)?;
    writer.boolean(Token::WorkgroupPresent, workgroup_size.is_some())?;
    if let Some(workgroup_size) = workgroup_size {
        let WorkgroupSize { x, y, z } = workgroup_size;
        writer.u32(Token::WorkgroupX, *x)?;
        writer.u32(Token::WorkgroupY, *y)?;
        writer.u32(Token::WorkgroupZ, *z)?;
    }
    project_capabilities(writer, Token::KernelCapabilities, required_capabilities)
}

fn project_launch_domain(
    writer: &mut ProjectionWriter,
    domain: &LaunchDomain,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    match domain {
        LaunchDomain::D1 { x } => {
            writer.u8(Token::LaunchDomain, 1)?;
            project_launch_extent(writer, *x)
        }
        LaunchDomain::D2 { x, y } => {
            writer.u8(Token::LaunchDomain, 2)?;
            project_launch_extent(writer, *x)?;
            project_launch_extent(writer, *y)
        }
        LaunchDomain::D3 { x, y, z } => {
            writer.u8(Token::LaunchDomain, 3)?;
            project_launch_extent(writer, *x)?;
            project_launch_extent(writer, *y)?;
            project_launch_extent(writer, *z)
        }
    }
}

fn project_launch_extent(
    writer: &mut ProjectionWriter,
    extent: LaunchExtent,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    match extent {
        LaunchExtent::Dynamic => writer.u8(Token::LaunchExtent, 1),
        LaunchExtent::Static(value) => {
            writer.u8(Token::LaunchExtent, 2)?;
            writer.u32(Token::LaunchStaticExtent, value)
        }
    }
}

fn project_capabilities(
    writer: &mut ProjectionWriter,
    count_token: Token,
    capabilities: &std::collections::BTreeSet<TargetCapability>,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    writer.count(count_token, capabilities.len())?;
    for capability in capabilities {
        match capability {
            TargetCapability::Float16 => writer.u8(Token::CapabilityKind, 1)?,
            TargetCapability::BFloat16 => writer.u8(Token::CapabilityKind, 2)?,
            TargetCapability::Float64 => writer.u8(Token::CapabilityKind, 3)?,
            TargetCapability::Int64 => writer.u8(Token::CapabilityKind, 4)?,
            TargetCapability::Subgroups => writer.u8(Token::CapabilityKind, 5)?,
            TargetCapability::SubgroupSize(size) => {
                writer.u8(Token::CapabilityKind, 6)?;
                writer.u32(Token::CapabilityArgument, *size)?;
            }
            TargetCapability::WorkgroupMemory => writer.u8(Token::CapabilityKind, 7)?,
            TargetCapability::WorkgroupBarrier => writer.u8(Token::CapabilityKind, 8)?,
            TargetCapability::Atomic {
                width_bits,
                address_space,
                max_scope,
            } => {
                writer.u8(Token::CapabilityKind, 9)?;
                writer.u16(Token::CapabilityArgument, *width_bits)?;
                writer.u8(Token::AddressSpace, address_space_tag(*address_space))?;
                writer.u8(
                    Token::CapabilityArgument,
                    synchronization_scope_tag(*max_scope),
                )?;
            }
            TargetCapability::DynamicWorkgroupMemory => {
                writer.u8(Token::CapabilityKind, 10)?;
            }
            TargetCapability::Extension { namespace, name } => {
                writer.u8(Token::CapabilityKind, 11)?;
                writer.token(Token::CapabilityNamespace, namespace.as_bytes())?;
                writer.token(Token::CapabilityName, name.as_bytes())?;
            }
            TargetCapability::WaveWidth(width) => {
                writer.u8(Token::CapabilityKind, 12)?;
                writer.u8(Token::CapabilityArgument, wave_width_tag(*width))?;
            }
        }
    }
    Ok(())
}

fn project_type(
    writer: &mut ProjectionWriter,
    ty: &Type,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    match ty {
        Type::Unit => writer.u8(Token::TypeKind, 1),
        Type::Scalar(scalar) => {
            writer.u8(Token::TypeKind, 2)?;
            writer.u8(Token::ScalarType, scalar_type_tag(*scalar))
        }
        Type::Pointer(pointer) => {
            let crate::PointerType {
                pointee,
                address_space,
                access,
            } = pointer;
            writer.u8(Token::TypeKind, 3)?;
            project_type(writer, pointee)?;
            writer.u8(Token::AddressSpace, address_space_tag(*address_space))?;
            writer.u8(Token::AccessMode, access_mode_tag(*access))
        }
        Type::Slice(slice) => {
            let crate::SliceType {
                element,
                address_space,
                access,
            } = slice;
            writer.u8(Token::TypeKind, 4)?;
            project_type(writer, element)?;
            writer.u8(Token::AddressSpace, address_space_tag(*address_space))?;
            writer.u8(Token::AccessMode, access_mode_tag(*access))
        }
    }
}

fn project_value_ids(
    writer: &mut ProjectionWriter,
    count_token: Token,
    values: &[ValueId],
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    writer.count(count_token, values.len())?;
    for value in values {
        project_value_id(writer, *value)?;
    }
    Ok(())
}

fn project_value_id(
    writer: &mut ProjectionWriter,
    value: ValueId,
) -> Result<(), ScalarGemmSemanticProjectionErrorV1> {
    writer.u32(Token::ValueId, value.0)
}

fn function_role_tag(role: FunctionRole) -> u8 {
    match role {
        FunctionRole::KernelEntry => 1,
        FunctionRole::InternalHelper => 2,
        FunctionRole::DeviceFfiExport => 3,
        FunctionRole::ExternalImport => 4,
    }
}

fn scalar_type_tag(scalar: ScalarType) -> u8 {
    match scalar {
        ScalarType::Bool => 1,
        ScalarType::I8 => 2,
        ScalarType::I16 => 3,
        ScalarType::I32 => 4,
        ScalarType::I64 => 5,
        ScalarType::I128 => 6,
        ScalarType::U8 => 7,
        ScalarType::U16 => 8,
        ScalarType::U32 => 9,
        ScalarType::U64 => 10,
        ScalarType::U128 => 11,
        ScalarType::Index => 12,
        ScalarType::F16 => 13,
        ScalarType::Bf16 => 14,
        ScalarType::F32 => 15,
        ScalarType::F64 => 16,
    }
}

fn address_space_tag(address_space: AddressSpace) -> u8 {
    match address_space {
        AddressSpace::Private => 1,
        AddressSpace::Workgroup => 2,
        AddressSpace::Global => 3,
        AddressSpace::Constant => 4,
        AddressSpace::Generic => 5,
    }
}

fn access_mode_tag(access: AccessMode) -> u8 {
    match access {
        AccessMode::ReadOnly => 1,
        AccessMode::ReadWrite => 2,
    }
}

fn axis_tag(axis: Axis) -> u8 {
    match axis {
        Axis::X => 1,
        Axis::Y => 2,
        Axis::Z => 3,
    }
}

fn index_kind_tag(kind: IndexKind) -> u8 {
    match kind {
        IndexKind::Global => 1,
        IndexKind::Workgroup => 2,
        IndexKind::Local => 3,
        IndexKind::WorkgroupSize => 4,
        IndexKind::WorkgroupCount => 5,
    }
}

fn binary_op_tag(operation: BinaryOp) -> Option<u8> {
    Some(match operation {
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
        BinaryOp::Checked(_) => return None,
    })
}

fn compare_predicate_tag(predicate: ComparePredicate) -> u8 {
    match predicate {
        ComparePredicate::Equal => 1,
        ComparePredicate::NotEqual => 2,
        ComparePredicate::LessThan => 3,
        ComparePredicate::LessThanOrEqual => 4,
        ComparePredicate::GreaterThan => 5,
        ComparePredicate::GreaterThanOrEqual => 6,
    }
}

fn cast_kind_tag(kind: CastKind) -> u8 {
    match kind {
        CastKind::Truncate => 1,
        CastKind::ZeroExtend => 2,
        CastKind::SignExtend => 3,
        CastKind::FloatExtend => 4,
        CastKind::FloatTruncate => 5,
        CastKind::IntegerToFloat => 6,
        CastKind::FloatToInteger => 7,
        CastKind::Bitcast => 8,
    }
}

fn wave_width_tag(width: WaveWidth) -> u8 {
    match width {
        WaveWidth::Wave32 => 1,
        WaveWidth::Wave64 => 2,
    }
}

fn synchronization_scope_tag(scope: crate::SynchronizationScope) -> u8 {
    match scope {
        crate::SynchronizationScope::Invocation => 1,
        crate::SynchronizationScope::Subgroup => 2,
        crate::SynchronizationScope::Workgroup => 3,
        crate::SynchronizationScope::Device => 4,
        crate::SynchronizationScope::System => 5,
    }
}

fn projection_identity(preimage: &[u8]) -> ScalarGemmSemanticProjectionIdentityV1 {
    let mut digest = Sha256::new();
    digest.update((IDENTITY_DOMAIN_V1.len() as u32).to_le_bytes());
    digest.update(IDENTITY_DOMAIN_V1);
    digest.update(SCALAR_GEMM_SEMANTIC_PROJECTION_POLICY_V1.to_le_bytes());
    digest.update((preimage.len() as u64).to_le_bytes());
    digest.update(preimage);
    ScalarGemmSemanticProjectionIdentityV1(digest.finalize().into())
}
