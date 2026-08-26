//! Fail-closed proof-carrying boundary for noncanonical ranked CFG loops.
//!
//! The compiler derives every structural component from the live ranked graph.
//! Invariant and well-founded-variant digests are claim identities only. A
//! separately imported exact receipt authenticates the complete request, but
//! does not make the claims logical premises and does not bypass aggregate
//! functional replay. Until a replayable schedule theorem interface exists,
//! semantic-contract admission remains unsupported.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementReceiptIdentityV2,
    FunctionalRefinementSubjectsV2, VerusToolchainIdentityV2,
};
#[cfg(feature = "internal-proof-staging")]
use fe2o3_functional_proof::{FunctionalRefinementBoundaryV2, ImportedFunctionalRefinementProofV2};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

#[cfg(feature = "internal-proof-staging")]
use super::ProductionRefinementStagingPolicyV2;
use super::{
    ProductionRankedKernelV1, ProductionRankedTerminatorV1, ProductionRankedValueV1,
    production_ranked_value_identity_v1,
};

const NONCANONICAL_LOOP_OBLIGATION_DOMAIN_V1: &[u8] = b"FE2O3/NONCANONICAL-LOOP/OBLIGATION/V1\0";
const NONCANONICAL_LOOP_MEMBERSHIP_DOMAIN_V1: &[u8] = b"FE2O3/NONCANONICAL-LOOP/MEMBERSHIP/V1\0";
const NONCANONICAL_LOOP_GUARD_DOMAIN_V1: &[u8] = b"FE2O3/NONCANONICAL-LOOP/GUARD/V1\0";
const NONCANONICAL_LOOP_TRANSITION_DOMAIN_V1: &[u8] = b"FE2O3/NONCANONICAL-LOOP/TRANSITION/V1\0";
const NONCANONICAL_LOOP_CARRIED_VALUES_DOMAIN_V1: &[u8] =
    b"FE2O3/NONCANONICAL-LOOP/CARRIED-VALUES/V1\0";
const NONCANONICAL_LOOP_OPERATIONS_DOMAIN_V1: &[u8] = b"FE2O3/NONCANONICAL-LOOP/OPERATIONS/V1\0";

/// Nonzero theorem claim identities selected before external proof execution.
/// Construction creates no evidence and grants no authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionNonCanonicalLoopClaimsV1 {
    contract_identity: u64,
    header_block: u32,
    invariant_claim: DigestV1,
    well_founded_variant_claim: DigestV1,
}

impl ProductionNonCanonicalLoopClaimsV1 {
    pub fn new(
        contract_identity: u64,
        header_block: u32,
        invariant_claim: DigestV1,
        well_founded_variant_claim: DigestV1,
    ) -> Result<Self, ProductionNonCanonicalLoopProofErrorV1> {
        if contract_identity == 0
            || invariant_claim.is_zero()
            || well_founded_variant_claim.is_zero()
        {
            return Err(ProductionNonCanonicalLoopProofErrorV1::InvalidClaim);
        }
        Ok(Self {
            contract_identity,
            header_block,
            invariant_claim,
            well_founded_variant_claim,
        })
    }

