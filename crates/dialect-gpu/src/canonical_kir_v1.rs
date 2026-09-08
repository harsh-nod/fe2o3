//! Self-contained canonical KIR operation carriers used by the production bridge.
//!
//! Every carrier has a distinct operation identity, real SSA operands/results,
//! an exact bounded KIR payload, and authenticated graph/source coordinates.

use dialect_kernel::{CanonicalIdentityAttr, SourceCoordinateAttr};
use fe2o3_kernel_ir::{
    BasicBlock as KirBlock, BlockId, Function, InlineAssembly, MatrixOperationKind, MemoryEffect,
    Module, Operation as KirOperation, OperationKind, Signature, TargetCapability, Terminator,
    ValueId, WaveOperationKind, decode_module_v13, encode_module_v13,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        ATTR_KEY_DEBUG_INFO,
        attributes::{OperandSegmentSizesAttr, StringAttr},
        op_interfaces::{
            ATTR_KEY_OPERAND_SEGMENT_SIZES, BranchOpInterface, IsTerminatorInterface,
            NRegionsInterface, NResultsInterface, OperandSegmentInterface,
        },
    },
    common_traits::Verify,
    context::{Context, Ptr},
    derive::{op_interface, op_interface_impl, pliron_attr, pliron_op},
    identifier::Identifier,
    op::{Op, op_cast},
    operation::Operation,
    opts::dce::SideEffects,
    result::Result,
    r#type::TypeHandle,
    value::Value,
    verify_err, verify_err_noloc,
};
use sha2::{Digest, Sha256};

use crate::{SynchronizationOpInterface, TargetNeutralGpuOpInterface};

const PAYLOAD_MODULE_ID: &str = "fe2o3.pliron.canonical-kir-payload.v1";
const PAYLOAD_FUNCTION_ID: &str = "payload";
const OPERATION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PLIRON/CANONICAL-KIR-OP/V1\0";
const TERMINATOR_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/PLIRON/CANONICAL-KIR-TERM/V1\0";

/// A deliberately tighter limit than the 16 MiB whole-module wire bound.
pub const MAX_CANONICAL_KIR_OPERATION_BYTES_V1: usize = 256 * 1024;
pub const MAX_CANONICAL_KIR_OPERATION_OPERANDS_V1: usize = 4096;
pub const MAX_CANONICAL_KIR_OPERATION_RESULTS_V1: usize = 4096;
pub const MAX_CANONICAL_KIR_TERMINATOR_ARGUMENTS_V1: usize = 4096;
pub const MAX_CANONICAL_KIR_TERMINATOR_SUCCESSORS_V1: usize = 4096;

#[pliron_attr(name = "gpu.canonical_kir_operation_v1", format = "$0")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CanonicalKirOperationAttr(StringAttr);

impl CanonicalKirOperationAttr {
    pub fn new(operation: &KirOperation) -> Option<Self> {
        let bytes = encode_operation(operation)?;
        Some(Self(StringAttr::new(encode_hex(&bytes))))
    }

    pub fn operation(&self) -> Option<KirOperation> {
        decode_operation(&decode_hex(
            self.0.as_str(),
            MAX_CANONICAL_KIR_OPERATION_BYTES_V1,
        )?)
    }

    fn bytes(&self) -> Option<Vec<u8>> {
        let bytes = decode_hex(self.0.as_str(), MAX_CANONICAL_KIR_OPERATION_BYTES_V1)?;
        decode_operation(&bytes)?;
        Some(bytes)
    }
}

impl Verify for CanonicalKirOperationAttr {
    fn verify(&self, _context: &Context) -> Result<()> {
        if self.bytes().is_none() {
            return verify_err_noloc!(
                "gpu canonical KIR operation payload is not canonical V1 data"
            );
        }
        Ok(())
    }
}

#[pliron_attr(name = "gpu.canonical_kir_terminator_v1", format = "$0")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CanonicalKirTerminatorAttr(StringAttr);

impl CanonicalKirTerminatorAttr {
    pub fn new(terminator: &Terminator) -> Option<Self> {
        let bytes = encode_terminator(terminator)?;
        Some(Self(StringAttr::new(encode_hex(&bytes))))
    }

    pub fn terminator(&self) -> Option<Terminator> {
        decode_terminator(&decode_hex(
            self.0.as_str(),
            MAX_CANONICAL_KIR_OPERATION_BYTES_V1,
        )?)
    }

    fn bytes(&self) -> Option<Vec<u8>> {
        let bytes = decode_hex(self.0.as_str(), MAX_CANONICAL_KIR_OPERATION_BYTES_V1)?;
        decode_terminator(&bytes)?;
        Some(bytes)
    }
}

impl Verify for CanonicalKirTerminatorAttr {
    fn verify(&self, _context: &Context) -> Result<()> {
        if self.bytes().is_none() {
            return verify_err_noloc!(
                "gpu canonical KIR terminator payload is not canonical V1 data"
            );
        }
        Ok(())
    }
}

/// Marker consumed by W4 inventory code to enumerate every safety-significant op.
#[op_interface]
pub trait CanonicalKirSafetyOpInterface {
    fn verify(_op: &dyn Op, _context: &Context) -> Result<()>
    where
        Self: Sized,
    {
        Ok(())
    }

    fn is_self_contained_canonical_kir(&self) -> bool {
        true
    }
}

/// Closed, stable family inventory for every safety-significant canonical KIR
/// carrier. Adding another interface implementation requires extending this
/// enum and the checked extractor below; unknown implementations fail closed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalKirSafetyFamilyV1 {
    Intrinsic,
    MemoryIntrinsic,
    Alloca,
    GuardedLoad,
    GuardedStore,
    Barrier,
    Atomic,
    Fence,
    WorkgroupBarrier,
    WorkgroupMemory,
    Matrix,
    TargetSpecificLdsTranspose,
    Wave,
    InlineAssembly,
    Switch,
    IntegerSwitch,
    Unreachable,
}

impl CanonicalKirSafetyFamilyV1 {
    pub const ALL: [Self; 17] = [
        Self::Intrinsic,
        Self::MemoryIntrinsic,
        Self::Alloca,
        Self::GuardedLoad,
        Self::GuardedStore,
        Self::Barrier,
        Self::Atomic,
        Self::Fence,
        Self::WorkgroupBarrier,
        Self::WorkgroupMemory,
        Self::Matrix,
        Self::TargetSpecificLdsTranspose,
        Self::Wave,
        Self::InlineAssembly,
        Self::Switch,
        Self::IntegerSwitch,
        Self::Unreachable,
    ];

    pub const fn ordinal(self) -> usize {
        match self {
            Self::Intrinsic => 0,
            Self::MemoryIntrinsic => 1,
            Self::Alloca => 2,
            Self::GuardedLoad => 3,
            Self::GuardedStore => 4,
            Self::Barrier => 5,
            Self::Atomic => 6,
            Self::Fence => 7,
            Self::WorkgroupBarrier => 8,
            Self::WorkgroupMemory => 9,
            Self::Matrix => 10,
            Self::TargetSpecificLdsTranspose => 11,
            Self::Wave => 12,
            Self::InlineAssembly => 13,
            Self::Switch => 14,
            Self::IntegerSwitch => 15,
            Self::Unreachable => 16,
        }
    }
}

/// Typed semantics read directly from one live canonical carrier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirSafetyContractV1 {
    Operation {
        family: CanonicalKirSafetyFamilyV1,
        contract: KirOperation,
        coordinate: SourceCoordinateAttr,
        graph_epoch: [u8; 32],
    },
    Terminator {
        family: CanonicalKirSafetyFamilyV1,
        contract: Terminator,
        coordinate: SourceCoordinateAttr,
        graph_epoch: [u8; 32],
    },
}

