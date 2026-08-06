//! Compiler-neutral closure of inert device FFI facts over an exact link plan.

use std::{collections::BTreeMap, fmt};

use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, LinkInputKindClosureV1, LinkPlanIdentityV1, LinkSymbolClosureV1,
    MultiInputLinkPlanV1, WorkerInputKindV1, WorkerProtocolError,
    worker_protocol::validate_symbols,
};

/// Maximum device FFI symbols accepted from one compiler closure.
pub const MAX_COMPILER_FFI_SYMBOLS_V1: usize = 128;
/// Maximum bytes in one compiler-derived physical ABI spelling.
pub const MAX_COMPILER_FFI_PHYSICAL_ABI_BYTES_V1: usize = 2_048;
/// Maximum bytes in one declared effect-set spelling.
pub const MAX_COMPILER_FFI_EFFECT_BYTES_V1: usize = 256;
/// Maximum bytes in a source crate name.
pub const MAX_COMPILER_FFI_CRATE_NAME_BYTES_V1: usize = 128;
/// Maximum bytes in a compiler source item path.
pub const MAX_COMPILER_FFI_ITEM_PATH_BYTES_V1: usize = 1_024;
/// Maximum bytes in a concrete compiler instance symbol.
pub const MAX_COMPILER_FFI_INSTANCE_SYMBOL_BYTES_V1: usize = 512;
/// Upper bound for one canonical compiler FFI closure.
pub const MAX_COMPILER_FFI_CLOSURE_BYTES_V1: usize = 512 * 1024;

const SOURCE_OWNER_DOMAIN_V1: &[u8] = b"FE2O3/COMPILER-FFI-SOURCE-OWNER/V1\0";
const COMPILER_CLOSURE_DOMAIN_V1: &[u8] = b"FE2O3/COMPILER-FFI-CLOSURE/V1\0";
const PLAN_BOUND_CLOSURE_DOMAIN_V1: &[u8] = b"FE2O3/PLAN-BOUND-COMPILER-FFI-CLOSURE/V1\0";
const DEVICE_FFI_CONTRACT_DOMAIN_V1: &[u8] = b"fe2o3.device-ffi-contract.v1\0";

/// Provenance class encoded beside every compiler FFI bridge field.
///
/// `CompilerDerived` records a fact that a compiler adapter must derive from
/// compiler state. `DeclaredClaim` remains an unverified source declaration.
/// `CallerBindingClaim` is supplied by the build/package layer. None of these
/// classes is an attestation or an authority grant.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompilerFfiFieldOriginV1 {
    CompilerDerived = 1,
    DeclaredClaim = 2,
    CallerBindingClaim = 3,
}

/// Named fields whose provenance can be inspected without decoding bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CompilerFfiSymbolFieldV1 {
    ContractIdentity,
    Direction,
    Symbol,
    PhysicalAbi,
    SourceOwner,
    Definition,
    Target,
    CodeObjectVersion,
    Effects,
    SemanticClaim,
}

impl CompilerFfiSymbolFieldV1 {
    pub const fn origin(self) -> CompilerFfiFieldOriginV1 {
        match self {
            Self::ContractIdentity
            | Self::Direction
            | Self::Symbol
            | Self::PhysicalAbi
            | Self::SourceOwner
            | Self::Definition => CompilerFfiFieldOriginV1::CompilerDerived,
            Self::Target | Self::CodeObjectVersion | Self::Effects | Self::SemanticClaim => {
                CompilerFfiFieldOriginV1::DeclaredClaim
            }
        }
    }
}

/// Stable identity of one exact compiler FFI contract record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerFfiContractIdentityV1([u8; 32]);

impl CompilerFfiContractIdentityV1 {
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, CompilerFfiBridgeError> {
        if bytes == [0; 32] {
            return Err(CompilerFfiBridgeError::ReservedIdentity(
                "compiler FFI contract",
            ));
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Stable identity of compiler-derived source ownership.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerFfiSourceOwnerIdentityV1([u8; 32]);

impl CompilerFfiSourceOwnerIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Compiler-derived owner of one concrete FFI declaration or definition.
///
/// The stable identity uses only `def_path_hash` and
/// `concrete_instance_symbol`. The crate name and item path are retained in
/// canonical records as diagnostic labels, but relabeling does not change
/// source-owner identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerFfiSourceOwnerV1 {
    crate_name: String,
    item_path: String,
    def_path_hash: [u8; 16],
    concrete_instance_symbol: String,
    identity: CompilerFfiSourceOwnerIdentityV1,
}

impl CompilerFfiSourceOwnerV1 {
    pub fn new(
        crate_name: impl Into<String>,
        item_path: impl Into<String>,
        def_path_hash: [u8; 16],
        concrete_instance_symbol: impl Into<String>,
    ) -> Result<Self, CompilerFfiBridgeError> {
        let crate_name = crate_name.into();
        let item_path = item_path.into();
        let concrete_instance_symbol = concrete_instance_symbol.into();
        validate_text(
            "source crate name",
            &crate_name,
            MAX_COMPILER_FFI_CRATE_NAME_BYTES_V1,
            false,
        )?;
        validate_text(
            "source item path",
            &item_path,
            MAX_COMPILER_FFI_ITEM_PATH_BYTES_V1,
            false,
        )?;
        validate_ascii_token(
            "concrete instance symbol",
            &concrete_instance_symbol,
            MAX_COMPILER_FFI_INSTANCE_SYMBOL_BYTES_V1,
        )?;

        let mut identity_preimage = Vec::new();
        identity_preimage.extend_from_slice(SOURCE_OWNER_DOMAIN_V1);
        identity_preimage.extend_from_slice(&def_path_hash);
        push_text(&mut identity_preimage, &concrete_instance_symbol);
        let identity = CompilerFfiSourceOwnerIdentityV1(Sha256::digest(identity_preimage).into());
        Ok(Self {
            crate_name,
            item_path,
            def_path_hash,
            concrete_instance_symbol,
            identity,
        })
    }

    pub fn crate_name(&self) -> &str {
        &self.crate_name
    }

    pub fn item_path(&self) -> &str {
        &self.item_path
    }

    pub const fn def_path_hash(&self) -> &[u8; 16] {
        &self.def_path_hash
    }

    pub fn concrete_instance_symbol(&self) -> &str {
        &self.concrete_instance_symbol
    }

    pub const fn identity(&self) -> CompilerFfiSourceOwnerIdentityV1 {
        self.identity
    }
}

/// Direction derived by the compiler from the validated declaration shape.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompilerFfiDirectionV1 {
    Import = 1,
    Export = 2,
}

/// Location in which a compiler says the symbol definition must reside.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompilerFfiDefinitionV1 {
    ExternalPlanInput = 1,
    RustCompilerBitcode = 2,
}

