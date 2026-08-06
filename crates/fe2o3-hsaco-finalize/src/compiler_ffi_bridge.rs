//! Assertion-only staging for a future G4-to-G1 device FFI request path.
//!
//! The current worker V1 wire schema cannot bind the complete staged FFI
//! identity. This module therefore stops before request construction. It does
//! not project its data into the generic `LinkSymbolClosureV1` or
//! `LinkInputKindClosureV1`, because doing so would erase ABI, declaration
//! owner, provider, effects, semantics, and producer claims.

use std::{collections::BTreeMap, fmt};

use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1,
    DeviceFfiContractIdV1, DeviceFfiGrammarError, derive_device_ffi_contract_id_v1,
    parse_device_ffi_effects_v1, validate_device_ffi_contract_grammar_v1,
};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, MAX_LINK_INPUTS, MultiInputLinkPlanV1, WorkerInputKindV1,
    WorkerProtocolError, worker_protocol::validate_symbols,
};

pub use reserved_fe2o3_symbols::DeviceFfiContractIdV1 as G4FfiContractIdV1;

/// Maximum G4 FFI symbol claims in one envelope.
pub const MAX_G4_FFI_SYMBOL_CLAIMS_V1: usize = 128;
/// Maximum non-kernel Rust definitions represented by the adapter claim.
pub const MAX_G4_RUST_DEFINITION_CLAIMS_V1: u32 = 4_096;
/// Maximum Rust kernels represented by the adapter claim.
pub const MAX_G4_KERNEL_CLAIMS_V1: u32 = 128;
/// Maximum aggregate bytes across one envelope's variable-length text fields.
pub const MAX_G4_FFI_AGGREGATE_TEXT_BYTES_V1: usize = 384 * 1024;
/// Maximum bytes in one declaration-owner crate label.
pub const MAX_G4_FFI_CRATE_LABEL_BYTES_V1: usize = 128;
/// Maximum bytes in one declaration-owner item label.
pub const MAX_G4_FFI_ITEM_LABEL_BYTES_V1: usize = 1_024;
/// Maximum bytes in one exact concrete instance symbol claim.
pub const MAX_G4_FFI_INSTANCE_SYMBOL_BYTES_V1: usize = 512;
/// Maximum bytes in an unauthenticated producer name.
pub const MAX_FFI_PRODUCER_NAME_BYTES_V1: usize = 128;
/// Maximum bytes in an unauthenticated producer version.
pub const MAX_FFI_PRODUCER_VERSION_BYTES_V1: usize = 256;
/// Maximum canonical bytes retained by an assertion-only G4 envelope.
pub const MAX_G4_FFI_ENVELOPE_BYTES_V1: usize = 512 * 1024;
/// Maximum canonical bytes accepted while deriving one staged-plan identity.
pub const MAX_STAGED_FFI_LINK_PLAN_BYTES_V1: usize = 1024 * 1024;

const DECLARATION_OWNER_DOMAIN_V1: &[u8] = b"FE2O3/G4-FFI-DECLARATION-OWNER-CLAIM/V1\0";
const PRODUCER_CLAIM_DOMAIN_V1: &[u8] = b"FE2O3/UNAUTHENTICATED-PRODUCER-CLAIM/V1\0";
const G4_CLAIM_ENVELOPE_DOMAIN_V1: &[u8] = b"FE2O3/G4-FFI-ASSERTION-ONLY-ENVELOPE/V1\0";
const FINAL_SYMBOLS_CLAIM_DOMAIN_V1: &[u8] = b"FE2O3/EXPECTED-FINAL-DEFINED-SYMBOLS-CLAIM/V1\0";
const STAGED_FFI_LINK_PLAN_DOMAIN_V1: &[u8] = b"FE2O3/STAGED-FFI-LINK-PLAN/V1\0";

/// Claim provenance encoded in staging records.
///
/// Every variant remains caller-supplied data at this crate boundary. In
/// particular, `G4AssertionOnly` does not attest that rustc produced the value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum FfiClaimOriginV1 {
    G4AssertionOnly = 1,
    CallerBindingAssertionOnly = 2,
    UnauthenticatedProducerClaim = 3,
    UnauthenticatedEvidenceClaim = 4,
}

/// Fields carried by one G4 assertion-only symbol claim.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum G4FfiSymbolClaimFieldV1 {
    ContractIdentity,
    Direction,
    Symbol,
    PhysicalAbi,
    DeclarationOwner,
    ProviderClass,
    Target,
    CodeObjectVersion,
    Effects,
    SemanticIdentity,
}

impl G4FfiSymbolClaimFieldV1 {
    pub const fn claim_origin(self) -> FfiClaimOriginV1 {
        let _ = self;
        FfiClaimOriginV1::G4AssertionOnly
    }
}

/// Identity of an assertion-only declaration-owner record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct G4DeclarationOwnerClaimIdentityV1([u8; 32]);

impl G4DeclarationOwnerClaimIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Declaration ownership copied from private G4 state by a future adapter.
///
/// This is not an artifact producer identity. The stable claim identity uses
/// the `DefPathHash` and concrete instance symbol; labels remain diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct G4DeclarationOwnerClaimV1 {
    crate_label: String,
    item_label: String,
    def_path_hash: [u8; 16],
    concrete_instance_symbol: String,
    identity: G4DeclarationOwnerClaimIdentityV1,
}

impl G4DeclarationOwnerClaimV1 {
    pub fn new(
        crate_label: impl Into<String>,
        item_label: impl Into<String>,
        def_path_hash: [u8; 16],
        concrete_instance_symbol: impl Into<String>,
    ) -> Result<Self, StagedFfiLinkError> {
        let crate_label = crate_label.into();
        let item_label = item_label.into();
        let concrete_instance_symbol = concrete_instance_symbol.into();
        validate_text(
            "declaration owner crate label",
            &crate_label,
            MAX_G4_FFI_CRATE_LABEL_BYTES_V1,
            false,
        )?;
        validate_text(
            "declaration owner item label",
            &item_label,
            MAX_G4_FFI_ITEM_LABEL_BYTES_V1,
            false,
        )?;
        validate_ascii_token(
            "concrete instance symbol",
            &concrete_instance_symbol,
            MAX_G4_FFI_INSTANCE_SYMBOL_BYTES_V1,
        )?;

        let mut preimage = Vec::new();
        preimage.extend_from_slice(DECLARATION_OWNER_DOMAIN_V1);
        preimage.extend_from_slice(&def_path_hash);
        push_text(&mut preimage, &concrete_instance_symbol);
        let identity = G4DeclarationOwnerClaimIdentityV1(Sha256::digest(preimage).into());
        Ok(Self {
            crate_label,
            item_label,
            def_path_hash,
            concrete_instance_symbol,
            identity,
        })
    }

    pub fn crate_label(&self) -> &str {
        &self.crate_label
    }

    pub fn item_label(&self) -> &str {
        &self.item_label
    }

    pub const fn def_path_hash(&self) -> &[u8; 16] {
        &self.def_path_hash
    }

    pub fn concrete_instance_symbol(&self) -> &str {
        &self.concrete_instance_symbol
    }

    pub const fn identity(&self) -> G4DeclarationOwnerClaimIdentityV1 {
        self.identity
    }

    pub const fn claim_origin(&self) -> FfiClaimOriginV1 {
        FfiClaimOriginV1::G4AssertionOnly
    }
}