impl CanonicalKirSafetyContractV1 {
    pub const fn family(&self) -> CanonicalKirSafetyFamilyV1 {
        match self {
            Self::Operation { family, .. } | Self::Terminator { family, .. } => *family,
        }
    }

    pub const fn coordinate(&self) -> SourceCoordinateAttr {
        match self {
            Self::Operation { coordinate, .. } | Self::Terminator { coordinate, .. } => *coordinate,
        }
    }

    pub const fn graph_epoch(&self) -> [u8; 32] {
        match self {
            Self::Operation { graph_epoch, .. } | Self::Terminator { graph_epoch, .. } => {
                *graph_epoch
            }
        }
    }
}

/// Lossless, typed semantic projection of one authenticated safety carrier.
///
/// `canonical_payload` is the bounded canonical KIR encoding of the exact
/// operation or terminator. The remaining fields are derived from that payload
/// and retain the authenticated PLIRON source/epoch identity. Consumers never
/// need an operation-name allowlist to recognize this closed family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalKirSafetySemanticProjectionV1 {
    contract: CanonicalKirSafetyContractV1,
    canonical_payload: Box<[u8]>,
    carrier_identity: [u8; 32],
    memory_effects: Box<[MemoryEffect]>,
    required_capabilities: Box<[TargetCapability]>,
}

impl CanonicalKirSafetySemanticProjectionV1 {
    fn try_new(
        contract: CanonicalKirSafetyContractV1,
    ) -> core::result::Result<Self, CanonicalKirSafetyCarrierErrorV1> {
        let family = contract.family();
        let (canonical_payload, carrier_identity, memory_effects, required_capabilities) =
            match &contract {
                CanonicalKirSafetyContractV1::Operation {
                    contract,
                    coordinate,
                    graph_epoch,
                    ..
                } => {
                    let payload = CanonicalKirOperationAttr::new(contract)
                        .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?;
                    let identity = operation_identity(&payload, *graph_epoch, *coordinate)
                        .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?;
                    let bytes = payload
                        .bytes()
                        .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?;
                    let effects = contract
                        .effect_summary()
                        .effects()
                        .iter()
                        .cloned()
                        .collect();
                    let capabilities = contract.required_capabilities().into_iter().collect();
                    (bytes, identity, effects, capabilities)
                }
                CanonicalKirSafetyContractV1::Terminator {
                    contract,
                    coordinate,
                    graph_epoch,
                    ..
                } => {
                    let payload = CanonicalKirTerminatorAttr::new(contract)
                        .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?;
                    let identity = terminator_identity(&payload, *graph_epoch, *coordinate)
                        .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?;
                    let bytes = payload
                        .bytes()
                        .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?;
                    (bytes, identity, Vec::new(), Vec::new())
                }
            };
        Ok(Self {
            contract,
            canonical_payload: canonical_payload.into_boxed_slice(),
            carrier_identity,
            memory_effects: memory_effects.into_boxed_slice(),
            required_capabilities: required_capabilities.into_boxed_slice(),
        })
    }

    pub const fn contract(&self) -> &CanonicalKirSafetyContractV1 {
        &self.contract
    }

    pub const fn family(&self) -> CanonicalKirSafetyFamilyV1 {
        self.contract.family()
    }

    pub const fn coordinate(&self) -> SourceCoordinateAttr {
        self.contract.coordinate()
    }

    pub const fn graph_epoch(&self) -> [u8; 32] {
        self.contract.graph_epoch()
    }

    pub const fn carrier_identity(&self) -> &[u8; 32] {
        &self.carrier_identity
    }

    pub fn canonical_payload(&self) -> &[u8] {
        &self.canonical_payload
    }

    pub fn memory_effects(&self) -> &[MemoryEffect] {
        &self.memory_effects
    }

    pub fn required_capabilities(&self) -> &[TargetCapability] {
        &self.required_capabilities
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirSafetyCarrierErrorV1 {
    NonSelfContainedInterface,
    MalformedCarrier(CanonicalKirSafetyFamilyV1),
    UnknownInterfaceImplementation,
}

impl core::fmt::Display for CanonicalKirSafetyCarrierErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NonSelfContainedInterface => formatter
                .write_str("canonical KIR safety interface is not a self-contained carrier"),
            Self::MalformedCarrier(family) => {
                write!(formatter, "canonical KIR {family:?} carrier is malformed")
            }
            Self::UnknownInterfaceImplementation => formatter.write_str(
                "canonical KIR safety interface implementation is absent from the closed family inventory",
            ),
        }
    }
}

impl std::error::Error for CanonicalKirSafetyCarrierErrorV1 {}

/// Typed access used by the bridge without inspecting operation names.
pub trait CanonicalKirOperationCarrier: Op + Verify {
    fn canonical_contract(&self, context: &Context) -> Option<KirOperation>;
    fn canonical_operands(&self, context: &Context) -> Vec<Value>;
    fn canonical_coordinate(&self, context: &Context) -> Option<SourceCoordinateAttr>;
    fn canonical_graph_epoch(&self, context: &Context) -> Option<[u8; 32]>;
}