/// Declaration-origin fields that remain claims after compiler collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerFfiDeclaredClaimsV1 {
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    effects: String,
    semantic_claim: [u8; 32],
}

impl CompilerFfiDeclaredClaimsV1 {
    pub fn new(
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        effects: impl Into<String>,
        semantic_claim: [u8; 32],
    ) -> Result<Self, CompilerFfiBridgeError> {
        let effects = effects.into();
        validate_effects(&effects)?;
        if semantic_claim == [0; 32] {
            return Err(CompilerFfiBridgeError::ReservedIdentity("semantic claim"));
        }
        Ok(Self {
            target,
            code_object_version,
            effects,
            semantic_claim,
        })
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub fn effects(&self) -> &str {
        &self.effects
    }

    pub const fn semantic_claim(&self) -> &[u8; 32] {
        &self.semantic_claim
    }

    pub const fn origin(&self) -> CompilerFfiFieldOriginV1 {
        CompilerFfiFieldOriginV1::DeclaredClaim
    }

    pub const fn effects_are_derived(&self) -> bool {
        false
    }

    pub const fn semantics_are_verified(&self) -> bool {
        false
    }
}

/// One compiler-neutral, inert FFI symbol record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerFfiSymbolV1 {
    contract_identity: CompilerFfiContractIdentityV1,
    direction: CompilerFfiDirectionV1,
    symbol: String,
    physical_abi: String,
    source_owner: CompilerFfiSourceOwnerV1,
    definition: CompilerFfiDefinitionV1,
    declared: CompilerFfiDeclaredClaimsV1,
}

impl CompilerFfiSymbolV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contract_identity: CompilerFfiContractIdentityV1,
        direction: CompilerFfiDirectionV1,
        symbol: impl Into<String>,
        physical_abi: impl Into<String>,
        source_owner: CompilerFfiSourceOwnerV1,
        definition: CompilerFfiDefinitionV1,
        declared: CompilerFfiDeclaredClaimsV1,
    ) -> Result<Self, CompilerFfiBridgeError> {
        let symbol = symbol.into();
        validate_symbols(std::slice::from_ref(&symbol))
            .map_err(CompilerFfiBridgeError::InvalidSymbolSet)?;
        let physical_abi = physical_abi.into();
        validate_ascii_token(
            "physical ABI",
            &physical_abi,
            MAX_COMPILER_FFI_PHYSICAL_ABI_BYTES_V1,
        )?;
        let expected = match direction {
            CompilerFfiDirectionV1::Import => CompilerFfiDefinitionV1::ExternalPlanInput,
            CompilerFfiDirectionV1::Export => CompilerFfiDefinitionV1::RustCompilerBitcode,
        };
        if definition != expected {
            return Err(CompilerFfiBridgeError::DirectionDefinitionMismatch {
                symbol,
                direction,
                definition,
            });
        }
        let derived_contract_identity =
            derive_contract_identity(direction, &symbol, &physical_abi, &declared);
        if derived_contract_identity != contract_identity {
            return Err(CompilerFfiBridgeError::ContractIdentityMismatch {
                claimed: contract_identity,
                derived: derived_contract_identity,
            });
        }
        Ok(Self {
            contract_identity,
            direction,
            symbol,
            physical_abi,
            source_owner,
            definition,
            declared,
        })
    }

    pub const fn contract_identity(&self) -> CompilerFfiContractIdentityV1 {
        self.contract_identity
    }

    pub const fn direction(&self) -> CompilerFfiDirectionV1 {
        self.direction
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn physical_abi(&self) -> &str {
        &self.physical_abi
    }

    pub const fn source_owner(&self) -> &CompilerFfiSourceOwnerV1 {
        &self.source_owner
    }

    pub const fn definition(&self) -> CompilerFfiDefinitionV1 {
        self.definition
    }

    pub const fn declared(&self) -> &CompilerFfiDeclaredClaimsV1 {
        &self.declared
    }

    pub const fn field_origin(field: CompilerFfiSymbolFieldV1) -> CompilerFfiFieldOriginV1 {
        field.origin()
    }
}

/// Stable identity of a complete canonical compiler FFI closure.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerFfiClosureIdentityV1([u8; 32]);

