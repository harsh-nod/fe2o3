use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, str,
};

use reserved_fe2o3_symbols::{
    DeviceFfiAddressSpaceV1, DeviceFfiPhysicalResultV1, DeviceFfiPhysicalTypeV1,
    parse_device_ffi_physical_abi_v1, validate_device_ffi_contract_grammar_v1,
};
use sha2::{Digest, Sha256};

use super::{CodeObjectVersion, DeviceTargetV1, MAX_DEVICE_FFI_TARGET_BYTES_V1};

const MANIFEST_DOMAIN_V1: &[u8] = b"FE2O3/EXTERNAL-DEVICE-LIBRARY-MANIFEST/V1\0";

/// Reviewed target triple for V1 device-function libraries.
pub const EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1: &str = "amdgcn-amd-amdhsa";
/// Target-machine data layout reviewed for the gfx942 V1 lane.
pub const EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1: &str =
    fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM_DATA_LAYOUT_V1;

pub const MAX_EXTERNAL_DEVICE_LIBRARY_SYMBOLS_V1: usize = 1_024;
pub const MAX_EXTERNAL_DEVICE_LIBRARY_DEPENDENCIES_V1: usize = 256;
pub const MAX_EXTERNAL_DEVICE_LIBRARY_CAPABILITIES_V1: usize = 64;
pub const MAX_EXTERNAL_DEVICE_LIBRARY_LLVM_TOKEN_BYTES_V1: usize = 256;
pub const MAX_EXTERNAL_DEVICE_LIBRARY_LLVM_DATA_LAYOUT_BYTES_V1: usize = 2_048;
pub const MAX_EXTERNAL_DEVICE_LIBRARY_MANIFEST_BYTES_V1: usize = 8 * 1024 * 1024;