fn encode_operation(operation: &KirOperation) -> Option<Vec<u8>> {
    if operation.kind.operands().len() > MAX_CANONICAL_KIR_OPERATION_OPERANDS_V1
        || operation.results.len() > MAX_CANONICAL_KIR_OPERATION_RESULTS_V1
    {
        return None;
    }
    let mut block = KirBlock::new(BlockId(0));
    block.operations.push(operation.clone());
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new(PAYLOAD_MODULE_ID);
    module.functions.push(Function::internal_helper(
        PAYLOAD_FUNCTION_ID,
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let bytes = encode_module_v13(&module).ok()?;
    (bytes.len() <= MAX_CANONICAL_KIR_OPERATION_BYTES_V1).then_some(bytes)
}

fn decode_operation(bytes: &[u8]) -> Option<KirOperation> {
    if bytes.len() > MAX_CANONICAL_KIR_OPERATION_BYTES_V1 {
        return None;
    }
    let module = decode_module_v13(bytes).ok()?;
    if encode_module_v13(&module).ok()?.as_slice() != bytes
        || module.id.as_str() != PAYLOAD_MODULE_ID
        || !module.kernels.is_empty()
        || !module.required_capabilities.is_empty()
        || module.functions.len() != 1
    {
        return None;
    }
    let function = &module.functions[0];
    let body = function.body.as_ref()?;
    if function.id.as_str() != PAYLOAD_FUNCTION_ID
        || !function.signature.parameters.is_empty()
        || !function.signature.results.is_empty()
        || !function.required_capabilities.is_empty()
        || !body.parameters.is_empty()
        || body.blocks.len() != 1
    {
        return None;
    }
    let block = &body.blocks[0];
    if block.id != BlockId(0)
        || !block.parameters.is_empty()
        || block.operations.len() != 1
        || block.terminator != Some(Terminator::Return { values: vec![] })
    {
        return None;
    }
    let operation = block.operations[0].clone();
    (operation.kind.operands().len() <= MAX_CANONICAL_KIR_OPERATION_OPERANDS_V1
        && operation.results.len() <= MAX_CANONICAL_KIR_OPERATION_RESULTS_V1)
        .then_some(operation)
}

fn encode_terminator(terminator: &Terminator) -> Option<Vec<u8>> {
    let (arguments, successors) = terminator_shape(terminator)?;
    if arguments > MAX_CANONICAL_KIR_TERMINATOR_ARGUMENTS_V1
        || successors > MAX_CANONICAL_KIR_TERMINATOR_SUCCESSORS_V1
    {
        return None;
    }
    let mut block = KirBlock::new(BlockId(0));
    block.terminator = Some(terminator.clone());
    let mut module = Module::new(PAYLOAD_MODULE_ID);
    module.functions.push(Function::internal_helper(
        PAYLOAD_FUNCTION_ID,
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let bytes = encode_module_v13(&module).ok()?;
    (bytes.len() <= MAX_CANONICAL_KIR_OPERATION_BYTES_V1).then_some(bytes)
}

fn decode_terminator(bytes: &[u8]) -> Option<Terminator> {
    if bytes.len() > MAX_CANONICAL_KIR_OPERATION_BYTES_V1 {
        return None;
    }
    let module = decode_module_v13(bytes).ok()?;
    if encode_module_v13(&module).ok()?.as_slice() != bytes
        || module.id.as_str() != PAYLOAD_MODULE_ID
        || !module.kernels.is_empty()
        || !module.required_capabilities.is_empty()
        || module.functions.len() != 1
    {
        return None;
    }
    let function = &module.functions[0];
    let body = function.body.as_ref()?;
    if function.id.as_str() != PAYLOAD_FUNCTION_ID
        || !function.signature.parameters.is_empty()
        || !function.signature.results.is_empty()
        || !function.required_capabilities.is_empty()
        || !body.parameters.is_empty()
        || body.blocks.len() != 1
    {
        return None;
    }
    let block = &body.blocks[0];
    if block.id != BlockId(0) || !block.parameters.is_empty() || !block.operations.is_empty() {
        return None;
    }
    let terminator = block.terminator.clone()?;
    let (arguments, successors) = terminator_shape(&terminator)?;
    (arguments <= MAX_CANONICAL_KIR_TERMINATOR_ARGUMENTS_V1
        && successors <= MAX_CANONICAL_KIR_TERMINATOR_SUCCESSORS_V1)
        .then_some(terminator)
}

fn terminator_shape(terminator: &Terminator) -> Option<(usize, usize)> {
    let shape = match terminator {
        Terminator::Switch {
            cases,
            default_arguments,
            ..
        } => (
            1_usize.checked_add(
                cases
                    .iter()
                    .try_fold(default_arguments.len(), |total, case| {
                        total.checked_add(case.arguments.len())
                    })?,
            )?,
            cases.len().checked_add(1)?,
        ),
        Terminator::IntegerSwitch {
            cases,
            default_arguments,
            ..
        } => (
            1_usize.checked_add(
                cases
                    .iter()
                    .try_fold(default_arguments.len(), |total, case| {
                        total.checked_add(case.arguments.len())
                    })?,
            )?,
            cases.len().checked_add(1)?,
        ),
        Terminator::Unreachable => (0, 0),
        Terminator::Branch { .. }
        | Terminator::ConditionalBranch { .. }
        | Terminator::Return { .. } => return None,
    };
    Some(shape)
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut text, "{byte:02x}").expect("writing to String cannot fail");
    }
    text
}

fn decode_hex(text: &str, max_bytes: usize) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || text.len() / 2 > max_bytes {
        return None;
    }
    fn nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        }
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| Some((nibble(pair[0])? << 4) | nibble(pair[1])?))
        .collect()
}

fn operation_identity(
    contract: &CanonicalKirOperationAttr,
    graph_epoch: [u8; 32],
    coordinate: SourceCoordinateAttr,
) -> Option<[u8; 32]> {
    let bytes = contract.bytes()?;
    let (function, block, operation) = coordinate.components();
    let mut digest = Sha256::new();
    digest.update(OPERATION_IDENTITY_DOMAIN_V1);
    digest.update(graph_epoch);
    digest.update(function.to_le_bytes());
    digest.update(block.to_le_bytes());
    digest.update(operation.to_le_bytes());
    digest.update(u64::try_from(bytes.len()).ok()?.to_le_bytes());
    digest.update(bytes);
    Some(digest.finalize().into())
}

fn has_valid_debug_info(operation: &Operation, expected: usize) -> bool {
    let debug = operation.attributes.0.get(&*ATTR_KEY_DEBUG_INFO);
    let debug_valid = debug
        .map(|attribute| {
            let id = attribute.get_attr_id();
            id.dialect.as_ref() == "builtin" && AsRef::<str>::as_ref(&id.name) == "debug_info"
        })
        .unwrap_or(true);
    operation.attributes.0.len() == expected + usize::from(debug.is_some()) && debug_valid
}

fn verify_operation_contract(
    op: &dyn Op,
    context: &Context,
    expected: fn(&OperationKind) -> bool,
    contract: &CanonicalKirOperationAttr,
    graph_epoch: [u8; 32],
    coordinate: SourceCoordinateAttr,
    identity: [u8; 32],
) -> Result<()> {
    let raw = op.get_operation().deref(context);
    if raw.get_num_operands() > MAX_CANONICAL_KIR_OPERATION_OPERANDS_V1
        || raw.get_num_results() > MAX_CANONICAL_KIR_OPERATION_RESULTS_V1
        || raw.get_num_successors() != 0
        || raw.num_regions() != 0
        || !has_valid_debug_info(&raw, 4)
    {
        return verify_err!(
            op.loc(context),
            "canonical KIR operation has malformed bounded shape"
        );
    }
    let operation = contract
        .operation()
        .ok_or_else(|| pliron::verify_error!(op.loc(context), "invalid canonical KIR payload"))?;
    if !expected(&operation.kind)
        || operation.kind.operands().len() != raw.get_num_operands()
        || operation.results.len() != raw.get_num_results()
    {
        return verify_err!(
            op.loc(context),
            "canonical KIR payload family or SSA shape mismatch"
        );
    }
    if identity != operation_identity(contract, graph_epoch, coordinate).unwrap_or([0; 32]) {
        return verify_err!(op.loc(context), "canonical KIR operation identity mismatch");
    }
    Ok(())
}

