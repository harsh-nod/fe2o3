use std::fmt;

use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
use fe2o3_contracts::{MAX_SOURCE_INTEGER_SWITCHES_V1, MAX_SOURCE_LOOPS_V1};
use fe2o3_rustc_front::{
    ControlFlowContractV1, ControlFlowDecodeErrorV1, ControlFlowNodeKindV1,
    MAX_CONTROL_FLOW_CONTRACT_BYTES_V1, decode_control_flow_contract_v1,
};

use crate::{
    AuthenticatedProofExecutableBindingError, AuthenticatedProofExecutableBindingV1, Digest,
    PersistentlyFreshProofExecutableBindingV1, ProofOutcome, ProofProperty, ProofRequestV1,
    ProofTargetIdentity,
};

pub const CONTROL_FLOW_BINDING_VERSION_V1: u16 = 1;
pub const CONTROL_FLOW_SOURCE_BINDING_DOMAIN_V1: [u8; 8] = *b"FE2CFSB\0";
pub const CONTROL_FLOW_FUNCTIONAL_SPECIFICATION_DOMAIN_V1: [u8; 8] = *b"FE2CFFS\0";
pub const CONTROL_FLOW_REQUEST_BINDING_DOMAIN_V1: [u8; 8] = *b"FE2CFRQ\0";
pub const AUTHENTICATED_CONTROL_FLOW_EXECUTABLE_BINDING_DOMAIN_V1: [u8; 8] = *b"FE2ACFB\0";
pub const PERSISTENT_AUTHENTICATED_CONTROL_FLOW_EXECUTABLE_BINDING_DOMAIN_V1: [u8; 8] =
    *b"FE2PCFB\0";
pub const MAX_BOUND_CONTROL_FLOW_LOOPS_V1: usize = MAX_SOURCE_LOOPS_V1 as usize;
pub const MAX_BOUND_CONTROL_FLOW_SWITCHES_V1: usize = MAX_SOURCE_INTEGER_SWITCHES_V1 as usize;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ControlFlowLoopClaimV1 {
    node_id: u32,
    max_iterations: u32,
}

impl ControlFlowLoopClaimV1 {
    pub const fn new(node_id: u32, max_iterations: u32) -> Result<Self, ControlFlowBindingErrorV1> {
        if max_iterations == 0 {
            return Err(ControlFlowBindingErrorV1::ZeroLoopBound { node_id });
        }
        Ok(Self {
            node_id,
            max_iterations,
        })
    }

    pub const fn node_id(self) -> u32 {
        self.node_id
    }

