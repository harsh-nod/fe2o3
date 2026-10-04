#![forbid(unsafe_code)]

//! Source-only G7 evidence for a bidirectional device FFI fixture.
//!
//! This crate inspects checked-in source declarations. It has no compiler-
//! derived closure, executable artifact, runtime context, module handle,
//! function handle, load operation, or launch operation.

mod cpu_oracle;

use std::fmt;

use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
use fe2o3_kernel_descriptor::ffi_contract::{
    DeviceFfiAbiTypeV1, DeviceFfiContractError, DeviceFfiContractV1, DeviceFfiDirectionV1,
    DeviceFfiScalarV1, DeviceFfiSemanticIdentityV1, DevicePhysicalAbiV1,
};
use serde::{Deserialize, Serialize};

pub use cpu_oracle::evaluate as cpu_oracle;

pub const FIXTURE_TARGET: &str = "gfx942:sramecc+:xnack-";
pub const FIXTURE_CODE_OBJECT_VERSION: u16 = 5;
pub const EXTERNAL_AFFINE_SYMBOL: &str = "external_scale_bias_v1";
pub const RUST_ACCUMULATE_SYMBOL: &str = "rust_accumulate_v1";

pub const GRANTS_LOAD: bool = false;
pub const GRANTS_LAUNCH: bool = false;
pub const HAS_COMPILER_DERIVED_CLOSURE: bool = false;

const _: () = {
    assert!(!HAS_COMPILER_DERIVED_CLOSURE);
    assert!(!GRANTS_LOAD);
    assert!(!GRANTS_LAUNCH);
};

pub const SOURCE_EVIDENCE_LIMITATIONS: &[&str] = &[
    "no_compiler_derived_closure",
    "no_production_loader",
    "no_hardware_execution",
];

const EXTERNAL_MANIFEST_SCHEMA: &str = "fe2o3.external-device-ffi-source.v1";
const EXTERNAL_SOURCE_PATH: &str = "external.amdgpu.ll";
const EXTERNAL_SOURCE_KIND: &str = "llvm-ir-text";
const CONTRACT_SET_DOMAIN: &[u8] = b"fe2o3.device-link-ffi.source-contracts.v1\0";
const EXTERNAL_AFFINE_SEMANTIC: [u8; 32] = [0x11; 32];
const RUST_ACCUMULATE_SEMANTIC: [u8; 32] = [0x22; 32];
const EXTERNAL_AFFINE_SEMANTIC_HEX: &str =
    "1111111111111111111111111111111111111111111111111111111111111111";
const RUST_ACCUMULATE_SEMANTIC_HEX: &str =
    "2222222222222222222222222222222222222222222222222222222222222222";
const SCALAR_BINARY_ABI: &str = "C(u32[size=4,align=4],u32[size=4,align=4])->u32[size=4,align=4]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceContractEndpoint {
    RustImport,
    ExternalDefinition,
    RustExport,
    ExternalDeclaration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceContractSetError {
    Contract(DeviceFfiContractError),
    WrongDirection {
        endpoint: SourceContractEndpoint,
        expected: DeviceFfiDirectionV1,
        actual: DeviceFfiDirectionV1,
    },
    SymbolMismatch,
    TargetMismatch,
    CodeObjectVersionMismatch,
    AbiMismatch,
    EffectsMismatch,
    SemanticIdentityMismatch,
}

impl fmt::Display for SourceContractSetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => write!(formatter, "invalid device FFI contract: {error}"),
            Self::WrongDirection {
                endpoint,
                expected,
                actual,
            } => write!(
                formatter,
                "{endpoint:?} has direction {actual:?}; expected {expected:?}"
            ),
            Self::SymbolMismatch => formatter.write_str("counterpart symbols differ"),
            Self::TargetMismatch => formatter.write_str("counterpart targets differ"),
            Self::CodeObjectVersionMismatch => {
                formatter.write_str("counterpart code-object versions differ")
            }
            Self::AbiMismatch => formatter.write_str("counterpart physical ABIs differ"),
            Self::EffectsMismatch => formatter.write_str("counterpart memory effects differ"),
            Self::SemanticIdentityMismatch => {
                formatter.write_str("counterpart semantic identities differ")
            }
        }
    }
}