/// Identity of an unauthenticated producer claim.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UnauthenticatedProducerClaimIdentityV1([u8; 32]);

impl UnauthenticatedProducerClaimIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Separate, unauthenticated producer metadata for one exact plan input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnauthenticatedProducerClaimV1 {
    name: String,
    version: String,
    build_identity_claim: [u8; 32],
    identity: UnauthenticatedProducerClaimIdentityV1,
}

impl UnauthenticatedProducerClaimV1 {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        build_identity_claim: [u8; 32],
    ) -> Result<Self, StagedFfiLinkError> {
        let name = name.into();
        let version = version.into();
        validate_text("producer name", &name, MAX_FFI_PRODUCER_NAME_BYTES_V1, true)?;
        validate_text(
            "producer version",
            &version,
            MAX_FFI_PRODUCER_VERSION_BYTES_V1,
            true,
        )?;
        if build_identity_claim == [0; 32] {
            return Err(StagedFfiLinkError::ReservedIdentity(
                "producer build identity claim",
            ));
        }
        let mut preimage = Vec::new();
        preimage.extend_from_slice(PRODUCER_CLAIM_DOMAIN_V1);
        push_text(&mut preimage, &name);
        push_text(&mut preimage, &version);
        preimage.extend_from_slice(&build_identity_claim);
        let identity = UnauthenticatedProducerClaimIdentityV1(Sha256::digest(preimage).into());
        Ok(Self {
            name,
            version,
            build_identity_claim,
            identity,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub const fn build_identity_claim(&self) -> &[u8; 32] {
        &self.build_identity_claim
    }

    pub const fn identity(&self) -> UnauthenticatedProducerClaimIdentityV1 {
        self.identity
    }

    pub const fn claim_origin(&self) -> FfiClaimOriginV1 {
        FfiClaimOriginV1::UnauthenticatedProducerClaim
    }

    pub const fn is_authenticated(&self) -> bool {
        false
    }
}

/// G4's assertion-only import/export direction.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum G4FfiDirectionClaimV1 {
    Import = 1,
    Export = 2,
}

/// G4's assertion-only class for where a symbol provider must be bound.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum G4SymbolProviderClassClaimV1 {
    ExternalPlanInput = 1,
    CompilerModuleInput = 2,
}

/// Source-declared contract claims retained by G4.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct G4DeclaredContractClaimsV1 {
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    effects: String,
    semantic_identity: [u8; 32],
}

impl G4DeclaredContractClaimsV1 {
    pub fn new(
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        effects: impl Into<String>,
        semantic_identity: [u8; 32],
    ) -> Result<Self, StagedFfiLinkError> {
        let effects = effects.into();
        parse_device_ffi_effects_v1(&effects).map_err(map_device_ffi_grammar_error)?;
        if semantic_identity == [0; 32] {
            return Err(StagedFfiLinkError::ReservedIdentity(
                "semantic identity claim",
            ));
        }
        Ok(Self {
            target,
            code_object_version,
            effects,
            semantic_identity,
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

    pub const fn semantic_identity(&self) -> &[u8; 32] {
        &self.semantic_identity
    }

    pub const fn claim_origin(&self) -> FfiClaimOriginV1 {
        FfiClaimOriginV1::G4AssertionOnly
    }

    pub const fn effects_are_derived(&self) -> bool {
        false
    }

    pub const fn semantics_are_verified(&self) -> bool {
        false
    }
}

/// One exact assertion-only symbol record expected from a future G4 adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct G4FfiSymbolClaimV1 {
    contract_identity: DeviceFfiContractIdV1,
    direction: G4FfiDirectionClaimV1,
    symbol: String,
    physical_abi: String,
    declaration_owner: G4DeclarationOwnerClaimV1,
    provider_class: G4SymbolProviderClassClaimV1,
    declared: G4DeclaredContractClaimsV1,
}

impl G4FfiSymbolClaimV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contract_identity: DeviceFfiContractIdV1,
        direction: G4FfiDirectionClaimV1,
        symbol: impl Into<String>,
        physical_abi: impl Into<String>,
        declaration_owner: G4DeclarationOwnerClaimV1,
        provider_class: G4SymbolProviderClassClaimV1,
        declared: G4DeclaredContractClaimsV1,
    ) -> Result<Self, StagedFfiLinkError> {
        let symbol = symbol.into();
        let physical_abi = physical_abi.into();
        validate_device_ffi_contract_grammar_v1(&symbol, &physical_abi, &declared.effects)
            .map_err(map_device_ffi_grammar_error)?;
        let expected_provider = match direction {
            G4FfiDirectionClaimV1::Import => G4SymbolProviderClassClaimV1::ExternalPlanInput,
            G4FfiDirectionClaimV1::Export => G4SymbolProviderClassClaimV1::CompilerModuleInput,
        };
        if provider_class != expected_provider {
            return Err(StagedFfiLinkError::DirectionProviderClassMismatch {
                symbol,
                direction,
                provider_class,
            });
        }
        let semantic_identity = lower_hex(&declared.semantic_identity);
        let target = declared.target.to_string();
        let derived = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: direction_tag(direction),
            symbol: &symbol,
            calling_convention: "C",
            code_object_version: u16::from(code_object_version_byte(declared.code_object_version)),
            target: &target,
            physical_abi: &physical_abi,
            effects: &declared.effects,
            semantic_identity: &semantic_identity,
        });
        if derived != contract_identity {
            return Err(StagedFfiLinkError::ContractIdentityMismatch {
                claimed: contract_identity,
                derived,
            });
        }
        Ok(Self {
            contract_identity,
            direction,
            symbol,
            physical_abi,
            declaration_owner,
            provider_class,
            declared,
        })
    }

    pub const fn contract_identity(&self) -> DeviceFfiContractIdV1 {
        self.contract_identity
    }

    pub const fn direction(&self) -> G4FfiDirectionClaimV1 {
        self.direction
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub fn physical_abi(&self) -> &str {
        &self.physical_abi
    }

    pub const fn declaration_owner(&self) -> &G4DeclarationOwnerClaimV1 {
        &self.declaration_owner
    }

    pub const fn provider_class(&self) -> G4SymbolProviderClassClaimV1 {
        self.provider_class
    }

    pub const fn declared(&self) -> &G4DeclaredContractClaimsV1 {
        &self.declared
    }

    pub const fn field_claim_origin(field: G4FfiSymbolClaimFieldV1) -> FfiClaimOriginV1 {
        field.claim_origin()
    }
}

/// Identity of the exact assertion-only G4 claim envelope.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct G4FfiClaimEnvelopeIdentityV1([u8; 32]);

impl G4FfiClaimEnvelopeIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Exact public envelope a future rustc-side `CollectionResult` adapter needs.
///
/// No implementation of [`G4FfiClaimEnvelopeAdapterV1`] exists yet. Values
/// entering this crate remain caller claims, even when copied from G4.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct G4FfiClaimEnvelopeV1 {
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    compiler_required_symbols: Vec<String>,
    rust_definition_count: u32,
    kernel_count: u32,
    symbols: Vec<G4FfiSymbolClaimV1>,
    canonical_bytes: Vec<u8>,
    identity: G4FfiClaimEnvelopeIdentityV1,
}