macro_rules! canonical_operation {
    (
        $name:ident,
        $op_name:literal,
        $matches:expr,
        [$($interface:ty),* $(,)?],
        $contract_attr:ident,
        $epoch_attr:ident,
        $coordinate_attr:ident,
        $identity_attr:ident
    ) => {
        #[pliron_op(
            name = $op_name,
            format,
            interfaces = [CanonicalKirSafetyOpInterface, $($interface),*],
            attributes = (
                $contract_attr: CanonicalKirOperationAttr,
                $epoch_attr: CanonicalIdentityAttr,
                $coordinate_attr: SourceCoordinateAttr,
                $identity_attr: CanonicalIdentityAttr
            )
        )]
        pub struct $name;

        impl $name {
            pub fn new(
                context: &mut Context,
                contract: &KirOperation,
                operands: Vec<Value>,
                result_types: Vec<TypeHandle>,
                graph_epoch: CanonicalIdentityAttr,
                coordinate: SourceCoordinateAttr,
            ) -> Option<Self> {
                if !($matches)(&contract.kind)
                    || operands.len() != contract.kind.operands().len()
                    || operands.len() > MAX_CANONICAL_KIR_OPERATION_OPERANDS_V1
                    || result_types.len() != contract.results.len()
                    || result_types.len() > MAX_CANONICAL_KIR_OPERATION_RESULTS_V1
                {
                    return None;
                }
                let payload = CanonicalKirOperationAttr::new(contract)?;
                let epoch = graph_epoch.bytes()?;
                let identity = operation_identity(&payload, epoch, coordinate)?;
                let operation = Operation::new(
                    context,
                    Self::get_concrete_op_info(),
                    result_types,
                    operands,
                    vec![],
                    0,
                );
                let operation = Self::from_operation(operation);
                {
                    let mut raw = operation.get_operation().deref_mut(context);
                    raw.attributes.set(attr_key(stringify!($contract_attr)), payload);
                    raw.attributes.set(attr_key(stringify!($epoch_attr)), graph_epoch);
                    raw.attributes.set(attr_key(stringify!($coordinate_attr)), coordinate);
                    raw.attributes.set(
                        attr_key(stringify!($identity_attr)),
                        CanonicalIdentityAttr::from_bytes(identity),
                    );
                }
                Some(operation)
            }

            pub fn contract(&self, context: &Context) -> Option<KirOperation> {
                self.get_operation()
                    .deref(context)
                    .attributes
                    .get::<CanonicalKirOperationAttr>(&attr_key(stringify!($contract_attr)))?
                    .operation()
            }

            pub fn operands(&self, context: &Context) -> Vec<Value> {
                self.get_operation().deref(context).operands().collect()
            }

            pub fn coordinate(&self, context: &Context) -> Option<SourceCoordinateAttr> {
                self.get_operation()
                    .deref(context)
                    .attributes
                    .get::<SourceCoordinateAttr>(&attr_key(stringify!($coordinate_attr)))
                    .copied()
            }
        }

        impl CanonicalKirOperationCarrier for $name {
            fn canonical_contract(&self, context: &Context) -> Option<KirOperation> {
                self.contract(context)
            }

            fn canonical_operands(&self, context: &Context) -> Vec<Value> {
                self.operands(context)
            }

            fn canonical_coordinate(&self, context: &Context) -> Option<SourceCoordinateAttr> {
                self.coordinate(context)
            }

            fn canonical_graph_epoch(&self, context: &Context) -> Option<[u8; 32]> {
                self.get_operation()
                    .deref(context)
                    .attributes
                    .get::<CanonicalIdentityAttr>(&attr_key(stringify!($epoch_attr)))?
                    .bytes()
            }
        }

        impl Verify for $name {
            fn verify(&self, context: &Context) -> Result<()> {
                let raw = self.get_operation().deref(context);
                let contract = raw
                    .attributes
                    .get::<CanonicalKirOperationAttr>(&attr_key(stringify!($contract_attr)))
                    .cloned()
                    .ok_or_else(|| pliron::verify_error!(self.loc(context), "missing canonical KIR payload"))?;
                let graph_epoch = raw
                    .attributes
                    .get::<CanonicalIdentityAttr>(&attr_key(stringify!($epoch_attr)))
                    .and_then(|value| value.bytes())
                    .ok_or_else(|| pliron::verify_error!(self.loc(context), "missing canonical graph epoch"))?;
                let coordinate = raw
                    .attributes
                    .get::<SourceCoordinateAttr>(&attr_key(stringify!($coordinate_attr)))
                    .copied()
                    .ok_or_else(|| pliron::verify_error!(self.loc(context), "missing canonical source coordinate"))?;
                let identity = raw
                    .attributes
                    .get::<CanonicalIdentityAttr>(&attr_key(stringify!($identity_attr)))
                    .and_then(|value| value.bytes())
                    .ok_or_else(|| pliron::verify_error!(self.loc(context), "missing canonical operation identity"))?;
                verify_operation_contract(
                    self,
                    context,
                    $matches,
                    &contract,
                    graph_epoch,
                    coordinate,
                    identity,
                )
            }
        }

        #[op_interface_impl]
        impl SideEffects for $name {
            fn has_side_effects(&self, _context: &Context) -> bool {
                true
            }
        }
    };
}

fn attr_key(name: &str) -> Identifier {
    Identifier::try_from(name).expect("canonical KIR attribute key is a valid identifier")
}

canonical_operation!(
    IntrinsicOp,
    "gpu.kir_intrinsic",
    |kind: &OperationKind| matches!(kind, OperationKind::Intrinsic(_)),
    [TargetNeutralGpuOpInterface],
    gpu_kir_intrinsic_contract,
    gpu_kir_intrinsic_graph_epoch,
    gpu_kir_intrinsic_coordinate,
    gpu_kir_intrinsic_identity
);
canonical_operation!(
    MemoryIntrinsicOp,
    "gpu.kir_memory_intrinsic",
    |kind: &OperationKind| matches!(kind, OperationKind::MemoryIntrinsic(_)),
    [TargetNeutralGpuOpInterface],
    gpu_kir_memory_intrinsic_contract,
    gpu_kir_memory_intrinsic_graph_epoch,
    gpu_kir_memory_intrinsic_coordinate,
    gpu_kir_memory_intrinsic_identity
);
canonical_operation!(
    AllocaOp,
    "gpu.kir_alloca",
    |kind: &OperationKind| matches!(kind, OperationKind::Alloca { .. }),
    [TargetNeutralGpuOpInterface],
    gpu_kir_alloca_contract,
    gpu_kir_alloca_graph_epoch,
    gpu_kir_alloca_coordinate,
    gpu_kir_alloca_identity
);
canonical_operation!(
    GuardedLoadOp,
    "gpu.kir_guarded_load",
    |kind: &OperationKind| matches!(kind, OperationKind::GuardedLoad { .. }),
    [TargetNeutralGpuOpInterface],
    gpu_kir_guarded_load_contract,
    gpu_kir_guarded_load_graph_epoch,
    gpu_kir_guarded_load_coordinate,
    gpu_kir_guarded_load_identity
);
canonical_operation!(
    GuardedStoreOp,
    "gpu.kir_guarded_store",
    |kind: &OperationKind| matches!(kind, OperationKind::GuardedStore { .. }),
    [TargetNeutralGpuOpInterface],
    gpu_kir_guarded_store_contract,
    gpu_kir_guarded_store_graph_epoch,
    gpu_kir_guarded_store_coordinate,
    gpu_kir_guarded_store_identity
);
canonical_operation!(
    CanonicalBarrierOp,
    "gpu.kir_barrier",
    |kind: &OperationKind| matches!(kind, OperationKind::Barrier(_)),
    [TargetNeutralGpuOpInterface, SynchronizationOpInterface],
    gpu_kir_barrier_contract,
    gpu_kir_barrier_graph_epoch,
    gpu_kir_barrier_coordinate,
    gpu_kir_barrier_identity
);
canonical_operation!(
    AtomicOp,
    "gpu.kir_atomic",
    |kind: &OperationKind| matches!(kind, OperationKind::Atomic(_)),
    [TargetNeutralGpuOpInterface, SynchronizationOpInterface],
    gpu_kir_atomic_contract,
    gpu_kir_atomic_graph_epoch,
    gpu_kir_atomic_coordinate,
    gpu_kir_atomic_identity
);
canonical_operation!(
    CanonicalFenceOp,
    "gpu.kir_fence",
    |kind: &OperationKind| matches!(kind, OperationKind::Fence(_)),
    [TargetNeutralGpuOpInterface, SynchronizationOpInterface],
    gpu_kir_fence_contract,
    gpu_kir_fence_graph_epoch,
    gpu_kir_fence_coordinate,
    gpu_kir_fence_identity
);
canonical_operation!(
    WorkgroupBarrierOp,
    "gpu.kir_workgroup_barrier",
    |kind: &OperationKind| matches!(kind, OperationKind::WorkgroupBarrier(_)),
    [TargetNeutralGpuOpInterface, SynchronizationOpInterface],
    gpu_kir_workgroup_barrier_contract,
    gpu_kir_workgroup_barrier_graph_epoch,
    gpu_kir_workgroup_barrier_coordinate,
    gpu_kir_workgroup_barrier_identity
);
canonical_operation!(
    WorkgroupMemoryOp,
    "gpu.kir_workgroup_memory",
    |kind: &OperationKind| matches!(kind, OperationKind::WorkgroupMemory(_)),
    [TargetNeutralGpuOpInterface],
    gpu_kir_workgroup_memory_contract,
    gpu_kir_workgroup_memory_graph_epoch,
    gpu_kir_workgroup_memory_coordinate,
    gpu_kir_workgroup_memory_identity
);
canonical_operation!(
    MatrixOp,
    "gpu.kir_matrix",
    |kind: &OperationKind| matches!(kind, OperationKind::Matrix(_)),
    [TargetNeutralGpuOpInterface],
    gpu_kir_matrix_contract,
    gpu_kir_matrix_graph_epoch,
    gpu_kir_matrix_coordinate,
    gpu_kir_matrix_identity
);
canonical_operation!(
    Gfx950LdsTransposeOp,
    "gpu.kir_gfx950_lds_transpose",
    |kind: &OperationKind| matches!(kind, OperationKind::Gfx950LdsTranspose(_)),
    [],
    gpu_kir_gfx950_lds_transpose_contract,
    gpu_kir_gfx950_lds_transpose_graph_epoch,
    gpu_kir_gfx950_lds_transpose_coordinate,
    gpu_kir_gfx950_lds_transpose_identity
);
canonical_operation!(
    WaveOp,
    "gpu.kir_wave",
    |kind: &OperationKind| matches!(kind, OperationKind::Wave(_)),
    [TargetNeutralGpuOpInterface],
    gpu_kir_wave_contract,
    gpu_kir_wave_graph_epoch,
    gpu_kir_wave_coordinate,
    gpu_kir_wave_identity
);
canonical_operation!(
    InlineAssemblyOp,
    "gpu.kir_inline_assembly",
    |kind: &OperationKind| matches!(kind, OperationKind::InlineAssembly(_)),
    [],
    gpu_kir_inline_assembly_contract,
    gpu_kir_inline_assembly_graph_epoch,
    gpu_kir_inline_assembly_coordinate,
    gpu_kir_inline_assembly_identity
);