    pub const fn contract_identity(self) -> u64 {
        self.contract_identity
    }
    pub const fn header_block(self) -> u32 {
        self.header_block
    }
    pub const fn invariant_claim(self) -> DigestV1 {
        self.invariant_claim
    }
    pub const fn well_founded_variant_claim(self) -> DigestV1 {
        self.well_founded_variant_claim
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProductionNonCanonicalLoopShapeV1 {
    loop_blocks: Vec<u32>,
    entry_edges: Vec<(u32, u32)>,
    internal_edges: Vec<(u32, u32)>,
    backedges: Vec<(u32, u32)>,
    exit_edges: Vec<(u32, u32)>,
    membership_identity: DigestV1,
    guard_identity: DigestV1,
    transition_identity: DigestV1,
    carried_values_identity: DigestV1,
    operations_identity: DigestV1,
    exact_ranked_graph_identity: DigestV1,
}

/// Compiler-derived, non-authoritative description of one exact live loop SCC.
///
/// The mandatory MIR/PLIRON semantic-contract path returns this value when
/// automatic canonical-loop derivation cannot cover the live graph. The
/// evidence and MIR subjects are compiler inputs; invariant and variant claims
/// are deliberately absent until a caller binds explicit nonzero identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionNonCanonicalLoopProofRequirementV1 {
    header_block: u32,
    subjects: FunctionalRefinementSubjectsV2,
    pliron_evidence_identity: DigestV1,
    shape: ProductionNonCanonicalLoopShapeV1,
}

impl ProductionNonCanonicalLoopProofRequirementV1 {
    pub const fn header_block(&self) -> u32 {
        self.header_block
    }
    pub const fn subjects(&self) -> FunctionalRefinementSubjectsV2 {
        self.subjects
    }
    pub const fn pliron_evidence_identity(&self) -> DigestV1 {
        self.pliron_evidence_identity
    }
    pub fn loop_blocks(&self) -> &[u32] {
        &self.shape.loop_blocks
    }
    pub fn entry_edges(&self) -> &[(u32, u32)] {
        &self.shape.entry_edges
    }
    pub fn internal_edges(&self) -> &[(u32, u32)] {
        &self.shape.internal_edges
    }
    /// Natural backedges are reported using the production dominator analysis.
    /// `internal_edges` still binds every edge in irreducible loop regions.
    pub fn backedges(&self) -> &[(u32, u32)] {
        &self.shape.backedges
    }
    pub fn exit_edges(&self) -> &[(u32, u32)] {
        &self.shape.exit_edges
    }
    pub const fn membership_identity(&self) -> DigestV1 {
        self.shape.membership_identity
    }
    pub const fn guard_identity(&self) -> DigestV1 {
        self.shape.guard_identity
    }
    pub const fn transition_identity(&self) -> DigestV1 {
        self.shape.transition_identity
    }
    pub const fn carried_values_identity(&self) -> DigestV1 {
        self.shape.carried_values_identity
    }
    pub const fn operations_identity(&self) -> DigestV1 {
        self.shape.operations_identity
    }
    pub const fn exact_ranked_graph_identity(&self) -> DigestV1 {
        self.shape.exact_ranked_graph_identity
    }
    pub fn bind_claims(
        self,
        claims: ProductionNonCanonicalLoopClaimsV1,
    ) -> Result<ProductionNonCanonicalLoopProofRequestV1, ProductionNonCanonicalLoopProofErrorV1>
    {
        if claims.header_block() != self.header_block {
            return Err(ProductionNonCanonicalLoopProofErrorV1::ClaimHeaderMismatch);
        }
        let normalized_obligation = normalized_obligation(&self, claims);
        FunctionalRefinementBindingV2::from_subjects(self.subjects, normalized_obligation)
            .map_err(|_| ProductionNonCanonicalLoopProofErrorV1::InvalidObligation)?;
        Ok(ProductionNonCanonicalLoopProofRequestV1 {
            claims,
            requirement: self,
            normalized_obligation,
        })
    }

    pub const fn grants_noncanonical_loop_authority(&self) -> bool {
        false
    }
}

/// Explicitly claimed proof request over one compiler-derived live loop SCC.
#[must_use = "a noncanonical loop request is inert until an exact receipt is imported"]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionNonCanonicalLoopProofRequestV1 {
    claims: ProductionNonCanonicalLoopClaimsV1,
    requirement: ProductionNonCanonicalLoopProofRequirementV1,
    normalized_obligation: DigestV1,
}

impl ProductionNonCanonicalLoopProofRequestV1 {
    pub const fn claims(&self) -> ProductionNonCanonicalLoopClaimsV1 {
        self.claims
    }
    pub const fn subjects(&self) -> FunctionalRefinementSubjectsV2 {
        self.requirement.subjects
    }
    pub const fn requirement(&self) -> &ProductionNonCanonicalLoopProofRequirementV1 {
        &self.requirement
    }
    pub fn loop_blocks(&self) -> &[u32] {
        self.requirement.loop_blocks()
    }
    pub fn entry_edges(&self) -> &[(u32, u32)] {
        self.requirement.entry_edges()
    }
    pub fn internal_edges(&self) -> &[(u32, u32)] {
        self.requirement.internal_edges()
    }
    pub fn backedges(&self) -> &[(u32, u32)] {
        self.requirement.backedges()
    }
    pub fn exit_edges(&self) -> &[(u32, u32)] {
        self.requirement.exit_edges()
    }
    pub const fn membership_identity(&self) -> DigestV1 {
        self.requirement.membership_identity()
    }
    pub const fn guard_identity(&self) -> DigestV1 {
        self.requirement.guard_identity()
    }
    pub const fn transition_identity(&self) -> DigestV1 {
        self.requirement.transition_identity()
    }
    pub const fn carried_values_identity(&self) -> DigestV1 {
        self.requirement.carried_values_identity()
    }
    pub const fn operations_identity(&self) -> DigestV1 {
        self.requirement.operations_identity()
    }
    pub const fn exact_ranked_graph_identity(&self) -> DigestV1 {
        self.requirement.exact_ranked_graph_identity()
    }
    pub const fn normalized_obligation(&self) -> DigestV1 {
        self.normalized_obligation
    }
    pub const fn grants_noncanonical_loop_authority(&self) -> bool {
        false
    }
}