impl std::error::Error for SourceContractSetError {}

impl From<DeviceFfiContractError> for SourceContractSetError {
    fn from(error: DeviceFfiContractError) -> Self {
        Self::Contract(error)
    }
}

/// Canonical declarations extracted manually from the two fixture sources.
///
/// This value records source intent only. It does not establish that a compiler
/// emitted either symbol or that a linker resolved the closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BidirectionalSourceContractSetV1 {
    rust_import: DeviceFfiContractV1,
    external_definition: DeviceFfiContractV1,
    rust_export: DeviceFfiContractV1,
    external_declaration: DeviceFfiContractV1,
    identity: PayloadDigest,
}

impl BidirectionalSourceContractSetV1 {
    fn new(
        rust_import: DeviceFfiContractV1,
        external_definition: DeviceFfiContractV1,
        rust_export: DeviceFfiContractV1,
        external_declaration: DeviceFfiContractV1,
    ) -> Result<Self, SourceContractSetError> {
        require_direction(
            SourceContractEndpoint::RustImport,
            &rust_import,
            DeviceFfiDirectionV1::Import,
        )?;
        require_direction(
            SourceContractEndpoint::ExternalDefinition,
            &external_definition,
            DeviceFfiDirectionV1::Export,
        )?;
        require_direction(
            SourceContractEndpoint::RustExport,
            &rust_export,
            DeviceFfiDirectionV1::Export,
        )?;
        require_direction(
            SourceContractEndpoint::ExternalDeclaration,
            &external_declaration,
            DeviceFfiDirectionV1::Import,
        )?;
        require_counterparts(&rust_import, &external_definition)?;
        require_counterparts(&external_declaration, &rust_export)?;

        let mut bytes = CONTRACT_SET_DOMAIN.to_vec();
        for contract in [
            &rust_import,
            &external_definition,
            &rust_export,
            &external_declaration,
        ] {
            let record = contract.canonical_link_record();
            bytes.extend_from_slice(&(record.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&record);
        }
        let identity = DigestAlgorithm::Sha256.calculate(&bytes);
        Ok(Self {
            rust_import,
            external_definition,
            rust_export,
            external_declaration,
            identity,
        })
    }

    pub fn fixture_source_declarations() -> Result<Self, SourceContractSetError> {
        Self::new(
            fixture_contract(
                DeviceFfiDirectionV1::Import,
                EXTERNAL_AFFINE_SYMBOL,
                EXTERNAL_AFFINE_SEMANTIC,
            )?,
            fixture_contract(
                DeviceFfiDirectionV1::Export,
                EXTERNAL_AFFINE_SYMBOL,
                EXTERNAL_AFFINE_SEMANTIC,
            )?,
            fixture_contract(
                DeviceFfiDirectionV1::Export,
                RUST_ACCUMULATE_SYMBOL,
                RUST_ACCUMULATE_SEMANTIC,
            )?,
            fixture_contract(
                DeviceFfiDirectionV1::Import,
                RUST_ACCUMULATE_SYMBOL,
                RUST_ACCUMULATE_SEMANTIC,
            )?,
        )
    }

    pub const fn identity(&self) -> PayloadDigest {
        self.identity
    }

    pub const fn rust_import(&self) -> &DeviceFfiContractV1 {
        &self.rust_import
    }

    pub const fn external_definition(&self) -> &DeviceFfiContractV1 {
        &self.external_definition
    }

    pub const fn rust_export(&self) -> &DeviceFfiContractV1 {
        &self.rust_export
    }

    pub const fn external_declaration(&self) -> &DeviceFfiContractV1 {
        &self.external_declaration
    }
}

fn require_direction(
    endpoint: SourceContractEndpoint,
    contract: &DeviceFfiContractV1,
    expected: DeviceFfiDirectionV1,
) -> Result<(), SourceContractSetError> {
    let actual = contract.direction();
    if actual == expected {
        Ok(())
    } else {
        Err(SourceContractSetError::WrongDirection {
            endpoint,
            expected,
            actual,
        })
    }
}

fn require_counterparts(
    import: &DeviceFfiContractV1,
    export: &DeviceFfiContractV1,
) -> Result<(), SourceContractSetError> {
    if import.symbol() != export.symbol() {
        return Err(SourceContractSetError::SymbolMismatch);
    }
    if import.target() != export.target() {
        return Err(SourceContractSetError::TargetMismatch);
    }
    if import.code_object_version() != export.code_object_version() {
        return Err(SourceContractSetError::CodeObjectVersionMismatch);
    }
    if import.abi() != export.abi() {
        return Err(SourceContractSetError::AbiMismatch);
    }
    if import.effects() != export.effects() {
        return Err(SourceContractSetError::EffectsMismatch);
    }
    if import.semantic_identity() != export.semantic_identity() {
        return Err(SourceContractSetError::SemanticIdentityMismatch);
    }
    Ok(())
}

fn scalar_binary_abi() -> Result<DevicePhysicalAbiV1, DeviceFfiContractError> {
    DevicePhysicalAbiV1::new(
        vec![
            DeviceFfiAbiTypeV1::Scalar(DeviceFfiScalarV1::U32),
            DeviceFfiAbiTypeV1::Scalar(DeviceFfiScalarV1::U32),
        ],
        Some(DeviceFfiAbiTypeV1::Scalar(DeviceFfiScalarV1::U32)),
    )
}

fn fixture_contract(
    direction: DeviceFfiDirectionV1,
    symbol: &str,
    semantic: [u8; 32],
) -> Result<DeviceFfiContractV1, DeviceFfiContractError> {
    DeviceFfiContractV1::new(
        direction,
        symbol,
        AmdTargetId::parse(FIXTURE_TARGET).expect("fixture target is canonical"),
        FIXTURE_CODE_OBJECT_VERSION,
        scalar_binary_abi()?,
        vec![],
        DeviceFfiSemanticIdentityV1::from_opaque_bytes(semantic),
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExternalEvidenceManifestV1 {
    schema: String,
    source: ExternalSourceV1,
    definitions: Vec<ExternalSymbolV1>,
    declarations: Vec<ExternalSymbolV1>,
    compiler_derived_closure: bool,
    grants_load: bool,
    grants_launch: bool,
    limitations: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExternalSourceV1 {
    path: String,
    kind: String,
    sha256: String,
    target: String,
    code_object_version: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExternalSymbolV1 {
    symbol: String,
    abi: String,
    effects: String,
    semantic_identity: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceEvidenceError {
    ContractSet,
    MalformedExternalManifest,
    NonCanonicalExternalManifest,
    WrongExternalManifestSchema,
    WrongExternalSourceIdentity,
    ExternalIrDigestMismatch,
    ExternalDefinitionMismatch,
    ExternalDeclarationMismatch,
    CompilerDerivedClosureClaimed,
    RuntimeAuthorityClaimed,
    LimitationsMismatch,
}

impl fmt::Display for SourceEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ContractSet => "fixture source contracts are invalid",
            Self::MalformedExternalManifest => "external evidence manifest is malformed",
            Self::NonCanonicalExternalManifest => "external evidence manifest is not canonical",
            Self::WrongExternalManifestSchema => "external evidence manifest has the wrong schema",
            Self::WrongExternalSourceIdentity => {
                "external evidence manifest names the wrong source contract"
            }
            Self::ExternalIrDigestMismatch => {
                "external LLVM IR does not match its evidence manifest"
            }
            Self::ExternalDefinitionMismatch => {
                "external definition does not match the source contract"
            }
            Self::ExternalDeclarationMismatch => {
                "external declaration does not match the source contract"
            }
            Self::CompilerDerivedClosureClaimed => {
                "source evidence cannot claim compiler-derived closure"
            }
            Self::RuntimeAuthorityClaimed => {
                "source evidence cannot grant load or launch authority"
            }
            Self::LimitationsMismatch => "source evidence limitations are not exact",
        })
    }
}

impl std::error::Error for SourceEvidenceError {}

/// Digests and declarations derived from fixed source fixture bytes.
///
/// This report is descriptive only. Its authority fields are permanently
/// false, and it cannot be converted into a production artifact or runtime
/// handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceEvidenceV1 {
    rust_source_digest: PayloadDigest,
    external_ir_digest: PayloadDigest,
    external_manifest_digest: PayloadDigest,
    source_contract_identity: PayloadDigest,
    compiler_derived_closure: bool,
    grants_load: bool,
    grants_launch: bool,
}

impl SourceEvidenceV1 {
    pub const fn rust_source_digest(self) -> PayloadDigest {
        self.rust_source_digest
    }