pub fn remap_canonical_operation(
    mut operation: KirOperation,
    operands: &[ValueId],
) -> Option<KirOperation> {
    if operation.kind.operands().len() != operands.len() {
        return None;
    }
    let mut next = operands.iter().copied();
    let take = |next: &mut std::iter::Copied<std::slice::Iter<'_, ValueId>>| next.next();
    match &mut operation.kind {
        OperationKind::Intrinsic(_)
        | OperationKind::Barrier(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::WorkgroupMemory(_) => {}
        OperationKind::MemoryIntrinsic(intrinsic) => match intrinsic {
            fe2o3_kernel_ir::MemoryIntrinsicOperation::PointerDistance {
                pointer, origin, ..
            } => {
                *pointer = take(&mut next)?;
                *origin = take(&mut next)?;
            }
            fe2o3_kernel_ir::MemoryIntrinsicOperation::VolatileLoad { pointer, .. } => {
                *pointer = take(&mut next)?;
            }
            fe2o3_kernel_ir::MemoryIntrinsicOperation::VolatileStore { pointer, value, .. } => {
                *pointer = take(&mut next)?;
                *value = take(&mut next)?;
            }
            fe2o3_kernel_ir::MemoryIntrinsicOperation::CopyNonOverlapping {
                source,
                destination,
                count,
                ..
            } => {
                *source = take(&mut next)?;
                *destination = take(&mut next)?;
                *count = take(&mut next)?;
            }
        },
        OperationKind::Alloca { count, .. } => {
            if count.is_some() {
                *count = Some(take(&mut next)?);
            }
        }
        OperationKind::GuardedLoad {
            pointer,
            predicate,
            fallback,
            ..
        } => {
            *pointer = take(&mut next)?;
            *predicate = take(&mut next)?;
            *fallback = take(&mut next)?;
        }
        OperationKind::GuardedStore {
            pointer,
            predicate,
            value,
            ..
        } => {
            *pointer = take(&mut next)?;
            *predicate = take(&mut next)?;
            *value = take(&mut next)?;
        }
        OperationKind::Atomic(atomic) => {
            atomic.pointer = take(&mut next)?;
            if atomic.value.is_some() {
                atomic.value = Some(take(&mut next)?);
            }
            if atomic.compare.is_some() {
                atomic.compare = Some(take(&mut next)?);
            }
        }
        OperationKind::Matrix(matrix) => match &mut matrix.kind {
            MatrixOperationKind::MultiplyAccumulate {
                lhs,
                rhs,
                accumulator,
                ..
            } => {
                for value in lhs.iter_mut().chain(rhs).chain(accumulator) {
                    *value = take(&mut next)?;
                }
            }
            MatrixOperationKind::ScaledMultiplyAccumulate {
                lhs,
                rhs,
                accumulator,
                ..
            } => {
                for value in lhs.iter_mut().chain(rhs).chain(accumulator) {
                    *value = take(&mut next)?;
                }
            }
            MatrixOperationKind::LdsLoad { base, .. } => *base = take(&mut next)?,
            MatrixOperationKind::LdsStore { base, values, .. } => {
                *base = take(&mut next)?;
                for value in values {
                    *value = take(&mut next)?;
                }
            }
        },
        OperationKind::Gfx950LdsTranspose(transpose) => match &mut transpose.kind {
            fe2o3_kernel_ir::Gfx950LdsTransposeOperationKindV1::Current { .. } => {}
            fe2o3_kernel_ir::Gfx950LdsTransposeOperationKindV1::Stage {
                storage,
                source_slice,
                offset,
                rows,
                columns,
                stride,
                token_base,
                reduction_base,
                ..
            } => {
                for value in [
                    storage,
                    source_slice,
                    offset,
                    rows,
                    columns,
                    stride,
                    token_base,
                    reduction_base,
                ] {
                    *value = take(&mut next)?;
                }
            }
            fe2o3_kernel_ir::Gfx950LdsTransposeOperationKindV1::Publish { storage, .. }
            | fe2o3_kernel_ir::Gfx950LdsTransposeOperationKindV1::Read { storage, .. } => {
                *storage = take(&mut next)?;
            }
        },
        OperationKind::Wave(wave) => match &mut wave.kind {
            WaveOperationKind::LaneId => {}
            WaveOperationKind::Ballot { predicate }
            | WaveOperationKind::Any { predicate }
            | WaveOperationKind::All { predicate } => *predicate = take(&mut next)?,
            WaveOperationKind::ShuffleIndex {
                value, source_lane, ..
            }
            | WaveOperationKind::BroadcastF32 {
                value, source_lane, ..
            } => {
                *value = take(&mut next)?;
                *source_lane = take(&mut next)?;
            }
            WaveOperationKind::ReduceF32 { value, .. } => *value = take(&mut next)?,
        },
        OperationKind::InlineAssembly(assembly) => remap_inline_assembly(assembly, &mut next)?,
        OperationKind::Constant(_)
        | OperationKind::Unary { .. }
        | OperationKind::Binary { .. }
        | OperationKind::Compare { .. }
        | OperationKind::Cast { .. }
        | OperationKind::Select { .. }
        | OperationKind::Call { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. }
        | OperationKind::Load { .. }
        | OperationKind::Store { .. }
        | OperationKind::KernelContextIssue(_)
        | OperationKind::GlobalCapabilityBind(_)
        | OperationKind::GlobalCapabilityIndex(_)
        | OperationKind::ExecutionCapability(_) => return None,
    }
    next.next().is_none().then_some(operation)
}

fn remap_inline_assembly(
    assembly: &mut InlineAssembly,
    next: &mut impl Iterator<Item = ValueId>,
) -> Option<()> {
    for operand in &mut assembly.operands {
        match &mut operand.kind {
            fe2o3_kernel_ir::AssemblyOperandKind::Input(value)
            | fe2o3_kernel_ir::AssemblyOperandKind::InOut { input: value, .. } => {
                *value = next.next()?;
            }
            fe2o3_kernel_ir::AssemblyOperandKind::Output { .. }
            | fe2o3_kernel_ir::AssemblyOperandKind::ImmediateI32(_) => {}
        }
    }
    Some(())
}

fn terminator_identity(
    contract: &CanonicalKirTerminatorAttr,
    graph_epoch: [u8; 32],
    coordinate: SourceCoordinateAttr,
) -> Option<[u8; 32]> {
    let bytes = contract.bytes()?;
    let (function, block, operation) = coordinate.components();
    let mut digest = Sha256::new();
    digest.update(TERMINATOR_IDENTITY_DOMAIN_V1);
    digest.update(graph_epoch);
    digest.update(function.to_le_bytes());
    digest.update(block.to_le_bytes());
    digest.update(operation.to_le_bytes());
    digest.update(u64::try_from(bytes.len()).ok()?.to_le_bytes());
    digest.update(bytes);
    Some(digest.finalize().into())
}

fn switch_segments(terminator: &Terminator) -> Option<Vec<usize>> {
    match terminator {
        Terminator::Switch {
            cases,
            default_arguments,
            ..
        } => Some(
            [1].into_iter()
                .chain(cases.iter().map(|case| case.arguments.len()))
                .chain([default_arguments.len()])
                .collect(),
        ),
        Terminator::IntegerSwitch {
            cases,
            default_arguments,
            ..
        } => Some(
            [1].into_iter()
                .chain(cases.iter().map(|case| case.arguments.len()))
                .chain([default_arguments.len()])
                .collect(),
        ),
        _ => None,
    }
}

fn verify_terminator_identity(
    op: &dyn Op,
    context: &Context,
    contract: &CanonicalKirTerminatorAttr,
    graph_epoch: [u8; 32],
    coordinate: SourceCoordinateAttr,
    identity: [u8; 32],
) -> Result<()> {
    if identity != terminator_identity(contract, graph_epoch, coordinate).unwrap_or([0; 32]) {
        return verify_err!(
            op.loc(context),
            "canonical KIR terminator identity mismatch"
        );
    }
    Ok(())
}

pub trait CanonicalKirSwitchCarrier: Op + Verify + BranchOpInterface {
    fn canonical_terminator(&self, context: &Context) -> Option<Terminator>;
    fn canonical_selector(&self, context: &Context) -> Option<Value>;
    fn canonical_coordinate(&self, context: &Context) -> Option<SourceCoordinateAttr>;
    fn canonical_graph_epoch(&self, context: &Context) -> Option<[u8; 32]>;
}

macro_rules! canonical_switch {
    (
        $name:ident,
        $op_name:literal,
        $matches:expr,
        $contract_attr:ident,
        $epoch_attr:ident,
        $coordinate_attr:ident,
        $identity_attr:ident
    ) => {
        #[pliron_op(
                                                    name = $op_name,
                                                    format,
                                                    interfaces = [
                                                        CanonicalKirSafetyOpInterface,
                                                        TargetNeutralGpuOpInterface,
                                                        IsTerminatorInterface,
                                                        NResultsInterface<0>,
                                                        NRegionsInterface<0>
                                                    ],
                                                    attributes = (
                                                        $contract_attr: CanonicalKirTerminatorAttr,
                                                        $epoch_attr: CanonicalIdentityAttr,
                                                        $coordinate_attr: SourceCoordinateAttr,
                                                        $identity_attr: CanonicalIdentityAttr
                                                    )
                                                )]
        pub struct $name;

        impl $name {
            pub fn new(
                context: &mut Context,
                terminator: &Terminator,
                selector: Value,
                successors: Vec<Ptr<BasicBlock>>,
                successor_arguments: Vec<Vec<Value>>,
                graph_epoch: CanonicalIdentityAttr,
                coordinate: SourceCoordinateAttr,
            ) -> Option<Self> {
                if !($matches)(terminator)
                    || successors.is_empty()
                    || successors.len() != successor_arguments.len()
                    || terminator.successors().len() != successors.len()
                    || terminator.operands().len()
                        != 1 + successor_arguments.iter().map(Vec::len).sum::<usize>()
                    || terminator.operands().len() > MAX_CANONICAL_KIR_TERMINATOR_ARGUMENTS_V1
                    || successors.len() > MAX_CANONICAL_KIR_TERMINATOR_SUCCESSORS_V1
                {
                    return None;
                }
                let contract = CanonicalKirTerminatorAttr::new(terminator)?;
                let epoch = graph_epoch.bytes()?;
                let identity = terminator_identity(&contract, epoch, coordinate)?;
                let mut segments = Vec::with_capacity(successor_arguments.len() + 1);
                segments.push(vec![selector]);
                segments.extend(successor_arguments);
                let (operands, sizes) = Self::compute_segment_sizes(segments);
                let operation = Self::from_operation(Operation::new(
                    context,
                    Self::get_concrete_op_info(),
                    vec![],
                    operands,
                    successors,
                    0,
                ));
                operation.set_operand_segment_sizes(context, sizes);
                {
                    let mut raw = operation.get_operation().deref_mut(context);
                    raw.attributes
                        .set(attr_key(stringify!($contract_attr)), contract);
                    raw.attributes
                        .set(attr_key(stringify!($epoch_attr)), graph_epoch);
                    raw.attributes
                        .set(attr_key(stringify!($coordinate_attr)), coordinate);
                    raw.attributes.set(
                        attr_key(stringify!($identity_attr)),
                        CanonicalIdentityAttr::from_bytes(identity),
                    );
                }
                Some(operation)
            }

            pub fn contract(&self, context: &Context) -> Option<Terminator> {
                self.get_operation()
                    .deref(context)
                    .attributes
                    .get::<CanonicalKirTerminatorAttr>(&attr_key(stringify!($contract_attr)))?
                    .terminator()
            }

            pub fn selector(&self, context: &Context) -> Option<Value> {
                self.get_segment(context, 0).first().copied()
            }

            pub fn coordinate(&self, context: &Context) -> Option<SourceCoordinateAttr> {
                self.get_operation()
                    .deref(context)
                    .attributes
                    .get::<SourceCoordinateAttr>(&attr_key(stringify!($coordinate_attr)))
                    .copied()
            }
        }

        #[op_interface_impl]
        impl OperandSegmentInterface for $name {}

        impl CanonicalKirSwitchCarrier for $name {
            fn canonical_terminator(&self, context: &Context) -> Option<Terminator> {
                self.contract(context)
            }

            fn canonical_selector(&self, context: &Context) -> Option<Value> {
                self.selector(context)
            }

            fn canonical_coordinate(&self, context: &Context) -> Option<SourceCoordinateAttr> {
                self.coordinate(context)
            }

            fn canonical_graph_epoch(&self, context: &Context) -> Option<[u8; 32]> {
                self.get_operation()
                    .deref(context)
                    .attributes
                    .get::<CanonicalIdentityAttr>(&attr_key(stringify!($epoch_attr)))?
                    .bytes()
            }
        }

        #[op_interface_impl]
        impl BranchOpInterface for $name {
            fn successor_operands(&self, context: &Context, successor: usize) -> Vec<Value> {
                assert!(
                    successor < self.get_operation().deref(context).get_num_successors(),
                    "canonical switch successor index out of range"
                );
                self.get_segment(context, successor + 1)
            }

            fn add_successor_operand(
                &self,
                context: &mut Context,
                successor: usize,
                operand: Value,
            ) -> usize {
                assert!(
                    successor < self.get_operation().deref(context).get_num_successors(),
                    "canonical switch successor index out of range"
                );
                self.push_to_segment(context, successor + 1, operand)
            }

            fn remove_successor_operand(
                &self,
                context: &mut Context,
                successor: usize,
                operand: usize,
            ) -> Value {
                assert!(
                    successor < self.get_operation().deref(context).get_num_successors(),
                    "canonical switch successor index out of range"
                );
                self.remove_from_segment(context, successor + 1, operand)
            }
        }

        impl Verify for $name {
            fn verify(&self, context: &Context) -> Result<()> {
                let raw = self.get_operation().deref(context);
                if raw.get_num_results() != 0
                    || raw.num_regions() != 0
                    || raw.get_num_successors() == 0
                    || raw.get_num_successors() > MAX_CANONICAL_KIR_TERMINATOR_SUCCESSORS_V1
                    || raw.get_num_operands() > MAX_CANONICAL_KIR_TERMINATOR_ARGUMENTS_V1
                    || !has_valid_debug_info(&raw, 5)
                {
                    return verify_err!(
                        self.loc(context),
                        "canonical switch has malformed bounded shape"
                    );
                }
                let contract = raw
                    .attributes
                    .get::<CanonicalKirTerminatorAttr>(&attr_key(stringify!($contract_attr)))
                    .cloned()
                    .ok_or_else(|| {
                        pliron::verify_error!(
                            self.loc(context),
                            "missing canonical terminator payload"
                        )
                    })?;
                let terminator = contract.terminator().filter($matches).ok_or_else(|| {
                    pliron::verify_error!(self.loc(context), "wrong canonical switch family")
                })?;
                let expected_segments = switch_segments(&terminator).ok_or_else(|| {
                    pliron::verify_error!(self.loc(context), "invalid switch segments")
                })?;
                let actual_segments = raw
                    .attributes
                    .get::<OperandSegmentSizesAttr>(&ATTR_KEY_OPERAND_SEGMENT_SIZES)
                    .map(|segments| {
                        segments
                            .0
                            .iter()
                            .map(|size| *size as usize)
                            .collect::<Vec<_>>()
                    })
                    .ok_or_else(|| {
                        pliron::verify_error!(self.loc(context), "missing switch operand segments")
                    })?;
                if actual_segments != expected_segments
                    || terminator.successors().len() != raw.get_num_successors()
                    || terminator.operands().len() != raw.get_num_operands()
                {
                    return verify_err!(
                        self.loc(context),
                        "canonical switch CFG shape disagrees with payload"
                    );
                }
                let graph_epoch = raw
                    .attributes
                    .get::<CanonicalIdentityAttr>(&attr_key(stringify!($epoch_attr)))
                    .and_then(|value| value.bytes())
                    .ok_or_else(|| {
                        pliron::verify_error!(self.loc(context), "missing canonical graph epoch")
                    })?;
                let coordinate = raw
                    .attributes
                    .get::<SourceCoordinateAttr>(&attr_key(stringify!($coordinate_attr)))
                    .copied()
                    .ok_or_else(|| {
                        pliron::verify_error!(
                            self.loc(context),
                            "missing canonical source coordinate"
                        )
                    })?;
                let identity = raw
                    .attributes
                    .get::<CanonicalIdentityAttr>(&attr_key(stringify!($identity_attr)))
                    .and_then(|value| value.bytes())
                    .ok_or_else(|| {
                        pliron::verify_error!(
                            self.loc(context),
                            "missing canonical terminator identity"
                        )
                    })?;
                verify_terminator_identity(
                    self,
                    context,
                    &contract,
                    graph_epoch,
                    coordinate,
                    identity,
                )
            }
        }
    };
}