/// Exact SHA-256 and byte length of a retained blob.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeviceBlobIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ExternalDeviceBlobIdentityV1 {
    pub fn new(
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        if byte_len == 0 {
            return Err(ExternalDeviceLibraryManifestErrorV1::EmptyBlob);
        }
        require_identity(sha256)?;
        Ok(Self { sha256, byte_len })
    }

    pub fn calculate(bytes: &[u8]) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        let byte_len = u64::try_from(bytes.len())
            .map_err(|_| ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded)?;
        Self::new(Sha256::digest(bytes).into(), byte_len)
    }

    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn matches(self, bytes: &[u8]) -> bool {
        self.byte_len == bytes.len() as u64
            && self.sha256 == <[u8; 32]>::from(Sha256::digest(bytes))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ExternalDeviceLibraryContentKindV1 {
    LlvmBitcode = 1,
    RelocatableObject = 2,
    CodeObject = 3,
    StaticArchive = 4,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeviceLibraryContentIdentityV1 {
    kind: ExternalDeviceLibraryContentKindV1,
    blob: ExternalDeviceBlobIdentityV1,
}

impl ExternalDeviceLibraryContentIdentityV1 {
    pub const fn new(
        kind: ExternalDeviceLibraryContentKindV1,
        blob: ExternalDeviceBlobIdentityV1,
    ) -> Self {
        Self { kind, blob }
    }

    pub const fn kind(self) -> ExternalDeviceLibraryContentKindV1 {
        self.kind
    }

    pub const fn blob(self) -> ExternalDeviceBlobIdentityV1 {
        self.blob
    }

    pub fn matches(self, bytes: &[u8]) -> bool {
        self.blob.matches(bytes)
    }
}

/// Exact LLVM producer identity required to interpret and link a library.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeviceLlvmIdentityV1 {
    major: u16,
    version: String,
    commit: [u8; 20],
    executable: ExternalDeviceBlobIdentityV1,
    target_triple: String,
    data_layout: String,
}

impl ExternalDeviceLlvmIdentityV1 {
    pub fn new(
        major: u16,
        version: impl Into<String>,
        commit: [u8; 20],
        executable: ExternalDeviceBlobIdentityV1,
        target_triple: impl Into<String>,
        data_layout: impl Into<String>,
    ) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        if major == 0 {
            return Err(ExternalDeviceLibraryManifestErrorV1::InvalidLlvmIdentity);
        }
        let version = version.into();
        let target_triple = target_triple.into();
        let data_layout = data_layout.into();
        validate_ascii_token(&version, MAX_EXTERNAL_DEVICE_LIBRARY_LLVM_TOKEN_BYTES_V1)?;
        let parsed_major = parse_llvm_major(&version)?;
        if parsed_major != major {
            return Err(ExternalDeviceLibraryManifestErrorV1::LlvmVersionMajorMismatch);
        }
        validate_ascii_token(
            &target_triple,
            MAX_EXTERNAL_DEVICE_LIBRARY_LLVM_TOKEN_BYTES_V1,
        )?;
        validate_ascii_text(
            &data_layout,
            MAX_EXTERNAL_DEVICE_LIBRARY_LLVM_DATA_LAYOUT_BYTES_V1,
        )?;
        if target_triple != EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1 {
            return Err(ExternalDeviceLibraryManifestErrorV1::InvalidLlvmTargetTriple);
        }
        if data_layout != EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1 {
            return Err(ExternalDeviceLibraryManifestErrorV1::InvalidLlvmDataLayout);
        }
        Ok(Self {
            major,
            version,
            commit,
            executable,
            target_triple,
            data_layout,
        })
    }

    pub const fn major(&self) -> u16 {
        self.major
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub const fn commit(&self) -> &[u8; 20] {
        &self.commit
    }

    pub const fn executable(&self) -> ExternalDeviceBlobIdentityV1 {
        self.executable
    }

    pub fn target_triple(&self) -> &str {
        &self.target_triple
    }

    pub fn data_layout(&self) -> &str {
        &self.data_layout
    }

    /// Compares the declared LLVM profile fields used by structural provider-set validation.
    ///
    /// Exact producer version, commit, and executable identities remain bound in each manifest,
    /// but patch builds of the same LLVM major may share a profile. This comparison does not
    /// inspect content, invoke LLVM, admit linker input, or grant link authority.
    pub fn has_compatible_profile_with(&self, other: &Self) -> bool {
        self.major == other.major
            && self.target_triple == other.target_triple
            && self.data_layout == other.data_layout
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ExternalDeviceLibraryProvenanceKindV1 {
    SourceBuild = 1,
    VendorSdk = 2,
    PrebuiltBinary = 3,
}

/// Producer-namespaced provenance claim. Construction does not authenticate it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeviceLibraryProvenanceV1 {
    kind: ExternalDeviceLibraryProvenanceKindV1,
    identity: [u8; 32],
}

impl ExternalDeviceLibraryProvenanceV1 {
    pub fn new(
        kind: ExternalDeviceLibraryProvenanceKindV1,
        identity: [u8; 32],
    ) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        require_identity(identity)?;
        Ok(Self { kind, identity })
    }

    pub const fn kind(self) -> ExternalDeviceLibraryProvenanceKindV1 {
        self.kind
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ExternalDeviceLibraryTrustClassV1 {
    Unverified = 1,
    DeclaredSpecification = 2,
    ExternalAttestation = 3,
}

/// Claimed trust class and exact external evidence identity, when present.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeviceLibraryTrustV1 {
    class: ExternalDeviceLibraryTrustClassV1,
    evidence_identity: Option<[u8; 32]>,
}

impl ExternalDeviceLibraryTrustV1 {
    pub const fn unverified() -> Self {
        Self {
            class: ExternalDeviceLibraryTrustClassV1::Unverified,
            evidence_identity: None,
        }
    }

    pub fn declared_specification(
        identity: [u8; 32],
    ) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        Self::with_evidence(
            ExternalDeviceLibraryTrustClassV1::DeclaredSpecification,
            identity,
        )
    }

    pub fn external_attestation(
        identity: [u8; 32],
    ) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        Self::with_evidence(
            ExternalDeviceLibraryTrustClassV1::ExternalAttestation,
            identity,
        )
    }

    fn with_evidence(
        class: ExternalDeviceLibraryTrustClassV1,
        identity: [u8; 32],
    ) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        require_identity(identity)?;
        Ok(Self {
            class,
            evidence_identity: Some(identity),
        })
    }

    pub const fn class(self) -> ExternalDeviceLibraryTrustClassV1 {
        self.class
    }

    pub const fn evidence_identity(&self) -> Option<&[u8; 32]> {
        self.evidence_identity.as_ref()
    }

    pub const fn authenticates_evidence(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeviceSemanticIdentityV1([u8; 32]);

impl ExternalDeviceSemanticIdentityV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        require_identity(bytes)?;
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeviceCapabilityIdentityV1([u8; 32]);

impl ExternalDeviceCapabilityIdentityV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        require_identity(bytes)?;
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ExternalDeviceSymbolRoleV1 {
    DeviceFunctionImport = 1,
    DeviceFunctionExport = 2,
}

impl ExternalDeviceSymbolRoleV1 {
    pub const fn is_import(self) -> bool {
        matches!(self, Self::DeviceFunctionImport)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ExternalDeviceCallingConventionV1 {
    C = 1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ExternalDeviceConvergenceV1 {
    Unconstrained = 1,
    Convergent = 2,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ExternalDeviceAddressSpaceV1 {
    Constant = 1,
    Global = 2,
    Private = 3,
    Workgroup = 4,
}

/// Canonical C ABI import or export contract.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeviceSymbolV1 {
    role: ExternalDeviceSymbolRoleV1,
    calling_convention: ExternalDeviceCallingConventionV1,
    convergence: ExternalDeviceConvergenceV1,
    symbol: String,
    physical_abi: String,
    address_spaces: Vec<ExternalDeviceAddressSpaceV1>,
    effects: String,
    semantic_identity: ExternalDeviceSemanticIdentityV1,
    required_capabilities: Vec<ExternalDeviceCapabilityIdentityV1>,
}

impl ExternalDeviceSymbolV1 {
    pub fn new(
        role: ExternalDeviceSymbolRoleV1,
        symbol: impl Into<String>,
        physical_abi: impl Into<String>,
        effects: impl Into<String>,
        convergence: ExternalDeviceConvergenceV1,
        semantic_identity: ExternalDeviceSemanticIdentityV1,
        required_capabilities: Vec<ExternalDeviceCapabilityIdentityV1>,
    ) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        if required_capabilities.len() > MAX_EXTERNAL_DEVICE_LIBRARY_CAPABILITIES_V1 {
            return Err(ExternalDeviceLibraryManifestErrorV1::TooManyCapabilities);
        }
        require_strict_order(
            &required_capabilities,
            ExternalDeviceLibraryManifestErrorV1::NonCanonicalCapabilityOrder,
        )?;
        let symbol = symbol.into();
        let physical_abi = physical_abi.into();
        let effects = effects.into();
        validate_device_ffi_contract_grammar_v1(&symbol, &physical_abi, &effects)
            .map_err(ExternalDeviceLibraryManifestErrorV1::FfiGrammar)?;
        if effects
            .split(',')
            .any(|effect| effect == "barrier_workgroup")
            && convergence != ExternalDeviceConvergenceV1::Convergent
        {
            return Err(ExternalDeviceLibraryManifestErrorV1::BarrierRequiresConvergence);
        }
        let address_spaces = address_spaces(&physical_abi)?;
        Ok(Self {
            role,
            calling_convention: ExternalDeviceCallingConventionV1::C,
            convergence,
            symbol,
            physical_abi,
            address_spaces,
            effects,
            semantic_identity,
            required_capabilities,
        })
    }

    pub const fn role(&self) -> ExternalDeviceSymbolRoleV1 {
        self.role
    }
    pub const fn calling_convention(&self) -> ExternalDeviceCallingConventionV1 {
        self.calling_convention
    }
    pub const fn convergence(&self) -> ExternalDeviceConvergenceV1 {
        self.convergence
    }
    pub fn symbol(&self) -> &str {
        &self.symbol
    }
    pub fn physical_abi(&self) -> &str {
        &self.physical_abi
    }
    pub fn address_spaces(&self) -> &[ExternalDeviceAddressSpaceV1] {
        &self.address_spaces
    }
    pub fn effects(&self) -> &str {
        &self.effects
    }
    pub const fn semantic_identity(&self) -> ExternalDeviceSemanticIdentityV1 {
        self.semantic_identity
    }
    pub fn required_capabilities(&self) -> &[ExternalDeviceCapabilityIdentityV1] {
        &self.required_capabilities
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeviceLibraryManifestIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ExternalDeviceLibraryManifestIdentityV1 {
    pub fn new(
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        if byte_len == 0 {
            return Err(ExternalDeviceLibraryManifestErrorV1::EmptyBlob);
        }
        require_identity(sha256)?;
        Ok(Self { sha256, byte_len })
    }

    fn calculate(bytes: &[u8]) -> Self {
        Self {
            sha256: Sha256::digest(bytes).into(),
            byte_len: bytes.len() as u64,
        }
    }

    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
    pub fn matches(self, bytes: &[u8]) -> bool {
        self.byte_len == bytes.len() as u64
            && self.sha256 == <[u8; 32]>::from(Sha256::digest(bytes))
    }
}

/// Exact provider manifest and content assigned to a canonical subset of imports.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExternalDeviceLibraryDependencyV1 {
    manifest_identity: ExternalDeviceLibraryManifestIdentityV1,
    content_identity: ExternalDeviceLibraryContentIdentityV1,
    resolved_imports: Vec<String>,
}

impl ExternalDeviceLibraryDependencyV1 {
    pub fn new(
        manifest_identity: ExternalDeviceLibraryManifestIdentityV1,
        content_identity: ExternalDeviceLibraryContentIdentityV1,
        resolved_imports: Vec<String>,
    ) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        if resolved_imports.len() > MAX_EXTERNAL_DEVICE_LIBRARY_SYMBOLS_V1 {
            return Err(ExternalDeviceLibraryManifestErrorV1::TooManyResolvedImports);
        }
        for symbol in &resolved_imports {
            reserved_fe2o3_symbols::validate_device_ffi_symbol_v1(symbol)
                .map_err(ExternalDeviceLibraryManifestErrorV1::FfiGrammar)?;
        }
        require_strict_order(
            &resolved_imports,
            ExternalDeviceLibraryManifestErrorV1::NonCanonicalResolvedImportOrder,
        )?;
        Ok(Self {
            manifest_identity,
            content_identity,
            resolved_imports,
        })
    }

    pub const fn manifest_identity(&self) -> ExternalDeviceLibraryManifestIdentityV1 {
        self.manifest_identity
    }
    pub const fn content_identity(&self) -> ExternalDeviceLibraryContentIdentityV1 {
        self.content_identity
    }
    pub fn resolved_imports(&self) -> impl Clone + ExactSizeIterator<Item = &str> {
        self.resolved_imports.iter().map(String::as_str)
    }
}

/// Bounded canonical external device library contract.
///
/// The format contains no path, search directory, linker flag, or implicit provider. Public
/// construction proves only structural consistency and exact byte identity; it authenticates no
/// provenance, trust evidence, compilation, verification, linking, loading, or execution.
#[derive(Clone, Eq, PartialEq)]
pub struct ExternalDeviceLibraryManifestV1 {
    content: ExternalDeviceLibraryContentIdentityV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    llvm: ExternalDeviceLlvmIdentityV1,
    provenance: ExternalDeviceLibraryProvenanceV1,
    trust: ExternalDeviceLibraryTrustV1,
    symbols: Vec<ExternalDeviceSymbolV1>,
    dependencies: Vec<ExternalDeviceLibraryDependencyV1>,
    canonical_bytes: Vec<u8>,
    identity: ExternalDeviceLibraryManifestIdentityV1,
}

impl fmt::Debug for ExternalDeviceLibraryManifestV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExternalDeviceLibraryManifestV1")
            .field("content", &self.content)
            .field("target", &self.target)
            .field("code_object_version", &self.code_object_version)
            .field("symbol_count", &self.symbols.len())
            .field("dependency_count", &self.dependencies.len())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl ExternalDeviceLibraryManifestV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        content: ExternalDeviceLibraryContentIdentityV1,
        target: DeviceTargetV1,
        code_object_version: CodeObjectVersion,
        llvm: ExternalDeviceLlvmIdentityV1,
        provenance: ExternalDeviceLibraryProvenanceV1,
        trust: ExternalDeviceLibraryTrustV1,
        symbols: Vec<ExternalDeviceSymbolV1>,
        dependencies: Vec<ExternalDeviceLibraryDependencyV1>,
    ) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        if symbols.is_empty() {
            return Err(ExternalDeviceLibraryManifestErrorV1::EmptySymbolSet);
        }
        if symbols.len() > MAX_EXTERNAL_DEVICE_LIBRARY_SYMBOLS_V1 {
            return Err(ExternalDeviceLibraryManifestErrorV1::TooManySymbols);
        }
        if dependencies.len() > MAX_EXTERNAL_DEVICE_LIBRARY_DEPENDENCIES_V1 {
            return Err(ExternalDeviceLibraryManifestErrorV1::TooManyDependencies);
        }
        validate_reviewed_profile(content, target, code_object_version, &llvm)?;
        validate_symbol_order(&symbols)?;
        validate_dependency_order(&dependencies)?;
        validate_symbol_uniqueness(&symbols)?;
        validate_dependency_closure(content, &symbols, &dependencies)?;

        let target_text = target.to_string();
        if target_text.is_empty() || target_text.len() > MAX_DEVICE_FFI_TARGET_BYTES_V1 {
            return Err(ExternalDeviceLibraryManifestErrorV1::InvalidTarget);
        }
        let exact_size = encoded_size(&target_text, &llvm, trust, &symbols, &dependencies)?;
        if exact_size > MAX_EXTERNAL_DEVICE_LIBRARY_MANIFEST_BYTES_V1 {
            return Err(ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded);
        }

        let mut canonical_bytes = Vec::with_capacity(exact_size);
        canonical_bytes.extend_from_slice(MANIFEST_DOMAIN_V1);
        encode_content(&mut canonical_bytes, content);
        push_text(&mut canonical_bytes, &target_text)?;
        canonical_bytes.push(code_object_version_tag(code_object_version));
        encode_llvm(&mut canonical_bytes, &llvm)?;
        canonical_bytes.push(provenance.kind as u8);
        canonical_bytes.extend_from_slice(&provenance.identity);
        encode_trust(&mut canonical_bytes, trust);
        push_u32(&mut canonical_bytes, symbols.len())?;
        for symbol in &symbols {
            encode_symbol(&mut canonical_bytes, symbol)?;
        }
        push_u32(&mut canonical_bytes, dependencies.len())?;
        for dependency in &dependencies {
            encode_dependency(&mut canonical_bytes, dependency)?;
        }
        debug_assert_eq!(canonical_bytes.len(), exact_size);
        let identity = ExternalDeviceLibraryManifestIdentityV1::calculate(&canonical_bytes);

        Ok(Self {
            content,
            target,
            code_object_version,
            llvm,
            provenance,
            trust,
            symbols,
            dependencies,
            canonical_bytes,
            identity,
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        if bytes.len() > MAX_EXTERNAL_DEVICE_LIBRARY_MANIFEST_BYTES_V1 {
            return Err(ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded);
        }
        let mut cursor = Cursor::new(bytes);
        if cursor.take(MANIFEST_DOMAIN_V1.len())? != MANIFEST_DOMAIN_V1 {
            return Err(ExternalDeviceLibraryManifestErrorV1::InvalidMagic);
        }
        let content = decode_content(&mut cursor)?;
        let target_text = cursor.text(MAX_DEVICE_FFI_TARGET_BYTES_V1)?;
        let target = DeviceTargetV1::parse(target_text)
            .map_err(|_| ExternalDeviceLibraryManifestErrorV1::InvalidTarget)?;
        let code_object_version = decode_code_object_version(cursor.byte()?)?;
        let llvm = decode_llvm(&mut cursor)?;
        let provenance = ExternalDeviceLibraryProvenanceV1::new(
            decode_provenance_kind(cursor.byte()?)?,
            cursor.fixed::<32>()?,
        )?;
        let trust = decode_trust(&mut cursor)?;

        let symbol_count = cursor.count(MAX_EXTERNAL_DEVICE_LIBRARY_SYMBOLS_V1)?;
        if symbol_count == 0 {
            return Err(ExternalDeviceLibraryManifestErrorV1::EmptySymbolSet);
        }
        let mut symbols = Vec::with_capacity(symbol_count);
        for _ in 0..symbol_count {
            symbols.push(decode_symbol(&mut cursor)?);
        }
        let dependency_count = cursor.count(MAX_EXTERNAL_DEVICE_LIBRARY_DEPENDENCIES_V1)?;
        let mut dependencies = Vec::with_capacity(dependency_count);
        for _ in 0..dependency_count {
            dependencies.push(decode_dependency(&mut cursor)?);
        }
        cursor.finish()?;

        let decoded = Self::new(
            content,
            target,
            code_object_version,
            llvm,
            provenance,
            trust,
            symbols,
            dependencies,
        )?;
        if decoded.canonical_bytes() != bytes {
            return Err(ExternalDeviceLibraryManifestErrorV1::NonCanonicalEncoding);
        }
        Ok(decoded)
    }

    pub fn decode_for(
        expected: ExternalDeviceLibraryManifestIdentityV1,
        bytes: &[u8],
    ) -> Result<Self, ExternalDeviceLibraryManifestErrorV1> {
        if bytes.len() > MAX_EXTERNAL_DEVICE_LIBRARY_MANIFEST_BYTES_V1 {
            return Err(ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded);
        }
        if !expected.matches(bytes) {
            return Err(ExternalDeviceLibraryManifestErrorV1::ManifestIdentityMismatch);
        }
        Self::decode(bytes)
    }

    pub const fn content(&self) -> ExternalDeviceLibraryContentIdentityV1 {
        self.content
    }
    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }
    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.code_object_version
    }
    pub const fn llvm(&self) -> &ExternalDeviceLlvmIdentityV1 {
        &self.llvm
    }
    pub const fn provenance(&self) -> ExternalDeviceLibraryProvenanceV1 {
        self.provenance
    }
    pub const fn trust(&self) -> ExternalDeviceLibraryTrustV1 {
        self.trust
    }
    pub fn symbols(&self) -> &[ExternalDeviceSymbolV1] {
        &self.symbols
    }
    pub fn dependencies(&self) -> &[ExternalDeviceLibraryDependencyV1] {
        &self.dependencies
    }
    pub fn imports(&self) -> impl Clone + Iterator<Item = &ExternalDeviceSymbolV1> {
        self.symbols.iter().filter(|symbol| symbol.role.is_import())
    }
    pub fn exports(&self) -> impl Clone + Iterator<Item = &ExternalDeviceSymbolV1> {
        self.symbols
            .iter()
            .filter(|symbol| !symbol.role.is_import())
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    pub const fn identity(&self) -> ExternalDeviceLibraryManifestIdentityV1 {
        self.identity
    }
    pub const fn authenticates_provenance(&self) -> bool {
        false
    }
    pub const fn authenticates_verification(&self) -> bool {
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

impl<'a> TryFrom<&'a [u8]> for ExternalDeviceLibraryManifestV1 {
    type Error = ExternalDeviceLibraryManifestErrorV1;

    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        Self::decode(bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExternalDeviceLibraryManifestErrorV1 {
    EmptyBlob,
    MissingIdentity,
    InvalidText,
    InvalidLlvmIdentity,
    LlvmVersionMajorMismatch,
    InvalidLlvmTargetTriple,
    InvalidLlvmDataLayout,
    InvalidTarget,
    UnsupportedTargetProfile,
    UnsupportedContentCodeObjectCombination,
    FfiGrammar(reserved_fe2o3_symbols::DeviceFfiGrammarError),
    BarrierRequiresConvergence,
    EmptySymbolSet,
    TooManySymbols,
    TooManyDependencies,
    TooManyCapabilities,
    TooManyResolvedImports,
    NonCanonicalSymbolOrder,
    NonCanonicalCapabilityOrder,
    NonCanonicalDependencyOrder,
    NonCanonicalResolvedImportOrder,
    DuplicateSymbol,
    DuplicateSemanticIdentity,
    DuplicateDependencyManifest,
    DuplicateDependencyContent,
    SelfDependencyContent,
    UnexpectedResolvedImport,
    DuplicateResolvedImport,
    MissingResolvedImport,
    AddressSpaceMismatch,
    ManifestByteBoundExceeded,
    InvalidMagic,
    InvalidTag,
    InvalidUtf8,
    Truncated,
    TrailingBytes,
    NonCanonicalEncoding,
    ManifestIdentityMismatch,
}

impl fmt::Display for ExternalDeviceLibraryManifestErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBlob => formatter.write_str("external device blob is empty"),
            Self::MissingIdentity => formatter.write_str("required external identity is absent"),
            Self::InvalidText => formatter.write_str("invalid external device manifest text"),
            Self::InvalidLlvmIdentity => formatter.write_str("invalid LLVM identity"),
            Self::LlvmVersionMajorMismatch => {
                formatter.write_str("LLVM version spelling disagrees with its declared major")
            }
            Self::InvalidLlvmTargetTriple => {
                formatter.write_str("LLVM target triple is not the reviewed AMDHSA triple")
            }
            Self::InvalidLlvmDataLayout => {
                formatter.write_str("LLVM data layout is not reviewed for the target")
            }
            Self::InvalidTarget => formatter.write_str("invalid device target"),
            Self::UnsupportedTargetProfile => {
                formatter.write_str("external device target profile is not reviewed")
            }
            Self::UnsupportedContentCodeObjectCombination => formatter
                .write_str("external content kind and code-object version are not reviewed"),
            Self::FfiGrammar(error) => write!(formatter, "invalid device FFI contract: {error}"),
            Self::BarrierRequiresConvergence => {
                formatter.write_str("workgroup barrier effects require a convergent contract")
            }
            Self::EmptySymbolSet => formatter.write_str("external device symbol set is empty"),
            Self::TooManySymbols => formatter.write_str("too many external device symbols"),
            Self::TooManyDependencies => formatter.write_str("too many external dependencies"),
            Self::TooManyCapabilities => formatter.write_str("too many required capabilities"),
            Self::TooManyResolvedImports => formatter.write_str("too many resolved imports"),
            Self::NonCanonicalSymbolOrder => {
                formatter.write_str("external symbols are not in canonical order")
            }
            Self::NonCanonicalCapabilityOrder => {
                formatter.write_str("required capabilities are not in canonical order")
            }
            Self::NonCanonicalDependencyOrder => {
                formatter.write_str("external dependencies are not in canonical order")
            }
            Self::NonCanonicalResolvedImportOrder => {
                formatter.write_str("resolved imports are not in canonical order")
            }
            Self::DuplicateSymbol => formatter.write_str("duplicate external symbol"),
            Self::DuplicateSemanticIdentity => {
                formatter.write_str("duplicate external semantic identity")
            }
            Self::DuplicateDependencyManifest => {
                formatter.write_str("duplicate dependency manifest identity")
            }
            Self::DuplicateDependencyContent => {
                formatter.write_str("duplicate dependency content identity")
            }
            Self::SelfDependencyContent => {
                formatter.write_str("external library depends on its own content")
            }
            Self::UnexpectedResolvedImport => {
                formatter.write_str("dependency resolves an undeclared import")
            }
            Self::DuplicateResolvedImport => {
                formatter.write_str("an import has multiple dependency providers")
            }
            Self::MissingResolvedImport => {
                formatter.write_str("an import has no exact dependency provider")
            }
            Self::AddressSpaceMismatch => {
                formatter.write_str("declared address spaces disagree with the physical ABI")
            }
            Self::ManifestByteBoundExceeded => {
                formatter.write_str("external device manifest byte bound exceeded")
            }
            Self::InvalidMagic => formatter.write_str("invalid external device manifest magic"),
            Self::InvalidTag => formatter.write_str("invalid external device manifest tag"),
            Self::InvalidUtf8 => formatter.write_str("invalid external device manifest UTF-8"),
            Self::Truncated => formatter.write_str("truncated external device manifest"),
            Self::TrailingBytes => formatter.write_str("trailing external device manifest bytes"),
            Self::NonCanonicalEncoding => {
                formatter.write_str("noncanonical external device manifest encoding")
            }
            Self::ManifestIdentityMismatch => {
                formatter.write_str("external device manifest identity mismatch")
            }
        }
    }
}

impl Error for ExternalDeviceLibraryManifestErrorV1 {}

fn require_identity(bytes: [u8; 32]) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    if bytes == [0; 32] {
        Err(ExternalDeviceLibraryManifestErrorV1::MissingIdentity)
    } else {
        Ok(())
    }
}

fn validate_ascii_token(
    text: &str,
    max: usize,
) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    if text.is_empty()
        || text.len() > max
        || !text.is_ascii()
        || text
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        Err(ExternalDeviceLibraryManifestErrorV1::InvalidText)
    } else {
        Ok(())
    }
}