/// Authenticated import result. It remains non-authoritative until its exact
/// invariant/variant theorem can be replayed and composed by the aggregate gate.
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionCheckedNonCanonicalLoopProofImportV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionCheckedNonCanonicalLoopProofImportV1>();
/// ```
#[must_use = "checked loop imports authenticate a request but grant no aggregate authority"]
#[derive(Debug)]
pub struct ProductionCheckedNonCanonicalLoopProofImportV1 {
    request: ProductionNonCanonicalLoopProofRequestV1,
    receipt_identity: FunctionalRefinementReceiptIdentityV2,
    signer_identity: DigestV1,
    toolchain: VerusToolchainIdentityV2,
    execution_identity: DigestV1,
}

impl ProductionCheckedNonCanonicalLoopProofImportV1 {
    pub const fn request(&self) -> &ProductionNonCanonicalLoopProofRequestV1 {
        &self.request
    }
    pub const fn receipt_identity(&self) -> FunctionalRefinementReceiptIdentityV2 {
        self.receipt_identity
    }
    pub const fn signer_identity(&self) -> DigestV1 {
        self.signer_identity
    }
    pub const fn toolchain(&self) -> VerusToolchainIdentityV2 {
        self.toolchain
    }
    pub const fn execution_identity(&self) -> DigestV1 {
        self.execution_identity
    }
    pub const fn signature_policy_and_exact_binding_checked(&self) -> bool {
        true
    }
    pub const fn grants_noncanonical_loop_authority(&self) -> bool {
        false
    }
    pub const fn composes_with_aggregate_functional_replay(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionNonCanonicalLoopProofErrorV1 {
    InvalidClaim,
    ClaimHeaderMismatch,
    InvalidHeader,
    InvalidEvidenceIdentity,
    HeaderUnreachable,
    NotCyclic,
    MissingExit,
    InvalidObligation,
    StaleRequest,
    BindingMismatch(FunctionalRefinementReceiptIdentityV2),
    WrongBoundary(FunctionalRefinementReceiptIdentityV2),
    WrongSigner(FunctionalRefinementReceiptIdentityV2),
    WrongToolchain(FunctionalRefinementReceiptIdentityV2),
    InertImportedEvidence(FunctionalRefinementReceiptIdentityV2),
}

impl fmt::Display for ProductionNonCanonicalLoopProofErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidClaim => formatter.write_str("noncanonical loop claims require nonzero contract, invariant, and well-founded variant identities"),
            Self::ClaimHeaderMismatch => formatter.write_str("noncanonical loop claims name a different header than the compiler-derived live loop region"),
            Self::InvalidHeader => formatter.write_str("noncanonical loop header is outside the exact ranked CFG"),
            Self::InvalidEvidenceIdentity => formatter.write_str("noncanonical loop proof requirement needs the nonzero live PLIRON evidence identity"),
            Self::HeaderUnreachable => formatter.write_str("noncanonical loop header is unreachable from ranked entry block zero"),
            Self::NotCyclic => formatter.write_str("the compiler-derived strongly connected region is not a loop"),
            Self::MissingExit => formatter.write_str("the compiler-derived loop region has no live CFG exit"),
            Self::InvalidObligation => formatter.write_str("compiler-derived noncanonical loop obligation is not a valid nonzero functional-refinement binding"),
            Self::StaleRequest => formatter.write_str("noncanonical loop request differs from the exact current ranked CFG, MIR subjects, membership, guard, transition, carried values, operations, invariant, or variant"),
            Self::BindingMismatch(identity) => write!(formatter, "noncanonical loop receipt {identity:?} does not bind the exact compiler-derived obligation and MIR subjects"),
            Self::WrongBoundary(identity) => write!(formatter, "noncanonical loop receipt {identity:?} does not cover safe-reference MIR to kernel MIR"),
            Self::WrongSigner(identity) => write!(formatter, "noncanonical loop receipt {identity:?} was not signed by the compiler policy"),
            Self::WrongToolchain(identity) => write!(formatter, "noncanonical loop receipt {identity:?} used a different Verus/solver/runtime closure"),
            Self::InertImportedEvidence(identity) => write!(formatter, "noncanonical loop receipt {identity:?} lacks verified signature and import-policy provenance"),
        }
    }
}