canonical_switch!(
    SwitchOp,
    "gpu.kir_switch",
    |terminator: &Terminator| matches!(terminator, Terminator::Switch { .. }),
    gpu_kir_switch_contract,
    gpu_kir_switch_graph_epoch,
    gpu_kir_switch_coordinate,
    gpu_kir_switch_identity
);
canonical_switch!(
    IntegerSwitchOp,
    "gpu.kir_integer_switch",
    |terminator: &Terminator| matches!(terminator, Terminator::IntegerSwitch { .. }),
    gpu_kir_integer_switch_contract,
    gpu_kir_integer_switch_graph_epoch,
    gpu_kir_integer_switch_coordinate,
    gpu_kir_integer_switch_identity
);

#[pliron_op(
    name = "gpu.kir_unreachable",
    format,
    interfaces = [
        CanonicalKirSafetyOpInterface,
        TargetNeutralGpuOpInterface,
        IsTerminatorInterface,
        NResultsInterface<0>,
        NRegionsInterface<0>
    ],
    attributes = (
        gpu_kir_unreachable_contract: CanonicalKirTerminatorAttr,
        gpu_kir_unreachable_graph_epoch: CanonicalIdentityAttr,
        gpu_kir_unreachable_coordinate: SourceCoordinateAttr,
        gpu_kir_unreachable_identity: CanonicalIdentityAttr
    )
)]
pub struct UnreachableOp;

