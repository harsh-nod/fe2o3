#![forbid(unsafe_code)]

//! Fixture-only G7 contract admission for bidirectional device FFI.
//!
//! The typed value produced here is inert. It binds deterministic evidence but
//! grants no HIP module, function, call, or launch authority.

mod cpu_oracle;

use std::fmt;
use std::marker::PhantomData;

use fe2o3_amd_target::AmdTargetId;
use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
use fe2o3_kernel_descriptor::ffi_contract::{
    DeviceFfiAbiTypeV1, DeviceFfiContractError, DeviceFfiContractV1, DeviceFfiDirectionV1,
    DeviceFfiScalarV1, DeviceFfiSemanticIdentityV1, DevicePhysicalAbiV1,
};

pub use cpu_oracle::evaluate as cpu_oracle;

pub const FIXTURE_TARGET: &str = "gfx942:sramecc+:xnack-";
pub const FIXTURE_CODE_OBJECT_VERSION: u16 = 5;
pub const EXTERNAL_AFFINE_SYMBOL: &str = "external_scale_bias_v1";
pub const RUST_ACCUMULATE_SYMBOL: &str = "rust_accumulate_v1";

const CONTRACT_SET_DOMAIN: &[u8] = b"fe2o3.device-link-ffi.fixture.v1\0";
const EXTERNAL_AFFINE_SEMANTIC: [u8; 32] = [0x11; 32];
const RUST_ACCUMULATE_SEMANTIC: [u8; 32] = [0x22; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractEndpoint {
    RustImport,
    ExternalExport,
    RustExport,
    ExternalImport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractSetError {
    Contract(DeviceFfiContractError),
    WrongDirection {
        endpoint: ContractEndpoint,
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

impl fmt::Display for ContractSetError {
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

impl std::error::Error for ContractSetError {}

impl From<DeviceFfiContractError> for ContractSetError {
    fn from(error: DeviceFfiContractError) -> Self {
        Self::Contract(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BidirectionalContractSetV1 {
    rust_import: DeviceFfiContractV1,
    external_export: DeviceFfiContractV1,
    rust_export: DeviceFfiContractV1,
    external_import: DeviceFfiContractV1,
    identity: PayloadDigest,
}

impl BidirectionalContractSetV1 {
    pub fn new(
        rust_import: DeviceFfiContractV1,
        external_export: DeviceFfiContractV1,
        rust_export: DeviceFfiContractV1,
        external_import: DeviceFfiContractV1,
    ) -> Result<Self, ContractSetError> {
        require_direction(
            ContractEndpoint::RustImport,
            &rust_import,
            DeviceFfiDirectionV1::Import,
        )?;
        require_direction(
            ContractEndpoint::ExternalExport,
            &external_export,
            DeviceFfiDirectionV1::Export,
        )?;
        require_direction(
            ContractEndpoint::RustExport,
            &rust_export,
            DeviceFfiDirectionV1::Export,
        )?;
        require_direction(
            ContractEndpoint::ExternalImport,
            &external_import,
            DeviceFfiDirectionV1::Import,
        )?;
        require_counterparts(&rust_import, &external_export)?;
        require_counterparts(&external_import, &rust_export)?;

        let mut bytes = CONTRACT_SET_DOMAIN.to_vec();
        for contract in [
            &rust_import,
            &external_export,
            &rust_export,
            &external_import,
        ] {
            let record = contract.canonical_link_record();
            bytes.extend_from_slice(&(record.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&record);
        }
        let identity = DigestAlgorithm::Sha256.calculate(&bytes);
        Ok(Self {
            rust_import,
            external_export,
            rust_export,
            external_import,
            identity,
        })
    }

    pub fn fixture() -> Result<Self, ContractSetError> {
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

    pub const fn external_export(&self) -> &DeviceFfiContractV1 {
        &self.external_export
    }

    pub const fn rust_export(&self) -> &DeviceFfiContractV1 {
        &self.rust_export
    }

    pub const fn external_import(&self) -> &DeviceFfiContractV1 {
        &self.external_import
    }
}

fn require_direction(
    endpoint: ContractEndpoint,
    contract: &DeviceFfiContractV1,
    expected: DeviceFfiDirectionV1,
) -> Result<(), ContractSetError> {
    let actual = contract.direction();
    if actual == expected {
        Ok(())
    } else {
        Err(ContractSetError::WrongDirection {
            endpoint,
            expected,
            actual,
        })
    }
}

fn require_counterparts(
    import: &DeviceFfiContractV1,
    export: &DeviceFfiContractV1,
) -> Result<(), ContractSetError> {
    if import.symbol() != export.symbol() {
        return Err(ContractSetError::SymbolMismatch);
    }
    if import.target() != export.target() {
        return Err(ContractSetError::TargetMismatch);
    }
    if import.code_object_version() != export.code_object_version() {
        return Err(ContractSetError::CodeObjectVersionMismatch);
    }
    if import.abi() != export.abi() {
        return Err(ContractSetError::AbiMismatch);
    }
    if import.effects() != export.effects() {
        return Err(ContractSetError::EffectsMismatch);
    }
    if import.semantic_identity() != export.semantic_identity() {
        return Err(ContractSetError::SemanticIdentityMismatch);
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ContextIdentityV1([u8; 32]);

impl ContextIdentityV1 {
    pub const fn from_opaque_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ObservedFixtureContextV1 {
    identity: ContextIdentityV1,
    target: AmdTargetId,
}

impl ObservedFixtureContextV1 {
    pub const fn new(identity: ContextIdentityV1, target: AmdTargetId) -> Self {
        Self { identity, target }
    }
}

pub struct UntrustedLinkedArtifactV1<'a> {
    pub payload: &'a [u8],
    pub claimed_payload_digest: PayloadDigest,
    pub target: AmdTargetId,
    pub contracts: &'a BidirectionalContractSetV1,
}

pub struct FixtureLoadRequestV1 {
    expected_payload_digest: PayloadDigest,
    expected_contract_identity: PayloadDigest,
    expected_target: AmdTargetId,
    expected_context: ContextIdentityV1,
}

impl FixtureLoadRequestV1 {
    pub fn new(
        expected_payload: &[u8],
        expected_contracts: &BidirectionalContractSetV1,
        expected_context: ContextIdentityV1,
    ) -> Self {
        Self {
            expected_payload_digest: DigestAlgorithm::Sha256.calculate(expected_payload),
            expected_contract_identity: expected_contracts.identity(),
            expected_target: AmdTargetId::parse(FIXTURE_TARGET)
                .expect("fixture target is canonical"),
            expected_context,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypedAdmissionError {
    ClaimedPayloadDigestMismatch,
    WrongArtifactIdentity,
    WrongContractIdentity,
    WrongArtifactTarget,
    WrongContextTarget,
    WrongContext,
}

impl fmt::Display for TypedAdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ClaimedPayloadDigestMismatch => "payload does not match its claimed digest",
            Self::WrongArtifactIdentity => "payload does not match the requested artifact",
            Self::WrongContractIdentity => "artifact carries the wrong device FFI closure",
            Self::WrongArtifactTarget => "artifact target does not match the typed request",
            Self::WrongContextTarget => "observed target does not match the typed request",
            Self::WrongContext => "observed context does not match the typed request",
        })
    }
}

impl std::error::Error for TypedAdmissionError {}

#[derive(Debug)]
pub enum BidirectionalFixtureKernelV1 {}

/// Inert evidence that one candidate matched this fixture's typed load request.
///
/// This type intentionally has no method that loads, calls, or launches code.
#[derive(Debug)]
pub struct TypedLoadAdmissionV1<K> {
    payload_digest: PayloadDigest,
    contract_identity: PayloadDigest,
    target: AmdTargetId,
    context: ContextIdentityV1,
    _kernel: PhantomData<fn() -> K>,
}

impl<K> TypedLoadAdmissionV1<K> {
    pub const fn payload_digest(&self) -> PayloadDigest {
        self.payload_digest
    }

    pub const fn contract_identity(&self) -> PayloadDigest {
        self.contract_identity
    }

    pub const fn target(&self) -> AmdTargetId {
        self.target
    }

    pub fn revalidate(
        &self,
        artifact: &UntrustedLinkedArtifactV1<'_>,
        observed: ObservedFixtureContextV1,
    ) -> Result<(), TypedAdmissionError> {
        validate_candidate(
            artifact,
            observed,
            self.payload_digest,
            self.contract_identity,
            self.target,
            self.context,
        )
    }
}

pub fn admit_fixture_artifact(
    request: &FixtureLoadRequestV1,
    artifact: &UntrustedLinkedArtifactV1<'_>,
    observed: ObservedFixtureContextV1,
) -> Result<TypedLoadAdmissionV1<BidirectionalFixtureKernelV1>, TypedAdmissionError> {
    validate_candidate(
        artifact,
        observed,
        request.expected_payload_digest,
        request.expected_contract_identity,
        request.expected_target,
        request.expected_context,
    )?;
    Ok(TypedLoadAdmissionV1 {
        payload_digest: request.expected_payload_digest,
        contract_identity: request.expected_contract_identity,
        target: request.expected_target,
        context: request.expected_context,
        _kernel: PhantomData,
    })
}

fn validate_candidate(
    artifact: &UntrustedLinkedArtifactV1<'_>,
    observed: ObservedFixtureContextV1,
    expected_payload_digest: PayloadDigest,
    expected_contract_identity: PayloadDigest,
    expected_target: AmdTargetId,
    expected_context: ContextIdentityV1,
) -> Result<(), TypedAdmissionError> {
    artifact
        .claimed_payload_digest
        .verify(artifact.payload)
        .map_err(|_| TypedAdmissionError::ClaimedPayloadDigestMismatch)?;
    if artifact.claimed_payload_digest != expected_payload_digest {
        return Err(TypedAdmissionError::WrongArtifactIdentity);
    }
    if artifact.contracts.identity() != expected_contract_identity {
        return Err(TypedAdmissionError::WrongContractIdentity);
    }
    if artifact.target != expected_target {
        return Err(TypedAdmissionError::WrongArtifactTarget);
    }
    if observed.target != expected_target {
        return Err(TypedAdmissionError::WrongContextTarget);
    }
    if observed.identity != expected_context {
        return Err(TypedAdmissionError::WrongContext);
    }
    Ok(())
}

/// CPU emulation of the two device FFI calls, used only as a test comparator.
pub fn emulate_linked_device_path(input: &[u32]) -> Vec<u32> {
    input
        .iter()
        .enumerate()
        .map(|(lane, value)| external_scale_bias(*value, lane as u32))
        .collect()
}

fn external_scale_bias(value: u32, lane: u32) -> u32 {
    rust_accumulate(value.wrapping_mul(3).wrapping_add(5), lane)
}

fn rust_accumulate(value: u32, lane: u32) -> u32 {
    value.wrapping_add(lane)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUST_DEVICE_SOURCE: &str =
        include_str!("../../../tests/fixtures/device-link/rust-device/src/lib.rs");
    const EXTERNAL_IR: &str =
        include_str!("../../../tests/fixtures/device-link/external.amdgpu.ll");
    const PAYLOAD: &[u8] = b"fixture-linked-hsaco-evidence-only";
    const CONTEXT: ContextIdentityV1 = ContextIdentityV1::from_opaque_bytes([0x41; 32]);

    fn target() -> AmdTargetId {
        AmdTargetId::parse(FIXTURE_TARGET).unwrap()
    }

    fn context() -> ObservedFixtureContextV1 {
        ObservedFixtureContextV1::new(CONTEXT, target())
    }

    fn admitted_parts() -> (BidirectionalContractSetV1, FixtureLoadRequestV1) {
        let contracts = BidirectionalContractSetV1::fixture().unwrap();
        let request = FixtureLoadRequestV1::new(PAYLOAD, &contracts, CONTEXT);
        (contracts, request)
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

    #[test]
    fn sources_form_one_bidirectional_call_chain() {
        for required in [
            "#[device_import(",
            "symbol = \"external_scale_bias_v1\"",
            "#[device_export(",
            "symbol = \"rust_accumulate_v1\"",
            "external_scale_bias(value, lane as u32)",
        ] {
            assert!(
                RUST_DEVICE_SOURCE.contains(required),
                "missing `{required}`"
            );
        }
        for required in [
            "define protected i32 @external_scale_bias_v1",
            "declare i32 @rust_accumulate_v1",
            "call i32 @rust_accumulate_v1",
        ] {
            assert!(EXTERNAL_IR.contains(required), "missing `{required}`");
        }
        for forbidden in ["amd_comgr", "ld.lld", "llvm-link"] {
            assert!(!RUST_DEVICE_SOURCE.contains(forbidden));
            assert!(!EXTERNAL_IR.contains(forbidden));
        }
    }

    #[test]
    fn contract_set_is_canonical_and_stable() {
        let first = BidirectionalContractSetV1::fixture().unwrap();
        let second = BidirectionalContractSetV1::fixture().unwrap();
        assert_eq!(first, second);
        assert_eq!(first.identity(), second.identity());
        assert_eq!(
            first.rust_import().direction(),
            DeviceFfiDirectionV1::Import
        );
        assert_eq!(
            first.external_export().direction(),
            DeviceFfiDirectionV1::Export
        );
        assert_eq!(
            first.rust_export().direction(),
            DeviceFfiDirectionV1::Export
        );
        assert_eq!(
            first.external_import().direction(),
            DeviceFfiDirectionV1::Import
        );
    }

    #[test]
    fn direction_swap_is_rejected() {
        let valid = BidirectionalContractSetV1::fixture().unwrap();
        let error = BidirectionalContractSetV1::new(
            valid.external_export().clone(),
            valid.rust_import().clone(),
            valid.rust_export().clone(),
            valid.external_import().clone(),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ContractSetError::WrongDirection {
                endpoint: ContractEndpoint::RustImport,
                ..
            }
        ));
    }

    #[test]
    fn symbol_substitution_is_rejected() {
        let valid = BidirectionalContractSetV1::fixture().unwrap();
        let replacement = fixture_contract(
            DeviceFfiDirectionV1::Export,
            "external_scale_bias_v2",
            EXTERNAL_AFFINE_SEMANTIC,
        )
        .unwrap();
        let error = BidirectionalContractSetV1::new(
            valid.rust_import().clone(),
            replacement,
            valid.rust_export().clone(),
            valid.external_import().clone(),
        )
        .unwrap_err();
        assert_eq!(error, ContractSetError::SymbolMismatch);
    }

    #[test]
    fn target_substitution_is_rejected() {
        let valid = BidirectionalContractSetV1::fixture().unwrap();
        let replacement = altered_contract(
            DeviceFfiDirectionV1::Export,
            EXTERNAL_AFFINE_SYMBOL,
            "gfx950",
            scalar_binary_abi().unwrap(),
            EXTERNAL_AFFINE_SEMANTIC,
        );
        let error = BidirectionalContractSetV1::new(
            valid.rust_import().clone(),
            replacement,
            valid.rust_export().clone(),
            valid.external_import().clone(),
        )
        .unwrap_err();
        assert_eq!(error, ContractSetError::TargetMismatch);
    }

    #[test]
    fn abi_substitution_is_rejected() {
        let valid = BidirectionalContractSetV1::fixture().unwrap();
        let changed_abi = DevicePhysicalAbiV1::new(
            vec![
                DeviceFfiAbiTypeV1::Scalar(DeviceFfiScalarV1::U64),
                DeviceFfiAbiTypeV1::Scalar(DeviceFfiScalarV1::U32),
            ],
            Some(DeviceFfiAbiTypeV1::Scalar(DeviceFfiScalarV1::U32)),
        )
        .unwrap();
        let replacement = altered_contract(
            DeviceFfiDirectionV1::Export,
            EXTERNAL_AFFINE_SYMBOL,
            FIXTURE_TARGET,
            changed_abi,
            EXTERNAL_AFFINE_SEMANTIC,
        );
        let error = BidirectionalContractSetV1::new(
            valid.rust_import().clone(),
            replacement,
            valid.rust_export().clone(),
            valid.external_import().clone(),
        )
        .unwrap_err();
        assert_eq!(error, ContractSetError::AbiMismatch);
    }

    #[test]
    fn semantic_identity_substitution_is_rejected() {
        let valid = BidirectionalContractSetV1::fixture().unwrap();
        let replacement = altered_contract(
            DeviceFfiDirectionV1::Export,
            EXTERNAL_AFFINE_SYMBOL,
            FIXTURE_TARGET,
            scalar_binary_abi().unwrap(),
            [0x33; 32],
        );
        let error = BidirectionalContractSetV1::new(
            valid.rust_import().clone(),
            replacement,
            valid.rust_export().clone(),
            valid.external_import().clone(),
        )
        .unwrap_err();
        assert_eq!(error, ContractSetError::SemanticIdentityMismatch);
    }

    #[test]
    fn exact_candidate_produces_only_an_inert_typed_admission() {
        let (contracts, request) = admitted_parts();
        let artifact = UntrustedLinkedArtifactV1 {
            payload: PAYLOAD,
            claimed_payload_digest: DigestAlgorithm::Sha256.calculate(PAYLOAD),
            target: target(),
            contracts: &contracts,
        };
        let token = admit_fixture_artifact(&request, &artifact, context()).unwrap();
        assert_eq!(token.payload_digest(), artifact.claimed_payload_digest);
        assert_eq!(token.contract_identity(), contracts.identity());
        assert_eq!(token.target(), target());
        token.revalidate(&artifact, context()).unwrap();
    }

    #[test]
    fn changed_payload_and_replayed_digest_are_rejected() {
        let (contracts, request) = admitted_parts();
        let changed = b"changed-linked-image";
        let stale_claim = UntrustedLinkedArtifactV1 {
            payload: changed,
            claimed_payload_digest: DigestAlgorithm::Sha256.calculate(PAYLOAD),
            target: target(),
            contracts: &contracts,
        };
        assert_eq!(
            admit_fixture_artifact(&request, &stale_claim, context()).unwrap_err(),
            TypedAdmissionError::ClaimedPayloadDigestMismatch
        );

        let substituted = UntrustedLinkedArtifactV1 {
            payload: changed,
            claimed_payload_digest: DigestAlgorithm::Sha256.calculate(changed),
            target: target(),
            contracts: &contracts,
        };
        assert_eq!(
            admit_fixture_artifact(&request, &substituted, context()).unwrap_err(),
            TypedAdmissionError::WrongArtifactIdentity
        );
    }

    #[test]
    fn wrong_contract_closure_is_rejected() {
        let (contracts, request) = admitted_parts();
        let alternate = BidirectionalContractSetV1::new(
            fixture_contract(
                DeviceFfiDirectionV1::Import,
                EXTERNAL_AFFINE_SYMBOL,
                [0x44; 32],
            )
            .unwrap(),
            fixture_contract(
                DeviceFfiDirectionV1::Export,
                EXTERNAL_AFFINE_SYMBOL,
                [0x44; 32],
            )
            .unwrap(),
            contracts.rust_export().clone(),
            contracts.external_import().clone(),
        )
        .unwrap();
        let artifact = UntrustedLinkedArtifactV1 {
            payload: PAYLOAD,
            claimed_payload_digest: DigestAlgorithm::Sha256.calculate(PAYLOAD),
            target: target(),
            contracts: &alternate,
        };
        assert_eq!(
            admit_fixture_artifact(&request, &artifact, context()).unwrap_err(),
            TypedAdmissionError::WrongContractIdentity
        );
    }

    #[test]
    fn artifact_and_context_targets_are_checked_independently() {
        let (contracts, request) = admitted_parts();
        let wrong_artifact = UntrustedLinkedArtifactV1 {
            payload: PAYLOAD,
            claimed_payload_digest: DigestAlgorithm::Sha256.calculate(PAYLOAD),
            target: AmdTargetId::parse("gfx950").unwrap(),
            contracts: &contracts,
        };
        assert_eq!(
            admit_fixture_artifact(&request, &wrong_artifact, context()).unwrap_err(),
            TypedAdmissionError::WrongArtifactTarget
        );

        let artifact = UntrustedLinkedArtifactV1 {
            target: target(),
            ..wrong_artifact
        };
        let wrong_context =
            ObservedFixtureContextV1::new(CONTEXT, AmdTargetId::parse("gfx950").unwrap());
        assert_eq!(
            admit_fixture_artifact(&request, &artifact, wrong_context).unwrap_err(),
            TypedAdmissionError::WrongContextTarget
        );
    }

    #[test]
    fn exact_context_identity_is_required_and_revalidated() {
        let (contracts, request) = admitted_parts();
        let artifact = UntrustedLinkedArtifactV1 {
            payload: PAYLOAD,
            claimed_payload_digest: DigestAlgorithm::Sha256.calculate(PAYLOAD),
            target: target(),
            contracts: &contracts,
        };
        let wrong_context = ObservedFixtureContextV1::new(
            ContextIdentityV1::from_opaque_bytes([0x42; 32]),
            target(),
        );
        assert_eq!(
            admit_fixture_artifact(&request, &artifact, wrong_context).unwrap_err(),
            TypedAdmissionError::WrongContext
        );
        let token = admit_fixture_artifact(&request, &artifact, context()).unwrap();
        assert_eq!(
            token.revalidate(&artifact, wrong_context).unwrap_err(),
            TypedAdmissionError::WrongContext
        );
    }

    #[test]
    fn independent_cpu_oracle_matches_boundary_emulation() {
        let mut values = vec![0, 1, u32::MAX, u32::MAX - 1, 0x8000_0000];
        let mut state = 0x4d59_5df4_u32;
        for _ in 0..1024 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            values.push(state);
        }
        assert_eq!(cpu_oracle(&values), emulate_linked_device_path(&values));
    }
}