    pub const fn external_ir_digest(self) -> PayloadDigest {
        self.external_ir_digest
    }

    pub const fn external_manifest_digest(self) -> PayloadDigest {
        self.external_manifest_digest
    }

    pub const fn source_contract_identity(self) -> PayloadDigest {
        self.source_contract_identity
    }

    pub const fn has_compiler_derived_closure(self) -> bool {
        self.compiler_derived_closure
    }

    pub const fn grants_load(self) -> bool {
        self.grants_load
    }

    pub const fn grants_launch(self) -> bool {
        self.grants_launch
    }

    pub const fn limitations(self) -> &'static [&'static str] {
        SOURCE_EVIDENCE_LIMITATIONS
    }
}

/// Inspects the fixed checked-in source fixture and returns inert evidence.
pub fn fixture_source_evidence() -> Result<SourceEvidenceV1, SourceEvidenceError> {
    inspect_source_evidence(
        include_bytes!("../../../tests/fixtures/device-link/rust-device/src/lib.rs"),
        include_bytes!("../../../tests/fixtures/device-link/external.amdgpu.ll"),
        include_bytes!("../../../tests/fixtures/device-link/external.evidence.v1.json"),
    )
}

fn inspect_source_evidence(
    rust_source: &[u8],
    external_ir: &[u8],
    manifest_bytes: &[u8],
) -> Result<SourceEvidenceV1, SourceEvidenceError> {
    let contracts = BidirectionalSourceContractSetV1::fixture_source_declarations()
        .map_err(|_| SourceEvidenceError::ContractSet)?;
    let manifest = decode_canonical_manifest(manifest_bytes)?;
    validate_manifest(&manifest, external_ir, &contracts)?;

    Ok(SourceEvidenceV1 {
        rust_source_digest: DigestAlgorithm::Sha256.calculate(rust_source),
        external_ir_digest: DigestAlgorithm::Sha256.calculate(external_ir),
        external_manifest_digest: DigestAlgorithm::Sha256.calculate(manifest_bytes),
        source_contract_identity: contracts.identity(),
        compiler_derived_closure: false,
        grants_load: false,
        grants_launch: false,
    })
}