impl UnreachableOp {
    pub fn new(
        context: &mut Context,
        graph_epoch: CanonicalIdentityAttr,
        coordinate: SourceCoordinateAttr,
    ) -> Option<Self> {
        let contract = CanonicalKirTerminatorAttr::new(&Terminator::Unreachable)?;
        let epoch = graph_epoch.bytes()?;
        let identity = terminator_identity(&contract, epoch, coordinate)?;
        let operation = Self::from_operation(Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        ));
        operation.set_attr_gpu_kir_unreachable_contract(context, contract);
        operation.set_attr_gpu_kir_unreachable_graph_epoch(context, graph_epoch);
        operation.set_attr_gpu_kir_unreachable_coordinate(context, coordinate);
        operation.set_attr_gpu_kir_unreachable_identity(
            context,
            CanonicalIdentityAttr::from_bytes(identity),
        );
        Some(operation)
    }

    pub fn contract(&self, context: &Context) -> Option<Terminator> {
        self.get_attr_gpu_kir_unreachable_contract(context)?
            .terminator()
    }

    pub fn coordinate(&self, context: &Context) -> Option<SourceCoordinateAttr> {
        self.get_attr_gpu_kir_unreachable_coordinate(context)
            .map(|value| *value)
    }

    pub fn graph_epoch(&self, context: &Context) -> Option<[u8; 32]> {
        self.get_attr_gpu_kir_unreachable_graph_epoch(context)?
            .bytes()
    }
}