fn parse_llvm_major(version: &str) -> Result<u16, ExternalDeviceLibraryManifestErrorV1> {
    let digit_count = version.bytes().take_while(u8::is_ascii_digit).count();
    if digit_count == 0
        || (digit_count > 1 && version.as_bytes()[0] == b'0')
        || version
            .as_bytes()
            .get(digit_count)
            .is_some_and(|separator| !matches!(separator, b'.' | b'-' | b'+'))
    {
        return Err(ExternalDeviceLibraryManifestErrorV1::InvalidLlvmIdentity);
    }
    version[..digit_count]
        .parse()
        .map_err(|_| ExternalDeviceLibraryManifestErrorV1::InvalidLlvmIdentity)
}

fn validate_reviewed_profile(
    content: ExternalDeviceLibraryContentIdentityV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    llvm: &ExternalDeviceLlvmIdentityV1,
) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    if target.as_amd_target_id().processor() != "gfx942"
        || llvm.target_triple() != EXTERNAL_DEVICE_LIBRARY_TARGET_TRIPLE_V1
        || llvm.data_layout() != EXTERNAL_DEVICE_LIBRARY_GFX942_DATA_LAYOUT_V1
    {
        return Err(ExternalDeviceLibraryManifestErrorV1::UnsupportedTargetProfile);
    }
    if !matches!(
        (content.kind(), code_object_version),
        (
            ExternalDeviceLibraryContentKindV1::LlvmBitcode
                | ExternalDeviceLibraryContentKindV1::RelocatableObject,
            CodeObjectVersion::V5 | CodeObjectVersion::V6
        )
    ) {
        return Err(ExternalDeviceLibraryManifestErrorV1::UnsupportedContentCodeObjectCombination);
    }
    Ok(())
}