    pub const fn max_iterations(self) -> u32 {
        self.max_iterations
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ControlFlowIntegerSwitchCaseClaimV1 {
    bits: u128,
    target_node_id: u32,
}

impl ControlFlowIntegerSwitchCaseClaimV1 {
    pub const fn new(bits: u128, target_node_id: u32) -> Self {
        Self {
            bits,
            target_node_id,
        }
    }

    pub const fn bits(self) -> u128 {
        self.bits
    }

    pub const fn target_node_id(self) -> u32 {
        self.target_node_id
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ControlFlowIntegerSwitchClaimV1 {
    node_id: u32,
    width: u16,
    signed: bool,
    cases: Vec<ControlFlowIntegerSwitchCaseClaimV1>,
    default_target_node_id: u32,
}

impl ControlFlowIntegerSwitchClaimV1 {
    pub fn new(
        node_id: u32,
        width: u16,
        signed: bool,
        mut cases: Vec<ControlFlowIntegerSwitchCaseClaimV1>,
        default_target_node_id: u32,
    ) -> Result<Self, ControlFlowBindingErrorV1> {
        if !matches!(width, 8 | 16 | 32 | 64 | 128) {
            return Err(ControlFlowBindingErrorV1::UnsupportedIntegerWidth { node_id, width });
        }
        if cases.len() > fe2o3_contracts::MAX_INTEGER_SWITCH_CASES_V1 as usize {
            return Err(ControlFlowBindingErrorV1::TooManySwitchCases {
                node_id,
                max: fe2o3_contracts::MAX_INTEGER_SWITCH_CASES_V1 as usize,
            });
        }
        for case in &cases {
            if !accepts_integer_bits(width, signed, case.bits) {
                return Err(ControlFlowBindingErrorV1::IntegerCaseOutOfRange {
                    node_id,
                    bits: case.bits,
                });
            }
        }
        cases.sort_unstable_by(|left, right| {
            compare_integer_bits(signed, left.bits, right.bits)
                .then_with(|| left.target_node_id.cmp(&right.target_node_id))
        });
        if let Some(case) = cases
            .windows(2)
            .find_map(|pair| (pair[0].bits == pair[1].bits).then_some(pair[1]))
        {
            return Err(ControlFlowBindingErrorV1::DuplicateIntegerCase {
                node_id,
                bits: case.bits,
            });
        }
        Ok(Self {
            node_id,
            width,
            signed,
            cases,
            default_target_node_id,
        })
    }

    pub const fn node_id(&self) -> u32 {
        self.node_id
    }

    pub const fn width(&self) -> u16 {
        self.width
    }

    pub const fn is_signed(&self) -> bool {
        self.signed
    }

    pub fn cases(&self) -> &[ControlFlowIntegerSwitchCaseClaimV1] {
        &self.cases
    }

    pub const fn default_target_node_id(&self) -> u32 {
        self.default_target_node_id
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ControlFlowClaimsV1 {
    loops: Vec<ControlFlowLoopClaimV1>,
    integer_switches: Vec<ControlFlowIntegerSwitchClaimV1>,
}

impl ControlFlowClaimsV1 {
    pub fn new(
        mut loops: Vec<ControlFlowLoopClaimV1>,
        mut integer_switches: Vec<ControlFlowIntegerSwitchClaimV1>,
    ) -> Result<Self, ControlFlowBindingErrorV1> {
        if loops.len() > MAX_BOUND_CONTROL_FLOW_LOOPS_V1 {
            return Err(ControlFlowBindingErrorV1::TooManyClaims {
                field: "loop claims",
                max: MAX_BOUND_CONTROL_FLOW_LOOPS_V1,
            });
        }
        if integer_switches.len() > MAX_BOUND_CONTROL_FLOW_SWITCHES_V1 {
            return Err(ControlFlowBindingErrorV1::TooManyClaims {
                field: "integer-switch claims",
                max: MAX_BOUND_CONTROL_FLOW_SWITCHES_V1,
            });
        }
        loops.sort_unstable_by_key(|claim| claim.node_id);
        integer_switches.sort_unstable_by_key(|claim| claim.node_id);
        if let Some(pair) = loops
            .windows(2)
            .find(|pair| pair[0].node_id == pair[1].node_id)
        {
            return Err(ControlFlowBindingErrorV1::DuplicateNodeClaim {
                node_id: pair[1].node_id,
            });
        }
        if let Some(pair) = integer_switches
            .windows(2)
            .find(|pair| pair[0].node_id == pair[1].node_id)
        {
            return Err(ControlFlowBindingErrorV1::DuplicateNodeClaim {
                node_id: pair[1].node_id,
            });
        }
        if let Some(node_id) = loops.iter().find_map(|loop_claim| {
            integer_switches
                .binary_search_by_key(&loop_claim.node_id, |claim| claim.node_id)
                .is_ok()
                .then_some(loop_claim.node_id)
        }) {
            return Err(ControlFlowBindingErrorV1::DuplicateNodeClaim { node_id });
        }
        Ok(Self {
            loops,
            integer_switches,
        })
    }

    pub fn loops(&self) -> &[ControlFlowLoopClaimV1] {
        &self.loops
    }

    pub fn integer_switches(&self) -> &[ControlFlowIntegerSwitchClaimV1] {
        &self.integer_switches
    }

    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut writer = IdentityWriter::with_domain(*b"FE2CFCL\0");
        writer.u16(self.loops.len() as u16);
        writer.u16(self.integer_switches.len() as u16);
        for claim in &self.loops {
            writer.u32(claim.node_id);
            writer.u32(claim.max_iterations);
        }
        for claim in &self.integer_switches {
            writer.u32(claim.node_id);
            writer.u16(claim.width);
            writer.u8(u8::from(claim.signed));
            writer.u8(0);
            writer.u16(claim.cases.len() as u16);
            writer.u16(0);
            writer.u32(claim.default_target_node_id);
            for case in &claim.cases {
                writer.u128(case.bits);
                writer.u32(case.target_node_id);
            }
        }
        writer.finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlFlowPayloadIdentityV1 {
    byte_len: u32,
    digest: Digest,
}

impl ControlFlowPayloadIdentityV1 {
    pub const fn byte_len(self) -> u32 {
        self.byte_len
    }

    pub const fn digest(self) -> Digest {
        self.digest
    }
}

/// Canonical reconciliation of one source sidecar, its emitted structural CFG
/// identity, and the exact claims intended as proof inputs.
///
/// This value is descriptive input identity only. It is constructible without
/// a compiler or verifier and grants no lowering or execution authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlFlowSourceBindingV1 {
    source_contract: ControlFlowPayloadIdentityV1,
    cfg_identity: ControlFlowPayloadIdentityV1,
    claims: ControlFlowClaimsV1,
    binding_identity: Digest,
}

impl ControlFlowSourceBindingV1 {
    pub const fn source_contract(&self) -> ControlFlowPayloadIdentityV1 {
        self.source_contract
    }

    pub const fn cfg_identity(&self) -> ControlFlowPayloadIdentityV1 {
        self.cfg_identity
    }

    pub const fn claims(&self) -> &ControlFlowClaimsV1 {
        &self.claims
    }

    pub const fn binding_identity(&self) -> Digest {
        self.binding_identity
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
}

/// Exact proof-request identity carrying one reconciled source control-flow
/// binding through the functional-specification axis.
///
/// Construction checks request properties and identities. It does not claim
/// that Verus ran or that the executable refines the source CFG.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlFlowProofRequestBindingV1 {
    source: ControlFlowSourceBindingV1,
    base_functional_specification_digest: Digest,
    functional_specification_digest: Digest,
    target: ProofTargetIdentity,
    request_digest: Digest,
    binding_identity: Digest,
}

impl ControlFlowProofRequestBindingV1 {
    pub const fn source(&self) -> &ControlFlowSourceBindingV1 {
        &self.source
    }

    pub const fn base_functional_specification_digest(&self) -> Digest {
        self.base_functional_specification_digest
    }

    pub const fn functional_specification_digest(&self) -> Digest {
        self.functional_specification_digest
    }

    pub const fn target(&self) -> ProofTargetIdentity {
        self.target
    }

    pub const fn request_digest(&self) -> Digest {
        self.request_digest
    }

    pub const fn binding_identity(&self) -> Digest {
        self.binding_identity
    }

    pub const fn grants_proof_authority(&self) -> bool {
        false
    }
}

/// Inert evidence joining exact source control flow to one measured proof and
/// finalized executable identity.
///
/// Construction requires the private-construction authenticated Verus bridge;
/// a source declaration or request binding alone cannot create this value. The
/// result is still evidence only and deliberately grants no compiler, module,
/// or launch authority.
///
/// ```compile_fail
/// # fn cannot_launch(value: fe2o3_verifier::AuthenticatedControlFlowExecutableBindingV1) {
/// value.launch();
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthenticatedControlFlowExecutableBindingV1 {
    request_binding: ControlFlowProofRequestBindingV1,
    proof_executable_binding: AuthenticatedProofExecutableBindingV1,
    binding_identity: Digest,
}

impl AuthenticatedControlFlowExecutableBindingV1 {
    pub const fn version(&self) -> u16 {
        CONTROL_FLOW_BINDING_VERSION_V1
    }

    pub const fn request_binding(&self) -> &ControlFlowProofRequestBindingV1 {
        &self.request_binding
    }

    pub const fn proof_executable_binding(&self) -> &AuthenticatedProofExecutableBindingV1 {
        &self.proof_executable_binding
    }

    pub const fn binding_identity(&self) -> Digest {
        self.binding_identity
    }

    pub fn validate_against(&self, actual: &Self) -> Result<(), ControlFlowBindingErrorV1> {
        if self.request_binding != actual.request_binding {
            return Err(ControlFlowBindingErrorV1::IdentityMismatch {
                field: "control-flow request binding",
            });
        }
        self.proof_executable_binding
            .validate_against(&actual.proof_executable_binding)
            .map_err(ControlFlowBindingErrorV1::AuthenticatedExecutableBinding)?;
        if self.binding_identity != actual.binding_identity {
            return Err(ControlFlowBindingErrorV1::IdentityMismatch {
                field: "authenticated control-flow binding",
            });
        }
        Ok(())
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Control-flow evidence whose proof/executable input carries a durable
/// freshness receipt in its canonical identity.
///
/// This non-clone type can only be constructed from
/// `PersistentlyFreshProofExecutableBindingV1`. A process-local proof binding
/// therefore cannot satisfy APIs that require persistent freshness.
///
/// ```compile_fail
/// # fn require_persistent(
/// #   request: fe2o3_verifier::ControlFlowProofRequestBindingV1,
/// #   local: fe2o3_verifier::AuthenticatedProofExecutableBindingV1,
/// # ) {
/// fe2o3_verifier::bind_persistently_fresh_authenticated_control_flow_executable_v1(
///     request,
///     local,
/// );
/// # }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1 {
    request_binding: ControlFlowProofRequestBindingV1,
    proof_executable_binding: PersistentlyFreshProofExecutableBindingV1,
    binding_identity: Digest,
}

impl PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1 {
    pub const fn version(&self) -> u16 {
        CONTROL_FLOW_BINDING_VERSION_V1
    }

    pub const fn request_binding(&self) -> &ControlFlowProofRequestBindingV1 {
        &self.request_binding
    }

    pub const fn proof_executable_binding(&self) -> &PersistentlyFreshProofExecutableBindingV1 {
        &self.proof_executable_binding
    }

    pub const fn binding_identity(&self) -> Digest {
        self.binding_identity
    }

    pub fn validate_against(&self, actual: &Self) -> Result<(), ControlFlowBindingErrorV1> {
        if self.request_binding != actual.request_binding {
            return Err(ControlFlowBindingErrorV1::IdentityMismatch {
                field: "persistent control-flow request binding",
            });
        }
        self.proof_executable_binding
            .validate_against(&actual.proof_executable_binding)
            .map_err(ControlFlowBindingErrorV1::AuthenticatedExecutableBinding)?;
        if self.binding_identity != actual.binding_identity {
            return Err(ControlFlowBindingErrorV1::IdentityMismatch {
                field: "persistent authenticated control-flow binding",
            });
        }
        Ok(())
    }

    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

pub fn reconcile_control_flow_source_v1(
    source_contract_bytes: &[u8],
    emitted_cfg_identity_bytes: &[u8],
    claims: ControlFlowClaimsV1,
) -> Result<ControlFlowSourceBindingV1, ControlFlowBindingErrorV1> {
    if source_contract_bytes.len() > MAX_CONTROL_FLOW_CONTRACT_BYTES_V1 {
        return Err(ControlFlowBindingErrorV1::SourceContractTooLarge);
    }
    if emitted_cfg_identity_bytes.len() > MAX_CONTROL_FLOW_CONTRACT_BYTES_V1 {
        return Err(ControlFlowBindingErrorV1::CfgIdentityTooLarge);
    }
    let contract = decode_control_flow_contract_v1(source_contract_bytes)
        .map_err(ControlFlowBindingErrorV1::SourceContract)?;
    let exact_cfg_identity = contract.cfg_identity();
    if exact_cfg_identity.as_bytes() != emitted_cfg_identity_bytes {
        return Err(ControlFlowBindingErrorV1::CfgIdentityMismatch);
    }
    let observed_claims = claims_from_contract(&contract)?;
    reconcile_claims(&observed_claims, &claims)?;

    let source_contract = payload_identity(source_contract_bytes, "source contract")?;
    let cfg_identity = payload_identity(emitted_cfg_identity_bytes, "CFG identity")?;
    let binding_identity = source_binding_identity(source_contract, cfg_identity, &claims);
    Ok(ControlFlowSourceBindingV1 {
        source_contract,
        cfg_identity,
        claims,
        binding_identity,
    })
}

pub fn derive_control_flow_functional_specification_digest_v1(
    base_functional_specification_digest: Digest,
    source: &ControlFlowSourceBindingV1,
) -> Result<Digest, ControlFlowBindingErrorV1> {
    require_measured(
        base_functional_specification_digest,
        "base functional specification",
    )?;
    let mut writer = IdentityWriter::with_domain(CONTROL_FLOW_FUNCTIONAL_SPECIFICATION_DOMAIN_V1);
    writer.digest(base_functional_specification_digest);
    writer.digest(source.binding_identity);
    Ok(sha256(&writer.finish()))
}

pub fn bind_control_flow_proof_request_v1(
    request: &ProofRequestV1,
    base_functional_specification_digest: Digest,
    source: ControlFlowSourceBindingV1,
) -> Result<ControlFlowProofRequestBindingV1, ControlFlowBindingErrorV1> {
    for property in [ProofProperty::Bounds, ProofProperty::FunctionalCorrectness] {
        if request.properties().binary_search(&property).is_err() {
            return Err(ControlFlowBindingErrorV1::MissingProofProperty { property });
        }
    }
    let functional_specification_digest = derive_control_flow_functional_specification_digest_v1(
        base_functional_specification_digest,
        &source,
    )?;
    let target = request.target();
    if target.functional_specification_digest != functional_specification_digest {
        return Err(ControlFlowBindingErrorV1::FunctionalSpecificationMismatch);
    }
    let request_digest = sha256(&request.to_canonical_bytes());
    let mut writer = IdentityWriter::with_domain(CONTROL_FLOW_REQUEST_BINDING_DOMAIN_V1);
    writer.digest(source.binding_identity);
    writer.digest(base_functional_specification_digest);
    writer.digest(functional_specification_digest);
    writer.digest(request_digest);
    for digest in target.digests() {
        writer.digest(digest);
    }
    let binding_identity = sha256(&writer.finish());
    Ok(ControlFlowProofRequestBindingV1 {
        source,
        base_functional_specification_digest,
        functional_specification_digest,
        target,
        request_digest,
        binding_identity,
    })
}

pub fn bind_authenticated_control_flow_executable_v1(
    request_binding: ControlFlowProofRequestBindingV1,
    proof_executable_binding: AuthenticatedProofExecutableBindingV1,
) -> Result<AuthenticatedControlFlowExecutableBindingV1, ControlFlowBindingErrorV1> {
    validate_authenticated_control_flow_executable(&request_binding, &proof_executable_binding)?;
    let binding_identity =
        authenticated_control_flow_binding_identity(&request_binding, &proof_executable_binding);
    Ok(AuthenticatedControlFlowExecutableBindingV1 {
        request_binding,
        proof_executable_binding,
        binding_identity,
    })
}

pub fn bind_persistently_fresh_authenticated_control_flow_executable_v1(
    request_binding: ControlFlowProofRequestBindingV1,
    proof_executable_binding: PersistentlyFreshProofExecutableBindingV1,
) -> Result<PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1, ControlFlowBindingErrorV1>
{
    validate_authenticated_control_flow_executable(
        &request_binding,
        proof_executable_binding.proof_binding(),
    )?;
    let authenticated_identity = authenticated_control_flow_binding_identity(
        &request_binding,
        proof_executable_binding.proof_binding(),
    );
    let persistent = proof_executable_binding.identity();
    let consumed = persistent.consumed_execution();
    let mut writer = IdentityWriter::with_domain(
        PERSISTENT_AUTHENTICATED_CONTROL_FLOW_EXECUTABLE_BINDING_DOMAIN_V1,
    );
    writer.digest(authenticated_identity);
    writer.digest(request_binding.binding_identity);
    writer.digest(proof_executable_binding.binding_identity());
    writer.digest(consumed.challenge());
    writer.digest(consumed.transcript());
    writer.digest(consumed.result());
    writer.digest(persistent.ledger_namespace());
    writer.u64(persistent.ledger_generation());
    writer.digest(persistent.ledger_state_identity());
    let binding_identity = sha256(&writer.finish());
    Ok(
        PersistentlyFreshAuthenticatedControlFlowExecutableBindingV1 {
            request_binding,
            proof_executable_binding,
            binding_identity,
        },
    )
}

fn validate_authenticated_control_flow_executable(
    request_binding: &ControlFlowProofRequestBindingV1,
    proof_executable_binding: &AuthenticatedProofExecutableBindingV1,
) -> Result<(), ControlFlowBindingErrorV1> {
    let evidence = proof_executable_binding.execution_evidence();
    let request = evidence.invocation_plan().request();
    if sha256(&request.to_canonical_bytes()) != request_binding.request_digest {
        return Err(ControlFlowBindingErrorV1::ProofRequestMismatch);
    }
    if request.target() != request_binding.target {
        return Err(ControlFlowBindingErrorV1::ProofTargetMismatch);
    }
    if proof_executable_binding
        .execution_identity()
        .request_digest()
        != request_binding.request_digest
    {
        return Err(ControlFlowBindingErrorV1::ProofExecutionRequestMismatch);
    }

    let result = evidence.result();
    if result.target() != request_binding.target {
        return Err(ControlFlowBindingErrorV1::ProofResultTargetMismatch);
    }
    if result.outcome() != ProofOutcome::Proved {
        return Err(ControlFlowBindingErrorV1::ProofNotProved);
    }
    for property in [ProofProperty::Bounds, ProofProperty::FunctionalCorrectness] {
        if result.proved_properties().binary_search(&property).is_err() {
            return Err(ControlFlowBindingErrorV1::MissingProvedProperty { property });
        }
    }

    let executable_functional_specification = proof_executable_binding
        .executable_binding()
        .executable()
        .source_contracts()
        .functional_specification_digest();
    if executable_functional_specification.algorithm() != DigestAlgorithm::Sha256 {
        return Err(ControlFlowBindingErrorV1::UnsupportedExecutableDigestAlgorithm);
    }
    let executable_functional_specification =
        Digest::from_bytes(*executable_functional_specification.bytes().as_bytes());
    if executable_functional_specification != request_binding.functional_specification_digest {
        return Err(ControlFlowBindingErrorV1::ExecutableFunctionalSpecificationMismatch);
    }

    let executable_binding_identity = proof_executable_binding
        .executable_binding()
        .binding_identity();
    let proof_record_digest = proof_executable_binding
        .executable_binding()
        .proof_record_digest();
    if executable_binding_identity.algorithm() != DigestAlgorithm::Sha256
        || proof_record_digest.algorithm() != DigestAlgorithm::Sha256
    {
        return Err(ControlFlowBindingErrorV1::UnsupportedExecutableDigestAlgorithm);
    }

    Ok(())
}

fn authenticated_control_flow_binding_identity(
    request_binding: &ControlFlowProofRequestBindingV1,
    proof_executable_binding: &AuthenticatedProofExecutableBindingV1,
) -> Digest {
    let executable_binding_identity = proof_executable_binding
        .executable_binding()
        .binding_identity();
    let proof_record_digest = proof_executable_binding
        .executable_binding()
        .proof_record_digest();
    let mut writer =
        IdentityWriter::with_domain(AUTHENTICATED_CONTROL_FLOW_EXECUTABLE_BINDING_DOMAIN_V1);
    writer.digest(request_binding.source.binding_identity);
    writer.digest(request_binding.binding_identity);
    writer.digest(request_binding.functional_specification_digest);
    writer.digest(proof_executable_binding.binding_identity());
    writer.digest(
        proof_executable_binding
            .execution_identity()
            .request_digest(),
    );
    writer.digest(
        proof_executable_binding
            .execution_identity()
            .transcript_digest(),
    );
    writer.digest(
        proof_executable_binding
            .execution_identity()
            .result()
            .digest(),
    );
    writer.payload_digest(proof_record_digest);
    writer.payload_digest(executable_binding_identity);
    sha256(&writer.finish())
}

fn claims_from_contract(
    contract: &ControlFlowContractV1,
) -> Result<ControlFlowClaimsV1, ControlFlowBindingErrorV1> {
    let mut loops = Vec::new();
    let mut switches = Vec::new();
    for node in contract.nodes() {
        match node.kind() {
            ControlFlowNodeKindV1::Loop { max_iterations, .. } => loops.push(
                ControlFlowLoopClaimV1::new(node.id().get(), *max_iterations)?,
            ),
            ControlFlowNodeKindV1::IntegerSwitch { ty, cases, default } => {
                let cases = cases
                    .iter()
                    .map(|case| {
                        ControlFlowIntegerSwitchCaseClaimV1::new(case.bits(), case.target().get())
                    })
                    .collect();
                switches.push(ControlFlowIntegerSwitchClaimV1::new(
                    node.id().get(),
                    ty.width(),
                    ty.is_signed(),
                    cases,
                    default.get(),
                )?);
            }
            _ => {}
        }
    }
    ControlFlowClaimsV1::new(loops, switches)
}

fn reconcile_claims(
    observed: &ControlFlowClaimsV1,
    claimed: &ControlFlowClaimsV1,
) -> Result<(), ControlFlowBindingErrorV1> {
    reconcile_loops(&observed.loops, &claimed.loops)?;
    reconcile_switches(&observed.integer_switches, &claimed.integer_switches)
}

fn reconcile_loops(
    observed: &[ControlFlowLoopClaimV1],
    claimed: &[ControlFlowLoopClaimV1],
) -> Result<(), ControlFlowBindingErrorV1> {
    let mut observed_index = 0;
    let mut claimed_index = 0;
    while let (Some(expected), Some(actual)) = (
        observed.get(observed_index).copied(),
        claimed.get(claimed_index).copied(),
    ) {
        match expected.node_id.cmp(&actual.node_id) {
            std::cmp::Ordering::Less => {
                return Err(ControlFlowBindingErrorV1::MissingLoopClaim {
                    node_id: expected.node_id,
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(ControlFlowBindingErrorV1::UnexpectedLoopClaim {
                    node_id: actual.node_id,
                });
            }
            std::cmp::Ordering::Equal if expected != actual => {
                return Err(ControlFlowBindingErrorV1::LoopClaimMismatch {
                    node_id: expected.node_id,
                });
            }
            std::cmp::Ordering::Equal => {
                observed_index += 1;
                claimed_index += 1;
            }
        }
    }
    if let Some(expected) = observed.get(observed_index) {
        return Err(ControlFlowBindingErrorV1::MissingLoopClaim {
            node_id: expected.node_id,
        });
    }
    if let Some(actual) = claimed.get(claimed_index) {
        return Err(ControlFlowBindingErrorV1::UnexpectedLoopClaim {
            node_id: actual.node_id,
        });
    }
    Ok(())
}

fn reconcile_switches(
    observed: &[ControlFlowIntegerSwitchClaimV1],
    claimed: &[ControlFlowIntegerSwitchClaimV1],
) -> Result<(), ControlFlowBindingErrorV1> {
    let mut observed_index = 0;
    let mut claimed_index = 0;
    while let (Some(expected), Some(actual)) =
        (observed.get(observed_index), claimed.get(claimed_index))
    {
        match expected.node_id.cmp(&actual.node_id) {
            std::cmp::Ordering::Less => {
                return Err(ControlFlowBindingErrorV1::MissingIntegerSwitchClaim {
                    node_id: expected.node_id,
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(ControlFlowBindingErrorV1::UnexpectedIntegerSwitchClaim {
                    node_id: actual.node_id,
                });
            }
            std::cmp::Ordering::Equal if expected != actual => {
                return Err(ControlFlowBindingErrorV1::IntegerSwitchClaimMismatch {
                    node_id: expected.node_id,
                });
            }
            std::cmp::Ordering::Equal => {
                observed_index += 1;
                claimed_index += 1;
            }
        }
    }
    if let Some(expected) = observed.get(observed_index) {
        return Err(ControlFlowBindingErrorV1::MissingIntegerSwitchClaim {
            node_id: expected.node_id,
        });
    }
    if let Some(actual) = claimed.get(claimed_index) {
        return Err(ControlFlowBindingErrorV1::UnexpectedIntegerSwitchClaim {
            node_id: actual.node_id,
        });
    }
    Ok(())
}

fn accepts_integer_bits(width: u16, signed: bool, bits: u128) -> bool {
    if signed {
        if width == 128 {
            return true;
        }
        let mask = (1_u128 << width) - 1;
        let truncated = bits & mask;
        let sign_bit = 1_u128 << (width - 1);
        let canonical = if truncated & sign_bit == 0 {
            truncated
        } else {
            truncated | !mask
        };
        bits == canonical
    } else {
        width == 128 || bits < (1_u128 << width)
    }
}

fn compare_integer_bits(signed: bool, left: u128, right: u128) -> std::cmp::Ordering {
    if signed {
        (left as i128).cmp(&(right as i128))
    } else {
        left.cmp(&right)
    }
}

fn payload_identity(
    bytes: &[u8],
    field: &'static str,
) -> Result<ControlFlowPayloadIdentityV1, ControlFlowBindingErrorV1> {
    let byte_len = u32::try_from(bytes.len())
        .map_err(|_| ControlFlowBindingErrorV1::PayloadLengthOverflow { field })?;
    Ok(ControlFlowPayloadIdentityV1 {
        byte_len,
        digest: sha256(bytes),
    })
}

fn source_binding_identity(
    source_contract: ControlFlowPayloadIdentityV1,
    cfg_identity: ControlFlowPayloadIdentityV1,
    claims: &ControlFlowClaimsV1,
) -> Digest {
    let mut writer = IdentityWriter::with_domain(CONTROL_FLOW_SOURCE_BINDING_DOMAIN_V1);
    writer.u32(source_contract.byte_len);
    writer.digest(source_contract.digest);
    writer.u32(cfg_identity.byte_len);
    writer.digest(cfg_identity.digest);
    writer.bytes(&claims.to_canonical_bytes());
    sha256(&writer.finish())
}

fn require_measured(digest: Digest, field: &'static str) -> Result<(), ControlFlowBindingErrorV1> {
    if digest.as_bytes().iter().all(|byte| *byte == 0) {
        Err(ControlFlowBindingErrorV1::UnmeasuredIdentity { field })
    } else {
        Ok(())
    }
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

struct IdentityWriter {
    bytes: Vec<u8>,
}

impl IdentityWriter {
    fn with_domain(domain: [u8; 8]) -> Self {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&domain);
        bytes.extend_from_slice(&CONTROL_FLOW_BINDING_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        Self { bytes }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn digest(&mut self, value: Digest) {
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn payload_digest(&mut self, value: PayloadDigest) {
        self.u8(match value.algorithm() {
            DigestAlgorithm::Sha256 => 1,
            _ => 0,
        });
        self.bytes.extend_from_slice(value.bytes().as_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.u32(value.len() as u32);
        self.bytes.extend_from_slice(value);
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ControlFlowBindingErrorV1 {
    SourceContractTooLarge,
    CfgIdentityTooLarge,
    SourceContract(ControlFlowDecodeErrorV1),
    CfgIdentityMismatch,
    TooManyClaims { field: &'static str, max: usize },
    TooManySwitchCases { node_id: u32, max: usize },
    ZeroLoopBound { node_id: u32 },
    UnsupportedIntegerWidth { node_id: u32, width: u16 },
    IntegerCaseOutOfRange { node_id: u32, bits: u128 },
    DuplicateIntegerCase { node_id: u32, bits: u128 },
    DuplicateNodeClaim { node_id: u32 },
    MissingLoopClaim { node_id: u32 },
    UnexpectedLoopClaim { node_id: u32 },
    LoopClaimMismatch { node_id: u32 },
    MissingIntegerSwitchClaim { node_id: u32 },
    UnexpectedIntegerSwitchClaim { node_id: u32 },
    IntegerSwitchClaimMismatch { node_id: u32 },
    PayloadLengthOverflow { field: &'static str },
    UnmeasuredIdentity { field: &'static str },
    MissingProofProperty { property: ProofProperty },
    FunctionalSpecificationMismatch,
    ProofRequestMismatch,
    ProofTargetMismatch,
    ProofExecutionRequestMismatch,
    ProofResultTargetMismatch,
    ProofNotProved,
    MissingProvedProperty { property: ProofProperty },
    UnsupportedExecutableDigestAlgorithm,
    ExecutableFunctionalSpecificationMismatch,
    IdentityMismatch { field: &'static str },
    AuthenticatedExecutableBinding(AuthenticatedProofExecutableBindingError),
}

impl fmt::Display for ControlFlowBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceContractTooLarge => formatter.write_str("source contract is too large"),
            Self::CfgIdentityTooLarge => formatter.write_str("CFG identity is too large"),
            Self::SourceContract(error) => write!(formatter, "invalid source contract: {error}"),
            Self::CfgIdentityMismatch => {
                formatter.write_str("emitted CFG identity does not match the source contract")
            }
            Self::TooManyClaims { field, max } => write!(formatter, "{field} exceeds {max}"),
            Self::TooManySwitchCases { node_id, max } => {
                write!(
                    formatter,
                    "integer switch node {node_id} exceeds {max} cases"
                )
            }
            Self::ZeroLoopBound { node_id } => {
                write!(formatter, "loop claim for node {node_id} has a zero bound")
            }
            Self::UnsupportedIntegerWidth { node_id, width } => write!(
                formatter,
                "integer switch claim for node {node_id} has unsupported width {width}"
            ),
            Self::IntegerCaseOutOfRange { node_id, bits } => write!(
                formatter,
                "integer switch claim for node {node_id} has out-of-range case {bits:#034x}"
            ),
            Self::DuplicateIntegerCase { node_id, bits } => write!(
                formatter,
                "integer switch claim for node {node_id} duplicates case {bits:#034x}"
            ),
            Self::DuplicateNodeClaim { node_id } => {
                write!(
                    formatter,
                    "control-flow node {node_id} is claimed more than once"
                )
            }
            Self::MissingLoopClaim { node_id } => {
                write!(formatter, "loop claim for source node {node_id} is missing")
            }
            Self::UnexpectedLoopClaim { node_id } => {
                write!(
                    formatter,
                    "loop claim for source node {node_id} is unexpected"
                )
            }
            Self::LoopClaimMismatch { node_id } => {
                write!(
                    formatter,
                    "loop claim for source node {node_id} does not match"
                )
            }
            Self::MissingIntegerSwitchClaim { node_id } => write!(
                formatter,
                "integer-switch claim for source node {node_id} is missing"
            ),
            Self::UnexpectedIntegerSwitchClaim { node_id } => write!(
                formatter,
                "integer-switch claim for source node {node_id} is unexpected"
            ),
            Self::IntegerSwitchClaimMismatch { node_id } => write!(
                formatter,
                "integer-switch claim for source node {node_id} does not match"
            ),
            Self::PayloadLengthOverflow { field } => {
                write!(formatter, "{field} length does not fit the identity format")
            }
            Self::UnmeasuredIdentity { field } => write!(formatter, "{field} is unmeasured"),
            Self::MissingProofProperty { property } => {
                write!(formatter, "proof request is missing {}", property.as_str())
            }
            Self::FunctionalSpecificationMismatch => formatter.write_str(
                "proof request functional specification does not bind the control-flow source",
            ),
            Self::ProofRequestMismatch => formatter
                .write_str("authenticated proof request does not match the control-flow request"),
            Self::ProofTargetMismatch => formatter
                .write_str("authenticated proof target does not match the control-flow request"),
            Self::ProofExecutionRequestMismatch => formatter.write_str(
                "authenticated proof execution did not retain the control-flow request identity",
            ),
            Self::ProofResultTargetMismatch => formatter.write_str(
                "authenticated proof result target does not match the control-flow request",
            ),
            Self::ProofNotProved => {
                formatter.write_str("authenticated control-flow proof did not succeed")
            }
            Self::MissingProvedProperty { property } => write!(
                formatter,
                "authenticated result did not prove {}",
                property.as_str()
            ),
            Self::UnsupportedExecutableDigestAlgorithm => formatter
                .write_str("authenticated control-flow executable binding requires SHA-256"),
            Self::ExecutableFunctionalSpecificationMismatch => formatter.write_str(
                "finalized executable evidence does not retain the control-flow specification",
            ),
            Self::IdentityMismatch { field } => write!(formatter, "{field} does not match"),
            Self::AuthenticatedExecutableBinding(error) => write!(
                formatter,
                "authenticated proof executable binding does not match: {error}"
            ),
        }
    }
}

impl std::error::Error for ControlFlowBindingErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceContract(error) => Some(error),
            Self::AuthenticatedExecutableBinding(error) => Some(error),
            _ => None,
        }
    }
}
