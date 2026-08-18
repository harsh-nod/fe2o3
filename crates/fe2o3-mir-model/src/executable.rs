use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::control_flow::{MirControlFlowAnalysis, MirControlFlowError, analyze_mir_control_flow};
use crate::{
    MirAddressSpace, MirMutability, MirScalarType, MirSemanticType, MirTypeKind,
    MirTypeValidationError,
};

pub const EXECUTABLE_MIR_VERSION: u16 = 1;
pub const MAX_EXECUTABLE_TYPES: usize = 1_024;
pub const MAX_EXECUTABLE_FUNCTIONS: usize = 256;
pub const MAX_EXECUTABLE_LOCALS: usize = 4_096;
pub const MAX_EXECUTABLE_BLOCKS: usize = 4_096;
pub const MAX_EXECUTABLE_BLOCK_PARAMETERS: usize = 4_096;
pub const MAX_EXECUTABLE_STATEMENTS_PER_BLOCK: usize = 8_192;
pub const MAX_EXECUTABLE_STATEMENTS: usize = 65_536;
pub const MAX_EXECUTABLE_PROJECTIONS: usize = 32;
pub const MAX_EXECUTABLE_EDGE_ARGUMENTS: usize = 4_096;
pub const MAX_EXECUTABLE_CALL_ARGUMENTS: usize = 256;
pub const MAX_EXECUTABLE_CALLABLES: usize = 1_024;
pub const MAX_EXECUTABLE_SWITCH_TARGETS: usize = 1_024;
pub const MAX_EXECUTABLE_IDENTITY_BYTES: usize = 4_096;
pub const MAX_EXECUTABLE_SOURCE_FILE_BYTES: usize = 4_096;
pub const MAX_EXECUTABLE_TYPE_DEPTH: usize = 64;
pub const MAX_EXECUTABLE_TYPE_NODES: usize = 65_536;
pub const MAX_EXECUTABLE_TYPE_ITEMS: usize = 65_536;
pub const MAX_EXECUTABLE_FIELDS: usize = 4_096;
pub const MAX_EXECUTABLE_VARIANTS: usize = 1_024;
/// Closed AMDGPU address-space range supported by the gfx942 executable MIR V1 profile.
pub const MAX_EXECUTABLE_ADDRESS_SPACE: u32 = 6;
pub const GFX942_TARGET_TRIPLE: &str = "amdgcn-amd-amdhsa";
pub const GFX942_TARGET_CPU: &str = "gfx942";
pub const GFX942_TARGET_FEATURES: &str = "-wavefrontsize32,+wavefrontsize64";
pub const GFX942_TARGET_DATA_LAYOUT: &str = "e-p:64:64-p1:64:64-p2:32:32-p3:32:32-p4:64:64-p5:32:32-p6:32:32-p7:160:256:256:32-p8:128:128:128:48-p9:192:256:256:32-i64:64-v16:16-v24:32-v32:32-v48:64-v96:128-v192:256-v256:256-v512:512-v1024:1024-v2048:2048-n32:64-S32-A5-G1-ni:7:8:9";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirExecutableVersion {
    V1,
}