impl G4FfiClaimEnvelopeV1 {
    pub fn new(
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        compiler_required_symbols: Vec<String>,
        rust_definition_count: u32,
        kernel_count: u32,
        symbols: Vec<G4FfiSymbolClaimV1>,
    ) -> Result<Self, StagedFfiLinkError> {
        validate_symbols(&compiler_required_symbols)
            .map_err(StagedFfiLinkError::InvalidCompilerRequiredSymbols)?;
        if rust_definition_count > MAX_G4_RUST_DEFINITION_CLAIMS_V1 {
            return Err(StagedFfiLinkError::TooManyRustDefinitionClaims);
        }
        if kernel_count > MAX_G4_KERNEL_CLAIMS_V1 {
            return Err(StagedFfiLinkError::TooManyKernelClaims);
        }
        if symbols.len() > MAX_G4_FFI_SYMBOL_CLAIMS_V1 {
            return Err(StagedFfiLinkError::TooManyFfiSymbolClaims);
        }
        if (rust_definition_count != 0 || kernel_count != 0 || !symbols.is_empty())
            && compiler_required_symbols.is_empty()
        {
            return Err(StagedFfiLinkError::MissingCompilerRequiredSymbols);
        }
        let export_count = symbols
            .iter()
            .filter(|symbol| symbol.direction == G4FfiDirectionClaimV1::Export)
            .count();
        if export_count > rust_definition_count as usize {
            return Err(StagedFfiLinkError::RustDefinitionCountTooSmall {
                claimed: rust_definition_count,
                exports: export_count,
            });
        }
        validate_g4_symbol_claims(
            target,
            code_object_version,
            &compiler_required_symbols,
            &symbols,
        )?;
        let aggregate_text_bytes = envelope_text_bytes(&compiler_required_symbols, &symbols)?;
        if aggregate_text_bytes > MAX_G4_FFI_AGGREGATE_TEXT_BYTES_V1 {
            return Err(StagedFfiLinkError::AggregateTextBoundExceeded);
        }
        let canonical_bytes = encode_g4_envelope(
            target,
            code_object_version,
            &compiler_required_symbols,
            rust_definition_count,
            kernel_count,
            &symbols,
        );
        if canonical_bytes.len() > MAX_G4_FFI_ENVELOPE_BYTES_V1 {
            return Err(StagedFfiLinkError::EnvelopeByteBoundExceeded);
        }
        let identity = G4FfiClaimEnvelopeIdentityV1(Sha256::digest(&canonical_bytes).into());
        Ok(Self {
            target,
            code_object_version,
            compiler_required_symbols,
            rust_definition_count,
            kernel_count,
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

    pub fn compiler_required_symbols(&self) -> &[String] {
        &self.compiler_required_symbols
    }

    pub const fn rust_definition_count(&self) -> u32 {
        self.rust_definition_count
    }

    pub const fn kernel_count(&self) -> u32 {
        self.kernel_count
    }

    pub const fn requires_compiler_module(&self) -> bool {
        self.rust_definition_count != 0 || self.kernel_count != 0
    }

    pub fn symbols(&self) -> &[G4FfiSymbolClaimV1] {
        &self.symbols
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> G4FfiClaimEnvelopeIdentityV1 {
        self.identity
    }

    pub const fn claim_origin(&self) -> FfiClaimOriginV1 {
        FfiClaimOriginV1::G4AssertionOnly
    }

    pub const fn is_actual_compiler_integration(&self) -> bool {
        false
    }

    pub const fn grants_link_authority(&self) -> bool {
        false
    }
}

/// Adapter contract to be implemented on rustc-side state in a separate change.
///
/// The implementation must copy `CollectionResult.device_ffi`, exact compiler
/// required symbols, Rust definition count, and kernel count into one envelope.
/// It must not invent a compiler-module artifact identity.
pub trait G4FfiClaimEnvelopeAdapterV1 {
    fn assertion_only_g4_ffi_claim_envelope_v1(
        &self,
    ) -> Result<G4FfiClaimEnvelopeV1, StagedFfiLinkError>;
}

/// Caller-asserted role of an exact plan input.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum FfiPlanInputRoleClaimV1 {
    /// A future exact compiler-module artifact; no format is inferred.
    CompilerModule = 1,
    ExternalSymbolProvider = 2,
    LinkSupport = 3,
}

/// Exact caller claim for one input in the plan's canonical sequence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FfiPlanInputClaimV1 {
    identity: ContentIdentityV1,
    kind: WorkerInputKindV1,
    role: FfiPlanInputRoleClaimV1,
    producer: UnauthenticatedProducerClaimV1,
}

impl FfiPlanInputClaimV1 {
    pub const fn new(
        identity: ContentIdentityV1,
        kind: WorkerInputKindV1,
        role: FfiPlanInputRoleClaimV1,
        producer: UnauthenticatedProducerClaimV1,
    ) -> Self {
        Self {
            identity,
            kind,
            role,
            producer,
        }
    }

    pub const fn identity(&self) -> ContentIdentityV1 {
        self.identity
    }

    pub const fn kind(&self) -> WorkerInputKindV1 {
        self.kind
    }

    pub const fn role(&self) -> FfiPlanInputRoleClaimV1 {
        self.role
    }

    pub const fn producer(&self) -> &UnauthenticatedProducerClaimV1 {
        &self.producer
    }

    pub const fn claim_origin(&self) -> FfiClaimOriginV1 {
        FfiClaimOriginV1::CallerBindingAssertionOnly
    }
}

/// Exact caller binding of one FFI contract claim to one plan input claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FfiSymbolProviderBindingClaimV1 {
    contract_identity: DeviceFfiContractIdV1,
    declaration_owner_identity: G4DeclarationOwnerClaimIdentityV1,
    provider_input_identity: ContentIdentityV1,
    provider_input_kind: WorkerInputKindV1,
    producer_claim_identity: UnauthenticatedProducerClaimIdentityV1,
}

impl FfiSymbolProviderBindingClaimV1 {
    pub const fn new(
        contract_identity: DeviceFfiContractIdV1,
        declaration_owner_identity: G4DeclarationOwnerClaimIdentityV1,
        provider_input_identity: ContentIdentityV1,
        provider_input_kind: WorkerInputKindV1,
        producer_claim_identity: UnauthenticatedProducerClaimIdentityV1,
    ) -> Self {
        Self {
            contract_identity,
            declaration_owner_identity,
            provider_input_identity,
            provider_input_kind,
            producer_claim_identity,
        }
    }

    pub const fn contract_identity(&self) -> DeviceFfiContractIdV1 {
        self.contract_identity
    }

    pub const fn declaration_owner_identity(&self) -> G4DeclarationOwnerClaimIdentityV1 {
        self.declaration_owner_identity
    }

    pub const fn provider_input_identity(&self) -> ContentIdentityV1 {
        self.provider_input_identity
    }

    pub const fn provider_input_kind(&self) -> WorkerInputKindV1 {
        self.provider_input_kind
    }

    pub const fn producer_claim_identity(&self) -> UnauthenticatedProducerClaimIdentityV1 {
        self.producer_claim_identity
    }

    pub const fn claim_origin(&self) -> FfiClaimOriginV1 {
        FfiClaimOriginV1::CallerBindingAssertionOnly
    }
}

/// Claimed source of complete final-defined-symbol expectations.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum FinalSymbolEvidenceSourceClaimV1 {
    BoundedInputInspection = 1,
    AuthenticatedInputManifest = 2,
}