fn validate_ascii_text(text: &str, max: usize) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    if text.is_empty()
        || text.len() > max
        || !text.is_ascii()
        || text.bytes().any(|byte| byte.is_ascii_control())
    {
        Err(ExternalDeviceLibraryManifestErrorV1::InvalidText)
    } else {
        Ok(())
    }
}

fn require_strict_order<T: Ord>(
    values: &[T],
    error: ExternalDeviceLibraryManifestErrorV1,
) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        Err(error)
    } else {
        Ok(())
    }
}

fn address_spaces(
    physical_abi: &str,
) -> Result<Vec<ExternalDeviceAddressSpaceV1>, ExternalDeviceLibraryManifestErrorV1> {
    let abi = parse_device_ffi_physical_abi_v1(physical_abi)
        .map_err(ExternalDeviceLibraryManifestErrorV1::FfiGrammar)?;
    let mut spaces = BTreeSet::new();
    for argument in abi.arguments() {
        if let DeviceFfiPhysicalTypeV1::Pointer(pointer) = argument {
            spaces.insert(map_address_space(pointer.address_space()));
        }
    }
    if let DeviceFfiPhysicalResultV1::Value(DeviceFfiPhysicalTypeV1::Pointer(pointer)) =
        abi.result()
    {
        spaces.insert(map_address_space(pointer.address_space()));
    }
    Ok(spaces.into_iter().collect())
}