impl MirExecutableVersion {
    pub const fn number(self) -> u16 {
        match self {
            Self::V1 => EXECUTABLE_MIR_VERSION,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MirTypeId(pub u32);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MirLocalId(pub u32);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MirBlockId(pub u32);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MirValueId(pub u32);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirSourceSpan {
    pub file: String,
    pub byte_start: u32,
    pub byte_end: u32,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirExecutableTargetProfile {
    Gfx942,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirPointerAbi {
    pub address_space: MirAddressSpace,
    pub width_bits: u16,
    pub abi_alignment_bits: u16,
}

pub const GFX942_POINTER_ABIS: [MirPointerAbi; 7] = [
    MirPointerAbi {
        address_space: MirAddressSpace(0),
        width_bits: 64,
        abi_alignment_bits: 64,
    },
    MirPointerAbi {
        address_space: MirAddressSpace(1),
        width_bits: 64,
        abi_alignment_bits: 64,
    },
    MirPointerAbi {
        address_space: MirAddressSpace(2),
        width_bits: 32,
        abi_alignment_bits: 32,
    },
    MirPointerAbi {
        address_space: MirAddressSpace(3),
        width_bits: 32,
        abi_alignment_bits: 32,
    },
    MirPointerAbi {
        address_space: MirAddressSpace(4),
        width_bits: 64,
        abi_alignment_bits: 64,
    },
    MirPointerAbi {
        address_space: MirAddressSpace(5),
        width_bits: 32,
        abi_alignment_bits: 32,
    },
    MirPointerAbi {
        address_space: MirAddressSpace(6),
        width_bits: 32,
        abi_alignment_bits: 32,
    },
];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirExecutableTarget {
    pub profile: MirExecutableTargetProfile,
    pub triple: String,
    pub cpu: String,
    pub features: String,
    pub data_layout: String,
    /// Width of Rust `usize` and MIR index locals for this target.
    pub pointer_width_bits: u16,
    /// Width returned by target thread-index operations.
    pub thread_index_width_bits: u16,
    /// Canonical, strictly sorted pointer ABI entries for every supported
    /// executable address space.
    pub pointer_abis: Vec<MirPointerAbi>,
}

impl MirExecutableTarget {
    pub fn gfx942() -> Self {
        Self {
            profile: MirExecutableTargetProfile::Gfx942,
            triple: GFX942_TARGET_TRIPLE.to_owned(),
            cpu: GFX942_TARGET_CPU.to_owned(),
            features: GFX942_TARGET_FEATURES.to_owned(),
            data_layout: GFX942_TARGET_DATA_LAYOUT.to_owned(),
            pointer_width_bits: 64,
            thread_index_width_bits: 32,
            pointer_abis: GFX942_POINTER_ABIS.to_vec(),
        }
    }

    fn pointer_abi(&self, address_space: MirAddressSpace) -> Option<MirPointerAbi> {
        self.pointer_abis
            .binary_search_by_key(&address_space, |entry| entry.address_space)
            .ok()
            .map(|index| self.pointer_abis[index])
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirIntrinsic {
    CopyNonOverlapping,
    PointerDistance,
    VolatileLoad,
    VolatileStore,
}

impl MirIntrinsic {
    fn identity(&self) -> &'static str {
        match self {
            Self::CopyNonOverlapping => "fe2o3.copy_nonoverlapping",
            Self::PointerDistance => "fe2o3.pointer_distance",
            Self::VolatileLoad => "fe2o3.volatile_load",
            Self::VolatileStore => "fe2o3.volatile_store",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirCallAuthority {
    /// A function body present in this module.
    DefinedFunction,
    /// A separately verified device ABI contract.
    DeviceImport { contract: String },
    /// A closed, compiler-owned intrinsic operation.
    Intrinsic(MirIntrinsic),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirCallReturn {
    Diverging,
    Value(MirTypeId),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirCallSignature {
    pub inputs: Vec<MirTypeId>,
    pub output: MirCallReturn,
    pub can_unwind: bool,
}

/// A trusted device import signature expressed independently of a module's
/// attacker-controlled type table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirExternalCallSignature {
    pub inputs: Vec<MirSemanticType>,
    pub output: MirExternalCallReturn,
    pub can_unwind: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirExternalCallReturn {
    Diverging,
    Value(MirSemanticType),
}

/// One device import authorized by the embedding process's trust policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirAuthorizedDeviceImport {
    pub identity: String,
    pub contract: String,
    pub signature: MirExternalCallSignature,
}

/// An external trust root. This registry is deliberately absent from the
/// executable MIR wire format; module declarations can only reference it.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirExternalCallRegistry {
    entries: Vec<MirAuthorizedDeviceImport>,
}

impl MirExternalCallRegistry {
    pub fn try_new(
        entries: Vec<MirAuthorizedDeviceImport>,
    ) -> Result<Self, MirExecutableValidationError> {
        bounded_len(
            "external_registry.entries",
            entries.len(),
            0,
            MAX_EXECUTABLE_CALLABLES,
        )?;
        let mut previous: Option<&str> = None;
        let mut signature_types = 0_usize;
        for (index, entry) in entries.iter().enumerate() {
            let path = format!("external_registry.entries[{index}]");
            validate_identity(&format!("{path}.identity"), &entry.identity)?;
            validate_identity(&format!("{path}.contract"), &entry.contract)?;
            if is_intrinsic_identity(&entry.identity) {
                return Err(error(
                    format!("{path}.identity"),
                    "external imports cannot claim the compiler intrinsic namespace",
                ));
            }
            if previous.is_some_and(|value| value >= entry.identity.as_str()) {
                return Err(error(
                    format!("{path}.identity"),
                    "external registry entries must be strictly sorted by identity",
                ));
            }
            previous = Some(&entry.identity);
            bounded_len(
                &format!("{path}.signature.inputs"),
                entry.signature.inputs.len(),
                0,
                MAX_EXECUTABLE_CALL_ARGUMENTS,
            )?;
            signature_types = signature_types
                .checked_add(entry.signature.inputs.len())
                .and_then(|count| {
                    count.checked_add(usize::from(matches!(
                        &entry.signature.output,
                        MirExternalCallReturn::Value(_)
                    )))
                })
                .ok_or_else(|| error(&path, "external signature type count overflow"))?;
            if signature_types > MAX_EXECUTABLE_TYPE_ITEMS {
                return Err(error(
                    &path,
                    format!(
                        "external registry exceeds {MAX_EXECUTABLE_TYPE_ITEMS} signature types"
                    ),
                ));
            }
            validate_type_budget_at(&entry.signature.inputs, &format!("{path}.signature.inputs"))?;
            for (type_index, ty) in entry.signature.inputs.iter().enumerate() {
                let type_path = format!("{path}.signature.inputs[{type_index}]");
                ty.validate()
                    .map_err(|source| map_type_error(&type_path, source))?;
            }
            if let MirExternalCallReturn::Value(ty) = &entry.signature.output {
                let type_path = format!("{path}.signature.output");
                validate_type_budget_at(std::slice::from_ref(ty), &type_path)?;
                ty.validate()
                    .map_err(|source| map_type_error(&type_path, source))?;
            }
        }
        Ok(Self { entries })
    }

    fn find(&self, identity: &str) -> Option<&MirAuthorizedDeviceImport> {
        self.entries
            .binary_search_by(|entry| entry.identity.as_str().cmp(identity))
            .ok()
            .map(|index| &self.entries[index])
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirCallable {
    pub identity: String,
    pub authority: MirCallAuthority,
    pub signature: MirCallSignature,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirExecutableModule {
    pub version: MirExecutableVersion,
    pub target: MirExecutableTarget,
    /// Types are sorted by their canonical semantic representation. All type
    /// references in the module are stable indexes into this table.
    pub types: Vec<MirSemanticType>,
    /// Callable declarations are strictly sorted by identity.
    pub callables: Vec<MirCallable>,
    /// Functions are sorted by monomorphized identity.
    pub functions: Vec<MirFunction>,
}

/// An executable module whose complete structure and external authorities
/// were validated together. The authority context is retained privately so
/// verified transformations can revalidate their output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedMirExecutableModule {
    module: MirExecutableModule,
    registry: MirExternalCallRegistry,
}

impl ValidatedMirExecutableModule {
    pub fn as_module(&self) -> &MirExecutableModule {
        &self.module
    }

    pub fn into_unvalidated(self) -> MirExecutableModule {
        self.module
    }

    pub(crate) fn registry(&self) -> &MirExternalCallRegistry {
        &self.registry
    }
}

impl Deref for ValidatedMirExecutableModule {
    type Target = MirExecutableModule;

    fn deref(&self) -> &Self::Target {
        self.as_module()
    }
}

impl AsRef<MirExecutableModule> for ValidatedMirExecutableModule {
    fn as_ref(&self) -> &MirExecutableModule {
        self.as_module()
    }
}

impl PartialEq<MirExecutableModule> for ValidatedMirExecutableModule {
    fn eq(&self, other: &MirExecutableModule) -> bool {
        self.module == *other
    }
}

impl PartialEq<ValidatedMirExecutableModule> for MirExecutableModule {
    fn eq(&self, other: &ValidatedMirExecutableModule) -> bool {
        self == &other.module
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirFunction {
    pub identity: String,
    pub body: MirBody,
    pub span: Option<MirSourceSpan>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirBody {
    pub form: MirBodyForm,
    pub locals: Vec<MirLocalDecl>,
    pub blocks: Vec<MirBasicBlock>,
    pub entry: MirBlockId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirBodyForm {
    Places,
    /// A mixed slot/SSA form. Promoted locals may appear only as parameter
    /// origins and may never be accessed through a place.
    Ssa {
        promoted_locals: Vec<MirLocalId>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirLocalKind {
    Return,
    Argument,
    User,
    Temporary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirLocalDecl {
    pub ty: MirTypeId,
    pub kind: MirLocalKind,
    pub mutable: bool,
    /// Address space occupied by this local's storage slot.
    pub storage_address_space: MirAddressSpace,
    pub name: Option<String>,
    pub span: Option<MirSourceSpan>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirBasicBlock {
    pub parameters: Vec<MirBlockParameter>,
    pub statements: Vec<MirStatement>,
    pub terminator: MirTerminator,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirBlockParameter {
    pub value: MirValueId,
    pub ty: MirTypeId,
    /// The source local represented by a mem2reg parameter. `None` is reserved
    /// for values introduced by later verified transformations.
    pub origin: Option<MirLocalId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirPlace {
    pub local: MirLocalId,
    pub projection: Vec<MirProjection>,
    /// The importer records the final projected type. Verification derives it
    /// independently from the local and projection sequence.
    pub ty: MirTypeId,
}

impl MirPlace {
    pub const fn local(local: MirLocalId, ty: MirTypeId) -> Self {
        Self {
            local,
            projection: Vec::new(),
            ty,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirProjection {
    Deref,
    Field {
        index: u32,
    },
    Index {
        local: MirLocalId,
    },
    ConstantIndex {
        offset: u64,
        /// Canonical rustc metadata. Verification requires equality with the
        /// statically derived base-array length and never trusts it as a bound.
        min_length: u64,
        from_end: bool,
    },
    Subslice {
        from: u64,
        to: u64,
        from_end: bool,
        /// Canonical rustc metadata, not authority. V1 supports only static
        /// arrays and requires this to equal the base-array length.
        min_length: u64,
    },
    Downcast {
        variant: u32,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirConstant {
    pub ty: MirTypeId,
    pub value: MirConstantValue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirConstantValue {
    Unit,
    Bool(bool),
    /// Raw two's-complement bits for signed integers.
    Integer(u128),
    /// Raw IEEE or target-defined bits. The semantic type fixes the width.
    FloatBits(u128),
    ZeroSized,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirOperand {
    Copy(MirPlace),
    Move(MirPlace),
    Constant(MirConstant),
    Value(MirValueId),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitXor,
    BitAnd,
    BitOr,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl MirBinaryOp {
    fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::Ne | Self::Lt | Self::Le | Self::Gt | Self::Ge
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirUnaryOp {
    Not,
    Neg,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirCastKind {
    IntToInt,
    IntToFloat,
    FloatToInt,
    FloatToFloat,
    PointerToPointer,
    PointerToInt,
    IntToPointer,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirAggregateKind {
    Tuple,
    Array,
    Adt { identity: String, variant: u32 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirRvalue {
    Use(MirOperand),
    BinaryOp {
        op: MirBinaryOp,
        lhs: MirOperand,
        rhs: MirOperand,
    },
    CheckedBinaryOp {
        op: MirBinaryOp,
        lhs: MirOperand,
        rhs: MirOperand,
    },
    UnaryOp {
        op: MirUnaryOp,
        operand: MirOperand,
    },
    Cast {
        kind: MirCastKind,
        operand: MirOperand,
        ty: MirTypeId,
    },
    Ref {
        mutability: MirMutability,
        place: MirPlace,
        ty: MirTypeId,
    },
    AddressOf {
        mutability: MirMutability,
        place: MirPlace,
        ty: MirTypeId,
    },
    Len(MirPlace),
    Discriminant(MirPlace),
    Aggregate {
        kind: MirAggregateKind,
        operands: Vec<MirOperand>,
    },
    Repeat {
        operand: MirOperand,
        count: u64,
    },
    ThreadIndex1d,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirStatement {
    pub kind: MirStatementKind,
    pub span: Option<MirSourceSpan>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirStatementKind {
    Assign {
        place: MirPlace,
        value: MirRvalue,
    },
    Define {
        value: MirValueId,
        ty: MirTypeId,
        rvalue: MirRvalue,
    },
    SetDiscriminant {
        place: MirPlace,
        variant: u32,
    },
    StorageLive(MirLocalId),
    StorageDead(MirLocalId),
    Deinit(MirPlace),
    Nop,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirTerminator {
    pub kind: MirTerminatorKind,
    pub span: Option<MirSourceSpan>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirEdge {
    pub target: MirBlockId,
    pub arguments: Vec<MirOperand>,
}

impl MirEdge {
    pub const fn new(target: MirBlockId) -> Self {
        Self {
            target,
            arguments: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirCallee {
    Direct(String),
    Intrinsic(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirUnwindAction {
    Continue,
    Unreachable,
    Terminate,
    Cleanup(MirEdge),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirCall {
    pub callee: MirCallee,
    pub arguments: Vec<MirOperand>,
    pub destination: Option<MirPlace>,
    pub target: Option<MirEdge>,
    pub unwind: MirUnwindAction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirAssertMessage {
    BoundsCheck,
    Overflow,
    DivisionByZero,
    User(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirTerminatorKind {
    Goto(MirEdge),
    SwitchInt {
        discr: MirOperand,
        targets: Vec<(u128, MirEdge)>,
        otherwise: MirEdge,
    },
    Return,
    Unreachable,
    Call(MirCall),
    Drop {
        place: MirPlace,
        target: MirEdge,
        unwind: MirUnwindAction,
    },
    Assert {
        condition: MirOperand,
        expected: bool,
        message: MirAssertMessage,
        target: MirEdge,
        unwind: MirUnwindAction,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirExecutableValidationError {
    path: String,
    reason: String,
}

impl MirExecutableValidationError {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub(crate) fn new(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for MirExecutableValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for MirExecutableValidationError {}

impl MirExecutableModule {
    pub fn validate(&self) -> Result<ValidatedMirExecutableModule, MirExecutableValidationError> {
        self.validate_with_registry(&MirExternalCallRegistry::default())
    }

    /// Validates this module against a process-supplied device import trust
    /// root. The registry must never be populated from this module's bytes.
    pub fn validate_with_registry(
        &self,
        registry: &MirExternalCallRegistry,
    ) -> Result<ValidatedMirExecutableModule, MirExecutableValidationError> {
        if self.version.number() != EXECUTABLE_MIR_VERSION {
            return Err(error(
                "module.version",
                "unsupported executable MIR version",
            ));
        }
        bounded_len("module.types", self.types.len(), 1, MAX_EXECUTABLE_TYPES)?;
        validate_executable_target(&self.target)?;
        bounded_len(
            "module.functions",
            self.functions.len(),
            1,
            MAX_EXECUTABLE_FUNCTIONS,
        )?;

        validate_type_budget(&self.types)?;
        let mut previous_type = None;
        for (index, ty) in self.types.iter().enumerate() {
            let path = format!("module.types[{index}]");
            ty.validate()
                .map_err(|source| map_type_error(&path, source))?;
            validate_target_type_abi(&path, ty, &self.target)?;
            let canonical = ty
                .canonical_text()
                .map_err(|source| map_type_error(&path, source))?;
            if previous_type
                .as_ref()
                .is_some_and(|previous| previous >= &canonical)
            {
                return Err(error(
                    path,
                    "type table must be strictly sorted by canonical type text",
                ));
            }
            previous_type = Some(canonical);
        }

        self.validate_namespace(registry)?;
        self.validate_callables(registry)?;

        let mut previous_identity: Option<&str> = None;
        for (index, function) in self.functions.iter().enumerate() {
            let path = format!("module.functions[{index}]");
            validate_identity(&format!("{path}.identity"), &function.identity)?;
            if previous_identity.is_some_and(|previous| previous >= function.identity.as_str()) {
                return Err(error(
                    format!("{path}.identity"),
                    "functions must be strictly sorted by identity",
                ));
            }
            previous_identity = Some(&function.identity);
            validate_span_opt(&format!("{path}.span"), function.span.as_ref())?;
            Verifier::new(self, function, path).verify()?;
        }
        Ok(ValidatedMirExecutableModule {
            module: self.clone(),
            registry: registry.clone(),
        })
    }

    fn validate_namespace(
        &self,
        registry: &MirExternalCallRegistry,
    ) -> Result<(), MirExecutableValidationError> {
        bounded_len(
            "module.callables",
            self.callables.len(),
            0,
            MAX_EXECUTABLE_CALLABLES,
        )?;
        let mut previous_callable: Option<&str> = None;
        for (index, callable) in self.callables.iter().enumerate() {
            let path = format!("module.callables[{index}].identity");
            validate_identity(&path, &callable.identity)?;
            if previous_callable.is_some_and(|value| value >= callable.identity.as_str()) {
                return Err(error(
                    path,
                    "callable identities must be globally unique and strictly sorted",
                ));
            }
            previous_callable = Some(&callable.identity);
            match &callable.authority {
                MirCallAuthority::DefinedFunction => {
                    if is_intrinsic_identity(&callable.identity) {
                        return Err(error(
                            path,
                            "defined callable collides with the compiler intrinsic namespace",
                        ));
                    }
                    if registry.find(&callable.identity).is_some() {
                        return Err(error(
                            path,
                            "defined callable collides with the trusted import namespace",
                        ));
                    }
                }
                MirCallAuthority::DeviceImport { .. }
                    if is_intrinsic_identity(&callable.identity) =>
                {
                    return Err(error(
                        path,
                        "device import collides with the compiler intrinsic namespace",
                    ));
                }
                MirCallAuthority::DeviceImport { .. } | MirCallAuthority::Intrinsic(_) => {}
            }
        }

        let mut previous_function: Option<&str> = None;
        for (index, function) in self.functions.iter().enumerate() {
            let path = format!("module.functions[{index}].identity");
            validate_identity(&path, &function.identity)?;
            if previous_function.is_some_and(|value| value >= function.identity.as_str()) {
                return Err(error(
                    path,
                    "defined function identities must be globally unique and strictly sorted",
                ));
            }
            previous_function = Some(&function.identity);
            if is_intrinsic_identity(&function.identity) {
                return Err(error(
                    path,
                    "defined function collides with the compiler intrinsic namespace",
                ));
            }
            if registry.find(&function.identity).is_some() {
                return Err(error(
                    path,
                    "defined function collides with the trusted import namespace",
                ));
            }
            if let Ok(callable_index) = self
                .callables
                .binary_search_by(|callable| callable.identity.cmp(&function.identity))
                && !matches!(
                    self.callables[callable_index].authority,
                    MirCallAuthority::DefinedFunction
                )
            {
                return Err(error(
                    path,
                    "defined function cannot shadow an import or intrinsic declaration",
                ));
            }
        }
        Ok(())
    }

    fn validate_callables(
        &self,
        registry: &MirExternalCallRegistry,
    ) -> Result<(), MirExecutableValidationError> {
        bounded_len(
            "module.callables",
            self.callables.len(),
            0,
            MAX_EXECUTABLE_CALLABLES,
        )?;
        let mut previous: Option<&str> = None;
        for (index, callable) in self.callables.iter().enumerate() {
            let path = format!("module.callables[{index}]");
            validate_identity(&format!("{path}.identity"), &callable.identity)?;
            if previous.is_some_and(|value| value >= callable.identity.as_str()) {
                return Err(error(
                    format!("{path}.identity"),
                    "callables must be strictly sorted by identity",
                ));
            }
            previous = Some(&callable.identity);
            bounded_len(
                &format!("{path}.signature.inputs"),
                callable.signature.inputs.len(),
                0,
                MAX_EXECUTABLE_CALL_ARGUMENTS,
            )?;
            for (input_index, ty) in callable.signature.inputs.iter().copied().enumerate() {
                if self.type_at(ty).is_none() {
                    return Err(error(
                        format!("{path}.signature.inputs[{input_index}]"),
                        format!("type {} does not exist", ty.0),
                    ));
                }
            }
            if let MirCallReturn::Value(ty) = callable.signature.output
                && self.type_at(ty).is_none()
            {
                return Err(error(
                    format!("{path}.signature.output"),
                    format!("type {} does not exist", ty.0),
                ));
            }
            match &callable.authority {
                MirCallAuthority::DefinedFunction => {
                    let Some(function) = self
                        .functions
                        .iter()
                        .find(|function| function.identity == callable.identity)
                    else {
                        return Err(error(path, "defined callable has no function body"));
                    };
                    let inputs = function
                        .body
                        .locals
                        .iter()
                        .skip(1)
                        .take_while(|local| local.kind == MirLocalKind::Argument)
                        .map(|local| local.ty)
                        .collect::<Vec<_>>();
                    let return_ty = function
                        .body
                        .locals
                        .first()
                        .map(|local| local.ty)
                        .ok_or_else(|| error(&path, "defined callable body has no return local"))?;
                    if inputs != callable.signature.inputs
                        || callable.signature.output != MirCallReturn::Value(return_ty)
                    {
                        return Err(error(
                            path,
                            "defined callable signature does not match its body",
                        ));
                    }
                    if !callable.signature.can_unwind && self.body_may_unwind(function) {
                        return Err(error(
                            format!("{path}.signature.can_unwind"),
                            "defined callable body may unwind but its signature forbids unwinding",
                        ));
                    }
                }
                MirCallAuthority::DeviceImport { contract } => {
                    validate_identity(&format!("{path}.authority.contract"), contract)?;
                    let authorized = registry.find(&callable.identity).ok_or_else(|| {
                        error(
                            &path,
                            "device import is absent from the external authority registry",
                        )
                    })?;
                    if authorized.contract != *contract
                        || !self
                            .signature_matches_external(&callable.signature, &authorized.signature)
                    {
                        return Err(error(
                            &path,
                            "device import declaration does not exactly match its external authority",
                        ));
                    }
                }
                MirCallAuthority::Intrinsic(intrinsic) => {
                    if callable.identity != intrinsic.identity() {
                        return Err(error(
                            format!("{path}.identity"),
                            "intrinsic identity does not match its closed authority",
                        ));
                    }
                    self.validate_intrinsic_signature(&path, callable, intrinsic)?;
                }
            }
        }
        Ok(())
    }

    pub fn type_at(&self, id: MirTypeId) -> Option<&MirSemanticType> {
        self.types.get(id.0 as usize)
    }

    fn signature_matches_external(
        &self,
        declared: &MirCallSignature,
        authorized: &MirExternalCallSignature,
    ) -> bool {
        declared.can_unwind == authorized.can_unwind
            && declared.inputs.len() == authorized.inputs.len()
            && declared
                .inputs
                .iter()
                .zip(&authorized.inputs)
                .all(|(id, ty)| self.type_at(*id) == Some(ty))
            && match (&declared.output, &authorized.output) {
                (MirCallReturn::Diverging, MirExternalCallReturn::Diverging) => true,
                (MirCallReturn::Value(id), MirExternalCallReturn::Value(ty)) => {
                    self.type_at(*id) == Some(ty)
                }
                _ => false,
            }
    }

    fn body_may_unwind(&self, function: &MirFunction) -> bool {
        function
            .body
            .blocks
            .iter()
            .any(|block| match &block.terminator.kind {
                MirTerminatorKind::Call(call) => {
                    let identity = match &call.callee {
                        MirCallee::Direct(identity) | MirCallee::Intrinsic(identity) => identity,
                    };
                    self.callables
                        .binary_search_by(|callable| callable.identity.as_str().cmp(identity))
                        .ok()
                        .and_then(|index| self.callables.get(index))
                        .is_none_or(|callable| callable.signature.can_unwind)
                        || unwind_action_may_unwind(&call.unwind)
                }
                MirTerminatorKind::Drop { .. } | MirTerminatorKind::Assert { .. } => true,
                MirTerminatorKind::Goto(_)
                | MirTerminatorKind::SwitchInt { .. }
                | MirTerminatorKind::Return
                | MirTerminatorKind::Unreachable => false,
            })
    }

    fn validate_intrinsic_signature(
        &self,
        path: &str,
        callable: &MirCallable,
        intrinsic: &MirIntrinsic,
    ) -> Result<(), MirExecutableValidationError> {
        if callable.signature.can_unwind {
            return Err(error(
                format!("{path}.signature.can_unwind"),
                "intrinsics cannot unwind",
            ));
        }

        match intrinsic {
            MirIntrinsic::CopyNonOverlapping => {
                self.require_intrinsic_arity(path, callable, 3)?;
                self.require_intrinsic_unit_output(path, callable)?;
                let source = self.require_intrinsic_raw_pointer(
                    &format!("{path}.signature.inputs[0]"),
                    callable.signature.inputs[0],
                    MirMutability::Immutable,
                )?;
                let destination = self.require_intrinsic_raw_pointer(
                    &format!("{path}.signature.inputs[1]"),
                    callable.signature.inputs[1],
                    MirMutability::Mutable,
                )?;
                if source.0 != destination.0 || source.1 != destination.1 {
                    return Err(error(
                        format!("{path}.signature.inputs"),
                        "copy_nonoverlapping pointers must have the same pointee and pointer ABI",
                    ));
                }
                self.require_intrinsic_integer(
                    &format!("{path}.signature.inputs[2]"),
                    callable.signature.inputs[2],
                    false,
                    self.target.pointer_width_bits,
                )
            }
            MirIntrinsic::PointerDistance => {
                self.require_intrinsic_arity(path, callable, 2)?;
                let left = self.require_intrinsic_raw_pointer(
                    &format!("{path}.signature.inputs[0]"),
                    callable.signature.inputs[0],
                    MirMutability::Immutable,
                )?;
                let right = self.require_intrinsic_raw_pointer(
                    &format!("{path}.signature.inputs[1]"),
                    callable.signature.inputs[1],
                    MirMutability::Immutable,
                )?;
                if left != right
                    || self.type_at(callable.signature.inputs[0])
                        != self.type_at(callable.signature.inputs[1])
                {
                    return Err(error(
                        format!("{path}.signature.inputs"),
                        "pointer_distance requires identical pointer input types",
                    ));
                }
                let MirCallReturn::Value(output) = callable.signature.output else {
                    return Err(error(
                        format!("{path}.signature.output"),
                        "pointer_distance must return target isize",
                    ));
                };
                self.require_intrinsic_integer(
                    &format!("{path}.signature.output"),
                    output,
                    true,
                    left.1.width_bits,
                )
            }
            MirIntrinsic::VolatileLoad => {
                self.require_intrinsic_arity(path, callable, 1)?;
                let pointee = self.require_intrinsic_raw_pointer(
                    &format!("{path}.signature.inputs[0]"),
                    callable.signature.inputs[0],
                    MirMutability::Immutable,
                )?;
                let MirCallReturn::Value(output) = callable.signature.output else {
                    return Err(error(
                        format!("{path}.signature.output"),
                        "volatile_load must return its pointee type",
                    ));
                };
                if self.type_at(output) != Some(pointee.0) {
                    return Err(error(
                        format!("{path}.signature.output"),
                        "volatile_load output must exactly match its pointee type",
                    ));
                }
                Ok(())
            }
            MirIntrinsic::VolatileStore => {
                self.require_intrinsic_arity(path, callable, 2)?;
                self.require_intrinsic_unit_output(path, callable)?;
                let pointee = self.require_intrinsic_raw_pointer(
                    &format!("{path}.signature.inputs[0]"),
                    callable.signature.inputs[0],
                    MirMutability::Mutable,
                )?;
                if self.type_at(callable.signature.inputs[1]) != Some(pointee.0) {
                    return Err(error(
                        format!("{path}.signature.inputs[1]"),
                        "volatile_store value must exactly match its pointee type",
                    ));
                }
                Ok(())
            }
        }
    }

    fn require_intrinsic_arity(
        &self,
        path: &str,
        callable: &MirCallable,
        expected: usize,
    ) -> Result<(), MirExecutableValidationError> {
        if callable.signature.inputs.len() != expected {
            return Err(error(
                format!("{path}.signature.inputs"),
                format!("{} requires exactly {expected} inputs", callable.identity),
            ));
        }
        Ok(())
    }

    fn require_intrinsic_unit_output(
        &self,
        path: &str,
        callable: &MirCallable,
    ) -> Result<(), MirExecutableValidationError> {
        let MirCallReturn::Value(output) = callable.signature.output else {
            return Err(error(
                format!("{path}.signature.output"),
                "intrinsic must return unit",
            ));
        };
        if !matches!(
            self.type_at(output).map(|ty| &ty.kind),
            Some(MirTypeKind::Unit)
        ) {
            return Err(error(
                format!("{path}.signature.output"),
                "intrinsic must return unit",
            ));
        }
        Ok(())
    }

    fn require_intrinsic_raw_pointer(
        &self,
        path: &str,
        id: MirTypeId,
        expected_mutability: MirMutability,
    ) -> Result<(&MirSemanticType, MirPointerAbi), MirExecutableValidationError> {
        let ty = self
            .type_at(id)
            .expect("callable type references were checked before intrinsic authority");
        let MirTypeKind::RawPointer {
            pointee,
            mutability,
            address_space,
        } = &ty.kind
        else {
            return Err(error(path, "intrinsic input must be a raw pointer"));
        };
        if *mutability != expected_mutability {
            return Err(error(
                path,
                "intrinsic pointer input has the wrong mutability",
            ));
        }
        let abi = self.target.pointer_abi(*address_space).ok_or_else(|| {
            error(
                path,
                "intrinsic pointer address space is absent from the target ABI",
            )
        })?;
        if ty.layout.size != Some(u64::from(abi.width_bits / 8))
            || ty.layout.align != u64::from(abi.abi_alignment_bits / 8)
        {
            return Err(error(
                path,
                "intrinsic pointer input does not match its address-space pointer ABI",
            ));
        }
        Ok((pointee, abi))
    }

    fn require_intrinsic_integer(
        &self,
        path: &str,
        id: MirTypeId,
        signed: bool,
        bits: u16,
    ) -> Result<(), MirExecutableValidationError> {
        if !matches!(
            self.type_at(id).map(|ty| &ty.kind),
            Some(MirTypeKind::Scalar(MirScalarType::Int {
                signed: actual_signed,
                bits: actual_bits,
            })) if *actual_signed == signed && *actual_bits == bits
        ) {
            return Err(error(
                path,
                format!(
                    "intrinsic requires target {}{bits}",
                    if signed { 'i' } else { 'u' }
                ),
            ));
        }
        Ok(())
    }
}

struct Verifier<'a> {
    module: &'a MirExecutableModule,
    function: &'a MirFunction,
    path: String,
    value_types: BTreeMap<MirValueId, MirTypeId>,
    value_blocks: BTreeMap<MirValueId, MirBlockId>,
    promoted: BTreeSet<MirLocalId>,
    total_statements: usize,
    next_value: u32,
    control_flow: Option<MirControlFlowAnalysis>,
}

impl<'a> Verifier<'a> {
    fn new(module: &'a MirExecutableModule, function: &'a MirFunction, path: String) -> Self {
        Self {
            module,
            function,
            path,
            value_types: BTreeMap::new(),
            value_blocks: BTreeMap::new(),
            promoted: BTreeSet::new(),
            total_statements: 0,
            next_value: 0,
            control_flow: None,
        }
    }

    fn verify(mut self) -> Result<(), MirExecutableValidationError> {
        let body = &self.function.body;
        bounded_len(
            &format!("{}.body.locals", self.path),
            body.locals.len(),
            1,
            MAX_EXECUTABLE_LOCALS,
        )?;
        bounded_len(
            &format!("{}.body.blocks", self.path),
            body.blocks.len(),
            1,
            MAX_EXECUTABLE_BLOCKS,
        )?;
        if body.entry != MirBlockId(0) {
            return Err(error(
                format!("{}.body.entry", self.path),
                "canonical executable MIR requires block 0 as entry",
            ));
        }
        self.verify_locals()?;
        self.verify_form()?;
        self.collect_values()?;
        self.control_flow = match analyze_mir_control_flow(body) {
            Ok(analysis) => Some(analysis),
            Err(
                error @ (MirControlFlowError::Irreducible { .. }
                | MirControlFlowError::BlockLimitExceeded { .. }
                | MirControlFlowError::WorkBudgetExceeded { .. }),
            ) => {
                return Err(crate::executable::error(
                    format!("{}.body", self.path),
                    error.to_string(),
                ));
            }
            // Existing edge and reachability verification below owns these
            // diagnostics and preserves their more precise field paths.
            Err(
                MirControlFlowError::EmptyBody
                | MirControlFlowError::InvalidEntry { .. }
                | MirControlFlowError::UnknownSuccessor(_)
                | MirControlFlowError::UnreachableBlock(_),
            ) => None,
        };

        for (index, block) in body.blocks.iter().enumerate() {
            self.verify_block(MirBlockId(index as u32), block)?;
        }
        if self.total_statements > MAX_EXECUTABLE_STATEMENTS {
            return Err(error(
                format!("{}.body", self.path),
                format!(
                    "statement count {} exceeds {MAX_EXECUTABLE_STATEMENTS}",
                    self.total_statements
                ),
            ));
        }
        self.verify_reachability()?;
        self.verify_initialization()?;
        self.verify_variant_state()
    }

    fn verify_locals(&self) -> Result<(), MirExecutableValidationError> {
        let mut saw_non_argument = false;
        for (index, local) in self.function.body.locals.iter().enumerate() {
            let path = format!("{}.body.locals[{index}]", self.path);
            self.require_type(&format!("{path}.ty"), local.ty)?;
            if self.type_at(local.ty).layout.size.is_none() {
                return Err(error(
                    format!("{path}.ty"),
                    "executable MIR local storage types must be Sized",
                ));
            }
            validate_executable_address_space(
                &format!("{path}.storage_address_space"),
                local.storage_address_space,
                &self.module.target,
            )?;
            validate_target_offset_ranges(
                &format!("{path}.ty"),
                self.type_at(local.ty),
                local.storage_address_space,
                &self.module.target,
            )?;
            validate_name_opt(&format!("{path}.name"), local.name.as_deref())?;
            validate_span_opt(&format!("{path}.span"), local.span.as_ref())?;
            if index == 0 {
                if local.kind != MirLocalKind::Return {
                    return Err(error(path, "local 0 must be the return local"));
                }
            } else if local.kind == MirLocalKind::Return {
                return Err(error(path, "only local 0 may be the return local"));
            }
            if index > 0 {
                match local.kind {
                    MirLocalKind::Argument if saw_non_argument => {
                        return Err(error(
                            path,
                            "argument locals must be a contiguous prefix after local 0",
                        ));
                    }
                    MirLocalKind::Argument => {}
                    _ => saw_non_argument = true,
                }
            }
        }
        Ok(())
    }

    fn verify_form(&mut self) -> Result<(), MirExecutableValidationError> {
        if let MirBodyForm::Ssa { promoted_locals } = &self.function.body.form {
            let mut previous = None;
            for (index, local) in promoted_locals.iter().copied().enumerate() {
                let path = format!("{}.body.form.promoted_locals[{index}]", self.path);
                self.require_local(&path, local)?;
                if local == MirLocalId(0) {
                    return Err(error(path, "the return local cannot be promoted"));
                }
                if previous.is_some_and(|value| value >= local) {
                    return Err(error(path, "promoted locals must be strictly sorted"));
                }
                previous = Some(local);
                self.promoted.insert(local);
            }
        }
        Ok(())
    }

    fn collect_values(&mut self) -> Result<(), MirExecutableValidationError> {
        for (block_index, block) in self.function.body.blocks.iter().enumerate() {
            let block_path = format!("{}.body.blocks[{block_index}]", self.path);
            bounded_len(
                &format!("{block_path}.parameters"),
                block.parameters.len(),
                0,
                MAX_EXECUTABLE_BLOCK_PARAMETERS,
            )?;
            if matches!(self.function.body.form, MirBodyForm::Places)
                && !block.parameters.is_empty()
            {
                return Err(error(
                    format!("{block_path}.parameters"),
                    "place form cannot contain block parameters",
                ));
            }
            let mut origins = BTreeSet::new();
            for (parameter_index, parameter) in block.parameters.iter().enumerate() {
                let path = format!("{block_path}.parameters[{parameter_index}]");
                self.collect_value(
                    &path,
                    MirBlockId(block_index as u32),
                    parameter.value,
                    parameter.ty,
                )?;
                if block_index == 0 && parameter.origin.is_none() {
                    return Err(error(
                        format!("{path}.origin"),
                        "entry parameters require an argument-local origin",
                    ));
                }
                if let Some(origin) = parameter.origin {
                    self.require_local(&format!("{path}.origin"), origin)?;
                    if !self.promoted.contains(&origin) {
                        return Err(error(
                            format!("{path}.origin"),
                            "parameter origin is not a promoted local",
                        ));
                    }
                    if self.local(origin).ty != parameter.ty {
                        return Err(error(
                            format!("{path}.ty"),
                            "parameter type does not match its source local",
                        ));
                    }
                    if !origins.insert(origin) {
                        return Err(error(
                            format!("{path}.origin"),
                            "a block has duplicate parameters for one promoted local",
                        ));
                    }
                    if block_index == 0 && self.local(origin).kind != MirLocalKind::Argument {
                        return Err(error(
                            format!("{path}.origin"),
                            "entry parameters may originate only from arguments",
                        ));
                    }
                }
            }
            bounded_len(
                &format!("{block_path}.statements"),
                block.statements.len(),
                0,
                MAX_EXECUTABLE_STATEMENTS_PER_BLOCK,
            )?;
            self.total_statements = self
                .total_statements
                .checked_add(block.statements.len())
                .ok_or_else(|| error(&block_path, "statement count overflow"))?;
            for (statement_index, statement) in block.statements.iter().enumerate() {
                if let MirStatementKind::Define { value, ty, .. } = statement.kind {
                    if matches!(self.function.body.form, MirBodyForm::Places) {
                        return Err(error(
                            format!("{block_path}.statements[{statement_index}]"),
                            "place form cannot contain SSA definitions",
                        ));
                    }
                    self.collect_value(
                        &format!("{block_path}.statements[{statement_index}].value"),
                        MirBlockId(block_index as u32),
                        value,
                        ty,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn collect_value(
        &mut self,
        path: &str,
        block: MirBlockId,
        value: MirValueId,
        ty: MirTypeId,
    ) -> Result<(), MirExecutableValidationError> {
        self.require_type(&format!("{path}.ty"), ty)?;
        if value != MirValueId(self.next_value) {
            return Err(error(
                path,
                format!(
                    "SSA values must be numbered canonically; expected {}, found {}",
                    self.next_value, value.0
                ),
            ));
        }
        self.next_value = self
            .next_value
            .checked_add(1)
            .ok_or_else(|| error(path, "SSA value identity overflow"))?;
        if self.value_types.insert(value, ty).is_some() {
            return Err(error(path, "duplicate SSA value identity"));
        }
        self.value_blocks.insert(value, block);
        Ok(())
    }

    fn verify_block(
        &self,
        block_id: MirBlockId,
        block: &MirBasicBlock,
    ) -> Result<(), MirExecutableValidationError> {
        let block_path = format!("{}.body.blocks[{}]", self.path, block_id.0);
        let mut available = self
            .control_flow
            .as_ref()
            .map(|control_flow| {
                self.value_blocks
                    .iter()
                    .filter_map(|(value, definition)| {
                        (*definition != block_id && control_flow.dominates(*definition, block_id))
                            .then_some(*value)
                    })
                    .collect::<BTreeSet<_>>()
            })
            .unwrap_or_default();
        for parameter in &block.parameters {
            available.insert(parameter.value);
        }
        for (index, statement) in block.statements.iter().enumerate() {
            let path = format!("{block_path}.statements[{index}]");
            validate_span_opt(&format!("{path}.span"), statement.span.as_ref())?;
            match &statement.kind {
                MirStatementKind::Assign { place, value } => {
                    let (destination, writable, _) =
                        self.verify_place_access(&format!("{path}.place"), place)?;
                    if !writable && !self.is_one_time_initialization_place(place) {
                        return Err(error(
                            format!("{path}.place"),
                            "assignment destination is not writable",
                        ));
                    }
                    let source = self.verify_rvalue(&format!("{path}.value"), value, &available)?;
                    self.require_same_type(&path, destination, source)?;
                }
                MirStatementKind::Define { value, ty, rvalue } => {
                    let actual =
                        self.verify_rvalue(&format!("{path}.rvalue"), rvalue, &available)?;
                    self.require_same_type(&path, *ty, actual)?;
                    available.insert(*value);
                }
                MirStatementKind::SetDiscriminant { place, variant } => {
                    let (ty, writable, _) =
                        self.verify_place_access(&format!("{path}.place"), place)?;
                    if !writable {
                        return Err(error(
                            format!("{path}.place"),
                            "set-discriminant destination is not writable",
                        ));
                    }
                    let MirTypeKind::Enum(enum_type) = &self.type_at(ty).kind else {
                        return Err(error(path, "set-discriminant place is not an enum"));
                    };
                    let variant = enum_type
                        .variants
                        .iter()
                        .find(|item| item.index == *variant)
                        .ok_or_else(|| error(&path, "set-discriminant variant does not exist"))?;
                    if !variant.aggregate.fields.is_empty() {
                        return Err(error(
                            path,
                            "set-discriminant cannot select a variant with payload fields",
                        ));
                    }
                }
                MirStatementKind::StorageLive(local) | MirStatementKind::StorageDead(local) => {
                    self.require_local(&path, *local)?;
                    if self.promoted.contains(local) {
                        return Err(error(path, "promoted local retains storage markers"));
                    }
                }
                MirStatementKind::Deinit(place) => {
                    let (_, writable, _) =
                        self.verify_place_access(&format!("{path}.place"), place)?;
                    if !writable {
                        return Err(error(
                            format!("{path}.place"),
                            "deinit destination is not writable",
                        ));
                    }
                }
                MirStatementKind::Nop => {}
            }
        }
        let path = format!("{block_path}.terminator");
        validate_span_opt(&format!("{path}.span"), block.terminator.span.as_ref())?;
        self.verify_terminator(block_id, &path, &block.terminator.kind, &available)
    }

    fn verify_terminator(
        &self,
        source: MirBlockId,
        path: &str,
        terminator: &MirTerminatorKind,
        available: &BTreeSet<MirValueId>,
    ) -> Result<(), MirExecutableValidationError> {
        match terminator {
            MirTerminatorKind::Goto(edge) => self.verify_edge(source, path, edge, available),
            MirTerminatorKind::SwitchInt {
                discr,
                targets,
                otherwise,
            } => {
                let discr_ty = self.verify_operand(&format!("{path}.discr"), discr, available)?;
                if !is_integer_or_bool(&self.type_at(discr_ty).kind) {
                    return Err(error(
                        format!("{path}.discr"),
                        "switch discriminant must be integer or bool",
                    ));
                }
                bounded_len(
                    &format!("{path}.targets"),
                    targets.len(),
                    1,
                    MAX_EXECUTABLE_SWITCH_TARGETS,
                )?;
                let mut previous = None;
                for (index, (value, edge)) in targets.iter().enumerate() {
                    if !switch_value_fits(&self.type_at(discr_ty).kind, *value) {
                        return Err(error(
                            format!("{path}.targets[{index}].0"),
                            "switch value does not fit the discriminant type",
                        ));
                    }
                    if previous.is_some_and(|previous| previous >= *value) {
                        return Err(error(
                            format!("{path}.targets[{index}]"),
                            "switch values must be strictly increasing",
                        ));
                    }
                    previous = Some(*value);
                    self.verify_edge(source, &format!("{path}.targets[{index}]"), edge, available)?;
                }
                self.verify_edge(source, &format!("{path}.otherwise"), otherwise, available)
            }
            MirTerminatorKind::Return | MirTerminatorKind::Unreachable => Ok(()),
            MirTerminatorKind::Call(call) => {
                let identity = match &call.callee {
                    MirCallee::Direct(identity) | MirCallee::Intrinsic(identity) => identity,
                };
                validate_identity(&format!("{path}.callee"), identity)?;
                let callable = self
                    .module
                    .callables
                    .binary_search_by(|item| item.identity.as_str().cmp(identity))
                    .ok()
                    .map(|index| &self.module.callables[index])
                    .ok_or_else(|| {
                        error(
                            format!("{path}.callee"),
                            "callable is absent from the authority registry",
                        )
                    })?;
                let authority_matches = matches!(
                    (&call.callee, &callable.authority),
                    (
                        MirCallee::Direct(_),
                        MirCallAuthority::DefinedFunction | MirCallAuthority::DeviceImport { .. }
                    ) | (MirCallee::Intrinsic(_), MirCallAuthority::Intrinsic(_))
                );
                if !authority_matches {
                    return Err(error(
                        format!("{path}.callee"),
                        "callee kind does not match its registered authority",
                    ));
                }
                bounded_len(
                    &format!("{path}.arguments"),
                    call.arguments.len(),
                    0,
                    MAX_EXECUTABLE_CALL_ARGUMENTS,
                )?;
                if call.arguments.len() != callable.signature.inputs.len() {
                    return Err(error(
                        format!("{path}.arguments"),
                        "call argument count does not match registered signature",
                    ));
                }
                for (index, (operand, expected)) in call
                    .arguments
                    .iter()
                    .zip(&callable.signature.inputs)
                    .enumerate()
                {
                    let actual = self.verify_operand(
                        &format!("{path}.arguments[{index}]"),
                        operand,
                        available,
                    )?;
                    self.require_same_type(
                        &format!("{path}.arguments[{index}]"),
                        *expected,
                        actual,
                    )?;
                }
                if let Some(destination) = &call.destination {
                    let (_, writable, _) =
                        self.verify_place_access(&format!("{path}.destination"), destination)?;
                    let MirCallReturn::Value(output) = callable.signature.output else {
                        return Err(error(path, "diverging callable cannot have a destination"));
                    };
                    self.require_same_type(&format!("{path}.destination"), output, destination.ty)?;
                    if !writable && !self.is_one_time_initialization_place(destination) {
                        return Err(error(
                            format!("{path}.destination"),
                            "call destination is not writable",
                        ));
                    }
                }
                if call.destination.is_some() != call.target.is_some() {
                    return Err(error(
                        path,
                        "call destination and normal target must either both exist or both be absent",
                    ));
                }
                match callable.signature.output {
                    MirCallReturn::Diverging if call.target.is_some() => {
                        return Err(error(
                            path,
                            "diverging callable cannot have a normal target",
                        ));
                    }
                    MirCallReturn::Value(_) if call.target.is_none() => {
                        return Err(error(
                            path,
                            "returning callable requires a destination and normal target",
                        ));
                    }
                    _ => {}
                }
                if !callable.signature.can_unwind
                    && !matches!(
                        call.unwind,
                        MirUnwindAction::Unreachable | MirUnwindAction::Terminate
                    )
                {
                    return Err(error(
                        format!("{path}.unwind"),
                        "callable signature does not authorize unwinding",
                    ));
                }
                if let Some(target) = &call.target {
                    self.verify_edge(source, &format!("{path}.target"), target, available)?;
                }
                self.verify_unwind(source, &format!("{path}.unwind"), &call.unwind, available)
            }
            MirTerminatorKind::Drop {
                place,
                target,
                unwind,
            } => {
                self.verify_place(&format!("{path}.place"), place)?;
                self.verify_edge(source, &format!("{path}.target"), target, available)?;
                self.verify_unwind(source, &format!("{path}.unwind"), unwind, available)
            }
            MirTerminatorKind::Assert {
                condition,
                message,
                target,
                unwind,
                ..
            } => {
                let condition_ty =
                    self.verify_operand(&format!("{path}.condition"), condition, available)?;
                if !matches!(
                    self.type_at(condition_ty).kind,
                    MirTypeKind::Scalar(MirScalarType::Bool)
                ) {
                    return Err(error(
                        format!("{path}.condition"),
                        "assert condition must be bool",
                    ));
                }
                if let MirAssertMessage::User(message) = message {
                    validate_name(&format!("{path}.message"), message)?;
                }
                self.verify_edge(source, &format!("{path}.target"), target, available)?;
                self.verify_unwind(source, &format!("{path}.unwind"), unwind, available)
            }
        }
    }

    fn verify_unwind(
        &self,
        source: MirBlockId,
        path: &str,
        unwind: &MirUnwindAction,
        available: &BTreeSet<MirValueId>,
    ) -> Result<(), MirExecutableValidationError> {
        if let MirUnwindAction::Cleanup(edge) = unwind {
            self.verify_edge(source, path, edge, available)?;
        }
        Ok(())
    }

    fn verify_edge(
        &self,
        source: MirBlockId,
        path: &str,
        edge: &MirEdge,
        available: &BTreeSet<MirValueId>,
    ) -> Result<(), MirExecutableValidationError> {
        self.require_block(&format!("{path}.target"), edge.target)?;
        if edge.target == self.function.body.entry {
            return Err(error(
                format!("{path}.target"),
                "edges to the canonical entry block are not supported",
            ));
        }
        bounded_len(
            &format!("{path}.arguments"),
            edge.arguments.len(),
            0,
            MAX_EXECUTABLE_EDGE_ARGUMENTS,
        )?;
        let parameters = &self.function.body.blocks[edge.target.0 as usize].parameters;
        if edge.arguments.len() != parameters.len() {
            return Err(error(
                format!("{path}.arguments"),
                format!(
                    "edge from block {} supplies {} arguments for {} parameters",
                    source.0,
                    edge.arguments.len(),
                    parameters.len()
                ),
            ));
        }
        for (index, (argument, parameter)) in
            edge.arguments.iter().zip(parameters.iter()).enumerate()
        {
            let actual =
                self.verify_operand(&format!("{path}.arguments[{index}]"), argument, available)?;
            self.require_same_type(&format!("{path}.arguments[{index}]"), parameter.ty, actual)?;
        }
        Ok(())
    }

    fn verify_rvalue(
        &self,
        path: &str,
        rvalue: &MirRvalue,
        available: &BTreeSet<MirValueId>,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        match rvalue {
            MirRvalue::Use(operand) => self.verify_operand(path, operand, available),
            MirRvalue::BinaryOp { op, lhs, rhs } => {
                let lhs_ty = self.verify_operand(&format!("{path}.lhs"), lhs, available)?;
                let rhs_ty = self.verify_operand(&format!("{path}.rhs"), rhs, available)?;
                self.require_same_type(path, lhs_ty, rhs_ty)?;
                if !valid_binary_op(*op, &self.type_at(lhs_ty).kind) {
                    return Err(error(
                        path,
                        "binary operation is invalid for its operand type",
                    ));
                }
                if op.is_comparison() {
                    self.bool_type(path)
                } else {
                    Ok(lhs_ty)
                }
            }
            MirRvalue::CheckedBinaryOp { op, lhs, rhs } => {
                let lhs_ty = self.verify_operand(&format!("{path}.lhs"), lhs, available)?;
                let rhs_ty = self.verify_operand(&format!("{path}.rhs"), rhs, available)?;
                self.require_same_type(path, lhs_ty, rhs_ty)?;
                if !matches!(op, MirBinaryOp::Add | MirBinaryOp::Sub | MirBinaryOp::Mul)
                    || !is_machine_integer(&self.type_at(lhs_ty).kind)
                {
                    return Err(error(
                        path,
                        "checked V1 operations are limited to integer add, sub, and mul",
                    ));
                }
                let bool_ty = self.bool_type(path)?;
                self.find_aggregate_type(path, &MirAggregateKind::Tuple, &[lhs_ty, bool_ty])
            }
            MirRvalue::UnaryOp { op, operand } => {
                let ty = self.verify_operand(&format!("{path}.operand"), operand, available)?;
                let kind = &self.type_at(ty).kind;
                let valid = match op {
                    MirUnaryOp::Not => is_integer_or_bool(kind),
                    MirUnaryOp::Neg => is_signed_integer_or_float(kind),
                };
                if !valid {
                    return Err(error(
                        path,
                        "unary operation is invalid for its operand type",
                    ));
                }
                Ok(ty)
            }
            MirRvalue::Cast { kind, operand, ty } => {
                self.require_type(&format!("{path}.ty"), *ty)?;
                let source = self.verify_operand(&format!("{path}.operand"), operand, available)?;
                if !valid_cast(*kind, self.type_at(source), self.type_at(*ty)) {
                    return Err(error(
                        path,
                        "cast kind does not match source and destination types",
                    ));
                }
                Ok(*ty)
            }
            MirRvalue::Ref {
                mutability,
                place,
                ty,
            } => {
                self.require_type(&format!("{path}.ty"), *ty)?;
                let (_, writable, address_space) =
                    self.verify_place_access(&format!("{path}.place"), place)?;
                if *mutability == MirMutability::Mutable && !writable {
                    return Err(error(path, "mutable reference requires a writable place"));
                }
                self.verify_reference_origin(&format!("{path}.place"), place)?;
                self.verify_reference_type(path, *ty, place.ty, *mutability, address_space, false)?;
                Ok(*ty)
            }
            MirRvalue::AddressOf {
                mutability,
                place,
                ty,
            } => {
                self.require_type(&format!("{path}.ty"), *ty)?;
                let (_, writable, address_space) =
                    self.verify_place_access(&format!("{path}.place"), place)?;
                if *mutability == MirMutability::Mutable && !writable {
                    return Err(error(path, "mutable raw address requires a writable place"));
                }
                self.verify_reference_type(path, *ty, place.ty, *mutability, address_space, true)?;
                Ok(*ty)
            }
            MirRvalue::Len(place) => {
                let ty = self.verify_place(&format!("{path}.place"), place)?;
                if !matches!(
                    self.type_at(ty).kind,
                    MirTypeKind::Slice { .. } | MirTypeKind::Array { .. }
                ) {
                    return Err(error(path, "len operand must be a slice or array"));
                }
                self.usize_type(path)
            }
            MirRvalue::Discriminant(place) => {
                let ty = self.verify_place(&format!("{path}.place"), place)?;
                let MirTypeKind::Enum(enum_ty) = &self.type_at(ty).kind else {
                    return Err(error(path, "discriminant operand must be an enum"));
                };
                self.find_scalar_type(path, enum_ty.discriminant)
            }
            MirRvalue::Aggregate { kind, operands } => {
                if let MirAggregateKind::Adt { identity, .. } = kind {
                    validate_identity(&format!("{path}.identity"), identity)?;
                }
                bounded_len(
                    &format!("{path}.operands"),
                    operands.len(),
                    0,
                    MAX_EXECUTABLE_CALL_ARGUMENTS,
                )?;
                let operand_types = operands
                    .iter()
                    .enumerate()
                    .map(|(index, operand)| {
                        self.verify_operand(
                            &format!("{path}.operands[{index}]"),
                            operand,
                            available,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.find_aggregate_type(path, kind, &operand_types)
            }
            MirRvalue::Repeat { operand, count } => {
                let element =
                    self.verify_operand(&format!("{path}.operand"), operand, available)?;
                self.find_array_type(path, element, *count)
            }
            MirRvalue::ThreadIndex1d => {
                self.unsigned_type(path, self.module.target.thread_index_width_bits)
            }
        }
    }

    fn verify_operand(
        &self,
        path: &str,
        operand: &MirOperand,
        available: &BTreeSet<MirValueId>,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        match operand {
            MirOperand::Copy(place) | MirOperand::Move(place) => self.verify_place(path, place),
            MirOperand::Constant(constant) => self.verify_constant(path, constant),
            MirOperand::Value(value) => {
                let ty =
                    self.value_types.get(value).copied().ok_or_else(|| {
                        error(path, format!("SSA value {} is not defined", value.0))
                    })?;
                if !available.contains(value) {
                    return Err(error(
                        path,
                        format!(
                            "SSA value {} does not dominate this use or is not a prior definition in this block",
                            value.0
                        ),
                    ));
                }
                Ok(ty)
            }
        }
    }

    fn verify_constant(
        &self,
        path: &str,
        constant: &MirConstant,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.require_type(&format!("{path}.ty"), constant.ty)?;
        let ty = self.type_at(constant.ty);
        let valid = match (&constant.value, &ty.kind) {
            (MirConstantValue::Unit, MirTypeKind::Unit) => true,
            (MirConstantValue::Bool(_), MirTypeKind::Scalar(MirScalarType::Bool)) => true,
            (
                MirConstantValue::Integer(bits),
                MirTypeKind::Scalar(MirScalarType::Int { bits: width, .. }),
            ) => *width == 128 || *bits < (1_u128 << *width),
            (MirConstantValue::Integer(bits), MirTypeKind::Scalar(MirScalarType::Char)) => {
                *bits <= u128::from(char::MAX as u32) && !(0xd800..=0xdfff).contains(bits)
            }
            (
                MirConstantValue::FloatBits(bits),
                MirTypeKind::Scalar(MirScalarType::Float { bits: width }),
            ) => *width == 128 || *bits < (1_u128 << *width),
            (MirConstantValue::ZeroSized, _) => ty.has_single_zero_sized_value().unwrap_or(false),
            _ => false,
        };
        if !valid {
            return Err(error(
                path,
                "constant payload does not match its semantic type",
            ));
        }
        Ok(constant.ty)
    }

    fn verify_place(
        &self,
        path: &str,
        place: &MirPlace,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.verify_place_access(path, place).map(|(ty, _, _)| ty)
    }

    fn verify_place_access(
        &self,
        path: &str,
        place: &MirPlace,
    ) -> Result<(MirTypeId, bool, MirAddressSpace), MirExecutableValidationError> {
        self.require_local(&format!("{path}.local"), place.local)?;
        self.require_type(&format!("{path}.ty"), place.ty)?;
        if self.promoted.contains(&place.local) {
            return Err(error(
                path,
                "promoted local is still accessed through a place",
            ));
        }
        bounded_len(
            &format!("{path}.projection"),
            place.projection.len(),
            0,
            MAX_EXECUTABLE_PROJECTIONS,
        )?;

        let mut current = ProjectionState {
            ty: ProjectionType::Type(self.type_at(self.local(place.local).ty)),
            writable: self.local(place.local).mutable,
            address_space: self.local(place.local).storage_address_space,
        };
        for (index, projection) in place.projection.iter().enumerate() {
            current = self.project(&format!("{path}.projection[{index}]"), current, projection)?;
        }
        let actual = match current.ty {
            ProjectionType::Type(ty) => ty,
            ProjectionType::Variant { .. } => {
                return Err(error(path, "a downcast place must project a variant field"));
            }
        };
        if actual != self.type_at(place.ty) {
            return Err(error(
                path,
                "recorded place type does not match its projection",
            ));
        }
        Ok((place.ty, current.writable, current.address_space))
    }

    fn project<'b>(
        &'b self,
        path: &str,
        current: ProjectionState<'b>,
        projection: &MirProjection,
    ) -> Result<ProjectionState<'b>, MirExecutableValidationError> {
        match projection {
            MirProjection::Deref => {
                let ProjectionType::Type(ty) = current.ty else {
                    return Err(error(path, "cannot dereference an enum downcast"));
                };
                match &ty.kind {
                    MirTypeKind::RawPointer {
                        pointee,
                        mutability,
                        address_space,
                    } => Ok(ProjectionState {
                        ty: ProjectionType::Type(pointee),
                        writable: *mutability == MirMutability::Mutable,
                        address_space: *address_space,
                    }),
                    MirTypeKind::Reference {
                        referent,
                        mutability,
                        address_space,
                    } => Ok(ProjectionState {
                        ty: ProjectionType::Type(referent),
                        writable: *mutability == MirMutability::Mutable,
                        address_space: *address_space,
                    }),
                    _ => Err(error(
                        path,
                        "deref projection requires a pointer or reference",
                    )),
                }
            }
            MirProjection::Field { index } => {
                let fields = match current.ty {
                    ProjectionType::Type(ty) => match &ty.kind {
                        MirTypeKind::Tuple(aggregate) => &aggregate.fields,
                        MirTypeKind::Struct(structure) => &structure.aggregate.fields,
                        _ => return Err(error(path, "field projection requires an aggregate")),
                    },
                    ProjectionType::Variant { fields } => fields,
                };
                fields
                    .get(*index as usize)
                    .map(|field| ProjectionState {
                        ty: ProjectionType::Type(&field.ty),
                        writable: current.writable,
                        address_space: current.address_space,
                    })
                    .ok_or_else(|| error(path, "field projection index is out of bounds"))
            }
            MirProjection::Index { .. } => Err(error(
                path,
                "dynamic index projections require an external range witness and are unsupported",
            )),
            MirProjection::ConstantIndex {
                offset,
                min_length,
                from_end,
            } => {
                self.require_target_usize(&format!("{path}.offset"), *offset)?;
                let ProjectionType::Type(ty) = current.ty else {
                    return Err(error(path, "constant index requires a static array"));
                };
                let MirTypeKind::Array { element, length } = &ty.kind else {
                    return Err(error(
                        path,
                        match &ty.kind {
                            MirTypeKind::Slice { .. } => {
                                "slice constant-index projections require an external bound witness"
                            }
                            _ => "constant index requires a static array",
                        },
                    ));
                };
                if min_length != length {
                    return Err(error(
                        format!("{path}.min_length"),
                        "constant-index metadata must equal the static array length",
                    ));
                }
                let in_bounds = if *from_end {
                    *offset > 0 && *offset <= *length
                } else {
                    *offset < *length
                };
                if !in_bounds {
                    return Err(error(
                        path,
                        "constant index is outside the static array bounds",
                    ));
                }
                Ok(ProjectionState {
                    ty: ProjectionType::Type(element),
                    writable: current.writable,
                    address_space: current.address_space,
                })
            }
            MirProjection::Subslice {
                from,
                to,
                from_end,
                min_length,
            } => {
                self.require_target_usize(&format!("{path}.from"), *from)?;
                self.require_target_usize(&format!("{path}.to"), *to)?;
                let ProjectionType::Type(ty) = current.ty else {
                    return Err(error(path, "subslice requires a static array"));
                };
                let MirTypeKind::Array { element, length } = &ty.kind else {
                    return Err(error(
                        path,
                        match &ty.kind {
                            MirTypeKind::Slice { .. } => {
                                "slice subslice projections require an external bound witness"
                            }
                            _ => "subslice requires a static array",
                        },
                    ));
                };
                if min_length != length {
                    return Err(error(
                        format!("{path}.min_length"),
                        "subslice metadata must equal the static array length",
                    ));
                }
                let result_length = if *from_end {
                    let removed = from.checked_add(*to).ok_or_else(|| {
                        error(path, "subslice bounds overflow the static array length")
                    })?;
                    length.checked_sub(removed).ok_or_else(|| {
                        error(path, "subslice bounds exceed the static array length")
                    })?
                } else {
                    if *from > *to || *to > *length {
                        return Err(error(
                            path,
                            "subslice bounds exceed the static array length",
                        ));
                    }
                    *to - *from
                };
                self.find_array_semantic_type(path, element, result_length)
                    .map(|ty| ProjectionState {
                        ty: ProjectionType::Type(ty),
                        writable: current.writable,
                        address_space: current.address_space,
                    })
            }
            MirProjection::Downcast { variant } => {
                let ProjectionType::Type(ty) = current.ty else {
                    return Err(error(path, "nested downcast is invalid"));
                };
                let MirTypeKind::Enum(enum_ty) = &ty.kind else {
                    return Err(error(path, "downcast requires an enum"));
                };
                let variant = enum_ty
                    .variants
                    .iter()
                    .find(|item| item.index == *variant)
                    .ok_or_else(|| error(path, "downcast variant does not exist"))?;
                Ok(ProjectionState {
                    ty: ProjectionType::Variant {
                        fields: &variant.aggregate.fields,
                    },
                    writable: current.writable,
                    address_space: current.address_space,
                })
            }
        }
    }

    fn verify_reachability(&self) -> Result<(), MirExecutableValidationError> {
        let mut reachable = BTreeSet::new();
        let mut queue = VecDeque::from([self.function.body.entry]);
        while let Some(block) = queue.pop_front() {
            if !reachable.insert(block) {
                continue;
            }
            for edge in
                terminator_edges(&self.function.body.blocks[block.0 as usize].terminator.kind)
            {
                queue.push_back(edge.target);
            }
        }
        if reachable.len() != self.function.body.blocks.len() {
            let missing = (0..self.function.body.blocks.len())
                .map(|index| MirBlockId(index as u32))
                .find(|block| !reachable.contains(block))
                .expect("length mismatch guarantees an unreachable block");
            return Err(error(
                format!("{}.body.blocks[{}]", self.path, missing.0),
                "block is unreachable from the entry",
            ));
        }
        Ok(())
    }

    fn verify_initialization(&self) -> Result<(), MirExecutableValidationError> {
        let body = &self.function.body;
        let mut entry_state = vec![LocalInitialization::Uninitialized; body.locals.len()];
        for (index, local) in body.locals.iter().enumerate() {
            if local.kind == MirLocalKind::Argument {
                entry_state[index] = LocalInitialization::Initialized;
            }
        }

        let mut inputs = vec![None; body.blocks.len()];
        inputs[body.entry.0 as usize] = Some(entry_state);
        let mut queue = VecDeque::from([body.entry]);
        while let Some(block_id) = queue.pop_front() {
            let input = inputs[block_id.0 as usize]
                .as_ref()
                .expect("queued blocks have an input state")
                .clone();
            for (target, state) in self.transfer_initialization(block_id, input, false)? {
                let slot = &mut inputs[target.0 as usize];
                let changed = match slot {
                    Some(current) => merge_initialization(current, &state),
                    None => {
                        *slot = Some(state);
                        true
                    }
                };
                if changed {
                    queue.push_back(target);
                }
            }
        }

        for (index, input) in inputs.into_iter().enumerate() {
            let input = input.ok_or_else(|| {
                error(
                    format!("{}.body.blocks[{index}]", self.path),
                    "initialization analysis did not reach block",
                )
            })?;
            self.transfer_initialization(MirBlockId(index as u32), input, true)?;
        }
        Ok(())
    }

    fn verify_variant_state(&self) -> Result<(), MirExecutableValidationError> {
        let body = &self.function.body;
        let entry_state = VariantFlowState {
            variants: body
                .locals
                .iter()
                .map(|local| match self.type_at(local.ty).kind {
                    MirTypeKind::Enum(_) => EnumVariantState::Unknown,
                    _ => EnumVariantState::NotEnum,
                })
                .collect(),
            discriminant_sources: vec![None; body.locals.len()],
        };
        let mut inputs = vec![None; body.blocks.len()];
        inputs[body.entry.0 as usize] = Some(entry_state);
        let mut queue = VecDeque::from([body.entry]);
        while let Some(block_id) = queue.pop_front() {
            let input = inputs[block_id.0 as usize]
                .as_ref()
                .expect("queued blocks have a variant input state")
                .clone();
            for (target, state) in self.transfer_variant_state(block_id, input, false)? {
                let slot = &mut inputs[target.0 as usize];
                let changed = match slot {
                    Some(current) => merge_variant_state(current, &state),
                    None => {
                        *slot = Some(state);
                        true
                    }
                };
                if changed {
                    queue.push_back(target);
                }
            }
        }

        for (index, input) in inputs.into_iter().enumerate() {
            let input = input.ok_or_else(|| {
                error(
                    format!("{}.body.blocks[{index}]", self.path),
                    "variant analysis did not reach block",
                )
            })?;
            self.transfer_variant_state(MirBlockId(index as u32), input, true)?;
        }
        Ok(())
    }

    fn transfer_variant_state(
        &self,
        block_id: MirBlockId,
        mut state: VariantFlowState,
        strict: bool,
    ) -> Result<Vec<(MirBlockId, VariantFlowState)>, MirExecutableValidationError> {
        let block = &self.function.body.blocks[block_id.0 as usize];
        let block_path = format!("{}.body.blocks[{}]", self.path, block_id.0);
        for (index, statement) in block.statements.iter().enumerate() {
            let path = format!("{block_path}.statements[{index}]");
            match &statement.kind {
                MirStatementKind::Assign { place, value } => {
                    self.check_variant_place(&format!("{path}.place"), place, &state, strict)?;
                    self.check_variant_rvalue(&format!("{path}.value"), value, &state, strict)?;
                    let next_variant = self.variant_state_from_rvalue(place.ty, value, &state);
                    let next_discriminant = self.discriminant_source_from_rvalue(value);
                    if let Some(borrowed) = self.mutably_borrowed_enum_local(value) {
                        invalidate_discriminant_observations(&mut state, borrowed);
                        state.variants[borrowed.0 as usize] = EnumVariantState::Unknown;
                    }
                    if place.projection.is_empty() {
                        let local = place.local.0 as usize;
                        invalidate_discriminant_observations(&mut state, place.local);
                        state.variants[local] = next_variant;
                        state.discriminant_sources[local] = next_discriminant;
                    }
                }
                MirStatementKind::Define { rvalue, .. } => {
                    self.check_variant_rvalue(&format!("{path}.rvalue"), rvalue, &state, strict)?;
                }
                MirStatementKind::SetDiscriminant { place, variant } => {
                    self.check_variant_place(&format!("{path}.place"), place, &state, strict)?;
                    if !place.projection.is_empty() {
                        return Err(error(
                            format!("{path}.place"),
                            "set-discriminant requires a direct local until field-sensitive variant state is modeled",
                        ));
                    }
                    invalidate_discriminant_observations(&mut state, place.local);
                    state.variants[place.local.0 as usize] = EnumVariantState::Active {
                        variant: *variant,
                        payload_initialized: true,
                    };
                }
                MirStatementKind::StorageLive(local) | MirStatementKind::StorageDead(local) => {
                    let index = local.0 as usize;
                    invalidate_discriminant_observations(&mut state, *local);
                    state.variants[index] = match self.type_at(self.local(*local).ty).kind {
                        MirTypeKind::Enum(_) => EnumVariantState::Unknown,
                        _ => EnumVariantState::NotEnum,
                    };
                    state.discriminant_sources[index] = None;
                }
                MirStatementKind::Deinit(place) => {
                    self.check_variant_place(&format!("{path}.place"), place, &state, strict)?;
                    if place.projection.is_empty() {
                        let local = place.local.0 as usize;
                        invalidate_discriminant_observations(&mut state, place.local);
                        if matches!(self.type_at(place.ty).kind, MirTypeKind::Enum(_)) {
                            state.variants[local] = EnumVariantState::Unknown;
                        }
                        state.discriminant_sources[local] = None;
                    }
                }
                MirStatementKind::Nop => {}
            }
        }

        let path = format!("{block_path}.terminator");
        let mut output = Vec::new();
        match &block.terminator.kind {
            MirTerminatorKind::Goto(edge) => {
                self.check_variant_edge(&path, edge, &state, strict)?;
                output.push((edge.target, state));
            }
            MirTerminatorKind::SwitchInt {
                discr,
                targets,
                otherwise,
            } => {
                self.check_variant_operand(&format!("{path}.discr"), discr, &state, strict)?;
                let source = self.discriminant_source_from_operand(discr, &state);
                for (index, (value, edge)) in targets.iter().enumerate() {
                    let mut edge_state = state.clone();
                    self.refine_variant_for_discriminant(source, *value, &mut edge_state);
                    self.check_variant_edge(
                        &format!("{path}.targets[{index}]"),
                        edge,
                        &edge_state,
                        strict,
                    )?;
                    output.push((edge.target, edge_state));
                }
                let mut edge_state = state;
                self.refine_variant_for_otherwise(source, targets, &mut edge_state);
                self.check_variant_edge(
                    &format!("{path}.otherwise"),
                    otherwise,
                    &edge_state,
                    strict,
                )?;
                output.push((otherwise.target, edge_state));
            }
            MirTerminatorKind::Return | MirTerminatorKind::Unreachable => {}
            MirTerminatorKind::Call(call) => {
                for (index, argument) in call.arguments.iter().enumerate() {
                    self.check_variant_operand(
                        &format!("{path}.arguments[{index}]"),
                        argument,
                        &state,
                        strict,
                    )?;
                }
                if let Some(destination) = &call.destination {
                    self.check_variant_place(
                        &format!("{path}.destination"),
                        destination,
                        &state,
                        strict,
                    )?;
                }
                invalidate_all_enum_authority(&mut state);
                if let Some(edge) = &call.target {
                    let mut normal = state.clone();
                    if let Some(destination) = &call.destination
                        && destination.projection.is_empty()
                    {
                        let local = destination.local.0 as usize;
                        invalidate_discriminant_observations(&mut normal, destination.local);
                        normal.variants[local] = match self.type_at(destination.ty).kind {
                            MirTypeKind::Enum(_) => EnumVariantState::Unknown,
                            _ => EnumVariantState::NotEnum,
                        };
                        normal.discriminant_sources[local] = None;
                    }
                    self.check_variant_edge(&format!("{path}.target"), edge, &normal, strict)?;
                    output.push((edge.target, normal));
                }
                self.push_variant_unwind(&path, &call.unwind, &state, strict, &mut output)?;
            }
            MirTerminatorKind::Drop {
                place,
                target,
                unwind,
            } => {
                self.check_variant_place(&format!("{path}.place"), place, &state, strict)?;
                let mut after_drop = state;
                if place.projection.is_empty() {
                    invalidate_discriminant_observations(&mut after_drop, place.local);
                    if matches!(self.type_at(place.ty).kind, MirTypeKind::Enum(_)) {
                        after_drop.variants[place.local.0 as usize] = EnumVariantState::Unknown;
                    }
                }
                self.check_variant_edge(&format!("{path}.target"), target, &after_drop, strict)?;
                output.push((target.target, after_drop.clone()));
                self.push_variant_unwind(&path, unwind, &after_drop, strict, &mut output)?;
            }
            MirTerminatorKind::Assert {
                condition,
                target,
                unwind,
                ..
            } => {
                self.check_variant_operand(
                    &format!("{path}.condition"),
                    condition,
                    &state,
                    strict,
                )?;
                self.check_variant_edge(&format!("{path}.target"), target, &state, strict)?;
                output.push((target.target, state.clone()));
                self.push_variant_unwind(&path, unwind, &state, strict, &mut output)?;
            }
        }
        Ok(output)
    }

    fn push_variant_unwind(
        &self,
        path: &str,
        unwind: &MirUnwindAction,
        state: &VariantFlowState,
        strict: bool,
        output: &mut Vec<(MirBlockId, VariantFlowState)>,
    ) -> Result<(), MirExecutableValidationError> {
        if let MirUnwindAction::Cleanup(edge) = unwind {
            self.check_variant_edge(path, edge, state, strict)?;
            output.push((edge.target, state.clone()));
        }
        Ok(())
    }

    fn check_variant_edge(
        &self,
        path: &str,
        edge: &MirEdge,
        state: &VariantFlowState,
        strict: bool,
    ) -> Result<(), MirExecutableValidationError> {
        for (index, argument) in edge.arguments.iter().enumerate() {
            self.check_variant_operand(
                &format!("{path}.arguments[{index}]"),
                argument,
                state,
                strict,
            )?;
        }
        Ok(())
    }

    fn check_variant_rvalue(
        &self,
        path: &str,
        rvalue: &MirRvalue,
        state: &VariantFlowState,
        strict: bool,
    ) -> Result<(), MirExecutableValidationError> {
        match rvalue {
            MirRvalue::Use(operand)
            | MirRvalue::UnaryOp { operand, .. }
            | MirRvalue::Cast { operand, .. }
            | MirRvalue::Repeat { operand, .. } => {
                self.check_variant_operand(path, operand, state, strict)
            }
            MirRvalue::BinaryOp { lhs, rhs, .. } | MirRvalue::CheckedBinaryOp { lhs, rhs, .. } => {
                self.check_variant_operand(&format!("{path}.lhs"), lhs, state, strict)?;
                self.check_variant_operand(&format!("{path}.rhs"), rhs, state, strict)
            }
            MirRvalue::Ref { place, .. }
            | MirRvalue::AddressOf { place, .. }
            | MirRvalue::Len(place)
            | MirRvalue::Discriminant(place) => {
                self.check_variant_place(&format!("{path}.place"), place, state, strict)
            }
            MirRvalue::Aggregate { operands, .. } => {
                for (index, operand) in operands.iter().enumerate() {
                    self.check_variant_operand(
                        &format!("{path}.operands[{index}]"),
                        operand,
                        state,
                        strict,
                    )?;
                }
                Ok(())
            }
            MirRvalue::ThreadIndex1d => Ok(()),
        }
    }

    fn check_variant_operand(
        &self,
        path: &str,
        operand: &MirOperand,
        state: &VariantFlowState,
        strict: bool,
    ) -> Result<(), MirExecutableValidationError> {
        match operand {
            MirOperand::Copy(place) | MirOperand::Move(place) => {
                self.check_variant_place(path, place, state, strict)
            }
            MirOperand::Constant(_) | MirOperand::Value(_) => Ok(()),
        }
    }

    fn check_variant_place(
        &self,
        path: &str,
        place: &MirPlace,
        state: &VariantFlowState,
        strict: bool,
    ) -> Result<(), MirExecutableValidationError> {
        let local = self.local(place.local);
        let mut current = ProjectionState {
            ty: ProjectionType::Type(self.type_at(local.ty)),
            writable: local.mutable,
            address_space: local.storage_address_space,
        };
        for (index, projection) in place.projection.iter().enumerate() {
            let projection_path = format!("{path}.projection[{index}]");
            if let MirProjection::Downcast { variant } = projection {
                let ProjectionType::Type(ty) = &current.ty else {
                    return Err(error(&projection_path, "nested downcast is invalid"));
                };
                let MirTypeKind::Enum(enum_ty) = &ty.kind else {
                    return Err(error(&projection_path, "downcast requires an enum"));
                };
                let selected = enum_ty
                    .variants
                    .iter()
                    .find(|candidate| candidate.index == *variant)
                    .expect("structural verification checked the variant index");
                if !selected.aggregate.fields.is_empty() {
                    if index != 0 {
                        return Err(error(
                            projection_path,
                            "nested payload downcast requires field-sensitive variant authority",
                        ));
                    }
                    if strict
                        && state.variants[place.local.0 as usize]
                            != (EnumVariantState::Active {
                                variant: *variant,
                                payload_initialized: true,
                            })
                    {
                        return Err(error(
                            projection_path,
                            "payload downcast requires an exact active variant with initialized payload",
                        ));
                    }
                }
            }
            current = self.project(&projection_path, current, projection)?;
        }
        Ok(())
    }

    fn variant_state_from_rvalue(
        &self,
        destination: MirTypeId,
        rvalue: &MirRvalue,
        state: &VariantFlowState,
    ) -> EnumVariantState {
        let MirTypeKind::Enum(destination_enum) = &self.type_at(destination).kind else {
            return EnumVariantState::NotEnum;
        };
        match rvalue {
            MirRvalue::Aggregate {
                kind: MirAggregateKind::Adt { identity, variant },
                ..
            } if *identity == destination_enum.identity => EnumVariantState::Active {
                variant: *variant,
                payload_initialized: true,
            },
            MirRvalue::Use(MirOperand::Copy(place) | MirOperand::Move(place))
                if place.projection.is_empty() =>
            {
                state.variants[place.local.0 as usize]
            }
            _ => EnumVariantState::Unknown,
        }
    }

    fn discriminant_source_from_rvalue(&self, rvalue: &MirRvalue) -> Option<MirLocalId> {
        match rvalue {
            MirRvalue::Discriminant(place) if place.projection.is_empty() => Some(place.local),
            _ => None,
        }
    }

    fn mutably_borrowed_enum_local(&self, rvalue: &MirRvalue) -> Option<MirLocalId> {
        let place = match rvalue {
            MirRvalue::Ref {
                mutability: MirMutability::Mutable,
                place,
                ..
            }
            | MirRvalue::AddressOf {
                mutability: MirMutability::Mutable,
                place,
                ..
            } if place.projection.is_empty() => place,
            _ => return None,
        };
        matches!(self.type_at(place.ty).kind, MirTypeKind::Enum(_)).then_some(place.local)
    }

    fn discriminant_source_from_operand(
        &self,
        operand: &MirOperand,
        state: &VariantFlowState,
    ) -> Option<MirLocalId> {
        match operand {
            MirOperand::Copy(place) | MirOperand::Move(place) if place.projection.is_empty() => {
                state.discriminant_sources[place.local.0 as usize]
            }
            MirOperand::Constant(_)
            | MirOperand::Value(_)
            | MirOperand::Copy(_)
            | MirOperand::Move(_) => None,
        }
    }

    fn refine_variant_for_discriminant(
        &self,
        source: Option<MirLocalId>,
        discriminant: u128,
        state: &mut VariantFlowState,
    ) {
        let Some(source) = source else {
            return;
        };
        let MirTypeKind::Enum(enum_ty) = &self.type_at(self.local(source).ty).kind else {
            return;
        };
        if let Some(variant) = enum_ty
            .variants
            .iter()
            .find(|variant| variant.discriminant == discriminant)
        {
            state.variants[source.0 as usize] = EnumVariantState::Active {
                variant: variant.index,
                payload_initialized: true,
            };
        }
    }

    fn refine_variant_for_otherwise(
        &self,
        source: Option<MirLocalId>,
        targets: &[(u128, MirEdge)],
        state: &mut VariantFlowState,
    ) {
        let Some(source) = source else {
            return;
        };
        let MirTypeKind::Enum(enum_ty) = &self.type_at(self.local(source).ty).kind else {
            return;
        };
        let mut remaining = enum_ty.variants.iter().filter(|variant| {
            !targets
                .iter()
                .any(|(value, _)| *value == variant.discriminant)
        });
        if let Some(variant) = remaining.next()
            && remaining.next().is_none()
        {
            state.variants[source.0 as usize] = EnumVariantState::Active {
                variant: variant.index,
                payload_initialized: true,
            };
        }
    }

    fn transfer_initialization(
        &self,
        block_id: MirBlockId,
        mut state: Vec<LocalInitialization>,
        strict: bool,
    ) -> Result<Vec<(MirBlockId, Vec<LocalInitialization>)>, MirExecutableValidationError> {
        let block = &self.function.body.blocks[block_id.0 as usize];
        let block_path = format!("{}.body.blocks[{}]", self.path, block_id.0);
        for (index, statement) in block.statements.iter().enumerate() {
            let path = format!("{block_path}.statements[{index}]");
            match &statement.kind {
                MirStatementKind::Assign { place, value } => {
                    self.flow_rvalue(&format!("{path}.value"), value, &mut state, strict)?;
                    self.flow_write_place(&format!("{path}.place"), place, &mut state, strict)?;
                }
                MirStatementKind::Define { rvalue, .. } => {
                    self.flow_rvalue(&format!("{path}.rvalue"), rvalue, &mut state, strict)?;
                }
                MirStatementKind::SetDiscriminant { place, .. } => {
                    self.flow_read_place(&format!("{path}.place"), place, &state, strict)?;
                }
                MirStatementKind::StorageLive(local) => {
                    if strict && state[local.0 as usize] == LocalInitialization::Initialized {
                        return Err(error(path, "storage-live resets an initialized local"));
                    }
                    state[local.0 as usize] = LocalInitialization::Uninitialized;
                }
                MirStatementKind::StorageDead(local) => {
                    state[local.0 as usize] = LocalInitialization::Uninitialized;
                }
                MirStatementKind::Deinit(place) => {
                    self.flow_read_place(&format!("{path}.place"), place, &state, strict)?;
                    if place.projection.is_empty() {
                        state[place.local.0 as usize] = LocalInitialization::Uninitialized;
                    } else if strict {
                        return Err(error(
                            format!("{path}.place"),
                            "projected deinitialization requires partial-move tracking",
                        ));
                    }
                }
                MirStatementKind::Nop => {}
            }
        }

        let path = format!("{block_path}.terminator");
        let mut output = Vec::new();
        match &block.terminator.kind {
            MirTerminatorKind::Goto(edge) => {
                let edge_state = self.flow_edge(&path, edge, &state, strict)?;
                output.push((edge.target, edge_state));
            }
            MirTerminatorKind::SwitchInt {
                discr,
                targets,
                otherwise,
            } => {
                self.flow_operand(&format!("{path}.discr"), discr, &mut state, strict)?;
                for (index, (_, edge)) in targets.iter().enumerate() {
                    let edge_state =
                        self.flow_edge(&format!("{path}.targets[{index}]"), edge, &state, strict)?;
                    output.push((edge.target, edge_state));
                }
                let edge_state =
                    self.flow_edge(&format!("{path}.otherwise"), otherwise, &state, strict)?;
                output.push((otherwise.target, edge_state));
            }
            MirTerminatorKind::Return => {
                self.require_initialized(&path, MirLocalId(0), &state, strict)?;
            }
            MirTerminatorKind::Unreachable => {}
            MirTerminatorKind::Call(call) => {
                for (index, argument) in call.arguments.iter().enumerate() {
                    self.flow_operand(
                        &format!("{path}.arguments[{index}]"),
                        argument,
                        &mut state,
                        strict,
                    )?;
                }
                if let Some(edge) = &call.target {
                    let mut normal = state.clone();
                    if let Some(destination) = &call.destination {
                        self.flow_write_place(
                            &format!("{path}.destination"),
                            destination,
                            &mut normal,
                            strict,
                        )?;
                    }
                    let edge_state =
                        self.flow_edge(&format!("{path}.target"), edge, &normal, strict)?;
                    output.push((edge.target, edge_state));
                }
                self.flow_unwind(
                    &format!("{path}.unwind"),
                    &call.unwind,
                    &state,
                    strict,
                    &mut output,
                )?;
            }
            MirTerminatorKind::Drop {
                place,
                target,
                unwind,
            } => {
                self.flow_read_place(&format!("{path}.place"), place, &state, strict)?;
                if place.projection.is_empty() {
                    state[place.local.0 as usize] = LocalInitialization::Moved;
                } else if strict {
                    return Err(error(
                        format!("{path}.place"),
                        "projected drop requires partial-move tracking",
                    ));
                }
                let edge_state =
                    self.flow_edge(&format!("{path}.target"), target, &state, strict)?;
                output.push((target.target, edge_state));
                self.flow_unwind(
                    &format!("{path}.unwind"),
                    unwind,
                    &state,
                    strict,
                    &mut output,
                )?;
            }
            MirTerminatorKind::Assert {
                condition,
                target,
                unwind,
                ..
            } => {
                self.flow_operand(&format!("{path}.condition"), condition, &mut state, strict)?;
                let edge_state =
                    self.flow_edge(&format!("{path}.target"), target, &state, strict)?;
                output.push((target.target, edge_state));
                self.flow_unwind(
                    &format!("{path}.unwind"),
                    unwind,
                    &state,
                    strict,
                    &mut output,
                )?;
            }
        }
        Ok(output)
    }

    fn flow_unwind(
        &self,
        path: &str,
        unwind: &MirUnwindAction,
        state: &[LocalInitialization],
        strict: bool,
        output: &mut Vec<(MirBlockId, Vec<LocalInitialization>)>,
    ) -> Result<(), MirExecutableValidationError> {
        if let MirUnwindAction::Cleanup(edge) = unwind {
            let edge_state = self.flow_edge(path, edge, state, strict)?;
            output.push((edge.target, edge_state));
        }
        Ok(())
    }

    fn flow_edge(
        &self,
        path: &str,
        edge: &MirEdge,
        state: &[LocalInitialization],
        strict: bool,
    ) -> Result<Vec<LocalInitialization>, MirExecutableValidationError> {
        let mut edge_state = state.to_vec();
        for (index, argument) in edge.arguments.iter().enumerate() {
            self.flow_operand(
                &format!("{path}.arguments[{index}]"),
                argument,
                &mut edge_state,
                strict,
            )?;
        }
        Ok(edge_state)
    }

    fn flow_rvalue(
        &self,
        path: &str,
        rvalue: &MirRvalue,
        state: &mut [LocalInitialization],
        strict: bool,
    ) -> Result<(), MirExecutableValidationError> {
        match rvalue {
            MirRvalue::Use(operand)
            | MirRvalue::UnaryOp { operand, .. }
            | MirRvalue::Cast { operand, .. }
            | MirRvalue::Repeat { operand, .. } => {
                self.flow_operand(path, operand, state, strict)?;
            }
            MirRvalue::BinaryOp { lhs, rhs, .. } | MirRvalue::CheckedBinaryOp { lhs, rhs, .. } => {
                self.flow_operand(&format!("{path}.lhs"), lhs, state, strict)?;
                self.flow_operand(&format!("{path}.rhs"), rhs, state, strict)?;
            }
            MirRvalue::Ref { place, .. }
            | MirRvalue::Len(place)
            | MirRvalue::Discriminant(place) => {
                self.flow_read_place(&format!("{path}.place"), place, state, strict)?;
            }
            MirRvalue::AddressOf { place, .. } => {
                self.flow_address_place(&format!("{path}.place"), place, state, strict)?;
            }
            MirRvalue::Aggregate { operands, .. } => {
                for (index, operand) in operands.iter().enumerate() {
                    self.flow_operand(
                        &format!("{path}.operands[{index}]"),
                        operand,
                        state,
                        strict,
                    )?;
                }
            }
            MirRvalue::ThreadIndex1d => {}
        }
        Ok(())
    }

    fn flow_operand(
        &self,
        path: &str,
        operand: &MirOperand,
        state: &mut [LocalInitialization],
        strict: bool,
    ) -> Result<(), MirExecutableValidationError> {
        match operand {
            MirOperand::Copy(place) => {
                self.flow_read_place(path, place, state, strict)?;
                if strict && !is_copy_type(&self.type_at(place.ty).kind) {
                    return Err(error(
                        path,
                        "Copy operand names a type without Copy authority",
                    ));
                }
            }
            MirOperand::Move(place) => {
                self.flow_read_place(path, place, state, strict)?;
                if !is_copy_type(&self.type_at(place.ty).kind) {
                    if strict && !place.projection.is_empty() {
                        return Err(error(path, "projected move requires partial-move tracking"));
                    }
                    state[place.local.0 as usize] = LocalInitialization::Moved;
                }
            }
            MirOperand::Constant(_) | MirOperand::Value(_) => {}
        }
        Ok(())
    }

    fn flow_read_place(
        &self,
        path: &str,
        place: &MirPlace,
        state: &[LocalInitialization],
        strict: bool,
    ) -> Result<(), MirExecutableValidationError> {
        self.require_initialized(path, place.local, state, strict)?;
        for (index, projection) in place.projection.iter().enumerate() {
            if let MirProjection::Index { local } = projection {
                self.require_initialized(
                    &format!("{path}.projection[{index}].local"),
                    *local,
                    state,
                    strict,
                )?;
            }
        }
        Ok(())
    }

    fn flow_address_place(
        &self,
        path: &str,
        place: &MirPlace,
        state: &[LocalInitialization],
        strict: bool,
    ) -> Result<(), MirExecutableValidationError> {
        for (index, projection) in place.projection.iter().enumerate() {
            match projection {
                MirProjection::Index { local } => self.require_initialized(
                    &format!("{path}.projection[{index}].local"),
                    *local,
                    state,
                    strict,
                )?,
                MirProjection::Deref | MirProjection::Downcast { .. } => {
                    self.require_initialized(path, place.local, state, strict)?;
                }
                MirProjection::Field { .. }
                | MirProjection::ConstantIndex { .. }
                | MirProjection::Subslice { .. } => {}
            }
        }
        Ok(())
    }

    fn flow_write_place(
        &self,
        path: &str,
        place: &MirPlace,
        state: &mut [LocalInitialization],
        strict: bool,
    ) -> Result<(), MirExecutableValidationError> {
        if !place.projection.is_empty() {
            return self.flow_read_place(path, place, state, strict);
        }
        let slot = &mut state[place.local.0 as usize];
        if !self.local(place.local).mutable && *slot != LocalInitialization::Uninitialized && strict
        {
            return Err(error(
                path,
                "immutable local may be initialized exactly once",
            ));
        }
        *slot = LocalInitialization::Initialized;
        Ok(())
    }

    fn require_initialized(
        &self,
        path: &str,
        local: MirLocalId,
        state: &[LocalInitialization],
        strict: bool,
    ) -> Result<(), MirExecutableValidationError> {
        let actual = state[local.0 as usize];
        if strict && actual != LocalInitialization::Initialized {
            let reason = match actual {
                LocalInitialization::Uninitialized => "local is not definitely initialized",
                LocalInitialization::Moved => "local was moved and not reinitialized",
                LocalInitialization::MaybeInvalid => {
                    "local is initialized on only some incoming paths"
                }
                LocalInitialization::Initialized => unreachable!(),
            };
            return Err(error(path, reason));
        }
        Ok(())
    }

    fn is_one_time_initialization_place(&self, place: &MirPlace) -> bool {
        place.projection.is_empty() && self.local(place.local).kind != MirLocalKind::Argument
    }

    fn verify_reference_type(
        &self,
        path: &str,
        result: MirTypeId,
        referent: MirTypeId,
        mutability: MirMutability,
        address_space: MirAddressSpace,
        raw: bool,
    ) -> Result<(), MirExecutableValidationError> {
        let valid = match &self.type_at(result).kind {
            MirTypeKind::RawPointer {
                pointee,
                mutability: candidate_mutability,
                address_space: candidate_address_space,
            } if raw => {
                **pointee == *self.type_at(referent)
                    && *candidate_mutability == mutability
                    && *candidate_address_space == address_space
            }
            MirTypeKind::Reference {
                referent: candidate_referent,
                mutability: candidate_mutability,
                address_space: candidate_address_space,
            } if !raw => {
                **candidate_referent == *self.type_at(referent)
                    && *candidate_mutability == mutability
                    && *candidate_address_space == address_space
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(error(
                path,
                "result pointer/reference type does not match referent, mutability, and address space",
            ))
        }
    }

    fn verify_reference_origin(
        &self,
        path: &str,
        place: &MirPlace,
    ) -> Result<(), MirExecutableValidationError> {
        let local = self.local(place.local);
        let mut current = ProjectionState {
            ty: ProjectionType::Type(self.type_at(local.ty)),
            writable: local.mutable,
            address_space: local.storage_address_space,
        };
        for (index, projection) in place.projection.iter().enumerate() {
            let projection_path = format!("{path}.projection[{index}]");
            if matches!(projection, MirProjection::Deref)
                && matches!(
                    &current.ty,
                    ProjectionType::Type(MirSemanticType {
                        kind: MirTypeKind::RawPointer { .. },
                        ..
                    })
                )
            {
                return Err(error(
                    projection_path,
                    "reference creation through a raw pointer requires external provenance authority",
                ));
            }
            current = self.project(&projection_path, current, projection)?;
        }
        Ok(())
    }

    fn find_scalar_type(
        &self,
        path: &str,
        scalar: MirScalarType,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.find_type(
            path,
            |kind| matches!(kind, MirTypeKind::Scalar(value) if *value == scalar),
        )
    }

    fn bool_type(&self, path: &str) -> Result<MirTypeId, MirExecutableValidationError> {
        self.find_scalar_type(path, MirScalarType::Bool)
    }

    fn usize_type(&self, path: &str) -> Result<MirTypeId, MirExecutableValidationError> {
        self.unsigned_type(path, self.module.target.pointer_width_bits)
    }

    fn unsigned_type(
        &self,
        path: &str,
        width: u16,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.find_type(path, |kind| {
            matches!(
                kind,
                MirTypeKind::Scalar(MirScalarType::Int {
                    signed: false,
                    bits,
                }) if *bits == width
            )
        })
    }

    fn find_array_type(
        &self,
        path: &str,
        element: MirTypeId,
        length: u64,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.find_type(path, |kind| match kind {
            MirTypeKind::Array {
                element: candidate,
                length: candidate_length,
            } => **candidate == *self.type_at(element) && *candidate_length == length,
            _ => false,
        })
    }

    fn find_aggregate_type(
        &self,
        path: &str,
        aggregate_kind: &MirAggregateKind,
        operand_types: &[MirTypeId],
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.find_type(path, |kind| {
            let fields = match (aggregate_kind, kind) {
                (MirAggregateKind::Tuple, MirTypeKind::Tuple(aggregate)) => &aggregate.fields,
                (MirAggregateKind::Array, MirTypeKind::Array { element, length }) => {
                    return *length == operand_types.len() as u64
                        && operand_types
                            .iter()
                            .all(|ty| self.type_at(*ty) == element.as_ref());
                }
                (MirAggregateKind::Adt { identity, variant }, MirTypeKind::Struct(structure))
                    if *variant == 0 && structure.identity == *identity =>
                {
                    &structure.aggregate.fields
                }
                (MirAggregateKind::Adt { identity, variant }, MirTypeKind::Enum(enum_ty))
                    if enum_ty.identity == *identity =>
                {
                    let Some(variant) = enum_ty.variants.iter().find(|item| item.index == *variant)
                    else {
                        return false;
                    };
                    &variant.aggregate.fields
                }
                _ => return false,
            };
            fields.len() == operand_types.len()
                && fields
                    .iter()
                    .zip(operand_types)
                    .all(|(field, ty)| field.ty == *self.type_at(*ty))
        })
    }

    fn find_array_semantic_type<'b>(
        &'b self,
        path: &str,
        element: &MirSemanticType,
        length: u64,
    ) -> Result<&'b MirSemanticType, MirExecutableValidationError> {
        self.module
            .types
            .iter()
            .find(|candidate| {
                matches!(
                    &candidate.kind,
                    MirTypeKind::Array {
                        element: candidate,
                        length: candidate_length,
                    } if candidate.as_ref() == element && *candidate_length == length
                )
            })
            .ok_or_else(|| error(path, "projected array type is absent from the type table"))
    }

    fn find_type(
        &self,
        path: &str,
        predicate: impl Fn(&MirTypeKind) -> bool,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.module
            .types
            .iter()
            .position(|ty| predicate(&ty.kind))
            .map(|index| MirTypeId(index as u32))
            .ok_or_else(|| error(path, "required result type is absent from the type table"))
    }

    fn require_same_type(
        &self,
        path: &str,
        expected: MirTypeId,
        actual: MirTypeId,
    ) -> Result<(), MirExecutableValidationError> {
        if expected != actual {
            return Err(error(
                path,
                format!("type mismatch: expected {}, found {}", expected.0, actual.0),
            ));
        }
        Ok(())
    }

    fn require_type(&self, path: &str, ty: MirTypeId) -> Result<(), MirExecutableValidationError> {
        if ty.0 as usize >= self.module.types.len() {
            return Err(error(path, format!("type {} does not exist", ty.0)));
        }
        Ok(())
    }

    fn require_target_usize(
        &self,
        path: &str,
        value: u64,
    ) -> Result<(), MirExecutableValidationError> {
        if self.module.target.pointer_width_bits == 32 && value > u64::from(u32::MAX) {
            return Err(error(path, "value does not fit the target usize width"));
        }
        Ok(())
    }

    fn require_local(
        &self,
        path: &str,
        local: MirLocalId,
    ) -> Result<(), MirExecutableValidationError> {
        if local.0 as usize >= self.function.body.locals.len() {
            return Err(error(path, format!("local {} does not exist", local.0)));
        }
        Ok(())
    }

    fn require_block(
        &self,
        path: &str,
        block: MirBlockId,
    ) -> Result<(), MirExecutableValidationError> {
        if block.0 as usize >= self.function.body.blocks.len() {
            return Err(error(path, format!("block {} does not exist", block.0)));
        }
        Ok(())
    }

    fn local(&self, id: MirLocalId) -> &MirLocalDecl {
        &self.function.body.locals[id.0 as usize]
    }

    fn type_at(&self, id: MirTypeId) -> &MirSemanticType {
        &self.module.types[id.0 as usize]
    }
}

enum ProjectionType<'a> {
    Type(&'a MirSemanticType),
    Variant { fields: &'a [crate::MirField] },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LocalInitialization {
    Uninitialized,
    Initialized,
    Moved,
    MaybeInvalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnumVariantState {
    NotEnum,
    Unknown,
    Active {
        variant: u32,
        payload_initialized: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VariantFlowState {
    variants: Vec<EnumVariantState>,
    discriminant_sources: Vec<Option<MirLocalId>>,
}

fn merge_initialization(
    current: &mut [LocalInitialization],
    incoming: &[LocalInitialization],
) -> bool {
    let mut changed = false;
    for (current, incoming) in current.iter_mut().zip(incoming) {
        let merged = if *current == *incoming {
            *current
        } else {
            LocalInitialization::MaybeInvalid
        };
        if *current != merged {
            *current = merged;
            changed = true;
        }
    }
    changed
}

fn merge_variant_state(current: &mut VariantFlowState, incoming: &VariantFlowState) -> bool {
    let mut changed = false;
    for (current, incoming) in current.variants.iter_mut().zip(&incoming.variants) {
        let merged = if *current == *incoming {
            *current
        } else {
            EnumVariantState::Unknown
        };
        if *current != merged {
            *current = merged;
            changed = true;
        }
    }
    for (current, incoming) in current
        .discriminant_sources
        .iter_mut()
        .zip(&incoming.discriminant_sources)
    {
        let merged = if *current == *incoming {
            *current
        } else {
            None
        };
        if *current != merged {
            *current = merged;
            changed = true;
        }
    }
    changed
}

fn invalidate_discriminant_observations(state: &mut VariantFlowState, source: MirLocalId) {
    for observed_source in &mut state.discriminant_sources {
        if *observed_source == Some(source) {
            *observed_source = None;
        }
    }
}

fn invalidate_all_enum_authority(state: &mut VariantFlowState) {
    for variant in &mut state.variants {
        if !matches!(variant, EnumVariantState::NotEnum) {
            *variant = EnumVariantState::Unknown;
        }
    }
    state.discriminant_sources.fill(None);
}

struct ProjectionState<'a> {
    ty: ProjectionType<'a>,
    writable: bool,
    address_space: MirAddressSpace,
}

fn is_intrinsic_identity(identity: &str) -> bool {
    matches!(
        identity,
        "fe2o3.copy_nonoverlapping"
            | "fe2o3.pointer_distance"
            | "fe2o3.volatile_load"
            | "fe2o3.volatile_store"
    )
}

fn unwind_action_may_unwind(action: &MirUnwindAction) -> bool {
    matches!(
        action,
        MirUnwindAction::Continue | MirUnwindAction::Cleanup(_)
    )
}

pub(crate) fn terminator_edges(terminator: &MirTerminatorKind) -> Vec<&MirEdge> {
    let mut edges = Vec::new();
    match terminator {
        MirTerminatorKind::Goto(edge) => edges.push(edge),
        MirTerminatorKind::SwitchInt {
            targets, otherwise, ..
        } => {
            edges.extend(targets.iter().map(|(_, edge)| edge));
            edges.push(otherwise);
        }
        MirTerminatorKind::Call(call) => {
            if let Some(edge) = &call.target {
                edges.push(edge);
            }
            push_unwind(&mut edges, &call.unwind);
        }
        MirTerminatorKind::Drop { target, unwind, .. }
        | MirTerminatorKind::Assert { target, unwind, .. } => {
            edges.push(target);
            push_unwind(&mut edges, unwind);
        }
        MirTerminatorKind::Return | MirTerminatorKind::Unreachable => {}
    }
    edges
}

fn push_unwind<'a>(edges: &mut Vec<&'a MirEdge>, unwind: &'a MirUnwindAction) {
    if let MirUnwindAction::Cleanup(edge) = unwind {
        edges.push(edge);
    }
}

fn valid_cast(kind: MirCastKind, source: &MirSemanticType, destination: &MirSemanticType) -> bool {
    if matches!(destination.kind, MirTypeKind::Scalar(MirScalarType::Char)) {
        return false;
    }
    if matches!(source.kind, MirTypeKind::Scalar(MirScalarType::Char)) {
        return kind == MirCastKind::IntToInt && is_machine_integer(&destination.kind);
    }
    match kind {
        MirCastKind::IntToInt => is_integer(&source.kind) && is_integer(&destination.kind),
        MirCastKind::IntToFloat => is_integer(&source.kind) && is_float(&destination.kind),
        MirCastKind::FloatToInt => is_float(&source.kind) && is_integer(&destination.kind),
        MirCastKind::FloatToFloat => is_float(&source.kind) && is_float(&destination.kind),
        MirCastKind::PointerToPointer => raw_pointer_cast_is_valid(source, destination),
        MirCastKind::PointerToInt => is_pointer(&source.kind) && is_integer(&destination.kind),
        MirCastKind::IntToPointer => {
            is_integer(&source.kind) && matches!(destination.kind, MirTypeKind::RawPointer { .. })
        }
    }
}

fn raw_pointer_cast_is_valid(source: &MirSemanticType, destination: &MirSemanticType) -> bool {
    let (
        MirTypeKind::RawPointer {
            mutability: source_mutability,
            address_space: source_address_space,
            ..
        },
        MirTypeKind::RawPointer {
            mutability: destination_mutability,
            address_space: destination_address_space,
            ..
        },
    ) = (&source.kind, &destination.kind)
    else {
        return false;
    };
    source.layout == destination.layout
        && source_address_space == destination_address_space
        && !matches!(
            (source_mutability, destination_mutability),
            (MirMutability::Immutable, MirMutability::Mutable)
        )
}

fn valid_binary_op(operation: MirBinaryOp, kind: &MirTypeKind) -> bool {
    match operation {
        MirBinaryOp::Add
        | MirBinaryOp::Sub
        | MirBinaryOp::Mul
        | MirBinaryOp::Div
        | MirBinaryOp::Rem => is_arithmetic_numeric(kind),
        MirBinaryOp::BitXor | MirBinaryOp::BitAnd | MirBinaryOp::BitOr => {
            is_machine_integer(kind) || matches!(kind, MirTypeKind::Scalar(MirScalarType::Bool))
        }
        MirBinaryOp::Shl | MirBinaryOp::Shr => is_machine_integer(kind),
        MirBinaryOp::Eq
        | MirBinaryOp::Ne
        | MirBinaryOp::Lt
        | MirBinaryOp::Le
        | MirBinaryOp::Gt
        | MirBinaryOp::Ge => matches!(kind, MirTypeKind::Scalar(_)),
    }
}

fn is_arithmetic_numeric(kind: &MirTypeKind) -> bool {
    is_machine_integer(kind) || is_float(kind)
}

fn is_machine_integer(kind: &MirTypeKind) -> bool {
    matches!(kind, MirTypeKind::Scalar(MirScalarType::Int { .. }))
}

fn is_integer_or_bool(kind: &MirTypeKind) -> bool {
    matches!(kind, MirTypeKind::Scalar(MirScalarType::Bool)) || is_integer(kind)
}

fn switch_value_fits(kind: &MirTypeKind, value: u128) -> bool {
    match kind {
        MirTypeKind::Scalar(MirScalarType::Bool) => value <= 1,
        MirTypeKind::Scalar(MirScalarType::Char) => {
            value <= u128::from(char::MAX as u32) && !(0xd800..=0xdfff).contains(&value)
        }
        MirTypeKind::Scalar(MirScalarType::Int { bits, .. }) => {
            *bits == 128 || value < (1_u128 << *bits)
        }
        _ => false,
    }
}

fn is_signed_integer_or_float(kind: &MirTypeKind) -> bool {
    matches!(
        kind,
        MirTypeKind::Scalar(MirScalarType::Int { signed: true, .. })
            | MirTypeKind::Scalar(MirScalarType::Float { .. })
    )
}

fn is_integer(kind: &MirTypeKind) -> bool {
    matches!(
        kind,
        MirTypeKind::Scalar(MirScalarType::Int { .. } | MirScalarType::Char)
    )
}

fn is_float(kind: &MirTypeKind) -> bool {
    matches!(kind, MirTypeKind::Scalar(MirScalarType::Float { .. }))
}

fn is_pointer(kind: &MirTypeKind) -> bool {
    matches!(
        kind,
        MirTypeKind::RawPointer { .. } | MirTypeKind::Reference { .. }
    )
}

fn is_copy_type(kind: &MirTypeKind) -> bool {
    matches!(
        kind,
        MirTypeKind::Unit | MirTypeKind::Scalar(_) | MirTypeKind::RawPointer { .. }
    ) || matches!(
        kind,
        MirTypeKind::Reference {
            mutability: MirMutability::Immutable,
            ..
        }
    )
}

fn validate_span_opt(
    path: &str,
    span: Option<&MirSourceSpan>,
) -> Result<(), MirExecutableValidationError> {
    let Some(span) = span else {
        return Ok(());
    };
    if span.file.is_empty() || span.file.len() > MAX_EXECUTABLE_SOURCE_FILE_BYTES {
        return Err(error(
            format!("{path}.file"),
            "source file identity is empty or exceeds its byte bound",
        ));
    }
    if span
        .file
        .bytes()
        .any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(error(
            format!("{path}.file"),
            "source file identity contains a control byte",
        ));
    }
    if span.byte_start > span.byte_end {
        return Err(error(path, "source span start exceeds its end"));
    }
    if span.line == 0 || span.column == 0 {
        return Err(error(path, "source line and column are one-based"));
    }
    Ok(())
}

fn validate_identity(path: &str, identity: &str) -> Result<(), MirExecutableValidationError> {
    validate_bounded_text(path, identity, MAX_EXECUTABLE_IDENTITY_BYTES)
}

fn validate_name_opt(path: &str, name: Option<&str>) -> Result<(), MirExecutableValidationError> {
    if let Some(name) = name {
        validate_name(path, name)?;
    }
    Ok(())
}

fn validate_name(path: &str, name: &str) -> Result<(), MirExecutableValidationError> {
    validate_bounded_text(path, name, MAX_EXECUTABLE_IDENTITY_BYTES)
}

fn validate_bounded_text(
    path: &str,
    text: &str,
    maximum: usize,
) -> Result<(), MirExecutableValidationError> {
    if text.is_empty() || text.len() > maximum {
        return Err(error(path, "text is empty or exceeds its byte bound"));
    }
    if text
        .bytes()
        .any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(error(path, "text contains a control byte"));
    }
    Ok(())
}

fn bounded_len(
    path: &str,
    actual: usize,
    minimum: usize,
    maximum: usize,
) -> Result<(), MirExecutableValidationError> {
    if actual < minimum || actual > maximum {
        return Err(error(
            path,
            format!("length {actual} is outside {minimum}..={maximum}"),
        ));
    }
    Ok(())
}

fn map_type_error(path: &str, source: MirTypeValidationError) -> MirExecutableValidationError {
    error(
        format!("{path}.{}", source.path()),
        source.reason().to_owned(),
    )
}

fn validate_executable_address_space(
    path: &str,
    address_space: MirAddressSpace,
    target: &MirExecutableTarget,
) -> Result<(), MirExecutableValidationError> {
    if target.pointer_abi(address_space).is_none() {
        return Err(error(
            path,
            format!(
                "address space {} is absent from the exact gfx942 executable pointer ABI",
                address_space.0
            ),
        ));
    }
    Ok(())
}

fn validate_executable_target(
    target: &MirExecutableTarget,
) -> Result<(), MirExecutableValidationError> {
    if target.profile != MirExecutableTargetProfile::Gfx942 {
        return Err(error(
            "module.target.profile",
            "executable MIR V1 supports only the reviewed gfx942 target profile",
        ));
    }
    for (path, actual, expected) in [
        ("triple", target.triple.as_str(), GFX942_TARGET_TRIPLE),
        ("cpu", target.cpu.as_str(), GFX942_TARGET_CPU),
        ("features", target.features.as_str(), GFX942_TARGET_FEATURES),
        (
            "data_layout",
            target.data_layout.as_str(),
            GFX942_TARGET_DATA_LAYOUT,
        ),
    ] {
        if actual != expected {
            return Err(error(
                format!("module.target.{path}"),
                "target identity does not exactly match the reviewed gfx942 profile",
            ));
        }
    }
    if target.pointer_width_bits != 64 {
        return Err(error(
            "module.target.pointer_width_bits",
            "gfx942 Rust usize and default pointers must be 64 bits",
        ));
    }
    if target.thread_index_width_bits != 32 {
        return Err(error(
            "module.target.thread_index_width_bits",
            "gfx942 thread indices must be 32 bits",
        ));
    }
    if target.pointer_abis.len() != GFX942_POINTER_ABIS.len() {
        return Err(error(
            "module.target.pointer_abis",
            "gfx942 pointer ABI map has missing or extra address spaces",
        ));
    }
    let mut previous = None;
    for (index, actual) in target.pointer_abis.iter().enumerate() {
        let path = format!("module.target.pointer_abis[{index}]");
        if previous.is_some_and(|address_space| address_space >= actual.address_space) {
            return Err(error(
                format!("{path}.address_space"),
                "pointer ABI address spaces must be unique and strictly sorted",
            ));
        }
        previous = Some(actual.address_space);
        if actual != &GFX942_POINTER_ABIS[index] {
            return Err(error(
                path,
                "pointer width or alignment does not match the exact gfx942 address-space ABI",
            ));
        }
    }
    Ok(())
}

fn validate_target_type_abi(
    root: &str,
    ty: &MirSemanticType,
    target: &MirExecutableTarget,
) -> Result<(), MirExecutableValidationError> {
    let mut stack = vec![(root.to_owned(), ty, MirAddressSpace::DEFAULT)];
    while let Some((path, ty, containing_address_space)) = stack.pop() {
        validate_layout_offset_range(&path, ty.layout, containing_address_space, target)?;
        match &ty.kind {
            MirTypeKind::RawPointer {
                pointee,
                address_space,
                ..
            } => {
                validate_executable_address_space(
                    &format!("{path}.address_space"),
                    *address_space,
                    target,
                )?;
                let abi = target
                    .pointer_abi(*address_space)
                    .expect("validated address spaces have a pointer ABI");
                let pointer_bytes = u64::from(abi.width_bits / 8);
                let pointer_alignment = u64::from(abi.abi_alignment_bits / 8);
                if ty.layout.size != Some(pointer_bytes) || ty.layout.align != pointer_alignment {
                    return Err(error(
                        &path,
                        "raw-pointer size and alignment must exactly match its address-space pointer ABI",
                    ));
                }
                if pointee.layout.size.is_none() {
                    return Err(error(
                        format!("{path}.pointee"),
                        "thin raw pointers require a Sized pointee",
                    ));
                }
                stack.push((format!("{path}.pointee"), pointee, *address_space));
            }
            MirTypeKind::Reference {
                referent,
                address_space,
                ..
            } => {
                validate_executable_address_space(
                    &format!("{path}.address_space"),
                    *address_space,
                    target,
                )?;
                let abi = target
                    .pointer_abi(*address_space)
                    .expect("validated address spaces have a pointer ABI");
                let pointer_bytes = u64::from(abi.width_bits / 8);
                let pointer_alignment = u64::from(abi.abi_alignment_bits / 8);
                if ty.layout.size != Some(pointer_bytes) || ty.layout.align != pointer_alignment {
                    return Err(error(
                        &path,
                        "reference size and alignment must exactly match its address-space pointer ABI",
                    ));
                }
                if referent.layout.size.is_none() {
                    return Err(error(
                        format!("{path}.referent"),
                        "thin references require a Sized referent",
                    ));
                }
                stack.push((format!("{path}.referent"), referent, *address_space));
            }
            MirTypeKind::Slice { element } => {
                stack.push((format!("{path}.element"), element, containing_address_space));
            }
            MirTypeKind::Array { element, length } => {
                if target.pointer_width_bits == 32 && *length > u64::from(u32::MAX) {
                    return Err(error(
                        &path,
                        "array length does not fit the target usize width",
                    ));
                }
                let element_size = element
                    .layout
                    .size
                    .expect("semantic validation requires sized array elements");
                let total = element_size.checked_mul(*length).ok_or_else(|| {
                    error(
                        &path,
                        "array element size times length overflows the executable layout width",
                    )
                })?;
                if ty.layout.size != Some(total) || ty.layout.align != element.layout.align {
                    return Err(error(
                        &path,
                        "array total layout is incoherent with its element size, length, or alignment",
                    ));
                }
                stack.push((format!("{path}.element"), element, containing_address_space));
            }
            MirTypeKind::Tuple(aggregate) => {
                for (index, field) in aggregate.fields.iter().enumerate() {
                    validate_field_offset_range(
                        &format!("{path}.fields[{index}]"),
                        field,
                        containing_address_space,
                        target,
                    )?;
                    stack.push((
                        format!("{path}.fields[{index}].type"),
                        &field.ty,
                        containing_address_space,
                    ));
                }
            }
            MirTypeKind::Struct(structure) => {
                for (index, field) in structure.aggregate.fields.iter().enumerate() {
                    validate_field_offset_range(
                        &format!("{path}.fields[{index}]"),
                        field,
                        containing_address_space,
                        target,
                    )?;
                    stack.push((
                        format!("{path}.fields[{index}].type"),
                        &field.ty,
                        containing_address_space,
                    ));
                }
            }
            MirTypeKind::Enum(enum_ty) => {
                for (variant_index, variant) in enum_ty.variants.iter().enumerate() {
                    for (field_index, field) in variant.aggregate.fields.iter().enumerate() {
                        validate_field_offset_range(
                            &format!("{path}.variants[{variant_index}].fields[{field_index}]"),
                            field,
                            containing_address_space,
                            target,
                        )?;
                        stack.push((
                            format!("{path}.variants[{variant_index}].fields[{field_index}].type"),
                            &field.ty,
                            containing_address_space,
                        ));
                    }
                }
            }
            MirTypeKind::Unit | MirTypeKind::Scalar(_) => {}
        }
    }
    Ok(())
}

fn validate_target_offset_ranges(
    root: &str,
    ty: &MirSemanticType,
    containing_address_space: MirAddressSpace,
    target: &MirExecutableTarget,
) -> Result<(), MirExecutableValidationError> {
    let mut stack = vec![(root.to_owned(), ty, containing_address_space)];
    while let Some((path, ty, address_space)) = stack.pop() {
        validate_layout_offset_range(&path, ty.layout, address_space, target)?;
        match &ty.kind {
            MirTypeKind::RawPointer {
                pointee,
                address_space: pointee_address_space,
                ..
            } => stack.push((format!("{path}.pointee"), pointee, *pointee_address_space)),
            MirTypeKind::Reference {
                referent,
                address_space: referent_address_space,
                ..
            } => stack.push((
                format!("{path}.referent"),
                referent,
                *referent_address_space,
            )),
            MirTypeKind::Slice { element } | MirTypeKind::Array { element, .. } => {
                stack.push((format!("{path}.element"), element, address_space));
            }
            MirTypeKind::Tuple(aggregate) => {
                push_offset_range_fields(
                    &path,
                    &aggregate.fields,
                    address_space,
                    target,
                    &mut stack,
                )?;
            }
            MirTypeKind::Struct(structure) => {
                push_offset_range_fields(
                    &path,
                    &structure.aggregate.fields,
                    address_space,
                    target,
                    &mut stack,
                )?;
            }
            MirTypeKind::Enum(enum_ty) => {
                for (variant_index, variant) in enum_ty.variants.iter().enumerate() {
                    push_offset_range_fields(
                        &format!("{path}.variants[{variant_index}]"),
                        &variant.aggregate.fields,
                        address_space,
                        target,
                        &mut stack,
                    )?;
                }
            }
            MirTypeKind::Unit | MirTypeKind::Scalar(_) => {}
        }
    }
    Ok(())
}

fn push_offset_range_fields<'a>(
    path: &str,
    fields: &'a [crate::MirField],
    address_space: MirAddressSpace,
    target: &MirExecutableTarget,
    stack: &mut Vec<(String, &'a MirSemanticType, MirAddressSpace)>,
) -> Result<(), MirExecutableValidationError> {
    for (index, field) in fields.iter().enumerate() {
        let field_path = format!("{path}.fields[{index}]");
        validate_field_offset_range(&field_path, field, address_space, target)?;
        stack.push((format!("{field_path}.type"), &field.ty, address_space));
    }
    Ok(())
}

fn validate_field_offset_range(
    path: &str,
    field: &crate::MirField,
    address_space: MirAddressSpace,
    target: &MirExecutableTarget,
) -> Result<(), MirExecutableValidationError> {
    let maximum = signed_pointer_offset_max(address_space, target)?;
    let field_size = field.ty.layout.size.unwrap_or(0);
    let end = field.offset.checked_add(field_size).ok_or_else(|| {
        error(
            path,
            "field offset plus layout size overflows the executable layout width",
        )
    })?;
    if field.offset > maximum || end > maximum {
        return Err(error(
            path,
            format!(
                "field offset exceeds the signed pointer-offset range for address space {}",
                address_space.0
            ),
        ));
    }
    Ok(())
}

fn validate_layout_offset_range(
    path: &str,
    layout: crate::MirLayout,
    address_space: MirAddressSpace,
    target: &MirExecutableTarget,
) -> Result<(), MirExecutableValidationError> {
    let maximum = signed_pointer_offset_max(address_space, target)?;
    if layout.align > maximum {
        return Err(error(
            format!("{path}.align"),
            format!(
                "layout alignment exceeds the signed pointer-offset range for address space {}",
                address_space.0
            ),
        ));
    }
    if layout.size.is_some_and(|size| size > maximum) {
        return Err(error(
            format!("{path}.size"),
            format!(
                "layout size exceeds the signed pointer-offset range for address space {}",
                address_space.0
            ),
        ));
    }
    Ok(())
}

fn signed_pointer_offset_max(
    address_space: MirAddressSpace,
    target: &MirExecutableTarget,
) -> Result<u64, MirExecutableValidationError> {
    let abi = target.pointer_abi(address_space).ok_or_else(|| {
        error(
            "module.target.pointer_abis",
            format!(
                "address space {} has no signed pointer-offset profile",
                address_space.0
            ),
        )
    })?;
    match abi.width_bits {
        32 => Ok(i32::MAX as u64),
        64 => Ok(i64::MAX as u64),
        _ => Err(error(
            "module.target.pointer_abis",
            "signed pointer-offset range requires a 32-bit or 64-bit pointer ABI",
        )),
    }
}

fn validate_type_budget(types: &[MirSemanticType]) -> Result<(), MirExecutableValidationError> {
    validate_type_budget_at(types, "module.types")
}

fn validate_type_budget_at(
    types: &[MirSemanticType],
    root: &str,
) -> Result<(), MirExecutableValidationError> {
    let mut stack = types
        .iter()
        .enumerate()
        .map(|(index, ty)| (format!("{root}[{index}]"), ty, 1_usize))
        .collect::<Vec<_>>();
    let mut nodes = 0_usize;
    let mut items = types.len();
    while let Some((path, ty, depth)) = stack.pop() {
        nodes = nodes
            .checked_add(1)
            .ok_or_else(|| error(&path, "semantic type node count overflow"))?;
        if nodes > MAX_EXECUTABLE_TYPE_NODES {
            return Err(error(
                path,
                format!("semantic type graph exceeds {MAX_EXECUTABLE_TYPE_NODES} nodes"),
            ));
        }
        if depth > MAX_EXECUTABLE_TYPE_DEPTH {
            return Err(error(
                path,
                format!("semantic type graph exceeds depth {MAX_EXECUTABLE_TYPE_DEPTH}"),
            ));
        }
        match &ty.kind {
            MirTypeKind::RawPointer { pointee, .. } => {
                stack.push((format!("{path}.pointee"), pointee, depth + 1));
            }
            MirTypeKind::Reference { referent, .. } => {
                stack.push((format!("{path}.referent"), referent, depth + 1));
            }
            MirTypeKind::Slice { element } | MirTypeKind::Array { element, .. } => {
                stack.push((format!("{path}.element"), element, depth + 1));
            }
            MirTypeKind::Tuple(aggregate) => {
                push_aggregate_types(&path, aggregate, depth, &mut stack, &mut items)?;
            }
            MirTypeKind::Struct(structure) => {
                validate_identity(&format!("{path}.identity"), &structure.identity)?;
                push_aggregate_types(&path, &structure.aggregate, depth, &mut stack, &mut items)?;
            }
            MirTypeKind::Enum(enum_ty) => {
                validate_identity(&format!("{path}.identity"), &enum_ty.identity)?;
                bounded_len(
                    &format!("{path}.variants"),
                    enum_ty.variants.len(),
                    0,
                    MAX_EXECUTABLE_VARIANTS,
                )?;
                add_type_items(&path, &mut items, enum_ty.variants.len())?;
                for (variant_index, variant) in enum_ty.variants.iter().enumerate() {
                    validate_name(
                        &format!("{path}.variants[{variant_index}].name"),
                        &variant.name,
                    )?;
                    push_aggregate_types(
                        &format!("{path}.variants[{variant_index}]"),
                        &variant.aggregate,
                        depth,
                        &mut stack,
                        &mut items,
                    )?;
                }
            }
            MirTypeKind::Unit | MirTypeKind::Scalar(_) => {}
        }
    }
    Ok(())
}

fn push_aggregate_types<'a>(
    path: &str,
    aggregate: &'a crate::MirAggregateLayout,
    depth: usize,
    stack: &mut Vec<(String, &'a MirSemanticType, usize)>,
    items: &mut usize,
) -> Result<(), MirExecutableValidationError> {
    bounded_len(
        &format!("{path}.fields"),
        aggregate.fields.len(),
        0,
        MAX_EXECUTABLE_FIELDS,
    )?;
    add_type_items(path, items, aggregate.fields.len())?;
    add_type_items(path, items, aggregate.padding.len())?;
    bounded_len(
        &format!("{path}.padding"),
        aggregate.padding.len(),
        0,
        MAX_EXECUTABLE_FIELDS,
    )?;
    for (field_index, field) in aggregate.fields.iter().enumerate() {
        validate_name_opt(
            &format!("{path}.fields[{field_index}].name"),
            field.name.as_deref(),
        )?;
        stack.push((
            format!("{path}.fields[{field_index}].type"),
            &field.ty,
            depth + 1,
        ));
    }
    Ok(())
}

fn add_type_items(
    path: &str,
    items: &mut usize,
    additional: usize,
) -> Result<(), MirExecutableValidationError> {
    *items = items
        .checked_add(additional)
        .ok_or_else(|| error(path, "semantic type item count overflow"))?;
    if *items > MAX_EXECUTABLE_TYPE_ITEMS {
        return Err(error(
            path,
            format!("semantic type graph exceeds {MAX_EXECUTABLE_TYPE_ITEMS} structural items"),
        ));
    }
    Ok(())
}

fn error(path: impl Into<String>, reason: impl Into<String>) -> MirExecutableValidationError {
    MirExecutableValidationError::new(path, reason)
}