impl Error for ProductionNonCanonicalLoopProofErrorV1 {}

/// Derives the structural half of a bounded request from the exact live ranked
/// graph, MIR subjects, and independently constructed PLIRON evidence.
pub fn derive_noncanonical_loop_proof_requirement_v1(
    kernel: &ProductionRankedKernelV1,
    header_block: u32,
    subjects: FunctionalRefinementSubjectsV2,
    pliron_evidence_identity: DigestV1,
) -> Result<ProductionNonCanonicalLoopProofRequirementV1, ProductionNonCanonicalLoopProofErrorV1> {
    if pliron_evidence_identity.is_zero() {
        return Err(ProductionNonCanonicalLoopProofErrorV1::InvalidEvidenceIdentity);
    }
    let shape = derive_loop_shape(kernel, header_block)?;
    Ok(ProductionNonCanonicalLoopProofRequirementV1 {
        header_block,
        subjects,
        pliron_evidence_identity,
        shape,
    })
}

/// Derives and binds a request. Claims select theorem identities but cannot
/// choose the live loop membership, CFG, transfers, operations, MIR subjects,
/// or PLIRON evidence identity.
pub fn derive_noncanonical_loop_proof_request_v1(
    kernel: &ProductionRankedKernelV1,
    claims: ProductionNonCanonicalLoopClaimsV1,
    subjects: FunctionalRefinementSubjectsV2,
    pliron_evidence_identity: DigestV1,
) -> Result<ProductionNonCanonicalLoopProofRequestV1, ProductionNonCanonicalLoopProofErrorV1> {
    derive_noncanonical_loop_proof_requirement_v1(
        kernel,
        claims.header_block(),
        subjects,
        pliron_evidence_identity,
    )?
    .bind_claims(claims)
}

/// Consumes one independently imported receipt and re-derives the request from
/// the current graph before checking its exact binding and compiler policy.
#[cfg(feature = "internal-proof-staging")]
pub fn import_noncanonical_loop_proof_v1(
    kernel: &ProductionRankedKernelV1,
    request: ProductionNonCanonicalLoopProofRequestV1,
    imported: ImportedFunctionalRefinementProofV2,
    policy: &ProductionRefinementStagingPolicyV2,
) -> Result<ProductionCheckedNonCanonicalLoopProofImportV1, ProductionNonCanonicalLoopProofErrorV1>
{
    let current = derive_noncanonical_loop_proof_request_v1(
        kernel,
        request.claims,
        request.requirement.subjects,
        request.requirement.pliron_evidence_identity,
    )?;
    if current != request {
        return Err(ProductionNonCanonicalLoopProofErrorV1::StaleRequest);
    }
    let identity = imported.receipt_identity();
    let expected_binding = FunctionalRefinementBindingV2::from_subjects(
        request.requirement.subjects,
        request.normalized_obligation,
    )
    .map_err(|_| ProductionNonCanonicalLoopProofErrorV1::InvalidObligation)?;
    if imported.binding() != expected_binding {
        return Err(ProductionNonCanonicalLoopProofErrorV1::BindingMismatch(
            identity,
        ));
    }
    if imported.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir {
        return Err(ProductionNonCanonicalLoopProofErrorV1::WrongBoundary(
            identity,
        ));
    }
    if !policy.accepts_signer(imported.signer_identity()) {
        return Err(ProductionNonCanonicalLoopProofErrorV1::WrongSigner(
            identity,
        ));
    }
    if imported.toolchain() != policy.toolchain() {
        return Err(ProductionNonCanonicalLoopProofErrorV1::WrongToolchain(
            identity,
        ));
    }
    if !imported.signature_and_policy_verified() {
        return Err(ProductionNonCanonicalLoopProofErrorV1::InertImportedEvidence(identity));
    }
    Ok(ProductionCheckedNonCanonicalLoopProofImportV1 {
        request,
        receipt_identity: identity,
        signer_identity: imported.signer_identity(),
        toolchain: imported.toolchain(),
        execution_identity: imported.execution_identity(),
    })
}