impl CompilerFfiClosureIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Inert compiler facts and declaration claims before provider binding.
///
/// `required_symbols` is a compiler-derived claim for the complete final
/// defined-symbol set, including non-FFI entry points. A later rustc adapter
/// must supply it independently of `symbols`; this type never invents kernel
/// entries from FFI names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerFfiClosureV1 {
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    required_symbols: Vec<String>,
    symbols: Vec<CompilerFfiSymbolV1>,
    canonical_bytes: Vec<u8>,
    identity: CompilerFfiClosureIdentityV1,
}

impl CompilerFfiClosureV1 {
    pub fn new(
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        required_symbols: Vec<String>,
        symbols: Vec<CompilerFfiSymbolV1>,
    ) -> Result<Self, CompilerFfiBridgeError> {
        validate_symbols(&required_symbols).map_err(CompilerFfiBridgeError::InvalidSymbolSet)?;
        if required_symbols.is_empty() {
            return Err(CompilerFfiBridgeError::EmptyRequiredSymbolSet);
        }
        if symbols.len() > MAX_COMPILER_FFI_SYMBOLS_V1 {
            return Err(CompilerFfiBridgeError::TooManyCompilerFfiSymbols);
        }
        validate_compiler_symbols(target, code_object_version, &required_symbols, &symbols)?;

        let canonical_bytes =
            encode_compiler_closure(target, code_object_version, &required_symbols, &symbols);
        if canonical_bytes.len() > MAX_COMPILER_FFI_CLOSURE_BYTES_V1 {
            return Err(CompilerFfiBridgeError::CompilerClosureTooLarge);
        }
        let identity = CompilerFfiClosureIdentityV1(Sha256::digest(&canonical_bytes).into());
        Ok(Self {
            target,
            code_object_version,
            required_symbols,
            symbols,
            canonical_bytes,
            identity,
        })
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub fn required_symbols(&self) -> &[String] {
        &self.required_symbols
    }

    pub fn symbols(&self) -> &[CompilerFfiSymbolV1] {
        &self.symbols
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> CompilerFfiClosureIdentityV1 {
        self.identity
    }

    pub const fn required_symbols_origin(&self) -> CompilerFfiFieldOriginV1 {
        CompilerFfiFieldOriginV1::CompilerDerived
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Caller-declared role of one exact input in the canonical plan sequence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompilerFfiPlanInputRoleV1 {
    RustCompilerBitcode = 1,
    ExternalDefinitionProvider = 2,
    LinkSupport = 3,
}

/// Exact kind and role claim for one plan input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerFfiPlanInputBindingV1 {
    identity: ContentIdentityV1,
    kind: WorkerInputKindV1,
    role: CompilerFfiPlanInputRoleV1,
}

impl CompilerFfiPlanInputBindingV1 {
    pub fn new(
        identity: ContentIdentityV1,
        kind: WorkerInputKindV1,
        role: CompilerFfiPlanInputRoleV1,
    ) -> Result<Self, CompilerFfiBridgeError> {
        if role == CompilerFfiPlanInputRoleV1::RustCompilerBitcode
            && kind != WorkerInputKindV1::LlvmBitcode
        {
            return Err(CompilerFfiBridgeError::CompilerInputIsNotLlvmBitcode);
        }
        Ok(Self {
            identity,
            kind,
            role,
        })
    }

    pub const fn identity(self) -> ContentIdentityV1 {
        self.identity
    }

    pub const fn kind(self) -> WorkerInputKindV1 {
        self.kind
    }

    pub const fn role(self) -> CompilerFfiPlanInputRoleV1 {
        self.role
    }

    pub const fn origin(self) -> CompilerFfiFieldOriginV1 {
        CompilerFfiFieldOriginV1::CallerBindingClaim
    }
}

/// Caller binding from one exact compiler contract and owner to one plan input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerFfiProviderBindingV1 {
    contract_identity: CompilerFfiContractIdentityV1,
    source_owner_identity: CompilerFfiSourceOwnerIdentityV1,
    provider_input_identity: ContentIdentityV1,
    provider_input_kind: WorkerInputKindV1,
}

impl CompilerFfiProviderBindingV1 {
    pub const fn new(
        contract_identity: CompilerFfiContractIdentityV1,
        source_owner_identity: CompilerFfiSourceOwnerIdentityV1,
        provider_input_identity: ContentIdentityV1,
        provider_input_kind: WorkerInputKindV1,
    ) -> Self {
        Self {
            contract_identity,
            source_owner_identity,
            provider_input_identity,
            provider_input_kind,
        }
    }

    pub const fn contract_identity(self) -> CompilerFfiContractIdentityV1 {
        self.contract_identity
    }

    pub const fn source_owner_identity(self) -> CompilerFfiSourceOwnerIdentityV1 {
        self.source_owner_identity
    }

    pub const fn provider_input_identity(self) -> ContentIdentityV1 {
        self.provider_input_identity
    }

    pub const fn provider_input_kind(self) -> WorkerInputKindV1 {
        self.provider_input_kind
    }

    pub const fn origin(self) -> CompilerFfiFieldOriginV1 {
        CompilerFfiFieldOriginV1::CallerBindingClaim
    }
}

/// Stable identity of the exact plan, compiler closure, and caller bindings.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlanBoundCompilerFfiClosureIdentityV1([u8; 32]);

impl PlanBoundCompilerFfiClosureIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// G1 closures produced only after exact plan/provider validation.
///
/// This is still inert build data. In particular, successful construction is
/// not proof that a provider defines a symbol or that declared effects and
/// semantics are true.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanBoundCompilerFfiClosureV1 {
    plan_identity: LinkPlanIdentityV1,
    compiler_closure_identity: CompilerFfiClosureIdentityV1,
    plan_inputs: Vec<CompilerFfiPlanInputBindingV1>,
    provider_bindings: Vec<CompilerFfiProviderBindingV1>,
    input_kinds: LinkInputKindClosureV1,
    symbols: LinkSymbolClosureV1,
    canonical_bytes: Vec<u8>,
    identity: PlanBoundCompilerFfiClosureIdentityV1,
}

impl PlanBoundCompilerFfiClosureV1 {
    pub const fn plan_identity(&self) -> LinkPlanIdentityV1 {
        self.plan_identity
    }