const fn map_address_space(value: DeviceFfiAddressSpaceV1) -> ExternalDeviceAddressSpaceV1 {
    match value {
        DeviceFfiAddressSpaceV1::Constant => ExternalDeviceAddressSpaceV1::Constant,
        DeviceFfiAddressSpaceV1::Global => ExternalDeviceAddressSpaceV1::Global,
        DeviceFfiAddressSpaceV1::Private => ExternalDeviceAddressSpaceV1::Private,
        DeviceFfiAddressSpaceV1::Workgroup => ExternalDeviceAddressSpaceV1::Workgroup,
    }
}

fn validate_symbol_uniqueness(
    symbols: &[ExternalDeviceSymbolV1],
) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    let mut roles = BTreeMap::new();
    let mut semantics = BTreeSet::new();
    for symbol in symbols {
        if roles.insert(symbol.symbol.as_str(), symbol.role).is_some() {
            return Err(ExternalDeviceLibraryManifestErrorV1::DuplicateSymbol);
        }
        if !semantics.insert(symbol.semantic_identity) {
            return Err(ExternalDeviceLibraryManifestErrorV1::DuplicateSemanticIdentity);
        }
    }
    Ok(())
}

fn validate_symbol_order(
    symbols: &[ExternalDeviceSymbolV1],
) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    for pair in symbols.windows(2) {
        let previous = (pair[0].role, pair[0].symbol.as_str());
        let current = (pair[1].role, pair[1].symbol.as_str());
        if previous >= current {
            return Err(ExternalDeviceLibraryManifestErrorV1::NonCanonicalSymbolOrder);
        }
    }
    Ok(())
}