fn derive_loop_shape(
    kernel: &ProductionRankedKernelV1,
    header: u32,
) -> Result<ProductionNonCanonicalLoopShapeV1, ProductionNonCanonicalLoopProofErrorV1> {
    let block_count = kernel.blocks().len();
    if header as usize >= block_count {
        return Err(ProductionNonCanonicalLoopProofErrorV1::InvalidHeader);
    }
    let successors = kernel
        .blocks()
        .iter()
        .map(|block| terminator_successors(block.terminator()))
        .collect::<Vec<_>>();
    let mut predecessors = vec![Vec::new(); block_count];
    for (source, targets) in successors.iter().enumerate() {
        for target in targets {
            if let Some(slot) = predecessors.get_mut(*target as usize) {
                slot.push(source as u32);
            }
        }
    }
    let reachable = reachable_from(0, &successors);
    if !reachable.contains(&header) {
        return Err(ProductionNonCanonicalLoopProofErrorV1::HeaderUnreachable);
    }
    let forward = reachable_from(header, &successors);
    let reverse = reachable_from(header, &predecessors);
    let members = forward
        .intersection(&reverse)
        .copied()
        .collect::<BTreeSet<_>>();
    let cyclic = members.len() > 1 || successors[header as usize].contains(&header);
    if !cyclic {
        return Err(ProductionNonCanonicalLoopProofErrorV1::NotCyclic);
    }
    let loop_blocks = members.iter().copied().collect::<Vec<_>>();
    let mut entry_edges = Vec::new();
    let mut internal_edges = Vec::new();
    let mut exit_edges = Vec::new();
    for (source, targets) in successors.iter().enumerate() {
        for target in targets {
            let source = source as u32;
            match (members.contains(&source), members.contains(target)) {
                (false, true) => entry_edges.push((source, *target)),
                (true, true) => internal_edges.push((source, *target)),
                (true, false) => exit_edges.push((source, *target)),
                (false, false) => {}
            }
        }
    }
    if exit_edges.is_empty() {
        return Err(ProductionNonCanonicalLoopProofErrorV1::MissingExit);
    }
    let backedges = super::mir_pliron_semantic_contract_v1::natural_backedges(kernel)
        .into_iter()
        .filter(|(source, target)| members.contains(source) && members.contains(target))
        .collect::<Vec<_>>();
    let membership_identity = hash_membership(
        header,
        &loop_blocks,
        &entry_edges,
        &internal_edges,
        &backedges,
        &exit_edges,
    );
    let guard_identity = hash_edge_set(NONCANONICAL_LOOP_GUARD_DOMAIN_V1, kernel, &exit_edges);
    let transition_identity = hash_edge_set(
        NONCANONICAL_LOOP_TRANSITION_DOMAIN_V1,
        kernel,
        &internal_edges,
    );
    let carried_values_identity = hash_carried_values(kernel, &members, &predecessors);
    let operations_identity = hash_loop_operations(kernel, &loop_blocks);
    let exact_ranked_graph_identity = DigestV1::from_untrusted_bytes(
        super::middle_end_evidence_v4::derive_exact_ranked_graph_identity_v1(kernel),
    );
    Ok(ProductionNonCanonicalLoopShapeV1 {
        loop_blocks,
        entry_edges,
        internal_edges,
        backedges,
        exit_edges,
        membership_identity,
        guard_identity,
        transition_identity,
        carried_values_identity,
        operations_identity,
        exact_ranked_graph_identity,
    })
}

