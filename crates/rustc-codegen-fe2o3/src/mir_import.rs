use crate::collector::CollectionResult;
use crate::semantic_features::{self, AuthenticatedSemanticItem};
use crate::trusted_device_items::TrustedDeviceItem;
use dialect_mir::{MirAttr, MirOp, MirOpRecord, MirType};
use fe2o3_compiler_ffi::CodeObjectVersion;
use fe2o3_rustc_front::FunctionIdentityV1;
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiContractIdV1,
    DeviceFfiEffectsV1, DeviceFfiPhysicalAbiV1, derive_device_ffi_contract_id_v1,
    parse_device_ffi_effects_v1, parse_device_ffi_physical_abi_v1,
    validate_device_ffi_effect_abi_v1,
};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::mir::{
    BasicBlock, BinOp, Body, ConstOperand, Local, Operand, Place, ProjectionElem, Rvalue,
    SourceInfo, StatementKind, TerminatorKind, UnOp,
};
use rustc_middle::ty::{
    FloatTy, Instance, IntTy, Mutability, Ty, TyCtxt, TyKind, TypingEnv, UintTy,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Write};

const MIR_FUNCTION_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.mir-function-identity.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirModule {
    pub functions: Vec<MirFunction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunction {
    pub export_name: String,
    pub rust_path: String,
    pub kind: MirFunctionKind,
    pub typed_profile: Option<MirKernelProfile>,
    pub arg_count: usize,
    pub local_count: usize,
    pub locals: Vec<MirLocal>,
    pub blocks: Vec<MirBlock>,
    pub frontend_contract: Option<crate::collector::AuthenticatedKernelFrontendContractV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirFunctionKind {
    KernelEntry,
    InternalHelper,
    DeviceFfiExport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirKernelProfile {
    VecAddRustcLayoutV2,
    GeneralScalarSliceRustcLayoutV3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirLocal {
    pub index: usize,
    pub role: MirLocalRole,
    pub ty: MirImportedType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirLocalRole {
    Return,
    Arg,
    Temp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirImportedType {
    pub kind: MirType,
    pub rust: String,
    pub shape: MirTypeShape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTypeShape {
    Unit,
    Bool,
    I32,
    U32,
    I64,
    ISize,
    USize,
    F32,
    F64,
    F16,
    Bf16,
    Bf16x2,
    DeviceMath,
    Slice {
        element: Box<MirTypeShape>,
        mutable: bool,
    },
    DisjointSlice {
        element: Box<MirTypeShape>,
    },
    Reference {
        pointee: Box<MirTypeShape>,
        mutable: bool,
    },
    RawPointer {
        pointee: Box<MirTypeShape>,
        mutable: bool,
    },
    Adt {
        identity: String,
    },
    Tuple(Vec<MirTypeShape>),
    Unknown,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MirSourceLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBlock {
    pub index: usize,
    pub statements: Vec<MirStatement>,
    pub terminator: Option<MirTerminator>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStatement {
    pub index: usize,
    pub kind: MirStatementKind,
    pub destination: Option<MirPlaceRef>,
    pub operands: Vec<MirOperandRef>,
    pub rvalue: Option<MirRvalueKind>,
    /// Compatibility spelling consumed by the legacy record recognizer.
    pub operation: Option<String>,
    pub source: Option<MirSourceLocation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStatementKind {
    Assign,
    StorageLive,
    StorageDead,
    SetDiscriminant,
    Intrinsic,
    Retag,
    Coverage,
    Nop,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPlaceRef {
    pub local: usize,
    pub projection: Vec<MirProjectionElem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirProjectionElem {
    Deref,
    Field(usize),
    Index {
        local: usize,
    },
    ConstantIndex {
        offset: u64,
        min_length: u64,
        from_end: bool,
    },
    Subslice {
        from: u64,
        to: u64,
        from_end: bool,
    },
    Downcast {
        variant: usize,
    },
    OpaqueCast,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirOperandRef {
    Place(MirPlaceRef),
    Constant {
        ty: MirImportedType,
        literal: MirConstant,
        /// Compatibility spelling consumed by the legacy record recognizer.
        value: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirConstant {
    Bool(bool),
    I32(i32),
    U32(u32),
    I64(i64),
    ISize(i64),
    USize(u64),
    F32Bits(u32),
    F64Bits(u64),
    Unevaluated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirTerminator {
    pub kind: MirTerminatorKind,
    pub source: Option<MirSourceLocation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCallee {
    identity: MirCalleeIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirExternalImport {
    contract_identity: DeviceFfiContractIdV1,
    symbol: String,
    target: String,
    code_object_version: u16,
    physical_abi: DeviceFfiPhysicalAbiV1,
    effects: DeviceFfiEffectsV1,
    semantic_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MirCalleeIdentity {
    Authenticated(AuthenticatedSemanticItem),
    ExternalImport(MirExternalImport),
    Untrusted(String),
}

impl MirCallee {
    fn authenticated(item: AuthenticatedSemanticItem) -> Self {
        Self {
            identity: MirCalleeIdentity::Authenticated(item),
        }
    }

    #[cfg(test)]
    fn trusted(item: TrustedDeviceItem) -> Self {
        Self::authenticated(AuthenticatedSemanticItem::trusted_device_for_test(item))
    }

    fn untrusted(identity: String) -> Self {
        Self {
            identity: MirCalleeIdentity::Untrusted(identity),
        }
    }

    fn external_import(import: MirExternalImport) -> Self {
        Self {
            identity: MirCalleeIdentity::ExternalImport(import),
        }
    }

    pub(crate) fn identity(&self) -> &str {
        match &self.identity {
            MirCalleeIdentity::Authenticated(item) => item.canonical_path(),
            MirCalleeIdentity::ExternalImport(import) => &import.symbol,
            MirCalleeIdentity::Untrusted(identity) => identity,
        }
    }

    pub(crate) fn authenticated_item(&self) -> Option<AuthenticatedSemanticItem> {
        match &self.identity {
            MirCalleeIdentity::Authenticated(item) => Some(*item),
            MirCalleeIdentity::ExternalImport(_) | MirCalleeIdentity::Untrusted(_) => None,
        }
    }

    pub(crate) fn trusted_item(&self) -> Option<TrustedDeviceItem> {
        self.authenticated_item()
            .map(AuthenticatedSemanticItem::trusted_device_item)
    }

    pub(crate) fn external_import_evidence(&self) -> Option<&MirExternalImport> {
        match &self.identity {
            MirCalleeIdentity::ExternalImport(import) => Some(import),
            MirCalleeIdentity::Authenticated(_) | MirCalleeIdentity::Untrusted(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn trusted_for_test(item: TrustedDeviceItem) -> Self {
        Self::trusted(item)
    }

    #[cfg(test)]
    pub(crate) fn untrusted_for_test(identity: impl Into<String>) -> Self {
        Self::untrusted(identity.into())
    }

    #[cfg(test)]
    pub(crate) fn external_import_for_test(
        symbol: &str,
        physical_abi: &str,
        effects: &str,
    ) -> Self {
        let physical_abi = parse_device_ffi_physical_abi_v1(physical_abi)
            .expect("test external-import ABI must be canonical");
        let effects =
            parse_device_ffi_effects_v1(effects).expect("test external-import effects must parse");
        validate_device_ffi_effect_abi_v1(&effects, &physical_abi)
            .expect("test external-import ABI/effects must agree");
        Self::external_import(MirExternalImport {
            contract_identity: DeviceFfiContractIdV1::from_bytes([0x5a; 32]),
            symbol: symbol.to_owned(),
            target: "gfx942:xnack-".to_owned(),
            code_object_version: 5,
            physical_abi,
            effects,
            semantic_identity: "6b".repeat(32),
        })
    }
}

impl MirExternalImport {
    pub(crate) fn physical_abi(&self) -> &DeviceFfiPhysicalAbiV1 {
        &self.physical_abi
    }

    pub(crate) fn effects(&self) -> &DeviceFfiEffectsV1 {
        &self.effects
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSwitchTarget {
    pub value: u128,
    pub target: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTerminatorKind {
    Return,
    Unreachable,
    Goto {
        target: usize,
    },
    SwitchInt {
        discriminant: MirOperandRef,
        targets: Vec<MirSwitchTarget>,
        otherwise: usize,
    },
    Call {
        callee: Option<MirCallee>,
        target: Option<usize>,
        destination: Option<MirPlaceRef>,
        operands: Vec<MirOperandRef>,
    },
    Assert {
        condition: MirOperandRef,
        expected: bool,
        target: usize,
    },
    Drop {
        target: usize,
    },
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirRvalueKind {
    Use,
    Repeat,
    Ref,
    RawPointer,
    Cast,
    Binary(MirBinaryOp),
    Unary(MirUnaryOp),
    Discriminant,
    Aggregate,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    Lt,
    Le,
    Ne,
    Ge,
    Gt,
    Cmp,
    Offset,
    AddUnchecked,
    SubUnchecked,
    MulUnchecked,
    ShlUnchecked,
    ShrUnchecked,
    AddWithOverflow,
    SubWithOverflow,
    MulWithOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirUnaryOp {
    Not,
    Neg,
    PtrMetadata,
}

#[derive(Clone, Debug)]
struct CompilerFfiImports {
    entries: Vec<(crate::device_ffi::DeviceFfiSourceOwner, MirExternalImport)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirImportError {
    message: String,
}

impl MirImportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for MirImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for MirImportError {}

impl CompilerFfiImports {
    fn from_collection<'tcx>(
        tcx: TyCtxt<'tcx>,
        collection: &CollectionResult<'tcx>,
    ) -> Result<Self, MirImportError> {
        let reconstructed = crate::compiler_ffi_adapter::adapt_collection_v1(tcx, collection)
            .map_err(|error| {
                MirImportError::new(format!(
                    "compiler FFI evidence could not be reconstructed before MIR import: {error}"
                ))
            })?;
        if reconstructed.as_ref() != collection.compiler_ffi_observation.as_ref() {
            return Err(MirImportError::new(
                "compiler FFI observation disagrees with the closed collection",
            ));
        }

        let imports = &collection.device_ffi.imports;
        if imports.is_empty() {
            return Ok(Self {
                entries: Vec::new(),
            });
        }
        let envelope = reconstructed.as_ref().ok_or_else(|| {
            MirImportError::new("reachable device FFI imports have no compiler envelope")
        })?;
        let envelope_symbols = envelope
            .directional_symbols()
            .imports()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let collected_symbols = imports
            .iter()
            .map(|entry| entry.contract.symbol.clone())
            .collect::<Vec<_>>();
        if envelope_symbols != collected_symbols {
            return Err(MirImportError::new(
                "compiler FFI import symbols disagree with the closed collection",
            ));
        }

        let envelope_target = envelope.target().to_string();
        let envelope_code_object_version =
            code_object_version_number(envelope.code_object_version());
        let mut entries = Vec::with_capacity(imports.len());
        for entry in imports {
            let contract = &entry.contract;
            if contract.direction != crate::device_ffi::DeviceFfiDirection::Import
                || entry.link_role_assertion.asserted_for_consistency_check()
                    != &crate::device_ffi::DeviceFfiLinkRole::RequiresExternalDefinition
            {
                return Err(MirImportError::new(format!(
                    "device FFI `{}` is not an external import",
                    contract.symbol
                )));
            }
            let asserted_code_object_version = *contract
                .code_object_version_assertion
                .asserted_for_consistency_check();
            let semantic_identity = contract
                .semantic_identity_assertion
                .asserted_for_consistency_check();
            let effects_text = contract.effects_assertion.asserted_for_consistency_check();
            entries.push((
                entry.owner.clone(),
                validate_external_import_fields(
                    contract.id,
                    &contract.symbol,
                    &contract.target,
                    asserted_code_object_version,
                    &contract.physical_abi,
                    effects_text,
                    semantic_identity,
                    &envelope_target,
                    envelope_code_object_version,
                )?,
            ));
        }
        Ok(Self { entries })
    }

    fn classify<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        def_id: rustc_hir::def_id::DefId,
    ) -> Option<MirExternalImport> {
        let def_path_hash = tcx.def_path_hash(def_id).0.to_le_bytes();
        self.entries.iter().find_map(|(owner, import)| {
            (owner.def_path_hash == def_path_hash
                && crate::device_ffi::source_owner_matches_instance(
                    tcx,
                    owner,
                    rustc_middle::ty::Instance::mono(tcx, def_id),
                ))
            .then(|| import.clone())
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_external_import_fields(
    contract_identity: DeviceFfiContractIdV1,
    symbol: &str,
    target: &str,
    code_object_version: u16,
    physical_abi_text: &str,
    effects_text: &str,
    semantic_identity: &str,
    envelope_target: &str,
    envelope_code_object_version: u16,
) -> Result<MirExternalImport, MirImportError> {
    if target != envelope_target {
        return Err(MirImportError::new(format!(
            "device FFI import `{symbol}` target `{target}` disagrees with compiler envelope target `{envelope_target}`"
        )));
    }
    if code_object_version != envelope_code_object_version {
        return Err(MirImportError::new(format!(
            "device FFI import `{symbol}` code-object version {code_object_version} disagrees with compiler envelope version {envelope_code_object_version}"
        )));
    }
    let derived_identity = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
        symbol,
        calling_convention: "C",
        code_object_version,
        target,
        physical_abi: physical_abi_text,
        effects: effects_text,
        semantic_identity,
    });
    if derived_identity != contract_identity {
        return Err(MirImportError::new(format!(
            "device FFI import `{symbol}` contract identity does not match its canonical fields"
        )));
    }
    let physical_abi = parse_device_ffi_physical_abi_v1(physical_abi_text).map_err(|_| {
        MirImportError::new(format!(
            "device FFI import `{symbol}` has a malformed physical ABI"
        ))
    })?;
    let effects = parse_device_ffi_effects_v1(effects_text).map_err(|_| {
        MirImportError::new(format!(
            "device FFI import `{symbol}` has malformed effects"
        ))
    })?;
    validate_device_ffi_effect_abi_v1(&effects, &physical_abi).map_err(|_| {
        MirImportError::new(format!(
            "device FFI import `{symbol}` effects disagree with its physical ABI"
        ))
    })?;
    if !effects.is_none() {
        return Err(MirImportError::new(format!(
            "device FFI import `{symbol}` declares effects that kernel IR cannot yet preserve"
        )));
    }
    Ok(MirExternalImport {
        contract_identity,
        symbol: symbol.to_owned(),
        target: target.to_owned(),
        code_object_version,
        physical_abi,
        effects,
        semantic_identity: semantic_identity.to_owned(),
    })
}

const fn code_object_version_number(version: CodeObjectVersion) -> u16 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}

pub fn import_collection<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
) -> Result<MirModule, MirImportError> {
    let compiler_ffi_imports = CompilerFfiImports::from_collection(tcx, collection)?;
    let functions = collection
        .functions
        .iter()
        .filter_map(|function| {
            let def_id = function.instance.def_id();
            if !tcx.is_mir_available(def_id) {
                return None;
            }

            let body = tcx.instance_mir(function.instance.def);
            let rust_path = imported_rust_path(tcx, def_id);
            Some(import_body(
                tcx,
                body,
                function.export_name.clone(),
                rust_path,
                import_function_kind(function.role),
                MirKernelMetadata {
                    typed_profile: import_kernel_profile(function.typed_profile),
                    frontend_contract: function.frontend_contract.clone(),
                },
                &compiler_ffi_imports,
            ))
        })
        .collect();

    MirModule::from_functions_v1(functions)
}

fn imported_rust_path(tcx: TyCtxt<'_>, def_id: rustc_hir::def_id::DefId) -> String {
    let path = tcx.def_path_str(def_id);
    if def_id.krate == LOCAL_CRATE {
        format!("{}::{path}", tcx.crate_name(LOCAL_CRATE))
    } else {
        path
    }
}

fn import_function_kind(role: crate::collector::CollectedFunctionRole) -> MirFunctionKind {
    match role {
        crate::collector::CollectedFunctionRole::KernelEntry => MirFunctionKind::KernelEntry,
        crate::collector::CollectedFunctionRole::InternalHelper => MirFunctionKind::InternalHelper,
        crate::collector::CollectedFunctionRole::DeviceFfiExport => {
            MirFunctionKind::DeviceFfiExport
        }
    }
}

fn import_kernel_profile(
    profile: Option<crate::collector::TypedKernelProfile>,
) -> Option<MirKernelProfile> {
    profile.map(|profile| match profile {
        crate::collector::TypedKernelProfile::VecAddRustcLayoutV2 => {
            MirKernelProfile::VecAddRustcLayoutV2
        }
        crate::collector::TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. } => {
            MirKernelProfile::GeneralScalarSliceRustcLayoutV3
        }
    })
}

impl MirModule {
    fn from_functions_v1(mut functions: Vec<MirFunction>) -> Result<Self, MirImportError> {
        if functions.is_empty() {
            return Err(MirImportError::new(
                "MIR module contains no imported functions",
            ));
        }

        let mut exports = BTreeMap::new();
        let mut kernel_paths = BTreeMap::new();
        let mut identities = BTreeSet::new();
        let mut kernel_count = 0_usize;
        for function in &functions {
            function.validate_identity_fields_v1()?;
            if let Some(previous) = exports.insert(&function.export_name, &function.rust_path) {
                return Err(MirImportError::new(format!(
                    "duplicate function export `{}` for source functions `{previous}` and `{}`",
                    function.export_name, function.rust_path
                )));
            }

            let identity = function.source_identity_v1()?;
            if !identities.insert(identity) {
                return Err(MirImportError::new(format!(
                    "duplicate MIR function identity {} for `{}`",
                    function_identity_hex_v1(identity),
                    function.rust_path
                )));
            }

            if function.kind == MirFunctionKind::KernelEntry {
                kernel_count += 1;
                if let Some(previous) =
                    kernel_paths.insert(&function.rust_path, &function.export_name)
                {
                    return Err(MirImportError::new(format!(
                        "ambiguous kernel roots `{previous}` and `{}` select the same source function `{}`",
                        function.export_name, function.rust_path
                    )));
                }
            }
        }
        if kernel_count == 0 {
            return Err(MirImportError::new("MIR module contains no kernel root"));
        }

        functions.sort_by(|lhs, rhs| {
            lhs.kind
                .canonical_order_v1()
                .cmp(&rhs.kind.canonical_order_v1())
                .then_with(|| {
                    lhs.source_identity_bytes_v1()
                        .cmp(&rhs.source_identity_bytes_v1())
                })
                .then_with(|| lhs.export_name.cmp(&rhs.export_name))
                .then_with(|| lhs.rust_path.cmp(&rhs.rust_path))
        });
        Ok(Self { functions })
    }

    pub fn summary(&self) -> String {
        let record_count = self.dialect_records().len();
        let mut output = format!(
            "\n=== fe2o3 MIR import scaffold ({}, {record_count} op records) ===\n",
            MirOp::Module.name(),
        );
        for function in &self.functions {
            let kind = match function.kind {
                MirFunctionKind::KernelEntry => "kernel-entry",
                MirFunctionKind::InternalHelper => "internal-helper",
                MirFunctionKind::DeviceFfiExport => "device-ffi-export",
            };
            let _ = writeln!(
                output,
                "  [{kind}] {} ({})",
                function.export_name,
                MirOp::Func.name()
            );
            let _ = writeln!(output, "      path: {}", function.rust_path);
            if let Ok(identity) = function.source_identity_v1() {
                let _ = writeln!(
                    output,
                    "      source identity v1: {}",
                    function_identity_hex_v1(identity)
                );
            }
            let _ = writeln!(
                output,
                "      MIR:  {} bb, {} locals, {} args",
                function.blocks.len(),
                function.local_count,
                function.arg_count
            );
            for local in function
                .locals
                .iter()
                .filter(|local| local.role != MirLocalRole::Temp)
            {
                let role = match local.role {
                    MirLocalRole::Return => "return",
                    MirLocalRole::Arg => "arg",
                    MirLocalRole::Temp => "temp",
                };
                let _ = writeln!(
                    output,
                    "      local{}: {role} {} ({})",
                    local.index,
                    local.ty.kind.name(),
                    local.ty.rust
                );
            }
            for block in &function.blocks {
                let terminator = block
                    .terminator
                    .as_ref()
                    .map(|terminator| terminator.kind.summary())
                    .unwrap_or("missing terminator".to_string());
                let _ = writeln!(
                    output,
                    "      bb{} ({}): {} stmt(s), {terminator}",
                    block.index,
                    MirOp::Block.name(),
                    block.statements.len()
                );
                for statement in &block.statements {
                    if let Some(summary) = statement.summary() {
                        let _ = writeln!(output, "          {summary}");
                    }
                }
            }
        }
        output.push_str("===================================\n");
        output
    }

    pub fn dialect_records(&self) -> Vec<MirOpRecord> {
        let source_identities = self.source_identities_by_path_v1();
        let kernel_count = self
            .functions
            .iter()
            .filter(|function| function.kind == MirFunctionKind::KernelEntry)
            .count();
        let helper_count = self
            .functions
            .iter()
            .filter(|function| function.kind == MirFunctionKind::InternalHelper)
            .count();
        let mut records = vec![
            MirOpRecord::new(MirOp::Module)
                .with_attr(MirAttr::usize("functions", self.functions.len()))
                .with_attr(MirAttr::usize("kernel_roots", kernel_count))
                .with_attr(MirAttr::usize("internal_helpers", helper_count)),
        ];

        for function in &self.functions {
            // `kind` is a V1 compatibility field consumed by record_lowering.
            // The closed role remains authoritative on `MirFunction::kind`.
            let kind = match function.kind {
                MirFunctionKind::KernelEntry => "kernel",
                MirFunctionKind::InternalHelper | MirFunctionKind::DeviceFfiExport => "device",
            };
            let mut function_record = MirOpRecord::new(MirOp::Func)
                .with_attr(MirAttr::string("symbol", &function.export_name))
                .with_attr(MirAttr::string("kind", kind))
                .with_attr(MirAttr::string("rust_path", &function.rust_path))
                .with_attr(MirAttr::usize("args", function.arg_count))
                .with_attr(MirAttr::usize("locals", function.local_count))
                .with_attr(MirAttr::usize("blocks", function.blocks.len()));
            if let Ok(identity) = function.source_identity_v1() {
                function_record.attrs.push(MirAttr::string(
                    "source_identity_v1",
                    function_identity_hex_v1(identity),
                ));
            }
            records.push(function_record);

            for local in &function.locals {
                let role = match local.role {
                    MirLocalRole::Return => "return",
                    MirLocalRole::Arg => "arg",
                    MirLocalRole::Temp => "temp",
                };
                let op = match local.role {
                    MirLocalRole::Arg => MirOp::Arg,
                    MirLocalRole::Return | MirLocalRole::Temp => MirOp::Local,
                };
                records.push(
                    MirOpRecord::new(op)
                        .with_attr(MirAttr::string("function", &function.export_name))
                        .with_attr(MirAttr::usize("index", local.index))
                        .with_attr(MirAttr::string("role", role))
                        .with_attr(MirAttr::string("type", local.ty.kind.name()))
                        .with_attr(MirAttr::string("rust_type", &local.ty.rust)),
                );
            }

            for block in &function.blocks {
                records.push(
                    MirOpRecord::new(MirOp::Block)
                        .with_attr(MirAttr::string("function", &function.export_name))
                        .with_attr(MirAttr::usize("index", block.index))
                        .with_attr(MirAttr::usize("statements", block.statements.len())),
                );

                for statement in &block.statements {
                    records.push(statement.record(&function.export_name, block.index));
                    if let Some(record) =
                        statement.lowering_record(&function.export_name, block.index)
                    {
                        records.push(record);
                    }
                }

                if let Some(terminator) = &block.terminator {
                    records.push(terminator.kind.record(
                        &function.export_name,
                        block.index,
                        &source_identities,
                    ));
                }
            }
        }

        records
    }

    fn source_identities_by_path_v1(&self) -> BTreeMap<&str, Option<FunctionIdentityV1>> {
        let mut identities = BTreeMap::new();
        for function in &self.functions {
            let identity = function.source_identity_v1().ok();
            identities
                .entry(function.rust_path.as_str())
                .and_modify(|existing| *existing = None)
                .or_insert(identity);
        }
        identities
    }
}

impl MirFunction {
    fn validate_identity_fields_v1(&self) -> Result<(), MirImportError> {
        for (field, value) in [
            ("function export", self.export_name.as_str()),
            ("source function path", self.rust_path.as_str()),
        ] {
            if value.is_empty() {
                return Err(MirImportError::new(format!("{field} must not be empty")));
            }
            if value.chars().any(char::is_control) {
                return Err(MirImportError::new(format!(
                    "{field} `{value:?}` contains control characters"
                )));
            }
        }
        if self.kind != MirFunctionKind::KernelEntry && self.typed_profile.is_some() {
            return Err(MirImportError::new(format!(
                "non-kernel function `{}` carries a typed kernel profile",
                self.rust_path
            )));
        }
        Ok(())
    }

    fn source_identity_v1(&self) -> Result<FunctionIdentityV1, MirImportError> {
        FunctionIdentityV1::new(self.source_identity_bytes_v1()).map_err(|error| {
            MirImportError::new(format!(
                "invalid source identity for `{}`: {error}",
                self.rust_path
            ))
        })
    }

    fn source_identity_bytes_v1(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        append_identity_field_v1(&mut digest, MIR_FUNCTION_IDENTITY_DOMAIN_V1);
        append_identity_field_v1(&mut digest, self.rust_path.as_bytes());
        append_identity_field_v1(&mut digest, self.export_name.as_bytes());
        digest.finalize().into()
    }
}

impl MirFunctionKind {
    const fn canonical_order_v1(self) -> u8 {
        match self {
            Self::KernelEntry => 0,
            Self::InternalHelper => 1,
            Self::DeviceFfiExport => 2,
        }
    }
}

fn append_identity_field_v1(digest: &mut Sha256, field: &[u8]) {
    digest.update(u64::try_from(field.len()).unwrap_or(u64::MAX).to_le_bytes());
    digest.update(field);
}

fn function_identity_hex_v1(identity: FunctionIdentityV1) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in identity.as_bytes() {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

impl MirStatement {
    fn summary(&self) -> Option<String> {
        if self.kind != MirStatementKind::Assign {
            return None;
        }

        let destination = self
            .destination
            .as_ref()
            .map(MirPlaceRef::label)
            .unwrap_or_else(|| "_".to_string());
        let operation = self.operation.as_deref().unwrap_or(self.kind.name());
        let dialect_op = self.lowering_op().unwrap_or_else(|| self.dialect_op());
        if self.operands.is_empty() {
            return Some(format!(
                "stmt{}: {} {destination} = {operation}",
                self.index,
                dialect_op.name()
            ));
        }

        let operands = self
            .operands
            .iter()
            .map(MirOperandRef::label)
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "stmt{}: {} {destination} = {operation}({operands})",
            self.index,
            dialect_op.name()
        ))
    }

    fn record(&self, function: &str, block: usize) -> MirOpRecord {
        let mut record = MirOpRecord::new(self.dialect_op())
            .with_attr(MirAttr::string("function", function))
            .with_attr(MirAttr::usize("block", block))
            .with_attr(MirAttr::usize("index", self.index))
            .with_attr(MirAttr::string("kind", self.kind.name()))
            .with_attr(MirAttr::usize("operand_count", self.operands.len()));

        if let Some(destination) = &self.destination {
            record
                .attrs
                .push(MirAttr::usize("destination_local", destination.local));
            record
                .attrs
                .push(MirAttr::string("destination", destination.label()));
        }
        if let Some(operation) = &self.operation {
            record.attrs.push(MirAttr::string("operation", operation));
        }
        if !self.operands.is_empty() {
            let operands = self
                .operands
                .iter()
                .map(MirOperandRef::label)
                .collect::<Vec<_>>()
                .join(", ");
            record.attrs.push(MirAttr::string("operands", operands));
        }

        record
    }

    fn lowering_record(&self, function: &str, block: usize) -> Option<MirOpRecord> {
        let op = self.lowering_op()?;
        let mut record = MirOpRecord::new(op)
            .with_attr(MirAttr::string("function", function))
            .with_attr(MirAttr::usize("block", block))
            .with_attr(MirAttr::usize("statement", self.index))
            .with_attr(MirAttr::string("source", self.dialect_op().name()))
            .with_attr(MirAttr::usize("operand_count", self.operands.len()));

        if let Some(destination) = &self.destination {
            record
                .attrs
                .push(MirAttr::usize("destination_local", destination.local));
            record
                .attrs
                .push(MirAttr::string("destination", destination.label()));
        }
        if let Some(operation) = &self.operation {
            record.attrs.push(MirAttr::string("operation", operation));
        }
        if !self.operands.is_empty() {
            let operands = self
                .operands
                .iter()
                .map(MirOperandRef::label)
                .collect::<Vec<_>>()
                .join(", ");
            record.attrs.push(MirAttr::string("operands", operands));
        }

        Some(record)
    }

    fn dialect_op(&self) -> MirOp {
        match self.kind {
            MirStatementKind::Assign => MirOp::Assign,
            MirStatementKind::StorageLive
            | MirStatementKind::StorageDead
            | MirStatementKind::SetDiscriminant
            | MirStatementKind::Intrinsic
            | MirStatementKind::Retag
            | MirStatementKind::Coverage
            | MirStatementKind::Nop
            | MirStatementKind::Other => MirOp::Statement,
        }
    }

    fn lowering_op(&self) -> Option<MirOp> {
        if self.kind != MirStatementKind::Assign {
            return None;
        }
        if self
            .destination
            .as_ref()
            .is_some_and(MirPlaceRef::is_memory_projection)
        {
            return Some(MirOp::Store);
        }

        match self.rvalue? {
            MirRvalueKind::Binary(
                MirBinaryOp::Add | MirBinaryOp::AddUnchecked | MirBinaryOp::AddWithOverflow,
            ) => Some(MirOp::Add),
            MirRvalueKind::Binary(
                MirBinaryOp::Sub | MirBinaryOp::SubUnchecked | MirBinaryOp::SubWithOverflow,
            ) => Some(MirOp::Sub),
            MirRvalueKind::Binary(
                MirBinaryOp::Mul | MirBinaryOp::MulUnchecked | MirBinaryOp::MulWithOverflow,
            ) => Some(MirOp::Mul),
            MirRvalueKind::Binary(MirBinaryOp::Div) => Some(MirOp::Div),
            MirRvalueKind::Binary(MirBinaryOp::Eq) => Some(MirOp::Eq),
            MirRvalueKind::Binary(MirBinaryOp::Lt) => Some(MirOp::Lt),
            MirRvalueKind::Binary(MirBinaryOp::Le) => Some(MirOp::Le),
            MirRvalueKind::Binary(MirBinaryOp::Ne) => Some(MirOp::Ne),
            MirRvalueKind::Binary(MirBinaryOp::Ge) => Some(MirOp::Ge),
            MirRvalueKind::Binary(MirBinaryOp::Gt) => Some(MirOp::Gt),
            MirRvalueKind::Binary(MirBinaryOp::Cmp) => Some(MirOp::Cmp),
            MirRvalueKind::Cast => Some(MirOp::Cast),
            MirRvalueKind::Binary(MirBinaryOp::Offset) => Some(MirOp::Gep),
            MirRvalueKind::Unary(MirUnaryOp::PtrMetadata) => Some(MirOp::SliceLen),
            MirRvalueKind::Use if self.operands.iter().any(MirOperandRef::is_memory_place) => {
                Some(MirOp::Load)
            }
            _ => None,
        }
    }
}

impl MirStatementKind {
    fn name(self) -> &'static str {
        match self {
            Self::Assign => "assign",
            Self::StorageLive => "storage_live",
            Self::StorageDead => "storage_dead",
            Self::SetDiscriminant => "set_discriminant",
            Self::Intrinsic => "intrinsic",
            Self::Retag => "retag",
            Self::Coverage => "coverage",
            Self::Nop => "nop",
            Self::Other => "other",
        }
    }
}

impl MirPlaceRef {
    fn local(local: Local) -> Self {
        Self {
            local: local.as_usize(),
            projection: Vec::new(),
        }
    }

    fn label(&self) -> String {
        let mut label = format!("local{}", self.local);
        for projection in &self.projection {
            label.push('.');
            label.push_str(&projection.label());
        }
        label
    }

    fn is_memory_projection(&self) -> bool {
        self.projection.iter().any(|projection| {
            matches!(
                projection,
                MirProjectionElem::Deref
                    | MirProjectionElem::Index { .. }
                    | MirProjectionElem::ConstantIndex { .. }
            )
        })
    }
}

impl MirProjectionElem {
    fn label(&self) -> String {
        match self {
            Self::Deref => "deref".to_string(),
            Self::Field(field) => format!("field{field}"),
            Self::Index { local } => format!("index_local{local}"),
            Self::ConstantIndex {
                offset,
                min_length,
                from_end,
            } => format!("constant_index{offset}_min{min_length}_from_end{from_end}"),
            Self::Subslice { from, to, from_end } => {
                format!("subslice{from}_{to}_from_end{from_end}")
            }
            Self::Downcast { variant } => format!("downcast{variant}"),
            Self::OpaqueCast => "opaque_cast".to_string(),
            Self::Other => "projection".to_string(),
        }
    }
}

impl MirOperandRef {
    fn label(&self) -> String {
        match self {
            Self::Place(place) => place.label(),
            Self::Constant { ty, value, .. } => format!("const:{}={value}", ty.kind.name()),
        }
    }

    fn is_memory_place(&self) -> bool {
        matches!(self, Self::Place(place) if place.is_memory_projection())
    }
}

impl MirTerminatorKind {
    fn record(
        &self,
        function: &str,
        block: usize,
        source_identities: &BTreeMap<&str, Option<FunctionIdentityV1>>,
    ) -> MirOpRecord {
        let mut record = MirOpRecord::new(self.dialect_op())
            .with_attr(MirAttr::string("function", function))
            .with_attr(MirAttr::usize("block", block));

        match self {
            Self::Goto { target } | Self::Assert { target, .. } | Self::Drop { target } => {
                record.attrs.push(MirAttr::usize("target", *target));
            }
            Self::SwitchInt { targets, .. } => {
                record
                    .attrs
                    .push(MirAttr::usize("targets", targets.len() + 1));
            }
            Self::Call {
                callee,
                target,
                destination,
                operands,
            } => {
                if let Some(callee) = callee {
                    record
                        .attrs
                        .push(MirAttr::string("callee", callee.identity()));
                    if let Some(Some(identity)) = source_identities.get(callee.identity()) {
                        record.attrs.push(MirAttr::string(
                            "callee_source_identity_v1",
                            function_identity_hex_v1(*identity),
                        ));
                    }
                    if let Some(import) = callee.external_import_evidence() {
                        record.attrs.push(MirAttr::string(
                            "device_ffi_contract",
                            import.contract_identity.to_hex(),
                        ));
                        record
                            .attrs
                            .push(MirAttr::string("device_ffi_target", &import.target));
                        record.attrs.push(MirAttr::usize(
                            "device_ffi_code_object_version",
                            usize::from(import.code_object_version),
                        ));
                        record.attrs.push(MirAttr::string(
                            "device_ffi_semantic_identity",
                            &import.semantic_identity,
                        ));
                    }
                }
                if let Some(target) = target {
                    record.attrs.push(MirAttr::usize("target", *target));
                }
                if let Some(destination) = destination {
                    record
                        .attrs
                        .push(MirAttr::usize("destination_local", destination.local));
                    record
                        .attrs
                        .push(MirAttr::string("destination", destination.label()));
                }
                record
                    .attrs
                    .push(MirAttr::usize("operand_count", operands.len()));
                if !operands.is_empty() {
                    let operands = operands
                        .iter()
                        .map(MirOperandRef::label)
                        .collect::<Vec<_>>()
                        .join(", ");
                    record.attrs.push(MirAttr::string("operands", operands));
                }
            }
            Self::Return | Self::Unreachable | Self::Other => {}
        }

        record
    }

    fn dialect_op(&self) -> MirOp {
        match self {
            Self::Return => MirOp::Return,
            Self::Unreachable => MirOp::Unreachable,
            Self::Goto { .. } => MirOp::Branch,
            Self::SwitchInt { .. } => MirOp::Switch,
            Self::Call { .. } => MirOp::Call,
            Self::Assert { .. } => MirOp::Assert,
            Self::Drop { .. } => MirOp::Drop,
            Self::Other => MirOp::Other,
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::Return => MirOp::Return.name().to_string(),
            Self::Unreachable => MirOp::Unreachable.name().to_string(),
            Self::Goto { target } => format!("{} -> bb{target}", MirOp::Branch.name()),
            Self::SwitchInt { targets, .. } => {
                format!("{} ({} target(s))", MirOp::Switch.name(), targets.len() + 1)
            }
            Self::Call { callee, target, .. } => {
                let callee = callee
                    .as_ref()
                    .map(MirCallee::identity)
                    .unwrap_or("<dynamic>");
                match target {
                    Some(target) => format!("{} {callee} -> bb{target}", MirOp::Call.name()),
                    None => format!("{} {callee} -> return", MirOp::Call.name()),
                }
            }
            Self::Assert { target, .. } => format!("{} -> bb{target}", MirOp::Assert.name()),
            Self::Drop { target } => format!("{} -> bb{target}", MirOp::Drop.name()),
            Self::Other => "other".to_string(),
        }
    }
}

struct MirKernelMetadata {
    typed_profile: Option<MirKernelProfile>,
    frontend_contract: Option<crate::collector::AuthenticatedKernelFrontendContractV1>,
}

fn import_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    export_name: String,
    rust_path: String,
    kind: MirFunctionKind,
    kernel_metadata: MirKernelMetadata,
    compiler_ffi_imports: &CompilerFfiImports,
) -> MirFunction {
    let blocks = body
        .basic_blocks
        .iter_enumerated()
        .map(|(index, block)| MirBlock {
            index: index.as_usize(),
            statements: block
                .statements
                .iter()
                .enumerate()
                .map(|(statement_index, statement)| {
                    import_statement(tcx, statement_index, &statement.kind, statement.source_info)
                })
                .collect(),
            terminator: block.terminator.as_ref().map(|terminator| MirTerminator {
                kind: terminator_kind(tcx, &terminator.kind, compiler_ffi_imports),
                source: Some(import_source_location(tcx, terminator.source_info)),
            }),
        })
        .collect();
    let locals = body
        .local_decls
        .iter_enumerated()
        .map(|(local, decl)| {
            let index = local.as_usize();
            let role = if index == 0 {
                MirLocalRole::Return
            } else if index <= body.arg_count {
                MirLocalRole::Arg
            } else {
                MirLocalRole::Temp
            };
            MirLocal {
                index,
                role,
                ty: import_type(tcx, decl.ty),
            }
        })
        .collect();

    MirFunction {
        export_name,
        rust_path,
        kind,
        typed_profile: kernel_metadata.typed_profile,
        arg_count: body.arg_count,
        local_count: body.local_decls.len(),
        locals,
        blocks,
        frontend_contract: kernel_metadata.frontend_contract,
    }
}

fn import_type<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> MirImportedType {
    let kind = match ty.kind() {
        TyKind::Bool => MirType::I1,
        TyKind::Int(IntTy::I32) => MirType::I32,
        TyKind::Uint(UintTy::U32) => MirType::I32,
        TyKind::Int(IntTy::I64) => MirType::I64,
        TyKind::Uint(UintTy::Usize) => MirType::USize,
        TyKind::Float(FloatTy::F32) => MirType::F32,
        TyKind::Float(FloatTy::F64) => MirType::F64,
        TyKind::Ref(_, pointee, _) => match pointee.kind() {
            TyKind::Slice(_) => MirType::Slice,
            _ => MirType::Ptr,
        },
        TyKind::RawPtr(_, _) => MirType::Ptr,
        TyKind::Adt(adt, _) => match semantic_features::classify(tcx, adt.did())
            .map(AuthenticatedSemanticItem::trusted_device_item)
        {
            Some(TrustedDeviceItem::DisjointSlice) => MirType::DisjointSlice,
            Some(TrustedDeviceItem::DeviceValue(
                dialect_amdgcn::DeviceValueDiagnosticItem::Bf16x2,
            )) => MirType::I32,
            Some(
                TrustedDeviceItem::DeviceValue(_)
                | TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::Context),
            ) => MirType::Unknown,
            _ => MirType::Unknown,
        },
        TyKind::Tuple(elements) if elements.is_empty() => MirType::Unit,
        _ => MirType::Unknown,
    };

    MirImportedType {
        kind,
        rust: ty.to_string(),
        shape: import_type_shape(tcx, ty),
    }
}

fn import_type_shape<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> MirTypeShape {
    match ty.kind() {
        TyKind::Bool => MirTypeShape::Bool,
        TyKind::Int(IntTy::I32) => MirTypeShape::I32,
        TyKind::Uint(UintTy::U32) => MirTypeShape::U32,
        TyKind::Int(IntTy::I64) => MirTypeShape::I64,
        TyKind::Int(IntTy::Isize) => MirTypeShape::ISize,
        TyKind::Uint(UintTy::Usize) => MirTypeShape::USize,
        TyKind::Float(FloatTy::F32) => MirTypeShape::F32,
        TyKind::Float(FloatTy::F64) => MirTypeShape::F64,
        TyKind::Ref(_, pointee, mutability) => match pointee.kind() {
            TyKind::Slice(element) => MirTypeShape::Slice {
                element: Box::new(import_type_shape(tcx, *element)),
                mutable: *mutability == Mutability::Mut,
            },
            _ => MirTypeShape::Reference {
                pointee: Box::new(import_type_shape(tcx, *pointee)),
                mutable: *mutability == Mutability::Mut,
            },
        },
        TyKind::RawPtr(pointee, mutability) => MirTypeShape::RawPointer {
            pointee: Box::new(import_type_shape(tcx, *pointee)),
            mutable: *mutability == Mutability::Mut,
        },
        TyKind::Adt(adt, args) => match semantic_features::classify(tcx, adt.did())
            .map(AuthenticatedSemanticItem::trusted_device_item)
        {
            Some(TrustedDeviceItem::DisjointSlice) => MirTypeShape::DisjointSlice {
                element: Box::new(import_type_shape(tcx, args.type_at(0))),
            },
            Some(item @ TrustedDeviceItem::ThreadIndex) => MirTypeShape::Adt {
                identity: item.canonical_path().to_owned(),
            },
            Some(TrustedDeviceItem::DeviceValue(
                dialect_amdgcn::DeviceValueDiagnosticItem::F16,
            )) => MirTypeShape::F16,
            Some(TrustedDeviceItem::DeviceValue(
                dialect_amdgcn::DeviceValueDiagnosticItem::Bf16,
            )) => MirTypeShape::Bf16,
            Some(TrustedDeviceItem::DeviceValue(
                dialect_amdgcn::DeviceValueDiagnosticItem::Bf16x2,
            )) => MirTypeShape::Bf16x2,
            Some(TrustedDeviceItem::DeviceMath(
                dialect_amdgcn::DeviceMathDiagnosticItem::Context,
            )) => MirTypeShape::DeviceMath,
            _ => MirTypeShape::Adt {
                identity: tcx.def_path_str(adt.did()),
            },
        },
        TyKind::Tuple(elements) if elements.is_empty() => MirTypeShape::Unit,
        TyKind::Tuple(elements) => MirTypeShape::Tuple(
            elements
                .iter()
                .map(|element| import_type_shape(tcx, element))
                .collect(),
        ),
        _ => MirTypeShape::Unknown,
    }
}

fn import_statement<'tcx>(
    tcx: TyCtxt<'tcx>,
    index: usize,
    kind: &StatementKind<'tcx>,
    source_info: SourceInfo,
) -> MirStatement {
    MirStatement {
        index,
        kind: statement_kind(kind),
        destination: statement_destination(kind),
        operands: statement_operands(tcx, kind),
        rvalue: statement_rvalue(kind),
        operation: statement_operation(kind),
        source: Some(import_source_location(tcx, source_info)),
    }
}

fn import_source_location(tcx: TyCtxt<'_>, source_info: SourceInfo) -> MirSourceLocation {
    let location = tcx.sess.source_map().lookup_char_pos(source_info.span.lo());
    MirSourceLocation {
        file: location
            .file
            .name
            .prefer_remapped_unconditionally()
            .to_string_lossy()
            .into_owned(),
        line: location.line,
        column: location.col.0 + 1,
    }
}

fn statement_kind(kind: &StatementKind<'_>) -> MirStatementKind {
    match kind {
        StatementKind::Assign(_) => MirStatementKind::Assign,
        StatementKind::StorageLive(_) => MirStatementKind::StorageLive,
        StatementKind::StorageDead(_) => MirStatementKind::StorageDead,
        StatementKind::SetDiscriminant { .. } => MirStatementKind::SetDiscriminant,
        StatementKind::Intrinsic(_) => MirStatementKind::Intrinsic,
        StatementKind::Retag(_, _) => MirStatementKind::Retag,
        StatementKind::Coverage(_) => MirStatementKind::Coverage,
        StatementKind::Nop => MirStatementKind::Nop,
        _ => MirStatementKind::Other,
    }
}

fn statement_destination(kind: &StatementKind<'_>) -> Option<MirPlaceRef> {
    match kind {
        StatementKind::Assign(assign) => {
            let (place, _) = &**assign;
            Some(import_place(*place))
        }
        StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
            Some(MirPlaceRef::local(*local))
        }
        StatementKind::SetDiscriminant { place, .. } => Some(import_place(**place)),
        _ => None,
    }
}

fn statement_operands<'tcx>(tcx: TyCtxt<'tcx>, kind: &StatementKind<'tcx>) -> Vec<MirOperandRef> {
    let StatementKind::Assign(assign) = kind else {
        return Vec::new();
    };
    let (_, rvalue) = &**assign;
    rvalue_operands(tcx, rvalue)
}

fn statement_operation(kind: &StatementKind<'_>) -> Option<String> {
    let StatementKind::Assign(assign) = kind else {
        return None;
    };
    let (_, rvalue) = &**assign;
    Some(rvalue_operation(rvalue).to_string())
}

fn statement_rvalue(kind: &StatementKind<'_>) -> Option<MirRvalueKind> {
    let StatementKind::Assign(assign) = kind else {
        return None;
    };
    let (_, rvalue) = &**assign;
    Some(import_rvalue_kind(rvalue))
}

fn rvalue_operands<'tcx>(tcx: TyCtxt<'tcx>, rvalue: &Rvalue<'tcx>) -> Vec<MirOperandRef> {
    match rvalue {
        Rvalue::Use(operand)
        | Rvalue::Repeat(operand, _)
        | Rvalue::Cast(_, operand, _)
        | Rvalue::UnaryOp(_, operand) => vec![import_operand(tcx, operand)],
        Rvalue::BinaryOp(_, operands) => vec![
            import_operand(tcx, &operands.0),
            import_operand(tcx, &operands.1),
        ],
        Rvalue::Ref(_, _, place) | Rvalue::RawPtr(_, place) | Rvalue::Discriminant(place) => {
            vec![MirOperandRef::Place(import_place(*place))]
        }
        Rvalue::Aggregate(_, operands) => operands
            .iter()
            .map(|operand| import_operand(tcx, operand))
            .collect(),
        _ => Vec::new(),
    }
}

fn rvalue_operation(rvalue: &Rvalue<'_>) -> &'static str {
    match rvalue {
        Rvalue::Use(_) => "use",
        Rvalue::Repeat(_, _) => "repeat",
        Rvalue::Ref(_, _, _) => "ref",
        Rvalue::RawPtr(_, _) => "raw_ptr",
        Rvalue::Cast(_, _, _) => "cast",
        Rvalue::BinaryOp(op, _) => bin_op_name(*op),
        Rvalue::UnaryOp(op, _) => unary_op_name(*op),
        Rvalue::Discriminant(_) => "discriminant",
        Rvalue::Aggregate(_, _) => "aggregate",
        _ => "other",
    }
}

fn import_rvalue_kind(rvalue: &Rvalue<'_>) -> MirRvalueKind {
    match rvalue {
        Rvalue::Use(_) => MirRvalueKind::Use,
        Rvalue::Repeat(_, _) => MirRvalueKind::Repeat,
        Rvalue::Ref(_, _, _) => MirRvalueKind::Ref,
        Rvalue::RawPtr(_, _) => MirRvalueKind::RawPointer,
        Rvalue::Cast(_, _, _) => MirRvalueKind::Cast,
        Rvalue::BinaryOp(op, _) => MirRvalueKind::Binary(import_binary_op(*op)),
        Rvalue::UnaryOp(op, _) => MirRvalueKind::Unary(import_unary_op(*op)),
        Rvalue::Discriminant(_) => MirRvalueKind::Discriminant,
        Rvalue::Aggregate(_, _) => MirRvalueKind::Aggregate,
        _ => MirRvalueKind::Other,
    }
}

fn import_operand<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> MirOperandRef {
    if let Some(place) = operand.place() {
        return MirOperandRef::Place(import_place(place));
    }

    let Operand::Constant(constant) = operand else {
        return MirOperandRef::Constant {
            ty: MirImportedType {
                kind: MirType::Unknown,
                rust: "<unknown>".to_string(),
                shape: MirTypeShape::Unknown,
            },
            literal: MirConstant::Unevaluated,
            value: "<unknown>".to_string(),
        };
    };

    MirOperandRef::Constant {
        ty: import_type(tcx, constant.const_.ty()),
        literal: import_constant(tcx, constant),
        value: constant_value_label(tcx, constant),
    }
}

fn import_constant<'tcx>(tcx: TyCtxt<'tcx>, constant: &ConstOperand<'tcx>) -> MirConstant {
    let typing_env = TypingEnv::fully_monomorphized();
    match constant.const_.ty().kind() {
        TyKind::Uint(UintTy::Usize) => constant
            .const_
            .try_eval_target_usize(tcx, typing_env)
            .map(MirConstant::USize)
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Int(IntTy::Isize) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::ISize(value.to_target_isize(tcx)))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Bool => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .and_then(|value| value.try_to_bool().ok())
            .map(MirConstant::Bool)
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Int(IntTy::I32) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::I32(value.to_i32()))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Uint(UintTy::U32) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::U32(value.to_u32()))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Int(IntTy::I64) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::I64(value.to_i64()))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Float(FloatTy::F32) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::F32Bits(value.to_u32()))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Float(FloatTy::F64) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::F64Bits(value.to_u64()))
            .unwrap_or(MirConstant::Unevaluated),
        _ => MirConstant::Unevaluated,
    }
}

fn constant_value_label<'tcx>(tcx: TyCtxt<'tcx>, constant: &ConstOperand<'tcx>) -> String {
    let debug = format!("{:?}", constant.const_);
    match constant.const_.ty().kind() {
        TyKind::Uint(UintTy::Usize) => constant
            .const_
            .try_eval_target_usize(tcx, TypingEnv::fully_monomorphized())
            .map(|value| format!("{debug};eval_u64={value}"))
            .unwrap_or(debug),
        TyKind::Int(IntTy::Isize) => constant
            .const_
            .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())
            .map(|value| format!("{debug};eval_i64={}", value.to_target_isize(tcx)))
            .unwrap_or(debug),
        _ => debug,
    }
}

fn import_place(place: Place<'_>) -> MirPlaceRef {
    MirPlaceRef {
        local: place.local.as_usize(),
        projection: place
            .projection
            .iter()
            .map(import_projection_elem)
            .collect(),
    }
}

fn import_projection_elem(element: ProjectionElem<Local, Ty<'_>>) -> MirProjectionElem {
    match element {
        ProjectionElem::Deref => MirProjectionElem::Deref,
        ProjectionElem::Field(field, _) => MirProjectionElem::Field(field.index()),
        ProjectionElem::Index(local) => MirProjectionElem::Index {
            local: local.as_usize(),
        },
        ProjectionElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        } => MirProjectionElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        },
        ProjectionElem::Subslice { from, to, from_end } => {
            MirProjectionElem::Subslice { from, to, from_end }
        }
        ProjectionElem::Downcast(_, variant) => MirProjectionElem::Downcast {
            variant: variant.index(),
        },
        ProjectionElem::OpaqueCast(_) => MirProjectionElem::OpaqueCast,
        _ => MirProjectionElem::Other,
    }
}

fn import_binary_op(op: BinOp) -> MirBinaryOp {
    match op {
        BinOp::Add => MirBinaryOp::Add,
        BinOp::Sub => MirBinaryOp::Sub,
        BinOp::Mul => MirBinaryOp::Mul,
        BinOp::Div => MirBinaryOp::Div,
        BinOp::Rem => MirBinaryOp::Rem,
        BinOp::BitXor => MirBinaryOp::BitXor,
        BinOp::BitAnd => MirBinaryOp::BitAnd,
        BinOp::BitOr => MirBinaryOp::BitOr,
        BinOp::Shl => MirBinaryOp::Shl,
        BinOp::Shr => MirBinaryOp::Shr,
        BinOp::Eq => MirBinaryOp::Eq,
        BinOp::Lt => MirBinaryOp::Lt,
        BinOp::Le => MirBinaryOp::Le,
        BinOp::Ne => MirBinaryOp::Ne,
        BinOp::Ge => MirBinaryOp::Ge,
        BinOp::Gt => MirBinaryOp::Gt,
        BinOp::Cmp => MirBinaryOp::Cmp,
        BinOp::Offset => MirBinaryOp::Offset,
        BinOp::AddUnchecked => MirBinaryOp::AddUnchecked,
        BinOp::SubUnchecked => MirBinaryOp::SubUnchecked,
        BinOp::MulUnchecked => MirBinaryOp::MulUnchecked,
        BinOp::ShlUnchecked => MirBinaryOp::ShlUnchecked,
        BinOp::ShrUnchecked => MirBinaryOp::ShrUnchecked,
        BinOp::AddWithOverflow => MirBinaryOp::AddWithOverflow,
        BinOp::SubWithOverflow => MirBinaryOp::SubWithOverflow,
        BinOp::MulWithOverflow => MirBinaryOp::MulWithOverflow,
    }
}

fn import_unary_op(op: UnOp) -> MirUnaryOp {
    match op {
        UnOp::Not => MirUnaryOp::Not,
        UnOp::Neg => MirUnaryOp::Neg,
        UnOp::PtrMetadata => MirUnaryOp::PtrMetadata,
    }
}

fn bin_op_name(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
        BinOp::Rem => "rem",
        BinOp::BitXor => "bitxor",
        BinOp::BitAnd => "bitand",
        BinOp::BitOr => "bitor",
        BinOp::Shl => "shl",
        BinOp::Shr => "shr",
        BinOp::Eq => "eq",
        BinOp::Lt => "lt",
        BinOp::Le => "le",
        BinOp::Ne => "ne",
        BinOp::Ge => "ge",
        BinOp::Gt => "gt",
        BinOp::Cmp => "cmp",
        BinOp::Offset => "offset",
        BinOp::AddUnchecked => "add_unchecked",
        BinOp::SubUnchecked => "sub_unchecked",
        BinOp::MulUnchecked => "mul_unchecked",
        BinOp::ShlUnchecked => "shl_unchecked",
        BinOp::ShrUnchecked => "shr_unchecked",
        BinOp::AddWithOverflow => "add_with_overflow",
        BinOp::SubWithOverflow => "sub_with_overflow",
        BinOp::MulWithOverflow => "mul_with_overflow",
    }
}

fn unary_op_name(op: UnOp) -> &'static str {
    match op {
        UnOp::Not => "not",
        UnOp::Neg => "neg",
        UnOp::PtrMetadata => "ptr_metadata",
    }
}

fn terminator_kind<'tcx>(
    tcx: TyCtxt<'tcx>,
    kind: &TerminatorKind<'tcx>,
    compiler_ffi_imports: &CompilerFfiImports,
) -> MirTerminatorKind {
    match kind {
        TerminatorKind::Return => MirTerminatorKind::Return,
        TerminatorKind::Unreachable => MirTerminatorKind::Unreachable,
        TerminatorKind::Goto { target } => MirTerminatorKind::Goto {
            target: target.as_usize(),
        },
        TerminatorKind::SwitchInt { discr, targets } => MirTerminatorKind::SwitchInt {
            discriminant: import_operand(tcx, discr),
            targets: targets
                .iter()
                .map(|(value, target)| MirSwitchTarget {
                    value,
                    target: target.as_usize(),
                })
                .collect(),
            otherwise: targets.otherwise().as_usize(),
        },
        TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            ..
        } => MirTerminatorKind::Call {
            callee: call_identity(tcx, func, compiler_ffi_imports),
            target: target.map(BasicBlock::as_usize),
            destination: Some(import_place(*destination)),
            operands: args
                .iter()
                .map(|arg| import_operand(tcx, &arg.node))
                .collect(),
        },
        TerminatorKind::Assert {
            cond,
            expected,
            target,
            ..
        } => MirTerminatorKind::Assert {
            condition: import_operand(tcx, cond),
            expected: *expected,
            target: target.as_usize(),
        },
        TerminatorKind::Drop { target, .. } => MirTerminatorKind::Drop {
            target: target.as_usize(),
        },
        _ => MirTerminatorKind::Other,
    }
}

fn call_identity<'tcx>(
    tcx: TyCtxt<'tcx>,
    func: &Operand<'tcx>,
    compiler_ffi_imports: &CompilerFfiImports,
) -> Option<MirCallee> {
    let Operand::Constant(constant) = func else {
        return None;
    };
    let TyKind::FnDef(def_id, args) = constant.const_.ty().kind() else {
        return None;
    };
    let resolved_def_id =
        Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, args)
            .ok()
            .flatten()
            .map(|instance| instance.def_id())
            .unwrap_or(*def_id);
    Some(
        if let Some(item) = semantic_features::classify(tcx, resolved_def_id) {
            MirCallee::authenticated(item)
        } else if let Some(import) = compiler_ffi_imports
            .classify(tcx, resolved_def_id)
            .or_else(|| compiler_ffi_imports.classify(tcx, *def_id))
        {
            MirCallee::external_import(import)
        } else {
            MirCallee::untrusted(imported_rust_path(tcx, resolved_def_id))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::CollectedFunctionRole;

    #[test]
    fn collection_roles_map_to_mir_roles_without_name_inference() {
        assert_eq!(
            import_function_kind(CollectedFunctionRole::KernelEntry),
            MirFunctionKind::KernelEntry
        );
        assert_eq!(
            import_function_kind(CollectedFunctionRole::InternalHelper),
            MirFunctionKind::InternalHelper
        );
        assert_eq!(
            import_function_kind(CollectedFunctionRole::DeviceFfiExport),
            MirFunctionKind::DeviceFfiExport
        );
    }

    #[test]
    fn summary_includes_function_and_block_shape() {
        let module = MirModule {
            functions: vec![MirFunction {
                export_name: "vecadd".to_string(),
                rust_path: "fe2o3_vecadd::fe2o3_kernel_vecadd".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                arg_count: 3,
                local_count: 17,
                locals: vec![
                    MirLocal {
                        index: 0,
                        role: MirLocalRole::Return,
                        ty: MirImportedType {
                            kind: MirType::Unit,
                            rust: "()".to_string(),
                            shape: MirTypeShape::Unit,
                        },
                    },
                    MirLocal {
                        index: 1,
                        role: MirLocalRole::Arg,
                        ty: MirImportedType {
                            kind: MirType::Slice,
                            rust: "&[f32]".to_string(),
                            shape: MirTypeShape::Slice {
                                element: Box::new(MirTypeShape::F32),
                                mutable: false,
                            },
                        },
                    },
                ],
                blocks: vec![MirBlock {
                    index: 0,
                    statements: vec![
                        simple_statement(0, MirStatementKind::StorageLive),
                        simple_statement(1, MirStatementKind::Assign),
                    ],
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Goto { target: 1 },
                        source: None,
                    }),
                }],
            }],
        };

        let summary = module.summary();

        assert!(summary.contains("[kernel-entry] vecadd (mir.func)"));
        assert!(summary.contains("fe2o3_vecadd::fe2o3_kernel_vecadd"));
        assert!(summary.contains("1 bb, 17 locals, 3 args"));
        assert!(summary.contains("local1: arg mir.slice (&[f32])"));
        assert!(summary.contains("bb0 (mir.block): 2 stmt(s), mir.br -> bb1"));
    }

    #[test]
    fn dialect_records_include_function_blocks_and_terminators() {
        let module = MirModule {
            functions: vec![MirFunction {
                export_name: "vecadd".to_string(),
                rust_path: "fe2o3_vecadd::fe2o3_kernel_vecadd".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                arg_count: 3,
                local_count: 17,
                locals: vec![MirLocal {
                    index: 1,
                    role: MirLocalRole::Arg,
                    ty: MirImportedType {
                        kind: MirType::Slice,
                        rust: "&[f32]".to_string(),
                        shape: MirTypeShape::Slice {
                            element: Box::new(MirTypeShape::F32),
                            mutable: false,
                        },
                    },
                }],
                blocks: vec![MirBlock {
                    index: 0,
                    statements: vec![MirStatement {
                        index: 0,
                        kind: MirStatementKind::Assign,
                        destination: Some(MirPlaceRef {
                            local: 3,
                            projection: Vec::new(),
                        }),
                        operands: vec![MirOperandRef::Place(MirPlaceRef {
                            local: 1,
                            projection: vec![
                                MirProjectionElem::Deref,
                                MirProjectionElem::Index { local: 2 },
                            ],
                        })],
                        rvalue: Some(MirRvalueKind::Use),
                        operation: Some("use".to_string()),
                        source: None,
                    }],
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Return,
                        source: None,
                    }),
                }],
            }],
        };

        let records = module.dialect_records();

        assert_eq!(records[0].op, MirOp::Module);
        assert_eq!(records[1].op, MirOp::Func);
        assert_eq!(record_string(&records[1], "kind"), Some("kernel"));
        assert_eq!(records[2].op, MirOp::Arg);
        assert_eq!(records[3].op, MirOp::Block);
        assert_eq!(records[4].op, MirOp::Assign);
        assert_eq!(records[5].op, MirOp::Load);
        assert_eq!(records[6].op, MirOp::Return);
        assert_eq!(record_usize(&records[4], "destination_local"), Some(3));
        assert_eq!(record_string(&records[4], "operation"), Some("use"));
        assert_eq!(
            record_string(&records[4], "operands"),
            Some("local1.deref.index_local2")
        );
        assert_eq!(record_usize(&records[5], "statement"), Some(0));
        assert_eq!(record_string(&records[5], "source"), Some("mir.assign"));
    }

    #[test]
    fn call_records_include_destination_and_operands() {
        let module = MirModule {
            functions: vec![MirFunction {
                export_name: "copy".to_string(),
                rust_path: "fe2o3_copy::fe2o3_kernel_copy".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                arg_count: 1,
                local_count: 3,
                locals: Vec::new(),
                blocks: vec![MirBlock {
                    index: 0,
                    statements: Vec::new(),
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Call {
                            callee: Some(MirCallee::trusted_for_test(
                                TrustedDeviceItem::ThreadIndex1d,
                            )),
                            target: Some(1),
                            destination: Some(local_place(2)),
                            operands: vec![MirOperandRef::Place(local_place(1))],
                        },
                        source: None,
                    }),
                }],
            }],
        };

        let records = module.dialect_records();

        assert_eq!(records[3].op, MirOp::Call);
        assert_eq!(
            record_string(&records[3], "callee"),
            Some("fe2o3_device::thread::index_1d")
        );
        assert_eq!(record_usize(&records[3], "target"), Some(1));
        assert_eq!(record_usize(&records[3], "destination_local"), Some(2));
        assert_eq!(record_string(&records[3], "destination"), Some("local2"));
        assert_eq!(record_usize(&records[3], "operand_count"), Some(1));
        assert_eq!(record_string(&records[3], "operands"), Some("local1"));
        assert_eq!(record_string(&records[3], "trusted_callee"), None);
    }

    #[test]
    fn callee_identity_cannot_mismatch_trusted_authority() {
        let items = [
            TrustedDeviceItem::DisjointSlice,
            TrustedDeviceItem::ThreadIndex,
            TrustedDeviceItem::ThreadIndex1d,
            TrustedDeviceItem::ThreadIndexGet,
            TrustedDeviceItem::ThreadIndexOffset,
            TrustedDeviceItem::ThreadIndexOffsetSigned,
            TrustedDeviceItem::ThreadIndexStride,
            TrustedDeviceItem::ThreadIndexStrideOffset,
            TrustedDeviceItem::DisjointSliceGetMut,
            TrustedDeviceItem::DisjointSliceGetMutAt,
        ];

        for item in items {
            let trusted = MirCallee::trusted_for_test(item);
            assert_eq!(trusted.identity(), item.canonical_path());
            assert_eq!(trusted.trusted_item(), Some(item));

            let same_spelling = MirCallee::untrusted_for_test(item.canonical_path());
            assert_eq!(same_spelling.identity(), item.canonical_path());
            assert_eq!(same_spelling.trusted_item(), None);
        }
    }

    #[test]
    fn external_import_evidence_uses_the_contract_symbol_and_retains_provenance() {
        let callee = MirCallee::external_import_for_test(
            "external_device_add_v1",
            "C(u32[size=4,align=4])->u32[size=4,align=4]",
            "none",
        );
        let module = MirModule {
            functions: vec![MirFunction {
                export_name: "consumer".to_string(),
                rust_path: "tests::consumer".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                arg_count: 0,
                local_count: 1,
                locals: Vec::new(),
                blocks: vec![MirBlock {
                    index: 0,
                    statements: Vec::new(),
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Call {
                            callee: Some(callee),
                            target: Some(1),
                            destination: Some(local_place(0)),
                            operands: Vec::new(),
                        },
                        source: None,
                    }),
                }],
            }],
        };

        let records = module.dialect_records();
        let call = records
            .iter()
            .find(|record| record.op == MirOp::Call)
            .expect("call record");
        assert_eq!(
            record_string(call, "callee"),
            Some("external_device_add_v1")
        );
        assert_eq!(
            record_string(call, "device_ffi_target"),
            Some("gfx942:xnack-")
        );
        assert_eq!(
            record_usize(call, "device_ffi_code_object_version"),
            Some(5)
        );
        assert!(record_string(call, "device_ffi_contract").is_some());
        assert!(record_string(call, "device_ffi_semantic_identity").is_some());
    }

    #[test]
    fn ordinary_extern_spelling_has_no_external_import_evidence() {
        let callee = MirCallee::untrusted_for_test("external_device_add_v1");

        assert_eq!(callee.identity(), "external_device_add_v1");
        assert_eq!(callee.trusted_item(), None);
        assert_eq!(callee.external_import_evidence(), None);
    }

    #[test]
    fn external_import_fields_fail_closed_independently() {
        const SYMBOL: &str = "external_device_add_v1";
        const TARGET: &str = "gfx942:xnack-";
        const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
        const SEMANTIC: &str = "6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b";
        let identity = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
            symbol: SYMBOL,
            calling_convention: "C",
            code_object_version: 5,
            target: TARGET,
            physical_abi: ABI,
            effects: "none",
            semantic_identity: SEMANTIC,
        });
        assert!(
            validate_external_import_fields(
                identity, SYMBOL, TARGET, 5, ABI, "none", SEMANTIC, TARGET, 5,
            )
            .is_ok()
        );

        let wrong_target = validate_external_import_fields(
            identity, SYMBOL, "gfx1100", 5, ABI, "none", SEMANTIC, TARGET, 5,
        )
        .expect_err("target mismatch");
        assert!(wrong_target.to_string().contains("target"));
        let wrong_cov = validate_external_import_fields(
            identity, SYMBOL, TARGET, 6, ABI, "none", SEMANTIC, TARGET, 5,
        )
        .expect_err("code-object version mismatch");
        assert!(wrong_cov.to_string().contains("code-object version"));
        let wrong_identity = validate_external_import_fields(
            DeviceFfiContractIdV1::from_bytes([0x11; 32]),
            SYMBOL,
            TARGET,
            5,
            ABI,
            "none",
            SEMANTIC,
            TARGET,
            5,
        )
        .expect_err("identity mismatch");
        assert!(wrong_identity.to_string().contains("contract identity"));

        let malformed_abi = "C(u32)->u32";
        let malformed_abi_identity = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
            symbol: SYMBOL,
            calling_convention: "C",
            code_object_version: 5,
            target: TARGET,
            physical_abi: malformed_abi,
            effects: "none",
            semantic_identity: SEMANTIC,
        });
        let malformed_abi_error = validate_external_import_fields(
            malformed_abi_identity,
            SYMBOL,
            TARGET,
            5,
            malformed_abi,
            "none",
            SEMANTIC,
            TARGET,
            5,
        )
        .expect_err("malformed ABI");
        assert!(malformed_abi_error.to_string().contains("physical ABI"));

        let incompatible_effects = "read_global";
        let incompatible_effects_identity =
            derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
                direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
                symbol: SYMBOL,
                calling_convention: "C",
                code_object_version: 5,
                target: TARGET,
                physical_abi: ABI,
                effects: incompatible_effects,
                semantic_identity: SEMANTIC,
            });
        let effects_error = validate_external_import_fields(
            incompatible_effects_identity,
            SYMBOL,
            TARGET,
            5,
            ABI,
            incompatible_effects,
            SEMANTIC,
            TARGET,
            5,
        )
        .expect_err("effects/ABI mismatch");
        assert!(effects_error.to_string().contains("effects disagree"));

        let semantic_error = validate_external_import_fields(
            identity,
            SYMBOL,
            TARGET,
            5,
            ABI,
            "none",
            &"7c".repeat(32),
            TARGET,
            5,
        )
        .expect_err("semantic identity mismatch");
        assert!(semantic_error.to_string().contains("contract identity"));
    }

    #[test]
    fn assignments_classify_lowering_ops() {
        let arithmetic = MirStatement {
            index: 0,
            kind: MirStatementKind::Assign,
            destination: Some(local_place(3)),
            operands: vec![
                MirOperandRef::Place(local_place(1)),
                MirOperandRef::Place(local_place(2)),
            ],
            rvalue: Some(MirRvalueKind::Binary(MirBinaryOp::MulWithOverflow)),
            operation: Some("mul_with_overflow".to_string()),
            source: None,
        };
        let load = MirStatement {
            index: 1,
            kind: MirStatementKind::Assign,
            destination: Some(local_place(4)),
            operands: vec![MirOperandRef::Place(MirPlaceRef {
                local: 1,
                projection: vec![
                    MirProjectionElem::Deref,
                    MirProjectionElem::Index { local: 2 },
                ],
            })],
            rvalue: Some(MirRvalueKind::Use),
            operation: Some("use".to_string()),
            source: None,
        };
        let store = MirStatement {
            index: 2,
            kind: MirStatementKind::Assign,
            destination: Some(MirPlaceRef {
                local: 5,
                projection: vec![MirProjectionElem::Deref],
            }),
            operands: vec![MirOperandRef::Place(local_place(4))],
            rvalue: Some(MirRvalueKind::Use),
            operation: Some("use".to_string()),
            source: None,
        };
        let compare = MirStatement {
            index: 3,
            kind: MirStatementKind::Assign,
            destination: Some(local_place(6)),
            operands: vec![
                MirOperandRef::Place(local_place(1)),
                MirOperandRef::Place(local_place(2)),
            ],
            rvalue: Some(MirRvalueKind::Binary(MirBinaryOp::Lt)),
            operation: Some("lt".to_string()),
            source: None,
        };

        assert_eq!(arithmetic.lowering_op(), Some(MirOp::Mul));
        assert_eq!(load.lowering_op(), Some(MirOp::Load));
        assert_eq!(store.lowering_op(), Some(MirOp::Store));
        assert_eq!(compare.lowering_op(), Some(MirOp::Lt));
    }

    #[test]
    fn general_two_kernel_shared_helper_is_represented_once_by_source_identity() {
        let helper = general_two_kernel_function(
            "shared_helper",
            "tests::shared_helper",
            MirFunctionKind::InternalHelper,
            None,
        );
        let helper_identity = function_identity_hex_v1(helper.source_identity_v1().unwrap());
        let module = MirModule::from_functions_v1(vec![
            general_two_kernel_function(
                "zeta",
                "tests::kernel_zeta",
                MirFunctionKind::KernelEntry,
                Some("tests::shared_helper"),
            ),
            helper,
            general_two_kernel_function(
                "alpha",
                "tests::kernel_alpha",
                MirFunctionKind::KernelEntry,
                Some("tests::shared_helper"),
            ),
        ])
        .unwrap();

        assert_eq!(
            module
                .functions
                .iter()
                .filter(|function| function.kind == MirFunctionKind::KernelEntry)
                .count(),
            2
        );
        assert_eq!(
            module
                .functions
                .iter()
                .filter(|function| function.kind == MirFunctionKind::InternalHelper)
                .count(),
            1
        );

        let records = module.dialect_records();
        assert_eq!(record_usize(&records[0], "kernel_roots"), Some(2));
        assert_eq!(record_usize(&records[0], "internal_helpers"), Some(1));
        let helper_record = records
            .iter()
            .find(|record| {
                record.op == MirOp::Func && record_string(record, "symbol") == Some("shared_helper")
            })
            .unwrap();
        assert_eq!(
            record_string(helper_record, "source_identity_v1"),
            Some(helper_identity.as_str())
        );
        let calls = records
            .iter()
            .filter(|record| record.op == MirOp::Call)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().all(|call| {
            record_string(call, "callee") == Some("tests::shared_helper")
                && record_string(call, "callee_source_identity_v1")
                    == Some(helper_identity.as_str())
        }));
    }

    #[test]
    fn general_two_kernel_serialization_is_deterministic_for_all_input_orders() {
        let functions = [
            general_two_kernel_function(
                "kernel_a",
                "tests::kernel_a",
                MirFunctionKind::KernelEntry,
                Some("tests::shared"),
            ),
            general_two_kernel_function(
                "kernel_b",
                "tests::kernel_b",
                MirFunctionKind::KernelEntry,
                Some("tests::shared"),
            ),
            general_two_kernel_function(
                "shared",
                "tests::shared",
                MirFunctionKind::InternalHelper,
                None,
            ),
        ];
        let permutations = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let expected = MirModule::from_functions_v1(
            permutations[0]
                .iter()
                .map(|index| functions[*index].clone())
                .collect(),
        )
        .unwrap()
        .dialect_records();

        for permutation in permutations.into_iter().skip(1) {
            let records = MirModule::from_functions_v1(
                permutation
                    .iter()
                    .map(|index| functions[*index].clone())
                    .collect(),
            )
            .unwrap()
            .dialect_records();
            assert_eq!(records, expected);
        }
    }

    #[test]
    fn general_two_kernel_malformed_and_duplicate_roots_fail_closed() {
        let malformed =
            general_two_kernel_function("kernel", "", MirFunctionKind::KernelEntry, None);
        assert!(
            MirModule::from_functions_v1(vec![malformed])
                .unwrap_err()
                .to_string()
                .contains("source function path must not be empty")
        );

        let duplicate_export = MirModule::from_functions_v1(vec![
            general_two_kernel_function("same", "tests::first", MirFunctionKind::KernelEntry, None),
            general_two_kernel_function(
                "same",
                "tests::second",
                MirFunctionKind::KernelEntry,
                None,
            ),
        ])
        .unwrap_err();
        assert!(
            duplicate_export
                .to_string()
                .contains("duplicate function export `same`")
        );

        let ambiguous_source = MirModule::from_functions_v1(vec![
            general_two_kernel_function(
                "first",
                "tests::same_root",
                MirFunctionKind::KernelEntry,
                None,
            ),
            general_two_kernel_function(
                "second",
                "tests::same_root",
                MirFunctionKind::KernelEntry,
                None,
            ),
        ])
        .unwrap_err();
        assert!(
            ambiguous_source
                .to_string()
                .contains("select the same source function `tests::same_root`")
        );

        let helper_only = general_two_kernel_function(
            "helper",
            "tests::helper",
            MirFunctionKind::InternalHelper,
            None,
        );
        assert_eq!(
            MirModule::from_functions_v1(vec![helper_only])
                .unwrap_err()
                .to_string(),
            "MIR module contains no kernel root"
        );
    }

    #[test]
    fn general_two_kernel_order_places_canonical_roots_before_shared_helpers() {
        let module = MirModule::from_functions_v1(vec![
            general_two_kernel_function(
                "helper",
                "tests::helper",
                MirFunctionKind::InternalHelper,
                None,
            ),
            general_two_kernel_function(
                "second",
                "tests::second",
                MirFunctionKind::KernelEntry,
                Some("tests::helper"),
            ),
            general_two_kernel_function(
                "first",
                "tests::first",
                MirFunctionKind::KernelEntry,
                Some("tests::helper"),
            ),
        ])
        .unwrap();

        assert_eq!(module.functions[2].kind, MirFunctionKind::InternalHelper);
        let root_identities = module.functions[..2]
            .iter()
            .map(|function| function.source_identity_v1().unwrap())
            .collect::<Vec<_>>();
        assert!(root_identities.windows(2).all(|pair| pair[0] < pair[1]));
    }

    fn general_two_kernel_function(
        export_name: &str,
        rust_path: &str,
        kind: MirFunctionKind,
        callee: Option<&str>,
    ) -> MirFunction {
        let blocks = if let Some(callee) = callee {
            vec![
                MirBlock {
                    index: 0,
                    statements: Vec::new(),
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Call {
                            callee: Some(MirCallee::untrusted_for_test(callee)),
                            target: Some(1),
                            destination: Some(local_place(0)),
                            operands: vec![MirOperandRef::Place(local_place(1))],
                        },
                        source: None,
                    }),
                },
                MirBlock {
                    index: 1,
                    statements: Vec::new(),
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Return,
                        source: None,
                    }),
                },
            ]
        } else {
            vec![MirBlock {
                index: 0,
                statements: Vec::new(),
                terminator: Some(MirTerminator {
                    kind: MirTerminatorKind::Return,
                    source: None,
                }),
            }]
        };
        MirFunction {
            export_name: export_name.to_owned(),
            rust_path: rust_path.to_owned(),
            kind,
            typed_profile: None,
            arg_count: 1,
            local_count: 2,
            locals: Vec::new(),
            blocks,
            frontend_contract: None,
        }
    }

    fn simple_statement(index: usize, kind: MirStatementKind) -> MirStatement {
        MirStatement {
            index,
            kind,
            destination: None,
            operands: Vec::new(),
            rvalue: None,
            operation: None,
            source: None,
        }
    }

    fn local_place(local: usize) -> MirPlaceRef {
        MirPlaceRef {
            local,
            projection: Vec::new(),
        }
    }

    fn record_usize(record: &MirOpRecord, name: &'static str) -> Option<usize> {
        record.attrs.iter().find_map(|attr| {
            if attr.name == name
                && let dialect_mir::MirAttrValue::Usize(value) = &attr.value
            {
                return Some(*value);
            }
            None
        })
    }

    fn record_string<'a>(record: &'a MirOpRecord, name: &'static str) -> Option<&'a str> {
        record.attrs.iter().find_map(|attr| {
            if attr.name == name
                && let dialect_mir::MirAttrValue::String(value) = &attr.value
            {
                return Some(value.as_str());
            }
            None
        })
    }
}