fn decode_canonical_manifest(
    bytes: &[u8],
) -> Result<ExternalEvidenceManifestV1, SourceEvidenceError> {
    let manifest: ExternalEvidenceManifestV1 = serde_json::from_slice(bytes)
        .map_err(|_| SourceEvidenceError::MalformedExternalManifest)?;
    let mut canonical = serde_json::to_vec(&manifest)
        .map_err(|_| SourceEvidenceError::MalformedExternalManifest)?;
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(SourceEvidenceError::NonCanonicalExternalManifest);
    }
    Ok(manifest)
}

fn validate_manifest(
    manifest: &ExternalEvidenceManifestV1,
    external_ir: &[u8],
    contracts: &BidirectionalSourceContractSetV1,
) -> Result<(), SourceEvidenceError> {
    if manifest.schema != EXTERNAL_MANIFEST_SCHEMA {
        return Err(SourceEvidenceError::WrongExternalManifestSchema);
    }
    if manifest.source.path != EXTERNAL_SOURCE_PATH
        || manifest.source.kind != EXTERNAL_SOURCE_KIND
        || manifest.source.target != FIXTURE_TARGET
        || manifest.source.code_object_version != FIXTURE_CODE_OBJECT_VERSION
    {
        return Err(SourceEvidenceError::WrongExternalSourceIdentity);
    }
    if manifest.source.sha256 != sha256_hex(external_ir) {
        return Err(SourceEvidenceError::ExternalIrDigestMismatch);
    }
    if manifest.definitions.as_slice()
        != [symbol_manifest(
            contracts.external_definition(),
            EXTERNAL_AFFINE_SEMANTIC_HEX,
        )]
    {
        return Err(SourceEvidenceError::ExternalDefinitionMismatch);
    }
    if manifest.declarations.as_slice()
        != [symbol_manifest(
            contracts.external_declaration(),
            RUST_ACCUMULATE_SEMANTIC_HEX,
        )]
    {
        return Err(SourceEvidenceError::ExternalDeclarationMismatch);
    }
    if manifest.compiler_derived_closure {
        return Err(SourceEvidenceError::CompilerDerivedClosureClaimed);
    }
    if manifest.grants_load || manifest.grants_launch {
        return Err(SourceEvidenceError::RuntimeAuthorityClaimed);
    }
    if manifest.limitations
        != SOURCE_EVIDENCE_LIMITATIONS
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    {
        return Err(SourceEvidenceError::LimitationsMismatch);
    }
    Ok(())
}