/// Finds live cyclic SCCs that the natural-backedge path cannot soundly cover.
/// A region is noncanonical when it has no dominator-defined backedge or has
/// multiple distinct live entry targets. The iterative two-pass algorithm
/// keeps this scan linear in the ranked CFG size without host recursion.
pub(super) fn noncanonical_cyclic_scc_headers_v1(
    kernel: &ProductionRankedKernelV1,
    natural_backedges: &BTreeSet<(u32, u32)>,
) -> Vec<u32> {
    let successors = kernel
        .blocks()
        .iter()
        .map(|block| terminator_successors(block.terminator()))
        .collect::<Vec<_>>();
    let reachable = reachable_from(0, &successors);
    let mut predecessors = vec![Vec::new(); successors.len()];
    for (source, targets) in successors.iter().enumerate() {
        if !reachable.contains(&(source as u32)) {
            continue;
        }
        for target in targets {
            predecessors[*target as usize].push(source as u32);
        }
    }
    let order = cfg_finish_order(&successors, &reachable);
    let mut assigned = vec![false; successors.len()];
    let mut headers = BTreeSet::new();
    for start in order.into_iter().rev() {
        if assigned[start as usize] {
            continue;
        }
        let mut members = BTreeSet::new();
        let mut pending = vec![start];
        assigned[start as usize] = true;
        while let Some(block) = pending.pop() {
            members.insert(block);
            for predecessor in &predecessors[block as usize] {
                if !assigned[*predecessor as usize] {
                    assigned[*predecessor as usize] = true;
                    pending.push(*predecessor);
                }
            }
        }
        let cyclic = members.len() > 1 || successors[start as usize].contains(&start);
        if !cyclic {
            continue;
        }
        let entry_targets = predecessors
            .iter()
            .enumerate()
            .filter(|(target, _)| members.contains(&(*target as u32)))
            .flat_map(|(target, sources)| {
                sources
                    .iter()
                    .filter(|source| !members.contains(source))
                    .map(move |_| target as u32)
            })
            .collect::<BTreeSet<_>>();
        let has_natural_backedge = natural_backedges
            .iter()
            .any(|edge| members.contains(&edge.0) && members.contains(&edge.1));
        if !has_natural_backedge || entry_targets.len() > 1 {
            let header = entry_targets
                .iter()
                .next()
                .copied()
                .or_else(|| members.iter().next().copied());
            if let Some(header) = header {
                headers.insert(header);
            }
        }
    }
    headers.into_iter().collect()
}

fn cfg_finish_order(adjacency: &[Vec<u32>], reachable: &BTreeSet<u32>) -> Vec<u32> {
    let mut visited = vec![false; adjacency.len()];
    let mut order = Vec::with_capacity(reachable.len());
    for start in 0..adjacency.len() {
        if visited[start] || !reachable.contains(&(start as u32)) {
            continue;
        }
        visited[start] = true;
        let mut stack = vec![(start as u32, 0_usize)];
        while let Some((block, successor)) = stack.last_mut() {
            if *successor < adjacency[*block as usize].len() {
                let target = adjacency[*block as usize][*successor] as usize;
                *successor += 1;
                if reachable.contains(&(target as u32)) && !visited[target] {
                    visited[target] = true;
                    stack.push((target as u32, 0));
                }
            } else {
                order.push(*block);
                stack.pop();
            }
        }
    }
    order
}