impl Verify for UnreachableOp {
    fn verify(&self, context: &Context) -> Result<()> {
        let raw = self.get_operation().deref(context);
        if raw.get_num_operands() != 0
            || raw.get_num_results() != 0
            || raw.get_num_successors() != 0
            || raw.num_regions() != 0
            || !has_valid_debug_info(&raw, 4)
            || self.contract(context) != Some(Terminator::Unreachable)
        {
            return verify_err!(
                self.loc(context),
                "canonical unreachable has malformed semantics"
            );
        }
        let contract = self
            .get_attr_gpu_kir_unreachable_contract(context)
            .ok_or_else(|| {
                pliron::verify_error!(self.loc(context), "missing unreachable payload")
            })?;
        let graph_epoch = self
            .get_attr_gpu_kir_unreachable_graph_epoch(context)
            .and_then(|value| value.bytes())
            .ok_or_else(|| {
                pliron::verify_error!(self.loc(context), "missing canonical graph epoch")
            })?;
        let coordinate = self
            .get_attr_gpu_kir_unreachable_coordinate(context)
            .map(|value| *value)
            .ok_or_else(|| pliron::verify_error!(self.loc(context), "missing source coordinate"))?;
        let identity = self
            .get_attr_gpu_kir_unreachable_identity(context)
            .and_then(|value| value.bytes())
            .ok_or_else(|| {
                pliron::verify_error!(self.loc(context), "missing terminator identity")
            })?;
        verify_terminator_identity(self, context, &contract, graph_epoch, coordinate, identity)
    }
}

/// Extracts the typed semantics of one safety carrier without consulting an
/// operation-name table or a separately editable semantic map.
pub fn canonical_kir_safety_contract_v1(
    op: &dyn Op,
    context: &Context,
) -> core::result::Result<Option<CanonicalKirSafetyContractV1>, CanonicalKirSafetyCarrierErrorV1> {
    let Some(interface) = op_cast::<dyn CanonicalKirSafetyOpInterface>(op) else {
        return Ok(None);
    };
    if !interface.is_self_contained_canonical_kir() {
        return Err(CanonicalKirSafetyCarrierErrorV1::NonSelfContainedInterface);
    }

    macro_rules! operation_carrier {
        ($ty:ty, $family:expr) => {
            if let Some(carrier) = op.downcast_ref::<$ty>() {
                let family = $family;
                let contract = carrier
                    .canonical_contract(context)
                    .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?;
                let coordinate = carrier
                    .canonical_coordinate(context)
                    .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?;
                let graph_epoch = carrier
                    .canonical_graph_epoch(context)
                    .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?;
                return Ok(Some(CanonicalKirSafetyContractV1::Operation {
                    family,
                    contract,
                    coordinate,
                    graph_epoch,
                }));
            }
        };
    }
    operation_carrier!(IntrinsicOp, CanonicalKirSafetyFamilyV1::Intrinsic);
    operation_carrier!(
        MemoryIntrinsicOp,
        CanonicalKirSafetyFamilyV1::MemoryIntrinsic
    );
    operation_carrier!(AllocaOp, CanonicalKirSafetyFamilyV1::Alloca);
    operation_carrier!(GuardedLoadOp, CanonicalKirSafetyFamilyV1::GuardedLoad);
    operation_carrier!(GuardedStoreOp, CanonicalKirSafetyFamilyV1::GuardedStore);
    operation_carrier!(CanonicalBarrierOp, CanonicalKirSafetyFamilyV1::Barrier);
    operation_carrier!(AtomicOp, CanonicalKirSafetyFamilyV1::Atomic);
    operation_carrier!(CanonicalFenceOp, CanonicalKirSafetyFamilyV1::Fence);
    operation_carrier!(
        WorkgroupBarrierOp,
        CanonicalKirSafetyFamilyV1::WorkgroupBarrier
    );
    operation_carrier!(
        WorkgroupMemoryOp,
        CanonicalKirSafetyFamilyV1::WorkgroupMemory
    );
    operation_carrier!(MatrixOp, CanonicalKirSafetyFamilyV1::Matrix);
    operation_carrier!(
        Gfx950LdsTransposeOp,
        CanonicalKirSafetyFamilyV1::TargetSpecificLdsTranspose
    );
    operation_carrier!(WaveOp, CanonicalKirSafetyFamilyV1::Wave);
    operation_carrier!(InlineAssemblyOp, CanonicalKirSafetyFamilyV1::InlineAssembly);

    macro_rules! switch_carrier {
        ($ty:ty, $family:expr) => {
            if let Some(carrier) = op.downcast_ref::<$ty>() {
                let family = $family;
                let contract = carrier
                    .canonical_terminator(context)
                    .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?;
                let coordinate = carrier
                    .canonical_coordinate(context)
                    .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?;
                let graph_epoch = carrier
                    .canonical_graph_epoch(context)
                    .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?;
                return Ok(Some(CanonicalKirSafetyContractV1::Terminator {
                    family,
                    contract,
                    coordinate,
                    graph_epoch,
                }));
            }
        };
    }
    switch_carrier!(SwitchOp, CanonicalKirSafetyFamilyV1::Switch);
    switch_carrier!(IntegerSwitchOp, CanonicalKirSafetyFamilyV1::IntegerSwitch);

    if let Some(carrier) = op.downcast_ref::<UnreachableOp>() {
        let family = CanonicalKirSafetyFamilyV1::Unreachable;
        return Ok(Some(CanonicalKirSafetyContractV1::Terminator {
            family,
            contract: carrier
                .contract(context)
                .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?,
            coordinate: carrier
                .coordinate(context)
                .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?,
            graph_epoch: carrier
                .graph_epoch(context)
                .ok_or(CanonicalKirSafetyCarrierErrorV1::MalformedCarrier(family))?,
        }));
    }

    Err(CanonicalKirSafetyCarrierErrorV1::UnknownInterfaceImplementation)
}

/// Extracts the complete typed projection used by ranked preservation.
/// Unknown interface implementations and malformed payloads fail closed.
pub fn canonical_kir_safety_semantic_projection_v1(
    op: &dyn Op,
    context: &Context,
) -> core::result::Result<
    Option<CanonicalKirSafetySemanticProjectionV1>,
    CanonicalKirSafetyCarrierErrorV1,
> {
    canonical_kir_safety_contract_v1(op, context)?
        .map(CanonicalKirSafetySemanticProjectionV1::try_new)
        .transpose()
}

pub fn remap_canonical_switch(
    terminator: Terminator,
    selector: ValueId,
    successors: &[BlockId],
    successor_arguments: &[Vec<ValueId>],
) -> Option<Terminator> {
    if successors.len() != successor_arguments.len() {
        return None;
    }
    Some(match terminator {
        Terminator::Switch {
            cases,
            default_target: _,
            default_arguments: _,
            ..
        } if successors.len() == cases.len() + 1 => {
            let case_count = cases.len();
            let cases = cases
                .into_iter()
                .zip(successors[..case_count].iter().copied())
                .zip(successor_arguments[..case_count].iter().cloned())
                .map(|((mut case, target), arguments)| {
                    case.target = target;
                    case.arguments = arguments;
                    case
                })
                .collect();
            Terminator::Switch {
                selector,
                cases,
                default_target: successors[case_count],
                default_arguments: successor_arguments[case_count].clone(),
            }
        }
        Terminator::IntegerSwitch {
            cases,
            default_target: _,
            default_arguments: _,
            ..
        } if successors.len() == cases.len() + 1 => {
            let case_count = cases.len();
            let cases = cases
                .into_iter()
                .zip(successors[..case_count].iter().copied())
                .zip(successor_arguments[..case_count].iter().cloned())
                .map(|((mut case, target), arguments)| {
                    case.target = target;
                    case.arguments = arguments;
                    case
                })
                .collect();
            Terminator::IntegerSwitch {
                selector,
                cases,
                default_target: successors[case_count],
                default_arguments: successor_arguments[case_count].clone(),
            }
        }
        _ => return None,
    })
}