fn symbol_manifest(contract: &DeviceFfiContractV1, semantic: &str) -> ExternalSymbolV1 {
    debug_assert!(contract.effects().is_empty());
    debug_assert_eq!(contract.abi().canonical_spelling(), SCALAR_BINARY_ABI);
    ExternalSymbolV1 {
        symbol: contract.symbol().to_owned(),
        abi: contract.abi().canonical_spelling(),
        effects: "none".to_owned(),
        semantic_identity: semantic.to_owned(),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    let mut value = String::with_capacity(64);
    for byte in digest.bytes().as_bytes() {
        value.push(HEX[usize::from(byte >> 4)] as char);
        value.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    value
}

/// CPU evaluation of the source-level boundary structure.
///
/// This is not GPU execution and does not inspect compiler output.
pub fn evaluate_source_model_path(input: &[u32], output_len: usize, untouched: u32) -> Vec<u32> {
    let mut output = vec![untouched; output_len];
    for (lane, destination) in output.iter_mut().enumerate() {
        if let Some(value) = input.get(lane) {
            *destination = external_scale_bias_source_model(*value, lane as u32);
        }
    }
    output
}

fn external_scale_bias_source_model(value: u32, lane: u32) -> u32 {
    rust_accumulate_source_model(value.wrapping_mul(3).wrapping_add(5), lane)
}

fn rust_accumulate_source_model(value: u32, lane: u32) -> u32 {
    value.wrapping_add(lane)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUST_DEVICE_SOURCE: &[u8] =
        include_bytes!("../../../tests/fixtures/device-link/rust-device/src/lib.rs");
    const EXTERNAL_IR: &[u8] =
        include_bytes!("../../../tests/fixtures/device-link/external.amdgpu.ll");
    const EXTERNAL_MANIFEST: &[u8] =
        include_bytes!("../../../tests/fixtures/device-link/external.evidence.v1.json");
    const UNTOUCHED: u32 = 0xfeed_cafe;

    fn contracts() -> BidirectionalSourceContractSetV1 {
        BidirectionalSourceContractSetV1::fixture_source_declarations().unwrap()
    }

    fn altered_contract(
        direction: DeviceFfiDirectionV1,
        symbol: &str,
        target_text: &str,
        abi: DevicePhysicalAbiV1,
        semantic: [u8; 32],
    ) -> DeviceFfiContractV1 {
        DeviceFfiContractV1::new(
            direction,
            symbol,
            AmdTargetId::parse(target_text).unwrap(),
            FIXTURE_CODE_OBJECT_VERSION,
            abi,
            vec![],
            DeviceFfiSemanticIdentityV1::from_opaque_bytes(semantic),
        )
        .unwrap()
    }

    fn canonical_manifest_with(update: impl FnOnce(&mut ExternalEvidenceManifestV1)) -> Vec<u8> {
        let mut manifest: ExternalEvidenceManifestV1 =
            serde_json::from_slice(EXTERNAL_MANIFEST).unwrap();
        update(&mut manifest);
        let mut bytes = serde_json::to_vec(&manifest).unwrap();
        bytes.push(b'\n');
        bytes
    }

    #[test]
    fn fixed_fixture_produces_source_evidence_without_authority() {
        let evidence = fixture_source_evidence().unwrap();
        assert_eq!(
            evidence.external_ir_digest(),
            DigestAlgorithm::Sha256.calculate(EXTERNAL_IR)
        );
        assert_eq!(
            evidence.rust_source_digest(),
            DigestAlgorithm::Sha256.calculate(RUST_DEVICE_SOURCE)
        );
        assert_eq!(evidence.source_contract_identity(), contracts().identity());
        assert!(!evidence.has_compiler_derived_closure());
        assert!(!evidence.grants_load());
        assert!(!evidence.grants_launch());
        assert_eq!(evidence.limitations(), SOURCE_EVIDENCE_LIMITATIONS);
    }

    #[test]
    fn external_manifest_is_canonical_and_binds_exact_ir_bytes() {
        let manifest = decode_canonical_manifest(EXTERNAL_MANIFEST).unwrap();
        assert_eq!(manifest.source.sha256, sha256_hex(EXTERNAL_IR));
        assert_eq!(manifest.definitions.len(), 1);
        assert_eq!(manifest.declarations.len(), 1);

        let mut changed_ir = EXTERNAL_IR.to_vec();
        changed_ir.push(b' ');
        assert_eq!(
            inspect_source_evidence(RUST_DEVICE_SOURCE, &changed_ir, EXTERNAL_MANIFEST),
            Err(SourceEvidenceError::ExternalIrDigestMismatch)
        );

        let pretty = serde_json::to_vec_pretty(&manifest).unwrap();
        assert_eq!(
            inspect_source_evidence(RUST_DEVICE_SOURCE, EXTERNAL_IR, &pretty),
            Err(SourceEvidenceError::NonCanonicalExternalManifest)
        );
    }

    #[test]
    fn external_manifest_rejects_contract_substitution() {
        let changed_definition = canonical_manifest_with(|manifest| {
            manifest.definitions[0].abi = "C(u64[size=8,align=8])->u32[size=4,align=4]".to_owned();
        });
        assert_eq!(
            inspect_source_evidence(RUST_DEVICE_SOURCE, EXTERNAL_IR, &changed_definition),
            Err(SourceEvidenceError::ExternalDefinitionMismatch)
        );

        let changed_declaration = canonical_manifest_with(|manifest| {
            manifest.declarations[0].semantic_identity = "33".repeat(32);
        });
        assert_eq!(
            inspect_source_evidence(RUST_DEVICE_SOURCE, EXTERNAL_IR, &changed_declaration),
            Err(SourceEvidenceError::ExternalDeclarationMismatch)
        );
    }

    #[test]
    fn source_manifest_cannot_claim_closure_or_runtime_authority() {
        let closure = canonical_manifest_with(|manifest| {
            manifest.compiler_derived_closure = true;
        });
        assert_eq!(
            inspect_source_evidence(RUST_DEVICE_SOURCE, EXTERNAL_IR, &closure),
            Err(SourceEvidenceError::CompilerDerivedClosureClaimed)
        );

        for bytes in [
            canonical_manifest_with(|manifest| manifest.grants_load = true),
            canonical_manifest_with(|manifest| manifest.grants_launch = true),
        ] {
            assert_eq!(
                inspect_source_evidence(RUST_DEVICE_SOURCE, EXTERNAL_IR, &bytes),
                Err(SourceEvidenceError::RuntimeAuthorityClaimed)
            );
        }
    }

    #[test]
    fn source_manifest_requires_exact_limitations() {
        let missing = canonical_manifest_with(|manifest| {
            manifest.limitations.pop();
        });
        assert_eq!(
            inspect_source_evidence(RUST_DEVICE_SOURCE, EXTERNAL_IR, &missing),
            Err(SourceEvidenceError::LimitationsMismatch)
        );
    }

    #[test]
    fn contract_set_is_canonical_and_stable() {
        let first = contracts();
        let second = contracts();
        assert_eq!(first, second);
        assert_eq!(first.identity(), second.identity());
        assert_eq!(
            first.rust_import().direction(),
            DeviceFfiDirectionV1::Import
        );
        assert_eq!(
            first.external_definition().direction(),
            DeviceFfiDirectionV1::Export
        );
        assert_eq!(
            first.rust_export().direction(),
            DeviceFfiDirectionV1::Export
        );
        assert_eq!(
            first.external_declaration().direction(),
            DeviceFfiDirectionV1::Import
        );
    }

    #[test]
    fn direction_swap_is_rejected() {
        let valid = contracts();
        let error = BidirectionalSourceContractSetV1::new(
            valid.external_definition().clone(),
            valid.rust_import().clone(),
            valid.rust_export().clone(),
            valid.external_declaration().clone(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            SourceContractSetError::WrongDirection {
                endpoint: SourceContractEndpoint::RustImport,
                ..
            }
        ));
    }

    #[test]
    fn symbol_target_abi_and_semantic_substitution_are_rejected() {
        let valid = contracts();
        let symbol = fixture_contract(
            DeviceFfiDirectionV1::Export,
            "external_scale_bias_v2",
            EXTERNAL_AFFINE_SEMANTIC,
        )
        .unwrap();
        assert_eq!(
            BidirectionalSourceContractSetV1::new(
                valid.rust_import().clone(),
                symbol,
                valid.rust_export().clone(),
                valid.external_declaration().clone(),
            )
            .unwrap_err(),
            SourceContractSetError::SymbolMismatch
        );

        let target = altered_contract(
            DeviceFfiDirectionV1::Export,
            EXTERNAL_AFFINE_SYMBOL,
            "gfx950",
            scalar_binary_abi().unwrap(),
            EXTERNAL_AFFINE_SEMANTIC,
        );
        assert_eq!(
            BidirectionalSourceContractSetV1::new(
                valid.rust_import().clone(),
                target,
                valid.rust_export().clone(),
                valid.external_declaration().clone(),
            )
            .unwrap_err(),
            SourceContractSetError::TargetMismatch
        );

        let changed_abi = DevicePhysicalAbiV1::new(
            vec![
                DeviceFfiAbiTypeV1::Scalar(DeviceFfiScalarV1::U64),
                DeviceFfiAbiTypeV1::Scalar(DeviceFfiScalarV1::U32),
            ],
            Some(DeviceFfiAbiTypeV1::Scalar(DeviceFfiScalarV1::U32)),
        )
        .unwrap();
        let abi = altered_contract(
            DeviceFfiDirectionV1::Export,
            EXTERNAL_AFFINE_SYMBOL,
            FIXTURE_TARGET,
            changed_abi,
            EXTERNAL_AFFINE_SEMANTIC,
        );
        assert_eq!(
            BidirectionalSourceContractSetV1::new(
                valid.rust_import().clone(),
                abi,
                valid.rust_export().clone(),
                valid.external_declaration().clone(),
            )
            .unwrap_err(),
            SourceContractSetError::AbiMismatch
        );

        let semantic = altered_contract(
            DeviceFfiDirectionV1::Export,
            EXTERNAL_AFFINE_SYMBOL,
            FIXTURE_TARGET,
            scalar_binary_abi().unwrap(),
            [0x33; 32],
        );
        assert_eq!(
            BidirectionalSourceContractSetV1::new(
                valid.rust_import().clone(),
                semantic,
                valid.rust_export().clone(),
                valid.external_declaration().clone(),
            )
            .unwrap_err(),
            SourceContractSetError::SemanticIdentityMismatch
        );
    }

    #[test]
    fn independent_cpu_oracle_matches_source_model_for_mismatched_extents() {
        let cases = [
            (vec![], 0),
            (vec![], 3),
            (vec![1, 2, 3], 0),
            (vec![1, 2, 3], 2),
            (vec![1, 2, 3], 3),
            (vec![1, 2, 3], 5),
            (vec![u32::MAX, 0, 0x8000_0000], 7),
        ];
        for (input, output_len) in cases {
            assert_eq!(
                cpu_oracle(&input, output_len, UNTOUCHED),
                evaluate_source_model_path(&input, output_len, UNTOUCHED)
            );
            let output = evaluate_source_model_path(&input, output_len, UNTOUCHED);
            for value in output.iter().skip(input.len()) {
                assert_eq!(*value, UNTOUCHED);
            }
        }
    }

    #[test]
    fn independent_cpu_oracle_matches_random_extent_pairs() {
        let mut values = vec![0, 1, u32::MAX, u32::MAX - 1, 0x8000_0000];
        let mut state = 0x4d59_5df4_u32;
        for _ in 0..1024 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            values.push(state);
        }
        for output_len in [0, 1, 4, values.len() / 2, values.len(), values.len() + 11] {
            assert_eq!(
                cpu_oracle(&values, output_len, UNTOUCHED),
                evaluate_source_model_path(&values, output_len, UNTOUCHED)
            );
        }
    }

    #[test]
    fn rust_source_guards_both_extents_and_external_ir_declares_no_memory_effects() {
        let rust_source = std::str::from_utf8(RUST_DEVICE_SOURCE).unwrap();
        let external_ir = std::str::from_utf8(EXTERNAL_IR).unwrap();
        assert!(rust_source.contains("input.get(lane)"));
        assert!(rust_source.contains("output.get_mut(index)"));
        assert!(external_ir.contains("declare i32 @rust_accumulate_v1(i32, i32) #0"));
        assert!(external_ir.contains("attributes #0 = { memory(none) nounwind"));
    }

    #[test]
    fn fixture_sources_never_name_link_or_runtime_authority() {
        let rust_source = std::str::from_utf8(RUST_DEVICE_SOURCE).unwrap();
        let external_ir = std::str::from_utf8(EXTERNAL_IR).unwrap();
        for forbidden in ["amd_comgr", "ld.lld", "llvm-link"] {
            assert!(!rust_source.contains(forbidden));
            assert!(!external_ir.contains(forbidden));
        }
    }

    #[test]
    fn canonical_abi_spelling_matches_external_manifest() {
        assert_eq!(
            scalar_binary_abi().unwrap().canonical_spelling(),
            SCALAR_BINARY_ABI
        );
    }
}