fn normalized_obligation(
    requirement: &ProductionNonCanonicalLoopProofRequirementV1,
    claims: ProductionNonCanonicalLoopClaimsV1,
) -> DigestV1 {
    let shape = &requirement.shape;
    let subjects = requirement.subjects;
    let mut digest = Sha256::new();
    put_blob(&mut digest, NONCANONICAL_LOOP_OBLIGATION_DOMAIN_V1);
    digest.update(claims.contract_identity().to_le_bytes());
    digest.update(claims.header_block().to_le_bytes());
    for identity in [
        claims.invariant_claim(),
        claims.well_founded_variant_claim(),
        shape.exact_ranked_graph_identity,
        shape.membership_identity,
        shape.guard_identity,
        shape.transition_identity,
        shape.carried_values_identity,
        shape.operations_identity,
        requirement.pliron_evidence_identity,
        subjects.safe_reference_identity(),
        subjects.safe_reference_source_hash(),
        subjects.safe_reference_mir_hash(),
        subjects.kernel_subject_identity(),
        subjects.kernel_mir_hash(),
    ] {
        put_digest(&mut digest, identity);
    }
    digest.update([subjects.safe_reference_kind() as u8]);
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn hash_membership(
    header: u32,
    blocks: &[u32],
    entries: &[(u32, u32)],
    internal: &[(u32, u32)],
    backedges: &[(u32, u32)],
    exits: &[(u32, u32)],
) -> DigestV1 {
    let mut digest = Sha256::new();
    put_blob(&mut digest, NONCANONICAL_LOOP_MEMBERSHIP_DOMAIN_V1);
    digest.update(header.to_le_bytes());
    put_blocks(&mut digest, blocks);
    for edges in [entries, internal, backedges, exits] {
        put_edges(&mut digest, edges);
    }
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn hash_edge_set(
    domain: &[u8],
    kernel: &ProductionRankedKernelV1,
    edges: &[(u32, u32)],
) -> DigestV1 {
    let mut digest = Sha256::new();
    put_blob(&mut digest, domain);
    digest.update((edges.len() as u64).to_le_bytes());
    for (source, target) in edges {
        digest.update(source.to_le_bytes());
        digest.update(target.to_le_bytes());
        let terminator = kernel.blocks()[*source as usize].terminator();
        digest.update(
            super::middle_end_evidence_v4::derive_exact_ranked_terminator_identity_v1(terminator),
        );
        hash_edge_transfer(&mut digest, terminator, *target);
    }
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn hash_carried_values(
    kernel: &ProductionRankedKernelV1,
    members: &BTreeSet<u32>,
    predecessors: &[Vec<u32>],
) -> DigestV1 {
    let mut digest = Sha256::new();
    put_blob(&mut digest, NONCANONICAL_LOOP_CARRIED_VALUES_DOMAIN_V1);
    for block in members {
        let argument_count = kernel.blocks()[*block as usize].index_argument_count();
        digest.update(block.to_le_bytes());
        digest.update(argument_count.to_le_bytes());
        for argument in 0..argument_count {
            put_digest(
                &mut digest,
                production_ranked_value_identity_v1(ProductionRankedValueV1::BlockArgument {
                    block: *block,
                    argument,
                }),
            );
        }
        let incoming = &predecessors[*block as usize];
        digest.update((incoming.len() as u64).to_le_bytes());
        for source in incoming {
            digest.update(source.to_le_bytes());
            let terminator = kernel.blocks()[*source as usize].terminator();
            digest.update(
                super::middle_end_evidence_v4::derive_exact_ranked_terminator_identity_v1(
                    terminator,
                ),
            );
            hash_edge_transfer(&mut digest, terminator, *block);
        }
    }
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn hash_loop_operations(kernel: &ProductionRankedKernelV1, blocks: &[u32]) -> DigestV1 {
    let mut digest = Sha256::new();
    put_blob(&mut digest, NONCANONICAL_LOOP_OPERATIONS_DOMAIN_V1);
    for block in blocks {
        let operations = kernel.blocks()[*block as usize].operations();
        digest.update(block.to_le_bytes());
        digest.update((operations.len() as u64).to_le_bytes());
        for (operation, item) in operations.iter().enumerate() {
            digest.update((operation as u64).to_le_bytes());
            digest.update(
                super::middle_end_evidence_v4::derive_exact_ranked_operation_identity_v1(item),
            );
        }
    }
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn reachable_from(start: u32, adjacency: &[Vec<u32>]) -> BTreeSet<u32> {
    let mut reached = BTreeSet::new();
    let mut pending = vec![start];
    while let Some(block) = pending.pop() {
        if block as usize >= adjacency.len() || !reached.insert(block) {
            continue;
        }
        pending.extend(adjacency[block as usize].iter().copied());
    }
    reached
}

fn terminator_successors(terminator: &ProductionRankedTerminatorV1) -> Vec<u32> {
    match terminator {
        ProductionRankedTerminatorV1::IndexLessThan {
            true_block,
            false_block,
            ..
        }
        | ProductionRankedTerminatorV1::IndexLessThanArgs {
            true_block,
            false_block,
            ..
        }
        | ProductionRankedTerminatorV1::IndexEqual {
            true_block,
            false_block,
            ..
        }
        | ProductionRankedTerminatorV1::IndexEqualArgs {
            true_block,
            false_block,
            ..
        } => vec![*true_block, *false_block],
        ProductionRankedTerminatorV1::AnalysisSplit {
            first_block,
            second_block,
            ..
        }
        | ProductionRankedTerminatorV1::AnalysisSplitArgs {
            first_block,
            second_block,
            ..
        } => vec![*first_block, *second_block],
        ProductionRankedTerminatorV1::Branch { target }
        | ProductionRankedTerminatorV1::BranchArgs { target, .. }
        | ProductionRankedTerminatorV1::BranchArgsAdd { target, .. }
        | ProductionRankedTerminatorV1::BranchArgsAddAt { target, .. } => vec![*target],
        ProductionRankedTerminatorV1::Return | ProductionRankedTerminatorV1::Trap => Vec::new(),
    }
}

fn hash_edge_transfer(digest: &mut Sha256, terminator: &ProductionRankedTerminatorV1, target: u32) {
    let mut values = Vec::new();
    let tag = match terminator {
        ProductionRankedTerminatorV1::IndexLessThanArgs {
            true_arguments,
            false_arguments,
            true_block,
            false_block,
            ..
        }
        | ProductionRankedTerminatorV1::IndexEqualArgs {
            true_arguments,
            false_arguments,
            true_block,
            false_block,
            ..
        } => {
            values.extend_from_slice(if *true_block == target {
                true_arguments
            } else if *false_block == target {
                false_arguments
            } else {
                &[]
            });
            1
        }
        ProductionRankedTerminatorV1::AnalysisSplitArgs {
            first_arguments,
            second_arguments,
            first_block,
            second_block,
            ..
        } => {
            values.extend_from_slice(if *first_block == target {
                first_arguments
            } else if *second_block == target {
                second_arguments
            } else {
                &[]
            });
            2
        }
        ProductionRankedTerminatorV1::BranchArgs { arguments, .. } => {
            values.extend_from_slice(arguments);
            3
        }
        ProductionRankedTerminatorV1::BranchArgsAdd { value, step, .. } => {
            values.extend([*value, *step]);
            4
        }
        ProductionRankedTerminatorV1::BranchArgsAddAt {
            arguments,
            add_argument,
            step,
            ..
        } => {
            digest.update(add_argument.to_le_bytes());
            values.extend_from_slice(arguments);
            values.push(*step);
            5
        }
        _ => 0,
    };
    digest.update([tag]);
    digest.update((values.len() as u64).to_le_bytes());
    for value in values {
        put_digest(digest, production_ranked_value_identity_v1(value));
    }
}

fn put_blocks(digest: &mut Sha256, blocks: &[u32]) {
    digest.update((blocks.len() as u64).to_le_bytes());
    for block in blocks {
        digest.update(block.to_le_bytes());
    }
}

fn put_edges(digest: &mut Sha256, edges: &[(u32, u32)]) {
    digest.update((edges.len() as u64).to_le_bytes());
    for (source, target) in edges {
        digest.update(source.to_le_bytes());
        digest.update(target.to_le_bytes());
    }
}

fn put_digest(digest: &mut Sha256, value: DigestV1) {
    put_blob(digest, value.as_bytes());
}

fn put_blob(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProductionRankedBlockV1, ProductionRankedOperationV1, ProductionRankedValueIdV1};

    #[test]
    fn unreachable_cycle_is_ignored_while_reachable_canonical_loop_remains() {
        let local =
            |identity| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(identity));
        let argument = |block| ProductionRankedValueV1::BlockArgument { block, argument: 0 };
        let kernel = ProductionRankedKernelV1::new(
            "reachable_loop_with_dead_cycle",
            0,
            vec![
                ProductionRankedBlockV1::new(
                    vec![
                        ProductionRankedOperationV1::IndexConstant {
                            result: ProductionRankedValueIdV1::new(0),
                            value: 0,
                        },
                        ProductionRankedOperationV1::IndexConstant {
                            result: ProductionRankedValueIdV1::new(1),
                            value: 4,
                        },
                        ProductionRankedOperationV1::IndexConstant {
                            result: ProductionRankedValueIdV1::new(2),
                            value: 1,
                        },
                    ],
                    ProductionRankedTerminatorV1::BranchArgs {
                        arguments: vec![local(0)],
                        target: 1,
                    },
                ),
                ProductionRankedBlockV1::with_index_arguments(
                    1,
                    vec![],
                    ProductionRankedTerminatorV1::IndexLessThanArgs {
                        lhs: argument(1),
                        rhs: local(1),
                        true_arguments: vec![argument(1)],
                        false_arguments: vec![],
                        true_block: 2,
                        false_block: 3,
                    },
                ),
                ProductionRankedBlockV1::with_index_arguments(
                    1,
                    vec![],
                    ProductionRankedTerminatorV1::BranchArgsAdd {
                        value: argument(2),
                        step: local(2),
                        target: 1,
                    },
                ),
                ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
                ProductionRankedBlockV1::new(
                    vec![],
                    ProductionRankedTerminatorV1::Branch { target: 5 },
                ),
                ProductionRankedBlockV1::new(
                    vec![],
                    ProductionRankedTerminatorV1::Branch { target: 4 },
                ),
            ],
        )
        .unwrap();
        let backedges = super::super::mir_pliron_semantic_contract_v1::natural_backedges(&kernel)
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(backedges, [(2, 1)].into_iter().collect());
        assert!(noncanonical_cyclic_scc_headers_v1(&kernel, &backedges).is_empty());

        let live = derive_loop_shape(&kernel, 1).unwrap();
        assert_eq!(live.loop_blocks, [1, 2]);
        assert_eq!(live.exit_edges, [(1, 3)]);
    }
}