/// Caller claim that one exact input was covered by symbol evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputSymbolEvidenceCoverageClaimV1 {
    input_identity: ContentIdentityV1,
    input_kind: WorkerInputKindV1,
    source: FinalSymbolEvidenceSourceClaimV1,
    evidence_identity_claim: [u8; 32],
}

impl InputSymbolEvidenceCoverageClaimV1 {
    pub fn new(
        input_identity: ContentIdentityV1,
        input_kind: WorkerInputKindV1,
        source: FinalSymbolEvidenceSourceClaimV1,
        evidence_identity_claim: [u8; 32],
    ) -> Result<Self, StagedFfiLinkError> {
        if evidence_identity_claim == [0; 32] {
            return Err(StagedFfiLinkError::ReservedIdentity(
                "input symbol evidence identity claim",
            ));
        }
        Ok(Self {
            input_identity,
            input_kind,
            source,
            evidence_identity_claim,
        })
    }

    pub const fn input_identity(&self) -> ContentIdentityV1 {
        self.input_identity
    }

    pub const fn input_kind(&self) -> WorkerInputKindV1 {
        self.input_kind
    }

    pub const fn source(&self) -> FinalSymbolEvidenceSourceClaimV1 {
        self.source
    }

    pub const fn evidence_identity_claim(&self) -> &[u8; 32] {
        &self.evidence_identity_claim
    }

    pub const fn claim_origin(&self) -> FfiClaimOriginV1 {
        FfiClaimOriginV1::UnauthenticatedEvidenceClaim
    }
}

/// Identity of complete, but still caller-asserted, final symbol expectations.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExpectedFinalDefinedSymbolsClaimIdentityV1([u8; 32]);

impl ExpectedFinalDefinedSymbolsClaimIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Optional exact final-defined-symbol expectations with all-input coverage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpectedFinalDefinedSymbolsClaimV1 {
    symbols: Vec<String>,
    coverage: Vec<InputSymbolEvidenceCoverageClaimV1>,
    canonical_bytes: Vec<u8>,
    identity: ExpectedFinalDefinedSymbolsClaimIdentityV1,
}

impl ExpectedFinalDefinedSymbolsClaimV1 {
    pub fn new(
        symbols: Vec<String>,
        coverage: Vec<InputSymbolEvidenceCoverageClaimV1>,
    ) -> Result<Self, StagedFfiLinkError> {
        validate_symbols(&symbols).map_err(StagedFfiLinkError::InvalidFinalDefinedSymbols)?;
        if symbols.is_empty() {
            return Err(StagedFfiLinkError::EmptyFinalDefinedSymbols);
        }
        if coverage.is_empty() || coverage.len() > MAX_LINK_INPUTS {
            return Err(StagedFfiLinkError::InvalidSymbolEvidenceCoverageCount);
        }
        validate_coverage_order(&coverage)?;
        let symbol_bytes = aggregate_string_bytes(&symbols)?;
        if symbol_bytes > MAX_G4_FFI_AGGREGATE_TEXT_BYTES_V1 {
            return Err(StagedFfiLinkError::AggregateTextBoundExceeded);
        }
        let canonical_bytes = encode_final_symbols_claim(&symbols, &coverage);
        let identity =
            ExpectedFinalDefinedSymbolsClaimIdentityV1(Sha256::digest(&canonical_bytes).into());
        Ok(Self {
            symbols,
            coverage,
            canonical_bytes,
            identity,
        })
    }

    pub fn symbols(&self) -> &[String] {
        &self.symbols
    }

    pub fn coverage(&self) -> &[InputSymbolEvidenceCoverageClaimV1] {
        &self.coverage
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> ExpectedFinalDefinedSymbolsClaimIdentityV1 {
        self.identity
    }

    pub const fn claim_origin(&self) -> FfiClaimOriginV1 {
        FfiClaimOriginV1::UnauthenticatedEvidenceClaim
    }

    pub const fn is_authenticated(&self) -> bool {
        false
    }
}

/// Identity of the complete opaque staged FFI plan.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StagedFfiLinkPlanIdentityV1([u8; 32]);

impl StagedFfiLinkPlanIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Why a staged FFI plan cannot become a worker V1 request.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StagedFfiExecutionBlockerV1 {
    MissingExpectedFinalDefinedSymbolsClaim,
    WorkerProtocolV1CannotBindCompleteFfiIdentity,
}

/// Non-authoritative summary that contains no generic request-construction inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StagedFfiLinkPlanInspectionV1 {
    input_claim_count: usize,
    provider_binding_claim_count: usize,
    has_expected_final_defined_symbols_claim: bool,
    execution_blocker: StagedFfiExecutionBlockerV1,
}

impl StagedFfiLinkPlanInspectionV1 {
    pub const fn input_claim_count(self) -> usize {
        self.input_claim_count
    }

    pub const fn provider_binding_claim_count(self) -> usize {
        self.provider_binding_claim_count
    }

    pub const fn has_expected_final_defined_symbols_claim(self) -> bool {
        self.has_expected_final_defined_symbols_claim
    }

    pub const fn execution_blocker(self) -> StagedFfiExecutionBlockerV1 {
        self.execution_blocker
    }
}

/// Opaque, assertion-only staging record that cannot construct a worker request.
///
/// Its complete identity is the only provenance-bearing value exposed. The
/// inspection summary contains counts and blocker state only; it cannot be
/// converted into generic symbol or input-kind closures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagedFfiLinkPlanV1 {
    identity: StagedFfiLinkPlanIdentityV1,
    inspection: StagedFfiLinkPlanInspectionV1,
}

impl StagedFfiLinkPlanV1 {
    pub const fn identity(&self) -> StagedFfiLinkPlanIdentityV1 {
        self.identity
    }

    pub const fn inspection(&self) -> StagedFfiLinkPlanInspectionV1 {
        self.inspection
    }
}