fn validate_dependency_order(
    dependencies: &[ExternalDeviceLibraryDependencyV1],
) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    for pair in dependencies.windows(2) {
        if pair[0].manifest_identity == pair[1].manifest_identity {
            return Err(ExternalDeviceLibraryManifestErrorV1::DuplicateDependencyManifest);
        }
        if pair[0].manifest_identity > pair[1].manifest_identity {
            return Err(ExternalDeviceLibraryManifestErrorV1::NonCanonicalDependencyOrder);
        }
    }
    Ok(())
}

fn validate_dependency_closure(
    own_content: ExternalDeviceLibraryContentIdentityV1,
    symbols: &[ExternalDeviceSymbolV1],
    dependencies: &[ExternalDeviceLibraryDependencyV1],
) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    let imports = symbols
        .iter()
        .filter(|symbol| symbol.role.is_import())
        .map(|symbol| symbol.symbol.as_str())
        .collect::<BTreeSet<_>>();
    let mut manifests = BTreeSet::new();
    let mut contents = BTreeSet::new();
    let mut resolved = BTreeSet::new();
    for dependency in dependencies {
        if !manifests.insert(dependency.manifest_identity) {
            return Err(ExternalDeviceLibraryManifestErrorV1::DuplicateDependencyManifest);
        }
        if dependency.content_identity.blob() == own_content.blob() {
            return Err(ExternalDeviceLibraryManifestErrorV1::SelfDependencyContent);
        }
        if !contents.insert(dependency.content_identity.blob()) {
            return Err(ExternalDeviceLibraryManifestErrorV1::DuplicateDependencyContent);
        }
        for symbol in &dependency.resolved_imports {
            if !imports.contains(symbol.as_str()) {
                return Err(ExternalDeviceLibraryManifestErrorV1::UnexpectedResolvedImport);
            }
            if !resolved.insert(symbol.as_str()) {
                return Err(ExternalDeviceLibraryManifestErrorV1::DuplicateResolvedImport);
            }
        }
    }
    if resolved != imports {
        return Err(ExternalDeviceLibraryManifestErrorV1::MissingResolvedImport);
    }
    Ok(())
}

fn encoded_size(
    target: &str,
    llvm: &ExternalDeviceLlvmIdentityV1,
    trust: ExternalDeviceLibraryTrustV1,
    symbols: &[ExternalDeviceSymbolV1],
    dependencies: &[ExternalDeviceLibraryDependencyV1],
) -> Result<usize, ExternalDeviceLibraryManifestErrorV1> {
    let mut size = MANIFEST_DOMAIN_V1.len() + 1 + 32 + 8 + 1 + 2 + 20 + 32 + 8 + 1 + 32 + 1 + 4 + 4;
    for text in [
        target,
        llvm.version(),
        llvm.target_triple(),
        llvm.data_layout(),
    ] {
        size = size
            .checked_add(4 + text.len())
            .ok_or(ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded)?;
    }
    if trust.evidence_identity.is_some() {
        size = size
            .checked_add(32)
            .ok_or(ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded)?;
    }
    for symbol in symbols {
        size = size
            .checked_add(1 + 1 + 1 + 1 + 32 + 2)
            .and_then(|value| value.checked_add(4 + symbol.symbol.len()))
            .and_then(|value| value.checked_add(4 + symbol.physical_abi.len()))
            .and_then(|value| value.checked_add(symbol.address_spaces.len()))
            .and_then(|value| value.checked_add(4 + symbol.effects.len()))
            .and_then(|value| value.checked_add(symbol.required_capabilities.len() * 32))
            .ok_or(ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded)?;
    }
    for dependency in dependencies {
        size = size
            .checked_add(32 + 8 + 1 + 32 + 8 + 4)
            .ok_or(ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded)?;
        for symbol in &dependency.resolved_imports {
            size = size
                .checked_add(4 + symbol.len())
                .ok_or(ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded)?;
        }
    }
    Ok(size)
}

fn encode_content(bytes: &mut Vec<u8>, content: ExternalDeviceLibraryContentIdentityV1) {
    bytes.push(content.kind as u8);
    bytes.extend_from_slice(content.blob.sha256());
    bytes.extend_from_slice(&content.blob.byte_len().to_le_bytes());
}

fn encode_llvm(
    bytes: &mut Vec<u8>,
    llvm: &ExternalDeviceLlvmIdentityV1,
) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    bytes.extend_from_slice(&llvm.major.to_le_bytes());
    push_text(bytes, &llvm.version)?;
    bytes.extend_from_slice(&llvm.commit);
    bytes.extend_from_slice(llvm.executable.sha256());
    bytes.extend_from_slice(&llvm.executable.byte_len().to_le_bytes());
    push_text(bytes, &llvm.target_triple)?;
    push_text(bytes, &llvm.data_layout)
}