    pub const fn compiler_closure_identity(&self) -> CompilerFfiClosureIdentityV1 {
        self.compiler_closure_identity
    }

    pub fn plan_inputs(&self) -> &[CompilerFfiPlanInputBindingV1] {
        &self.plan_inputs
    }

    pub fn provider_bindings(&self) -> &[CompilerFfiProviderBindingV1] {
        &self.provider_bindings
    }

    pub const fn input_kinds(&self) -> &LinkInputKindClosureV1 {
        &self.input_kinds
    }

    pub const fn symbols(&self) -> &LinkSymbolClosureV1 {
        &self.symbols
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> PlanBoundCompilerFfiClosureIdentityV1 {
        self.identity
    }

    pub const fn effects_are_derived(&self) -> bool {
        false
    }

    pub const fn semantics_are_verified(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Validates exact provider bindings and closes compiler FFI facts over `plan`.
///
/// `plan_inputs` must be in the plan's existing canonical input order. Provider
/// bindings must be strictly ordered by contract identity. The function never
/// sorts, guesses a provider by symbol name, or inspects provider bytes.
pub fn bind_compiler_ffi_closure_v1(
    plan: &MultiInputLinkPlanV1,
    compiler: &CompilerFfiClosureV1,
    plan_inputs: Vec<CompilerFfiPlanInputBindingV1>,
    provider_bindings: Vec<CompilerFfiProviderBindingV1>,
) -> Result<PlanBoundCompilerFfiClosureV1, CompilerFfiBridgeError> {
    if compiler.target != plan.target() {
        return Err(CompilerFfiBridgeError::PlanTargetMismatch);
    }
    let plan_code_object_version = plan_code_object_version(plan)?;
    if compiler.code_object_version != plan_code_object_version {
        return Err(CompilerFfiBridgeError::PlanCodeObjectVersionMismatch {
            plan: plan_code_object_version,
            compiler: compiler.code_object_version,
        });
    }
    validate_plan_inputs(plan, &plan_inputs)?;
    validate_provider_bindings(compiler, &plan_inputs, &provider_bindings)?;

    let input_kinds = LinkInputKindClosureV1::new(
        plan,
        plan_inputs.iter().map(|binding| binding.kind).collect(),
    )
    .map_err(CompilerFfiBridgeError::RequestClosure)?;
    let import_symbols = compiler
        .symbols
        .iter()
        .filter(|symbol| symbol.direction == CompilerFfiDirectionV1::Import)
        .map(|symbol| symbol.symbol.clone())
        .collect();
    let export_symbols = compiler
        .symbols
        .iter()
        .filter(|symbol| symbol.direction == CompilerFfiDirectionV1::Export)
        .map(|symbol| symbol.symbol.clone())
        .collect();
    let symbols = LinkSymbolClosureV1::new(
        compiler.required_symbols.clone(),
        import_symbols,
        export_symbols,
    )
    .map_err(CompilerFfiBridgeError::RequestClosure)?;

    let canonical_bytes = encode_plan_bound_closure(
        plan,
        compiler,
        &plan_inputs,
        &provider_bindings,
        &input_kinds,
        &symbols,
    );
    let identity = PlanBoundCompilerFfiClosureIdentityV1(Sha256::digest(&canonical_bytes).into());
    Ok(PlanBoundCompilerFfiClosureV1 {
        plan_identity: plan.identity(),
        compiler_closure_identity: compiler.identity,
        plan_inputs,
        provider_bindings,
        input_kinds,
        symbols,
        canonical_bytes,
        identity,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerFfiBridgeError {
    EmptyRequiredSymbolSet,
    TooManyCompilerFfiSymbols,
    CompilerClosureTooLarge,
    ReservedIdentity(&'static str),
    InvalidText(&'static str),
    InvalidEffects,
    InvalidSymbolSet(WorkerProtocolError),
    NonCanonicalCompilerSymbols,
    DuplicateContractIdentity(CompilerFfiContractIdentityV1),
    ContractIdentityMismatch {
        claimed: CompilerFfiContractIdentityV1,
        derived: CompilerFfiContractIdentityV1,
    },
    DuplicateCompilerSymbol(String),
    DuplicateSourceOwner(CompilerFfiSourceOwnerIdentityV1),
    MissingRequiredSymbol(String),
    SymbolTargetMismatch(String),
    SymbolCodeObjectVersionMismatch(String),
    DirectionDefinitionMismatch {
        symbol: String,
        direction: CompilerFfiDirectionV1,
        definition: CompilerFfiDefinitionV1,
    },
    PlanTargetMismatch,
    MissingPlanCodeObjectVersion,
    InvalidPlanCodeObjectVersion(String),
    PlanCodeObjectVersionMismatch {
        plan: CodeObjectVersion,
        compiler: CodeObjectVersion,
    },
    PlanInputCountMismatch {
        plan: usize,
        bindings: usize,
    },
    PlanInputSequenceMismatch {
        index: usize,
        plan: ContentIdentityV1,
        binding: ContentIdentityV1,
    },
    CompilerInputIsNotLlvmBitcode,
    MissingRustCompilerInput,
    MultipleRustCompilerInputs,
    NonCanonicalProviderBindings,
    DuplicateProviderBinding(CompilerFfiContractIdentityV1),
    ConflictingProviderBinding(CompilerFfiContractIdentityV1),
    UnreferencedProviderBinding(CompilerFfiContractIdentityV1),
    MissingProviderBinding(CompilerFfiContractIdentityV1),
    ProviderSourceOwnerMismatch(CompilerFfiContractIdentityV1),
    ProviderInputNotInPlan(ContentIdentityV1),
    UnreferencedProviderInput(ContentIdentityV1),
    ProviderInputKindMismatch {
        contract: CompilerFfiContractIdentityV1,
        declared: WorkerInputKindV1,
        planned: WorkerInputKindV1,
    },
    ProviderInputRoleMismatch {
        contract: CompilerFfiContractIdentityV1,
        definition: CompilerFfiDefinitionV1,
        role: CompilerFfiPlanInputRoleV1,
    },
    RequestClosure(crate::WorkerRequestConstructionError),
}

impl fmt::Display for CompilerFfiBridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRequiredSymbolSet => {
                formatter.write_str("compiler FFI closure has no required symbols")
            }
            Self::TooManyCompilerFfiSymbols => {
                formatter.write_str("compiler FFI symbol count exceeds the V1 bound")
            }
            Self::CompilerClosureTooLarge => {
                formatter.write_str("canonical compiler FFI closure exceeds the V1 byte bound")
            }
            Self::ReservedIdentity(field) => {
                write!(formatter, "{field} uses the reserved zero identity")
            }
            Self::InvalidText(field) => {
                write!(formatter, "{field} is empty, oversized, or noncanonical")
            }
            Self::InvalidEffects => {
                formatter.write_str("declared effects are not a bounded canonical set")
            }
            Self::InvalidSymbolSet(error) => {
                write!(formatter, "invalid compiler FFI symbol set: {error}")
            }
            Self::NonCanonicalCompilerSymbols => {
                formatter.write_str("compiler FFI symbols are not in canonical order")
            }
            Self::DuplicateContractIdentity(identity) => {
                write!(formatter, "duplicate compiler FFI contract {identity:?}")
            }
            Self::ContractIdentityMismatch { claimed, derived } => write!(
                formatter,
                "compiler FFI contract identity {claimed:?} does not match derived identity {derived:?}"
            ),
            Self::DuplicateCompilerSymbol(symbol) => {
                write!(formatter, "duplicate compiler FFI symbol {symbol}")
            }
            Self::DuplicateSourceOwner(identity) => write!(
                formatter,
                "duplicate compiler FFI source owner {identity:?}"
            ),
            Self::MissingRequiredSymbol(symbol) => write!(
                formatter,
                "compiler FFI symbol {symbol} is absent from the complete required-symbol set"
            ),
            Self::SymbolTargetMismatch(symbol) => write!(
                formatter,
                "compiler FFI symbol {symbol} disagrees with the closure target"
            ),
            Self::SymbolCodeObjectVersionMismatch(symbol) => write!(
                formatter,
                "compiler FFI symbol {symbol} disagrees with the closure code-object version"
            ),
            Self::DirectionDefinitionMismatch {
                symbol,
                direction,
                definition,
            } => write!(
                formatter,
                "compiler FFI symbol {symbol} direction {direction:?} conflicts with definition role {definition:?}"
            ),
            Self::PlanTargetMismatch => {
                formatter.write_str("compiler FFI closure target does not match the link plan")
            }
            Self::MissingPlanCodeObjectVersion => {
                formatter.write_str("link plan has no code-object-version option")
            }
            Self::InvalidPlanCodeObjectVersion(value) => write!(
                formatter,
                "link plan has unsupported code-object version {value}"
            ),
            Self::PlanCodeObjectVersionMismatch { plan, compiler } => write!(
                formatter,
                "compiler FFI code-object version {compiler:?} does not match plan {plan:?}"
            ),
            Self::PlanInputCountMismatch { plan, bindings } => write!(
                formatter,
                "plan has {plan} inputs but compiler FFI bridge received {bindings} input bindings"
            ),
            Self::PlanInputSequenceMismatch {
                index,
                plan,
                binding,
            } => write!(
                formatter,
                "input binding {index} identity {binding} does not match canonical plan input {plan}"
            ),
            Self::CompilerInputIsNotLlvmBitcode => {
                formatter.write_str("Rust compiler input role requires LLVM bitcode")
            }
            Self::MissingRustCompilerInput => formatter
                .write_str("Rust FFI definitions have no unique compiler LLVM-bitcode input"),
            Self::MultipleRustCompilerInputs => formatter
                .write_str("multiple plan inputs claim the unique Rust compiler-bitcode role"),
            Self::NonCanonicalProviderBindings => {
                formatter.write_str("provider bindings are not in canonical contract order")
            }
            Self::DuplicateProviderBinding(identity) => write!(
                formatter,
                "duplicate provider binding for contract {identity:?}"
            ),
            Self::ConflictingProviderBinding(identity) => write!(
                formatter,
                "conflicting provider bindings for contract {identity:?}"
            ),
            Self::UnreferencedProviderBinding(identity) => write!(
                formatter,
                "provider binding references unknown contract {identity:?}"
            ),
            Self::MissingProviderBinding(identity) => write!(
                formatter,
                "compiler FFI contract {identity:?} has no provider binding"
            ),
            Self::ProviderSourceOwnerMismatch(identity) => write!(
                formatter,
                "provider binding source owner does not match contract {identity:?}"
            ),
            Self::ProviderInputNotInPlan(identity) => write!(
                formatter,
                "provider input {identity} is absent from the exact link plan"
            ),
            Self::UnreferencedProviderInput(identity) => write!(
                formatter,
                "provider-role input {identity} is not referenced by any exact symbol binding"
            ),
            Self::ProviderInputKindMismatch {
                contract,
                declared,
                planned,
            } => write!(
                formatter,
                "provider for contract {contract:?} declares kind {declared:?} but plan binding declares {planned:?}"
            ),
            Self::ProviderInputRoleMismatch {
                contract,
                definition,
                role,
            } => write!(
                formatter,
                "provider for contract {contract:?} has input role {role:?}, incompatible with {definition:?}"
            ),
            Self::RequestClosure(error) => {
                write!(formatter, "G1 request closure construction failed: {error}")
            }
        }
    }
}

impl std::error::Error for CompilerFfiBridgeError {}

fn validate_compiler_symbols(
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    required_symbols: &[String],
    symbols: &[CompilerFfiSymbolV1],
) -> Result<(), CompilerFfiBridgeError> {
    let mut contracts = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut owners = BTreeMap::new();
    let mut previous = None;
    for symbol in symbols {
        let order_key = (
            symbol.symbol.as_str(),
            symbol.contract_identity,
            symbol.source_owner.identity,
        );
        if previous.is_some_and(|previous| previous >= order_key) {
            return Err(CompilerFfiBridgeError::NonCanonicalCompilerSymbols);
        }
        previous = Some(order_key);
        if contracts.insert(symbol.contract_identity, ()).is_some() {
            return Err(CompilerFfiBridgeError::DuplicateContractIdentity(
                symbol.contract_identity,
            ));
        }
        if names.insert(symbol.symbol.as_str(), ()).is_some() {
            return Err(CompilerFfiBridgeError::DuplicateCompilerSymbol(
                symbol.symbol.clone(),
            ));
        }
        if owners.insert(symbol.source_owner.identity, ()).is_some() {
            return Err(CompilerFfiBridgeError::DuplicateSourceOwner(
                symbol.source_owner.identity,
            ));
        }
        if required_symbols.binary_search(&symbol.symbol).is_err() {
            return Err(CompilerFfiBridgeError::MissingRequiredSymbol(
                symbol.symbol.clone(),
            ));
        }
        if symbol.declared.target != target {
            return Err(CompilerFfiBridgeError::SymbolTargetMismatch(
                symbol.symbol.clone(),
            ));
        }
        if symbol.declared.code_object_version != code_object_version {
            return Err(CompilerFfiBridgeError::SymbolCodeObjectVersionMismatch(
                symbol.symbol.clone(),
            ));
        }
    }
    Ok(())
}

fn validate_plan_inputs(
    plan: &MultiInputLinkPlanV1,
    bindings: &[CompilerFfiPlanInputBindingV1],
) -> Result<(), CompilerFfiBridgeError> {
    if plan.inputs().len() != bindings.len() {
        return Err(CompilerFfiBridgeError::PlanInputCountMismatch {
            plan: plan.inputs().len(),
            bindings: bindings.len(),
        });
    }
    let mut compiler_inputs = 0;
    for (index, (planned, binding)) in plan.inputs().iter().zip(bindings).enumerate() {
        if planned.identity() != binding.identity {
            return Err(CompilerFfiBridgeError::PlanInputSequenceMismatch {
                index,
                plan: planned.identity(),
                binding: binding.identity,
            });
        }
        if binding.role == CompilerFfiPlanInputRoleV1::RustCompilerBitcode {
            compiler_inputs += 1;
            if binding.kind != WorkerInputKindV1::LlvmBitcode {
                return Err(CompilerFfiBridgeError::CompilerInputIsNotLlvmBitcode);
            }
        }
    }
    if compiler_inputs > 1 {
        return Err(CompilerFfiBridgeError::MultipleRustCompilerInputs);
    }
    Ok(())
}

fn validate_provider_bindings(
    compiler: &CompilerFfiClosureV1,
    plan_inputs: &[CompilerFfiPlanInputBindingV1],
    bindings: &[CompilerFfiProviderBindingV1],
) -> Result<(), CompilerFfiBridgeError> {
    let has_rust_definitions = compiler
        .symbols
        .iter()
        .any(|symbol| symbol.definition == CompilerFfiDefinitionV1::RustCompilerBitcode);
    if has_rust_definitions
        && !plan_inputs
            .iter()
            .any(|input| input.role == CompilerFfiPlanInputRoleV1::RustCompilerBitcode)
    {
        return Err(CompilerFfiBridgeError::MissingRustCompilerInput);
    }
    let symbols_by_contract: BTreeMap<_, _> = compiler
        .symbols
        .iter()
        .map(|symbol| (symbol.contract_identity, symbol))
        .collect();
    let plan_inputs_by_identity: BTreeMap<_, _> = plan_inputs
        .iter()
        .map(|input| (input.identity, input))
        .collect();
    let mut previous: Option<&CompilerFfiProviderBindingV1> = None;
    let mut seen = BTreeMap::new();
    let mut referenced_inputs = BTreeMap::new();
    for binding in bindings {
        if let Some(previous) = previous {
            if previous.contract_identity > binding.contract_identity {
                return Err(CompilerFfiBridgeError::NonCanonicalProviderBindings);
            }
            if previous.contract_identity == binding.contract_identity {
                return Err(if previous == binding {
                    CompilerFfiBridgeError::DuplicateProviderBinding(binding.contract_identity)
                } else {
                    CompilerFfiBridgeError::ConflictingProviderBinding(binding.contract_identity)
                });
            }
        }
        previous = Some(binding);
        let symbol = symbols_by_contract.get(&binding.contract_identity).ok_or(
            CompilerFfiBridgeError::UnreferencedProviderBinding(binding.contract_identity),
        )?;
        if symbol.source_owner.identity != binding.source_owner_identity {
            return Err(CompilerFfiBridgeError::ProviderSourceOwnerMismatch(
                binding.contract_identity,
            ));
        }
        let input = plan_inputs_by_identity
            .get(&binding.provider_input_identity)
            .ok_or(CompilerFfiBridgeError::ProviderInputNotInPlan(
                binding.provider_input_identity,
            ))?;
        if input.kind != binding.provider_input_kind {
            return Err(CompilerFfiBridgeError::ProviderInputKindMismatch {
                contract: binding.contract_identity,
                declared: binding.provider_input_kind,
                planned: input.kind,
            });
        }
        let expected_role = match symbol.definition {
            CompilerFfiDefinitionV1::RustCompilerBitcode => {
                CompilerFfiPlanInputRoleV1::RustCompilerBitcode
            }
            CompilerFfiDefinitionV1::ExternalPlanInput => {
                CompilerFfiPlanInputRoleV1::ExternalDefinitionProvider
            }
        };
        if input.role != expected_role {
            return Err(CompilerFfiBridgeError::ProviderInputRoleMismatch {
                contract: binding.contract_identity,
                definition: symbol.definition,
                role: input.role,
            });
        }
        seen.insert(binding.contract_identity, ());
        referenced_inputs.insert(binding.provider_input_identity, ());
    }
    for symbol in &compiler.symbols {
        if !seen.contains_key(&symbol.contract_identity) {
            return Err(CompilerFfiBridgeError::MissingProviderBinding(
                symbol.contract_identity,
            ));
        }
    }
    for input in plan_inputs {
        if matches!(
            input.role,
            CompilerFfiPlanInputRoleV1::RustCompilerBitcode
                | CompilerFfiPlanInputRoleV1::ExternalDefinitionProvider
        ) && !referenced_inputs.contains_key(&input.identity)
        {
            return Err(CompilerFfiBridgeError::UnreferencedProviderInput(
                input.identity,
            ));
        }
    }
    Ok(())
}

fn plan_code_object_version(
    plan: &MultiInputLinkPlanV1,
) -> Result<CodeObjectVersion, CompilerFfiBridgeError> {
    let value = plan
        .options()
        .iter()
        .find(|option| option.name() == "code-object-version")
        .ok_or(CompilerFfiBridgeError::MissingPlanCodeObjectVersion)?
        .value();
    match value {
        "4" => Ok(CodeObjectVersion::V4),
        "5" => Ok(CodeObjectVersion::V5),
        "6" => Ok(CodeObjectVersion::V6),
        value => Err(CompilerFfiBridgeError::InvalidPlanCodeObjectVersion(
            value.to_owned(),
        )),
    }
}

fn validate_effects(effects: &str) -> Result<(), CompilerFfiBridgeError> {
    if effects.is_empty() || effects.len() > MAX_COMPILER_FFI_EFFECT_BYTES_V1 {
        return Err(CompilerFfiBridgeError::InvalidEffects);
    }
    if effects == "none" {
        return Ok(());
    }
    let mut previous = None;
    for effect in effects.split(',') {
        if !matches!(
            effect,
            "atomic_global"
                | "atomic_workgroup"
                | "barrier_workgroup"
                | "read_constant"
                | "read_global"
                | "read_private"
                | "read_workgroup"
                | "write_global"
                | "write_private"
                | "write_workgroup"
        ) || previous.is_some_and(|previous: &str| previous >= effect)
        {
            return Err(CompilerFfiBridgeError::InvalidEffects);
        }
        previous = Some(effect);
    }
    Ok(())
}

fn validate_text(
    field: &'static str,
    text: &str,
    max_bytes: usize,
    ascii: bool,
) -> Result<(), CompilerFfiBridgeError> {
    if text.is_empty()
        || text.len() > max_bytes
        || (ascii && !text.is_ascii())
        || text.chars().any(char::is_control)
    {
        return Err(CompilerFfiBridgeError::InvalidText(field));
    }
    Ok(())
}

fn validate_ascii_token(
    field: &'static str,
    text: &str,
    max_bytes: usize,
) -> Result<(), CompilerFfiBridgeError> {
    validate_text(field, text, max_bytes, true)?;
    if text.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(CompilerFfiBridgeError::InvalidText(field));
    }
    Ok(())
}

fn encode_compiler_closure(
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    required_symbols: &[String],
    symbols: &[CompilerFfiSymbolV1],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(COMPILER_CLOSURE_DOMAIN_V1);
    push_origin(&mut bytes, CompilerFfiFieldOriginV1::DeclaredClaim);
    push_text(&mut bytes, &target.to_string());
    push_origin(&mut bytes, CompilerFfiFieldOriginV1::DeclaredClaim);
    bytes.push(code_object_version_byte(code_object_version));
    push_origin(&mut bytes, CompilerFfiFieldOriginV1::CompilerDerived);
    push_strings(&mut bytes, required_symbols);
    push_u32(&mut bytes, symbols.len());
    for symbol in symbols {
        encode_symbol(&mut bytes, symbol);
    }
    bytes
}

fn encode_symbol(bytes: &mut Vec<u8>, symbol: &CompilerFfiSymbolV1) {
    push_origin(bytes, CompilerFfiSymbolFieldV1::ContractIdentity.origin());
    bytes.extend_from_slice(symbol.contract_identity.as_bytes());
    push_origin(bytes, CompilerFfiSymbolFieldV1::Direction.origin());
    bytes.push(symbol.direction as u8);
    push_origin(bytes, CompilerFfiSymbolFieldV1::Symbol.origin());
    push_text(bytes, &symbol.symbol);
    push_origin(bytes, CompilerFfiSymbolFieldV1::PhysicalAbi.origin());
    push_text(bytes, &symbol.physical_abi);
    push_origin(bytes, CompilerFfiSymbolFieldV1::SourceOwner.origin());
    bytes.extend_from_slice(symbol.source_owner.identity.as_bytes());
    push_text(bytes, &symbol.source_owner.crate_name);
    push_text(bytes, &symbol.source_owner.item_path);
    bytes.extend_from_slice(&symbol.source_owner.def_path_hash);
    push_text(bytes, &symbol.source_owner.concrete_instance_symbol);
    push_origin(bytes, CompilerFfiSymbolFieldV1::Definition.origin());
    bytes.push(symbol.definition as u8);
    push_origin(bytes, CompilerFfiSymbolFieldV1::Target.origin());
    push_text(bytes, &symbol.declared.target.to_string());
    push_origin(bytes, CompilerFfiSymbolFieldV1::CodeObjectVersion.origin());
    bytes.push(code_object_version_byte(
        symbol.declared.code_object_version,
    ));
    push_origin(bytes, CompilerFfiSymbolFieldV1::Effects.origin());
    push_text(bytes, &symbol.declared.effects);
    push_origin(bytes, CompilerFfiSymbolFieldV1::SemanticClaim.origin());
    bytes.extend_from_slice(&symbol.declared.semantic_claim);
}

fn encode_plan_bound_closure(
    plan: &MultiInputLinkPlanV1,
    compiler: &CompilerFfiClosureV1,
    plan_inputs: &[CompilerFfiPlanInputBindingV1],
    providers: &[CompilerFfiProviderBindingV1],
    input_kinds: &LinkInputKindClosureV1,
    symbols: &LinkSymbolClosureV1,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(PLAN_BOUND_CLOSURE_DOMAIN_V1);
    bytes.extend_from_slice(plan.identity().as_bytes());
    bytes.extend_from_slice(compiler.identity.as_bytes());
    push_origin(&mut bytes, CompilerFfiFieldOriginV1::CallerBindingClaim);
    push_u32(&mut bytes, plan_inputs.len());
    for input in plan_inputs {
        push_content_identity(&mut bytes, input.identity);
        bytes.push(input.kind as u8);
        bytes.push(input.role as u8);
    }
    push_origin(&mut bytes, CompilerFfiFieldOriginV1::CallerBindingClaim);
    push_u32(&mut bytes, providers.len());
    for provider in providers {
        bytes.extend_from_slice(provider.contract_identity.as_bytes());
        bytes.extend_from_slice(provider.source_owner_identity.as_bytes());
        push_content_identity(&mut bytes, provider.provider_input_identity);
        bytes.push(provider.provider_input_kind as u8);
    }
    bytes.extend_from_slice(input_kinds.identity().as_bytes());
    bytes.extend_from_slice(symbols.identity().as_bytes());
    bytes
}

fn push_origin(bytes: &mut Vec<u8>, origin: CompilerFfiFieldOriginV1) {
    bytes.push(origin as u8);
}

fn push_content_identity(bytes: &mut Vec<u8>, identity: ContentIdentityV1) {
    bytes.extend_from_slice(identity.sha256());
    bytes.extend_from_slice(&identity.byte_len().to_le_bytes());
}

fn push_strings(bytes: &mut Vec<u8>, strings: &[String]) {
    push_u32(bytes, strings.len());
    for string in strings {
        push_text(bytes, string);
    }
}

fn push_text(bytes: &mut Vec<u8>, text: &str) {
    push_u32(bytes, text.len());
    bytes.extend_from_slice(text.as_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(&(value as u32).to_le_bytes());
}

const fn code_object_version_byte(version: CodeObjectVersion) -> u8 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}

const fn direction_tag(direction: CompilerFfiDirectionV1) -> u16 {
    match direction {
        CompilerFfiDirectionV1::Import => 1,
        CompilerFfiDirectionV1::Export => 2,
    }
}

fn derive_contract_identity(
    direction: CompilerFfiDirectionV1,
    symbol: &str,
    physical_abi: &str,
    declared: &CompilerFfiDeclaredClaimsV1,
) -> CompilerFfiContractIdentityV1 {
    // This is the compiler-marker V1 preimage. Keeping the compatibility code
    // here avoids making the plan/request crate depend on compiler integration.
    let target = declared.target.to_string();
    let semantic_identity = lower_hex(&declared.semantic_claim);
    let mut digest = Sha256::new();
    digest.update(DEVICE_FFI_CONTRACT_DOMAIN_V1);
    digest.update(direction_tag(direction).to_le_bytes());
    hash_contract_field(&mut digest, symbol.as_bytes());
    hash_contract_field(&mut digest, b"C");
    digest.update(u16::from(code_object_version_byte(declared.code_object_version)).to_le_bytes());
    hash_contract_field(&mut digest, target.as_bytes());
    hash_contract_field(&mut digest, physical_abi.as_bytes());
    hash_contract_field(&mut digest, declared.effects.as_bytes());
    hash_contract_field(&mut digest, semantic_identity.as_bytes());
    hash_contract_field(&mut digest, b"nounwind;nopanic");
    CompilerFfiContractIdentityV1(digest.finalize().into())
}

fn hash_contract_field(digest: &mut Sha256, field: &[u8]) {
    digest.update((field.len() as u64).to_le_bytes());
    digest.update(field);
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