/// Stages exact claims over a canonical link plan without creating a request.
pub fn stage_g4_ffi_link_plan_v1(
    plan: &MultiInputLinkPlanV1,
    envelope: &G4FfiClaimEnvelopeV1,
    input_claims: Vec<FfiPlanInputClaimV1>,
    provider_binding_claims: Vec<FfiSymbolProviderBindingClaimV1>,
    final_symbols_claim: Option<ExpectedFinalDefinedSymbolsClaimV1>,
) -> Result<StagedFfiLinkPlanV1, StagedFfiLinkError> {
    if envelope.target != plan.target() {
        return Err(StagedFfiLinkError::PlanTargetMismatch);
    }
    let plan_code_object_version = plan_code_object_version(plan)?;
    if envelope.code_object_version != plan_code_object_version {
        return Err(StagedFfiLinkError::PlanCodeObjectVersionMismatch {
            plan: plan_code_object_version,
            g4_claim: envelope.code_object_version,
        });
    }
    validate_input_claims(plan, envelope, &input_claims)?;
    validate_provider_binding_claims(envelope, &input_claims, &provider_binding_claims)?;
    if let Some(final_symbols) = &final_symbols_claim {
        validate_final_symbols_claim(envelope, &input_claims, final_symbols)?;
    }

    let canonical_bytes = encode_staged_plan(
        plan,
        envelope,
        &input_claims,
        &provider_binding_claims,
        final_symbols_claim.as_ref(),
    );
    if canonical_bytes.len() > MAX_STAGED_FFI_LINK_PLAN_BYTES_V1 {
        return Err(StagedFfiLinkError::StagedPlanByteBoundExceeded);
    }
    let identity = StagedFfiLinkPlanIdentityV1(Sha256::digest(&canonical_bytes).into());
    let has_expected_final_defined_symbols_claim = final_symbols_claim.is_some();
    Ok(StagedFfiLinkPlanV1 {
        identity,
        inspection: StagedFfiLinkPlanInspectionV1 {
            input_claim_count: input_claims.len(),
            provider_binding_claim_count: provider_binding_claims.len(),
            has_expected_final_defined_symbols_claim,
            execution_blocker: if has_expected_final_defined_symbols_claim {
                StagedFfiExecutionBlockerV1::WorkerProtocolV1CannotBindCompleteFfiIdentity
            } else {
                StagedFfiExecutionBlockerV1::MissingExpectedFinalDefinedSymbolsClaim
            },
        },
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StagedFfiLinkError {
    ReservedIdentity(&'static str),
    InvalidText(&'static str),
    InvalidFfiSymbol,
    InvalidPhysicalAbi,
    TooManyPhysicalAbiArguments,
    InvalidEffects,
    EffectAbiMismatch(String),
    ContractIdentityMismatch {
        claimed: DeviceFfiContractIdV1,
        derived: DeviceFfiContractIdV1,
    },
    DirectionProviderClassMismatch {
        symbol: String,
        direction: G4FfiDirectionClaimV1,
        provider_class: G4SymbolProviderClassClaimV1,
    },
    InvalidCompilerRequiredSymbols(WorkerProtocolError),
    MissingCompilerRequiredSymbols,
    TooManyRustDefinitionClaims,
    TooManyKernelClaims,
    TooManyFfiSymbolClaims,
    RustDefinitionCountTooSmall {
        claimed: u32,
        exports: usize,
    },
    NonCanonicalFfiSymbolClaims,
    DuplicateContractClaim(DeviceFfiContractIdV1),
    DuplicateSymbolClaim(String),
    DuplicateDeclarationOwnerClaim(G4DeclarationOwnerClaimIdentityV1),
    MissingCompilerRequiredSymbol(String),
    SymbolTargetMismatch(String),
    SymbolCodeObjectVersionMismatch(String),
    AggregateTextBoundExceeded,
    EnvelopeByteBoundExceeded,
    PlanTargetMismatch,
    MissingPlanCodeObjectVersion,
    InvalidPlanCodeObjectVersion(String),
    PlanCodeObjectVersionMismatch {
        plan: CodeObjectVersion,
        g4_claim: CodeObjectVersion,
    },
    PlanInputClaimCountMismatch {
        plan: usize,
        claims: usize,
    },
    PlanInputClaimSequenceMismatch {
        index: usize,
        plan: ContentIdentityV1,
        claim: ContentIdentityV1,
    },
    MissingCompilerModuleInputClaim,
    UnexpectedCompilerModuleInputClaim,
    MultipleCompilerModuleInputClaims,
    NonCanonicalProviderBindingClaims,
    DuplicateProviderBindingClaim(DeviceFfiContractIdV1),
    ConflictingProviderBindingClaim(DeviceFfiContractIdV1),
    UnreferencedProviderBindingClaim(DeviceFfiContractIdV1),
    MissingProviderBindingClaim(DeviceFfiContractIdV1),
    ProviderDeclarationOwnerMismatch(DeviceFfiContractIdV1),
    ProviderInputAbsent(ContentIdentityV1),
    ProviderInputKindMismatch {
        contract: DeviceFfiContractIdV1,
        binding: WorkerInputKindV1,
        input: WorkerInputKindV1,
    },
    ProviderInputRoleMismatch {
        contract: DeviceFfiContractIdV1,
        provider_class: G4SymbolProviderClassClaimV1,
        input_role: FfiPlanInputRoleClaimV1,
    },
    ProviderProducerClaimMismatch(DeviceFfiContractIdV1),
    UnreferencedExternalProviderInput(ContentIdentityV1),
    InvalidFinalDefinedSymbols(WorkerProtocolError),
    EmptyFinalDefinedSymbols,
    InvalidSymbolEvidenceCoverageCount,
    NonCanonicalSymbolEvidenceCoverage,
    SymbolEvidenceCoverageCountMismatch {
        inputs: usize,
        coverage: usize,
    },
    SymbolEvidenceCoverageMismatch {
        index: usize,
    },
    CompilerRequiredSymbolAbsentFromFinalExpectation(String),
    StagedPlanByteBoundExceeded,
}

impl fmt::Display for StagedFfiLinkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedIdentity(field) => {
                write!(formatter, "{field} uses the reserved zero identity")
            }
            Self::InvalidText(field) => {
                write!(formatter, "{field} is empty, oversized, or noncanonical")
            }
            Self::InvalidFfiSymbol => {
                formatter.write_str("FFI symbol violates the authoritative V1 grammar")
            }
            Self::InvalidPhysicalAbi => {
                formatter.write_str("physical ABI is not in the exact G4 V1 grammar")
            }
            Self::TooManyPhysicalAbiArguments => {
                formatter.write_str("physical ABI argument count exceeds the G4 V1 bound")
            }
            Self::InvalidEffects => {
                formatter.write_str("effect claim is not a canonical G4 V1 effect set")
            }
            Self::EffectAbiMismatch(effect) => write!(
                formatter,
                "effect {effect} has no compatible physical ABI argument"
            ),
            Self::ContractIdentityMismatch { claimed, derived } => write!(
                formatter,
                "contract claim {claimed:?} does not match authoritative derivation {derived:?}"
            ),
            Self::DirectionProviderClassMismatch {
                symbol,
                direction,
                provider_class,
            } => write!(
                formatter,
                "symbol {symbol} direction {direction:?} conflicts with provider class claim {provider_class:?}"
            ),
            Self::InvalidCompilerRequiredSymbols(error) => {
                write!(formatter, "invalid compiler-required symbol claim: {error}")
            }
            Self::MissingCompilerRequiredSymbols => {
                formatter.write_str("nonempty G4 claim envelope has no compiler-required symbols")
            }
            Self::TooManyRustDefinitionClaims => {
                formatter.write_str("Rust definition count exceeds the G4 V1 bound")
            }
            Self::TooManyKernelClaims => {
                formatter.write_str("kernel count exceeds the G4 V1 bound")
            }
            Self::TooManyFfiSymbolClaims => {
                formatter.write_str("FFI symbol count exceeds the G4 V1 bound")
            }
            Self::RustDefinitionCountTooSmall { claimed, exports } => write!(
                formatter,
                "Rust definition count {claimed} is smaller than {exports} FFI exports"
            ),
            Self::NonCanonicalFfiSymbolClaims => {
                formatter.write_str("FFI symbol claims are not in canonical order")
            }
            Self::DuplicateContractClaim(identity) => {
                write!(formatter, "duplicate FFI contract claim {identity:?}")
            }
            Self::DuplicateSymbolClaim(symbol) => {
                write!(formatter, "duplicate FFI symbol claim {symbol}")
            }
            Self::DuplicateDeclarationOwnerClaim(identity) => {
                write!(formatter, "duplicate declaration-owner claim {identity:?}")
            }
            Self::MissingCompilerRequiredSymbol(symbol) => write!(
                formatter,
                "FFI symbol {symbol} is absent from compiler-required symbols"
            ),
            Self::SymbolTargetMismatch(symbol) => write!(
                formatter,
                "FFI symbol {symbol} disagrees with the envelope target"
            ),
            Self::SymbolCodeObjectVersionMismatch(symbol) => write!(
                formatter,
                "FFI symbol {symbol} disagrees with the envelope code-object version"
            ),
            Self::AggregateTextBoundExceeded => {
                formatter.write_str("aggregate FFI claim text exceeds the V1 bound")
            }
            Self::EnvelopeByteBoundExceeded => {
                formatter.write_str("canonical G4 claim envelope exceeds the V1 bound")
            }
            Self::PlanTargetMismatch => {
                formatter.write_str("G4 target claim does not match the link plan")
            }
            Self::MissingPlanCodeObjectVersion => {
                formatter.write_str("link plan has no code-object-version option")
            }
            Self::InvalidPlanCodeObjectVersion(value) => write!(
                formatter,
                "link plan has invalid code-object version {value}"
            ),
            Self::PlanCodeObjectVersionMismatch { plan, g4_claim } => write!(
                formatter,
                "G4 code-object claim {g4_claim:?} does not match plan {plan:?}"
            ),
            Self::PlanInputClaimCountMismatch { plan, claims } => write!(
                formatter,
                "plan has {plan} inputs but received {claims} input claims"
            ),
            Self::PlanInputClaimSequenceMismatch { index, plan, claim } => write!(
                formatter,
                "input claim {index} identity {claim} does not match plan input {plan}"
            ),
            Self::MissingCompilerModuleInputClaim => formatter.write_str(
                "Rust definitions or kernels require one exact compiler-module input claim",
            ),
            Self::UnexpectedCompilerModuleInputClaim => formatter
                .write_str("compiler-module input is claimed without Rust definitions or kernels"),
            Self::MultipleCompilerModuleInputClaims => {
                formatter.write_str("more than one compiler-module input is claimed")
            }
            Self::NonCanonicalProviderBindingClaims => {
                formatter.write_str("provider binding claims are not in canonical contract order")
            }
            Self::DuplicateProviderBindingClaim(identity) => write!(
                formatter,
                "duplicate provider binding claim for {identity:?}"
            ),
            Self::ConflictingProviderBindingClaim(identity) => write!(
                formatter,
                "conflicting provider binding claims for {identity:?}"
            ),
            Self::UnreferencedProviderBindingClaim(identity) => write!(
                formatter,
                "provider binding claim references unknown contract {identity:?}"
            ),
            Self::MissingProviderBindingClaim(identity) => write!(
                formatter,
                "FFI contract {identity:?} has no provider binding claim"
            ),
            Self::ProviderDeclarationOwnerMismatch(identity) => write!(
                formatter,
                "provider binding declaration owner disagrees for {identity:?}"
            ),
            Self::ProviderInputAbsent(identity) => write!(
                formatter,
                "provider input {identity} is absent from the exact plan claims"
            ),
            Self::ProviderInputKindMismatch {
                contract,
                binding,
                input,
            } => write!(
                formatter,
                "provider {contract:?} kind {binding:?} disagrees with input claim {input:?}"
            ),
            Self::ProviderInputRoleMismatch {
                contract,
                provider_class,
                input_role,
            } => write!(
                formatter,
                "provider {contract:?} class {provider_class:?} disagrees with input role {input_role:?}"
            ),
            Self::ProviderProducerClaimMismatch(identity) => write!(
                formatter,
                "provider producer claim disagrees for contract {identity:?}"
            ),
            Self::UnreferencedExternalProviderInput(identity) => write!(
                formatter,
                "external-provider input {identity} is not referenced by a symbol binding"
            ),
            Self::InvalidFinalDefinedSymbols(error) => {
                write!(formatter, "invalid final-defined-symbol claim: {error}")
            }
            Self::EmptyFinalDefinedSymbols => {
                formatter.write_str("final-defined-symbol claim is empty")
            }
            Self::InvalidSymbolEvidenceCoverageCount => formatter
                .write_str("symbol evidence coverage count is outside the plan-input bound"),
            Self::NonCanonicalSymbolEvidenceCoverage => {
                formatter.write_str("symbol evidence coverage is not in canonical input order")
            }
            Self::SymbolEvidenceCoverageCountMismatch { inputs, coverage } => write!(
                formatter,
                "{coverage} symbol evidence records do not cover {inputs} inputs"
            ),
            Self::SymbolEvidenceCoverageMismatch { index } => write!(
                formatter,
                "symbol evidence record {index} does not cover the exact input identity and kind"
            ),
            Self::CompilerRequiredSymbolAbsentFromFinalExpectation(symbol) => write!(
                formatter,
                "compiler-required symbol {symbol} is absent from final-defined-symbol expectations"
            ),
            Self::StagedPlanByteBoundExceeded => {
                formatter.write_str("canonical staged FFI plan exceeds the V1 bound")
            }
        }
    }
}

impl std::error::Error for StagedFfiLinkError {}

fn validate_g4_symbol_claims(
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    compiler_required_symbols: &[String],
    symbols: &[G4FfiSymbolClaimV1],
) -> Result<(), StagedFfiLinkError> {
    let mut contracts = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut owners = BTreeMap::new();
    let mut previous = None;
    for symbol in symbols {
        let key = (
            symbol.symbol.as_str(),
            symbol.contract_identity,
            symbol.declaration_owner.identity,
        );
        if previous.is_some_and(|previous| previous >= key) {
            return Err(StagedFfiLinkError::NonCanonicalFfiSymbolClaims);
        }
        previous = Some(key);
        if contracts.insert(symbol.contract_identity, ()).is_some() {
            return Err(StagedFfiLinkError::DuplicateContractClaim(
                symbol.contract_identity,
            ));
        }
        if names.insert(symbol.symbol.as_str(), ()).is_some() {
            return Err(StagedFfiLinkError::DuplicateSymbolClaim(
                symbol.symbol.clone(),
            ));
        }
        if owners
            .insert(symbol.declaration_owner.identity, ())
            .is_some()
        {
            return Err(StagedFfiLinkError::DuplicateDeclarationOwnerClaim(
                symbol.declaration_owner.identity,
            ));
        }
        if compiler_required_symbols
            .binary_search(&symbol.symbol)
            .is_err()
        {
            return Err(StagedFfiLinkError::MissingCompilerRequiredSymbol(
                symbol.symbol.clone(),
            ));
        }
        if symbol.declared.target != target {
            return Err(StagedFfiLinkError::SymbolTargetMismatch(
                symbol.symbol.clone(),
            ));
        }
        if symbol.declared.code_object_version != code_object_version {
            return Err(StagedFfiLinkError::SymbolCodeObjectVersionMismatch(
                symbol.symbol.clone(),
            ));
        }
    }
    Ok(())
}

fn validate_input_claims(
    plan: &MultiInputLinkPlanV1,
    envelope: &G4FfiClaimEnvelopeV1,
    claims: &[FfiPlanInputClaimV1],
) -> Result<(), StagedFfiLinkError> {
    if claims.len() != plan.inputs().len() {
        return Err(StagedFfiLinkError::PlanInputClaimCountMismatch {
            plan: plan.inputs().len(),
            claims: claims.len(),
        });
    }
    let mut compiler_modules = 0;
    for (index, (planned, claim)) in plan.inputs().iter().zip(claims).enumerate() {
        if planned.identity() != claim.identity {
            return Err(StagedFfiLinkError::PlanInputClaimSequenceMismatch {
                index,
                plan: planned.identity(),
                claim: claim.identity,
            });
        }
        if claim.role == FfiPlanInputRoleClaimV1::CompilerModule {
            compiler_modules += 1;
        }
    }
    if compiler_modules > 1 {
        return Err(StagedFfiLinkError::MultipleCompilerModuleInputClaims);
    }
    match (envelope.requires_compiler_module(), compiler_modules) {
        (true, 0) => Err(StagedFfiLinkError::MissingCompilerModuleInputClaim),
        (false, 1) => Err(StagedFfiLinkError::UnexpectedCompilerModuleInputClaim),
        _ => Ok(()),
    }
}

fn validate_provider_binding_claims(
    envelope: &G4FfiClaimEnvelopeV1,
    input_claims: &[FfiPlanInputClaimV1],
    bindings: &[FfiSymbolProviderBindingClaimV1],
) -> Result<(), StagedFfiLinkError> {
    let symbols: BTreeMap<_, _> = envelope
        .symbols
        .iter()
        .map(|symbol| (symbol.contract_identity, symbol))
        .collect();
    let inputs: BTreeMap<_, _> = input_claims
        .iter()
        .map(|input| (input.identity, input))
        .collect();
    let mut previous: Option<&FfiSymbolProviderBindingClaimV1> = None;
    let mut seen = BTreeMap::new();
    let mut referenced_inputs = BTreeMap::new();
    for binding in bindings {
        if let Some(previous) = previous {
            if previous.contract_identity > binding.contract_identity {
                return Err(StagedFfiLinkError::NonCanonicalProviderBindingClaims);
            }
            if previous.contract_identity == binding.contract_identity {
                return Err(if previous == binding {
                    StagedFfiLinkError::DuplicateProviderBindingClaim(binding.contract_identity)
                } else {
                    StagedFfiLinkError::ConflictingProviderBindingClaim(binding.contract_identity)
                });
            }
        }
        previous = Some(binding);
        let symbol = symbols.get(&binding.contract_identity).ok_or(
            StagedFfiLinkError::UnreferencedProviderBindingClaim(binding.contract_identity),
        )?;
        if symbol.declaration_owner.identity != binding.declaration_owner_identity {
            return Err(StagedFfiLinkError::ProviderDeclarationOwnerMismatch(
                binding.contract_identity,
            ));
        }
        let input = inputs.get(&binding.provider_input_identity).ok_or(
            StagedFfiLinkError::ProviderInputAbsent(binding.provider_input_identity),
        )?;
        if input.kind != binding.provider_input_kind {
            return Err(StagedFfiLinkError::ProviderInputKindMismatch {
                contract: binding.contract_identity,
                binding: binding.provider_input_kind,
                input: input.kind,
            });
        }
        let expected_role = match symbol.provider_class {
            G4SymbolProviderClassClaimV1::ExternalPlanInput => {
                FfiPlanInputRoleClaimV1::ExternalSymbolProvider
            }
            G4SymbolProviderClassClaimV1::CompilerModuleInput => {
                FfiPlanInputRoleClaimV1::CompilerModule
            }
        };
        if input.role != expected_role {
            return Err(StagedFfiLinkError::ProviderInputRoleMismatch {
                contract: binding.contract_identity,
                provider_class: symbol.provider_class,
                input_role: input.role,
            });
        }
        if input.producer.identity != binding.producer_claim_identity {
            return Err(StagedFfiLinkError::ProviderProducerClaimMismatch(
                binding.contract_identity,
            ));
        }
        seen.insert(binding.contract_identity, ());
        referenced_inputs.insert(binding.provider_input_identity, ());
    }
    for symbol in &envelope.symbols {
        if !seen.contains_key(&symbol.contract_identity) {
            return Err(StagedFfiLinkError::MissingProviderBindingClaim(
                symbol.contract_identity,
            ));
        }
    }
    for input in input_claims {
        if input.role == FfiPlanInputRoleClaimV1::ExternalSymbolProvider
            && !referenced_inputs.contains_key(&input.identity)
        {
            return Err(StagedFfiLinkError::UnreferencedExternalProviderInput(
                input.identity,
            ));
        }
    }
    Ok(())
}

fn validate_final_symbols_claim(
    envelope: &G4FfiClaimEnvelopeV1,
    input_claims: &[FfiPlanInputClaimV1],
    final_symbols: &ExpectedFinalDefinedSymbolsClaimV1,
) -> Result<(), StagedFfiLinkError> {
    if final_symbols.coverage.len() != input_claims.len() {
        return Err(StagedFfiLinkError::SymbolEvidenceCoverageCountMismatch {
            inputs: input_claims.len(),
            coverage: final_symbols.coverage.len(),
        });
    }
    for (index, (input, coverage)) in input_claims.iter().zip(&final_symbols.coverage).enumerate() {
        if input.identity != coverage.input_identity || input.kind != coverage.input_kind {
            return Err(StagedFfiLinkError::SymbolEvidenceCoverageMismatch { index });
        }
    }
    for required in &envelope.compiler_required_symbols {
        if final_symbols.symbols.binary_search(required).is_err() {
            return Err(
                StagedFfiLinkError::CompilerRequiredSymbolAbsentFromFinalExpectation(
                    required.clone(),
                ),
            );
        }
    }
    Ok(())
}

fn validate_coverage_order(
    coverage: &[InputSymbolEvidenceCoverageClaimV1],
) -> Result<(), StagedFfiLinkError> {
    for pair in coverage.windows(2) {
        if pair[0].input_identity >= pair[1].input_identity {
            return Err(StagedFfiLinkError::NonCanonicalSymbolEvidenceCoverage);
        }
    }
    Ok(())
}

fn plan_code_object_version(
    plan: &MultiInputLinkPlanV1,
) -> Result<CodeObjectVersion, StagedFfiLinkError> {
    let value = plan
        .options()
        .iter()
        .find(|option| option.name() == "code-object-version")
        .ok_or(StagedFfiLinkError::MissingPlanCodeObjectVersion)?
        .value();
    match value {
        "4" => Ok(CodeObjectVersion::V4),
        "5" => Ok(CodeObjectVersion::V5),
        "6" => Ok(CodeObjectVersion::V6),
        value => Err(StagedFfiLinkError::InvalidPlanCodeObjectVersion(
            value.to_owned(),
        )),
    }
}

fn map_device_ffi_grammar_error(error: DeviceFfiGrammarError) -> StagedFfiLinkError {
    match error {
        DeviceFfiGrammarError::InvalidSymbol => StagedFfiLinkError::InvalidFfiSymbol,
        DeviceFfiGrammarError::InvalidPhysicalAbi => StagedFfiLinkError::InvalidPhysicalAbi,
        DeviceFfiGrammarError::TooManyPhysicalAbiArguments => {
            StagedFfiLinkError::TooManyPhysicalAbiArguments
        }
        DeviceFfiGrammarError::InvalidEffects => StagedFfiLinkError::InvalidEffects,
        DeviceFfiGrammarError::EffectAbiMismatch(effect) => {
            StagedFfiLinkError::EffectAbiMismatch(effect.as_str().to_owned())
        }
        _ => StagedFfiLinkError::InvalidPhysicalAbi,
    }
}

fn validate_text(
    field: &'static str,
    text: &str,
    max_bytes: usize,
    ascii: bool,
) -> Result<(), StagedFfiLinkError> {
    if text.is_empty()
        || text.len() > max_bytes
        || (ascii && !text.is_ascii())
        || text.chars().any(char::is_control)
    {
        return Err(StagedFfiLinkError::InvalidText(field));
    }
    Ok(())
}

fn validate_ascii_token(
    field: &'static str,
    text: &str,
    max_bytes: usize,
) -> Result<(), StagedFfiLinkError> {
    validate_text(field, text, max_bytes, true)?;
    if text.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return Err(StagedFfiLinkError::InvalidText(field));
    }
    Ok(())
}

fn envelope_text_bytes(
    compiler_required_symbols: &[String],
    symbols: &[G4FfiSymbolClaimV1],
) -> Result<usize, StagedFfiLinkError> {
    let mut total = aggregate_string_bytes(compiler_required_symbols)?;
    for symbol in symbols {
        for text in [
            symbol.symbol.as_str(),
            symbol.physical_abi.as_str(),
            symbol.declaration_owner.crate_label.as_str(),
            symbol.declaration_owner.item_label.as_str(),
            symbol.declaration_owner.concrete_instance_symbol.as_str(),
            symbol.declared.effects.as_str(),
        ] {
            total = total
                .checked_add(text.len())
                .ok_or(StagedFfiLinkError::AggregateTextBoundExceeded)?;
        }
    }
    Ok(total)
}

fn aggregate_string_bytes(strings: &[String]) -> Result<usize, StagedFfiLinkError> {
    strings.iter().try_fold(0_usize, |total, text| {
        total
            .checked_add(text.len())
            .ok_or(StagedFfiLinkError::AggregateTextBoundExceeded)
    })
}

fn encode_g4_envelope(
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    compiler_required_symbols: &[String],
    rust_definition_count: u32,
    kernel_count: u32,
    symbols: &[G4FfiSymbolClaimV1],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(G4_CLAIM_ENVELOPE_DOMAIN_V1);
    push_claim_origin(&mut bytes, FfiClaimOriginV1::G4AssertionOnly);
    push_text(&mut bytes, &target.to_string());
    bytes.push(code_object_version_byte(code_object_version));
    bytes.extend_from_slice(&rust_definition_count.to_le_bytes());
    bytes.extend_from_slice(&kernel_count.to_le_bytes());
    push_strings(&mut bytes, compiler_required_symbols);
    push_u32(&mut bytes, symbols.len());
    for symbol in symbols {
        encode_g4_symbol_claim(&mut bytes, symbol);
    }
    bytes
}

fn encode_g4_symbol_claim(bytes: &mut Vec<u8>, symbol: &G4FfiSymbolClaimV1) {
    push_claim_origin(bytes, FfiClaimOriginV1::G4AssertionOnly);
    bytes.extend_from_slice(&symbol.contract_identity.as_bytes());
    bytes.push(symbol.direction as u8);
    push_text(bytes, &symbol.symbol);
    push_text(bytes, &symbol.physical_abi);
    bytes.extend_from_slice(symbol.declaration_owner.identity.as_bytes());
    push_text(bytes, &symbol.declaration_owner.crate_label);
    push_text(bytes, &symbol.declaration_owner.item_label);
    bytes.extend_from_slice(&symbol.declaration_owner.def_path_hash);
    push_text(bytes, &symbol.declaration_owner.concrete_instance_symbol);
    bytes.push(symbol.provider_class as u8);
    push_text(bytes, &symbol.declared.target.to_string());
    bytes.push(code_object_version_byte(
        symbol.declared.code_object_version,
    ));
    push_text(bytes, &symbol.declared.effects);
    bytes.extend_from_slice(&symbol.declared.semantic_identity);
}

fn encode_final_symbols_claim(
    symbols: &[String],
    coverage: &[InputSymbolEvidenceCoverageClaimV1],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(FINAL_SYMBOLS_CLAIM_DOMAIN_V1);
    push_claim_origin(&mut bytes, FfiClaimOriginV1::UnauthenticatedEvidenceClaim);
    push_strings(&mut bytes, symbols);
    push_u32(&mut bytes, coverage.len());
    for record in coverage {
        push_content_identity(&mut bytes, record.input_identity);
        bytes.push(record.input_kind as u8);
        bytes.push(record.source as u8);
        bytes.extend_from_slice(&record.evidence_identity_claim);
    }
    bytes
}

fn encode_staged_plan(
    plan: &MultiInputLinkPlanV1,
    envelope: &G4FfiClaimEnvelopeV1,
    input_claims: &[FfiPlanInputClaimV1],
    provider_binding_claims: &[FfiSymbolProviderBindingClaimV1],
    final_symbols_claim: Option<&ExpectedFinalDefinedSymbolsClaimV1>,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(STAGED_FFI_LINK_PLAN_DOMAIN_V1);
    bytes.extend_from_slice(plan.identity().as_bytes());
    bytes.extend_from_slice(envelope.identity.as_bytes());
    push_claim_origin(&mut bytes, FfiClaimOriginV1::CallerBindingAssertionOnly);
    push_u32(&mut bytes, input_claims.len());
    for input in input_claims {
        push_content_identity(&mut bytes, input.identity);
        bytes.push(input.kind as u8);
        bytes.push(input.role as u8);
        encode_producer_claim(&mut bytes, &input.producer);
    }
    push_u32(&mut bytes, provider_binding_claims.len());
    for binding in provider_binding_claims {
        bytes.extend_from_slice(&binding.contract_identity.as_bytes());
        bytes.extend_from_slice(binding.declaration_owner_identity.as_bytes());
        push_content_identity(&mut bytes, binding.provider_input_identity);
        bytes.push(binding.provider_input_kind as u8);
        bytes.extend_from_slice(binding.producer_claim_identity.as_bytes());
    }
    match final_symbols_claim {
        Some(claim) => {
            bytes.push(1);
            bytes.extend_from_slice(claim.identity.as_bytes());
        }
        None => bytes.push(0),
    }
    bytes
}

fn encode_producer_claim(bytes: &mut Vec<u8>, producer: &UnauthenticatedProducerClaimV1) {
    push_claim_origin(bytes, FfiClaimOriginV1::UnauthenticatedProducerClaim);
    bytes.extend_from_slice(producer.identity.as_bytes());
    push_text(bytes, &producer.name);
    push_text(bytes, &producer.version);
    bytes.extend_from_slice(&producer.build_identity_claim);
}

fn push_claim_origin(bytes: &mut Vec<u8>, origin: FfiClaimOriginV1) {
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

const fn direction_tag(direction: G4FfiDirectionClaimV1) -> u16 {
    match direction {
        G4FfiDirectionClaimV1::Import => DEVICE_FFI_DIRECTION_IMPORT_V1,
        G4FfiDirectionClaimV1::Export => DEVICE_FFI_DIRECTION_EXPORT_V1,
    }
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