fn encode_trust(bytes: &mut Vec<u8>, trust: ExternalDeviceLibraryTrustV1) {
    bytes.push(trust.class as u8);
    if let Some(identity) = trust.evidence_identity {
        bytes.extend_from_slice(&identity);
    }
}

fn encode_symbol(
    bytes: &mut Vec<u8>,
    symbol: &ExternalDeviceSymbolV1,
) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    bytes.push(symbol.role as u8);
    bytes.push(symbol.calling_convention as u8);
    bytes.push(symbol.convergence as u8);
    push_text(bytes, &symbol.symbol)?;
    push_text(bytes, &symbol.physical_abi)?;
    bytes.push(symbol.address_spaces.len() as u8);
    for address_space in &symbol.address_spaces {
        bytes.push(*address_space as u8);
    }
    push_text(bytes, &symbol.effects)?;
    bytes.extend_from_slice(symbol.semantic_identity.as_bytes());
    push_u16(bytes, symbol.required_capabilities.len())?;
    for capability in &symbol.required_capabilities {
        bytes.extend_from_slice(capability.as_bytes());
    }
    Ok(())
}

fn encode_dependency(
    bytes: &mut Vec<u8>,
    dependency: &ExternalDeviceLibraryDependencyV1,
) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    bytes.extend_from_slice(dependency.manifest_identity.sha256());
    bytes.extend_from_slice(&dependency.manifest_identity.byte_len().to_le_bytes());
    encode_content(bytes, dependency.content_identity);
    push_u32(bytes, dependency.resolved_imports.len())?;
    for symbol in &dependency.resolved_imports {
        push_text(bytes, symbol)?;
    }
    Ok(())
}

fn decode_content(
    cursor: &mut Cursor<'_>,
) -> Result<ExternalDeviceLibraryContentIdentityV1, ExternalDeviceLibraryManifestErrorV1> {
    let kind = decode_content_kind(cursor.byte()?)?;
    let blob = ExternalDeviceBlobIdentityV1::new(cursor.fixed::<32>()?, cursor.u64()?)?;
    Ok(ExternalDeviceLibraryContentIdentityV1::new(kind, blob))
}

fn decode_llvm(
    cursor: &mut Cursor<'_>,
) -> Result<ExternalDeviceLlvmIdentityV1, ExternalDeviceLibraryManifestErrorV1> {
    let major = cursor.u16()?;
    let version = cursor
        .text(MAX_EXTERNAL_DEVICE_LIBRARY_LLVM_TOKEN_BYTES_V1)?
        .to_owned();
    let commit = cursor.fixed::<20>()?;
    let executable = ExternalDeviceBlobIdentityV1::new(cursor.fixed::<32>()?, cursor.u64()?)?;
    let target_triple = cursor
        .text(MAX_EXTERNAL_DEVICE_LIBRARY_LLVM_TOKEN_BYTES_V1)?
        .to_owned();
    let data_layout = cursor
        .text(MAX_EXTERNAL_DEVICE_LIBRARY_LLVM_DATA_LAYOUT_BYTES_V1)?
        .to_owned();
    ExternalDeviceLlvmIdentityV1::new(
        major,
        version,
        commit,
        executable,
        target_triple,
        data_layout,
    )
}

fn decode_trust(
    cursor: &mut Cursor<'_>,
) -> Result<ExternalDeviceLibraryTrustV1, ExternalDeviceLibraryManifestErrorV1> {
    match cursor.byte()? {
        1 => Ok(ExternalDeviceLibraryTrustV1::unverified()),
        2 => ExternalDeviceLibraryTrustV1::declared_specification(cursor.fixed::<32>()?),
        3 => ExternalDeviceLibraryTrustV1::external_attestation(cursor.fixed::<32>()?),
        _ => Err(ExternalDeviceLibraryManifestErrorV1::InvalidTag),
    }
}

fn decode_symbol(
    cursor: &mut Cursor<'_>,
) -> Result<ExternalDeviceSymbolV1, ExternalDeviceLibraryManifestErrorV1> {
    let role = decode_symbol_role(cursor.byte()?)?;
    if cursor.byte()? != ExternalDeviceCallingConventionV1::C as u8 {
        return Err(ExternalDeviceLibraryManifestErrorV1::InvalidTag);
    }
    let convergence = decode_convergence(cursor.byte()?)?;
    let symbol = cursor
        .text(reserved_fe2o3_symbols::MAX_DEVICE_FFI_SYMBOL_BYTES_V1)?
        .to_owned();
    let physical_abi = cursor
        .text(reserved_fe2o3_symbols::MAX_DEVICE_FFI_PHYSICAL_ABI_BYTES_V1)?
        .to_owned();
    let address_count = usize::from(cursor.byte()?);
    if address_count > 4 {
        return Err(ExternalDeviceLibraryManifestErrorV1::AddressSpaceMismatch);
    }
    let mut encoded_address_spaces = Vec::with_capacity(address_count);
    for _ in 0..address_count {
        encoded_address_spaces.push(decode_address_space(cursor.byte()?)?);
    }
    require_strict_order(
        &encoded_address_spaces,
        ExternalDeviceLibraryManifestErrorV1::AddressSpaceMismatch,
    )?;
    let effects = cursor
        .text(reserved_fe2o3_symbols::MAX_DEVICE_FFI_EFFECT_BYTES_V1)?
        .to_owned();
    let semantic_identity = ExternalDeviceSemanticIdentityV1::new(cursor.fixed::<32>()?)?;
    let capability_count = usize::from(cursor.u16()?);
    if capability_count > MAX_EXTERNAL_DEVICE_LIBRARY_CAPABILITIES_V1 {
        return Err(ExternalDeviceLibraryManifestErrorV1::TooManyCapabilities);
    }
    let mut required_capabilities = Vec::with_capacity(capability_count);
    for _ in 0..capability_count {
        required_capabilities.push(ExternalDeviceCapabilityIdentityV1::new(
            cursor.fixed::<32>()?,
        )?);
    }
    let value = ExternalDeviceSymbolV1::new(
        role,
        symbol,
        physical_abi,
        effects,
        convergence,
        semantic_identity,
        required_capabilities,
    )?;
    if value.address_spaces != encoded_address_spaces {
        return Err(ExternalDeviceLibraryManifestErrorV1::AddressSpaceMismatch);
    }
    Ok(value)
}

fn decode_dependency(
    cursor: &mut Cursor<'_>,
) -> Result<ExternalDeviceLibraryDependencyV1, ExternalDeviceLibraryManifestErrorV1> {
    let manifest_identity =
        ExternalDeviceLibraryManifestIdentityV1::new(cursor.fixed::<32>()?, cursor.u64()?)?;
    let content_identity = decode_content(cursor)?;
    let count = cursor.count(MAX_EXTERNAL_DEVICE_LIBRARY_SYMBOLS_V1)?;
    let mut resolved_imports = Vec::with_capacity(count);
    for _ in 0..count {
        resolved_imports.push(
            cursor
                .text(reserved_fe2o3_symbols::MAX_DEVICE_FFI_SYMBOL_BYTES_V1)?
                .to_owned(),
        );
    }
    ExternalDeviceLibraryDependencyV1::new(manifest_identity, content_identity, resolved_imports)
}

const fn code_object_version_tag(value: CodeObjectVersion) -> u8 {
    match value {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}

fn decode_code_object_version(
    value: u8,
) -> Result<CodeObjectVersion, ExternalDeviceLibraryManifestErrorV1> {
    match value {
        4 => Ok(CodeObjectVersion::V4),
        5 => Ok(CodeObjectVersion::V5),
        6 => Ok(CodeObjectVersion::V6),
        _ => Err(ExternalDeviceLibraryManifestErrorV1::InvalidTag),
    }
}

fn decode_content_kind(
    value: u8,
) -> Result<ExternalDeviceLibraryContentKindV1, ExternalDeviceLibraryManifestErrorV1> {
    match value {
        1 => Ok(ExternalDeviceLibraryContentKindV1::LlvmBitcode),
        2 => Ok(ExternalDeviceLibraryContentKindV1::RelocatableObject),
        3 => Ok(ExternalDeviceLibraryContentKindV1::CodeObject),
        4 => Ok(ExternalDeviceLibraryContentKindV1::StaticArchive),
        _ => Err(ExternalDeviceLibraryManifestErrorV1::InvalidTag),
    }
}

fn decode_provenance_kind(
    value: u8,
) -> Result<ExternalDeviceLibraryProvenanceKindV1, ExternalDeviceLibraryManifestErrorV1> {
    match value {
        1 => Ok(ExternalDeviceLibraryProvenanceKindV1::SourceBuild),
        2 => Ok(ExternalDeviceLibraryProvenanceKindV1::VendorSdk),
        3 => Ok(ExternalDeviceLibraryProvenanceKindV1::PrebuiltBinary),
        _ => Err(ExternalDeviceLibraryManifestErrorV1::InvalidTag),
    }
}

fn decode_symbol_role(
    value: u8,
) -> Result<ExternalDeviceSymbolRoleV1, ExternalDeviceLibraryManifestErrorV1> {
    match value {
        1 => Ok(ExternalDeviceSymbolRoleV1::DeviceFunctionImport),
        2 => Ok(ExternalDeviceSymbolRoleV1::DeviceFunctionExport),
        _ => Err(ExternalDeviceLibraryManifestErrorV1::InvalidTag),
    }
}

fn decode_convergence(
    value: u8,
) -> Result<ExternalDeviceConvergenceV1, ExternalDeviceLibraryManifestErrorV1> {
    match value {
        1 => Ok(ExternalDeviceConvergenceV1::Unconstrained),
        2 => Ok(ExternalDeviceConvergenceV1::Convergent),
        _ => Err(ExternalDeviceLibraryManifestErrorV1::InvalidTag),
    }
}

fn decode_address_space(
    value: u8,
) -> Result<ExternalDeviceAddressSpaceV1, ExternalDeviceLibraryManifestErrorV1> {
    match value {
        1 => Ok(ExternalDeviceAddressSpaceV1::Constant),
        2 => Ok(ExternalDeviceAddressSpaceV1::Global),
        3 => Ok(ExternalDeviceAddressSpaceV1::Private),
        4 => Ok(ExternalDeviceAddressSpaceV1::Workgroup),
        _ => Err(ExternalDeviceLibraryManifestErrorV1::InvalidTag),
    }
}

fn push_text(bytes: &mut Vec<u8>, text: &str) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    push_u32(bytes, text.len())?;
    bytes.extend_from_slice(text.as_bytes());
    Ok(())
}

fn push_u16(bytes: &mut Vec<u8>, value: usize) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    let value = u16::try_from(value)
        .map_err(|_| ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded)?;
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn push_u32(bytes: &mut Vec<u8>, value: usize) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
    let value = u32::try_from(value)
        .map_err(|_| ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded)?;
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], ExternalDeviceLibraryManifestErrorV1> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(ExternalDeviceLibraryManifestErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(ExternalDeviceLibraryManifestErrorV1::Truncated)?;
        self.position = end;
        Ok(value)
    }
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], ExternalDeviceLibraryManifestErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| ExternalDeviceLibraryManifestErrorV1::Truncated)
    }
    fn byte(&mut self) -> Result<u8, ExternalDeviceLibraryManifestErrorV1> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, ExternalDeviceLibraryManifestErrorV1> {
        Ok(u16::from_le_bytes(self.fixed::<2>()?))
    }
    fn u64(&mut self) -> Result<u64, ExternalDeviceLibraryManifestErrorV1> {
        Ok(u64::from_le_bytes(self.fixed::<8>()?))
    }
    fn count(&mut self, max: usize) -> Result<usize, ExternalDeviceLibraryManifestErrorV1> {
        let count = u32::from_le_bytes(self.fixed::<4>()?) as usize;
        if count > max {
            return Err(ExternalDeviceLibraryManifestErrorV1::ManifestByteBoundExceeded);
        }
        Ok(count)
    }
    fn text(&mut self, max: usize) -> Result<&'a str, ExternalDeviceLibraryManifestErrorV1> {
        let len = self.count(max)?;
        if len == 0 {
            return Err(ExternalDeviceLibraryManifestErrorV1::InvalidText);
        }
        str::from_utf8(self.take(len)?)
            .map_err(|_| ExternalDeviceLibraryManifestErrorV1::InvalidUtf8)
    }
    fn finish(self) -> Result<(), ExternalDeviceLibraryManifestErrorV1> {
        if self.position == self.bytes.len() {
            Ok(())
        } else {
            Err(ExternalDeviceLibraryManifestErrorV1::TrailingBytes)
        }
    }
}
