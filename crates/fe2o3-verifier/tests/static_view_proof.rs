use fe2o3_artifacts::DigestAlgorithm;
use fe2o3_contracts::{
    AddressSpaceIdV1, AllocationProvenanceIdV1, AllocationSpecV1, ByteRegionV1,
    StaticViewAccessDescriptionV1, StaticViewDescriptionV1,
};
use fe2o3_rustc_front::{
    ControlFlowContractV1, ControlFlowNodeIdV1, ControlFlowNodeKindV1, ControlFlowNodeV1,
    FrontendSourceSpanV1, encode_control_flow_contract_v1,
};
use fe2o3_verifier::{
    Configuration, ControlFlowClaimsV1, ControlFlowSourceBindingV1, CorrelationId, Digest,
    ProofProperty, ProofRequestV1, ProofTargetIdentity, STATIC_VIEW_PROOF_EVIDENCE_DOMAIN_V1,
    STATIC_VIEW_PROOF_OBLIGATION_DOMAIN_V1, STATIC_VIEW_PROOF_REQUIRED_PROPERTIES_V1,
    STATIC_VIEW_PROOF_VERSION_V1, StaticViewLifetimeEpochClaimV1, StaticViewProofErrorV1,
    StaticViewProofObligationV1, VerificationModelIdentity, bind_control_flow_proof_request_v1,
    bind_static_view_proof_evidence_v1, derive_control_flow_functional_specification_digest_v1,
    derive_static_view_functional_specification_digest_v1, reconcile_control_flow_source_v1,
};

fn digest(seed: u8) -> Digest {
    Digest::from_bytes([seed; 32])
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

fn source(path: &str) -> ControlFlowSourceBindingV1 {
    let id = ControlFlowNodeIdV1::new;
    let contract = ControlFlowContractV1::new(
        id(0),
        vec![
            ControlFlowNodeV1::new(
                id(0),
                FrontendSourceSpanV1::new(path, 10, 1, 10, 8).unwrap(),
                ControlFlowNodeKindV1::Entry { target: id(1) },
            ),
            ControlFlowNodeV1::new(
                id(1),
                FrontendSourceSpanV1::new(path, 11, 1, 11, 8).unwrap(),
                ControlFlowNodeKindV1::Exit,
            ),
        ],
    )
    .unwrap();
    let encoded = encode_control_flow_contract_v1(&contract).unwrap();
    reconcile_control_flow_source_v1(
        &encoded,
        contract.cfg_identity().as_bytes(),
        ControlFlowClaimsV1::new(vec![], vec![]).unwrap(),
    )
    .unwrap()
}

fn description_with(
    access: StaticViewAccessDescriptionV1,
    provenance: u32,
) -> StaticViewDescriptionV1 {
    let allocation = AllocationSpecV1::new(
        AllocationProvenanceIdV1::new(provenance).unwrap(),
        AddressSpaceIdV1::new(3).unwrap(),
        0x1_0000,
        64,
        0x2_0000,
    )
    .unwrap();
    let parent = ByteRegionV1::for_allocation(allocation, 0, 64).unwrap();
    StaticViewDescriptionV1::describe(allocation, parent, 16, 3, 4, 4, 4, access).unwrap()
}

fn lifetime(epoch_seed: u8) -> StaticViewLifetimeEpochClaimV1 {
    StaticViewLifetimeEpochClaimV1::new(digest(epoch_seed), 10, 20, 12).unwrap()
}

fn obligation_with(
    source: &ControlFlowSourceBindingV1,
    access: StaticViewAccessDescriptionV1,
    source_tree: Digest,
    epoch_seed: u8,
    lease_seed: u8,
) -> StaticViewProofObligationV1 {
    StaticViewProofObligationV1::new(
        description_with(access, 7),
        source,
        source_tree,
        digest(0x22),
        digest(0x23),
        digest(0x24),
        digest(0x25),
        lifetime(epoch_seed),
        (access == StaticViewAccessDescriptionV1::ExclusiveWrite).then(|| digest(lease_seed)),
    )
    .unwrap()
}

fn target(functional_specification_digest: Digest) -> ProofTargetIdentity {
    ProofTargetIdentity {
        kernel_id: digest(1),
        instance_digest: digest(2),
        source_tree_digest: digest(0x21),
        crate_graph_digest: digest(4),
        executable_digest: digest(5),
        environment_digest: digest(6),
        artifact_selection_digest: digest(7),
        artifact_contract_digest: digest(8),
        memory_contract_digest: digest(0x22),
        effects_contract_digest: digest(10),
        type_layout_digest: digest(0x23),
        capability_semantics_digest: digest(0x24),
        functional_specification_digest,
    }
}

fn request(
    functional_specification_digest: Digest,
    properties: Vec<ProofProperty>,
) -> ProofRequestV1 {
    ProofRequestV1::new(
        CorrelationId::from_bytes([0x31; 16]),
        target(functional_specification_digest),
        Configuration::new(vec![]).unwrap(),
        VerificationModelIdentity::new("static-view-model-v1", digest(0x40)).unwrap(),
        properties,
        vec![],
    )
    .unwrap()
}

fn request_and_control_flow(
    source: &ControlFlowSourceBindingV1,
    obligation: &StaticViewProofObligationV1,
    properties: Vec<ProofProperty>,
) -> (
    ProofRequestV1,
    fe2o3_verifier::ControlFlowProofRequestBindingV1,
) {
    let base = derive_static_view_functional_specification_digest_v1(obligation);
    let functional = derive_control_flow_functional_specification_digest_v1(base, source).unwrap();
    let request = request(functional, properties);
    let control_flow = bind_control_flow_proof_request_v1(&request, base, source.clone()).unwrap();
    (request, control_flow)
}

#[test]
fn obligation_and_request_evidence_have_canonical_encodings() {
    let source = source("src/kernel.rs");
    let obligation = obligation_with(
        &source,
        StaticViewAccessDescriptionV1::ExclusiveWrite,
        digest(0x21),
        0x30,
        0x26,
    );
    let description = obligation.description();
    let allocation = description.described_allocation();
    let parent = description.described_parent_region();
    let region = description.described_region();

    let mut expected = Vec::new();
    expected.extend_from_slice(&STATIC_VIEW_PROOF_OBLIGATION_DOMAIN_V1);
    expected.extend_from_slice(&STATIC_VIEW_PROOF_VERSION_V1.to_le_bytes());
    expected.extend_from_slice(&0_u16.to_le_bytes());
    expected.extend_from_slice(&source.source_contract().byte_len().to_le_bytes());
    expected.extend_from_slice(source.source_contract().digest().as_bytes());
    expected.extend_from_slice(&source.cfg_identity().byte_len().to_le_bytes());
    expected.extend_from_slice(source.cfg_identity().digest().as_bytes());
    expected.extend_from_slice(source.binding_identity().as_bytes());
    for identity in [
        digest(0x21),
        digest(0x22),
        digest(0x23),
        digest(0x24),
        digest(0x25),
        digest(0x30),
    ] {
        expected.extend_from_slice(identity.as_bytes());
    }
    expected.extend_from_slice(&10_u64.to_le_bytes());
    expected.extend_from_slice(&20_u64.to_le_bytes());
    expected.extend_from_slice(&12_u64.to_le_bytes());
    expected.extend_from_slice(&allocation.provenance().get().to_le_bytes());
    expected.extend_from_slice(&allocation.address_space().get().to_le_bytes());
    expected.extend_from_slice(&0_u16.to_le_bytes());
    expected.extend_from_slice(&allocation.base_address().to_le_bytes());
    expected.extend_from_slice(&allocation.byte_length().to_le_bytes());
    expected.extend_from_slice(&allocation.address_space_size().to_le_bytes());
    for value in [parent, region] {
        expected.extend_from_slice(&value.provenance().get().to_le_bytes());
        expected.extend_from_slice(&value.address_space().get().to_le_bytes());
        expected.extend_from_slice(&0_u16.to_le_bytes());
        expected.extend_from_slice(&value.byte_offset().to_le_bytes());
        expected.extend_from_slice(&value.byte_length().to_le_bytes());
    }
    for value in [
        description.parent_element_count(),
        description.start_element(),
        description.element_count(),
        description.element_size(),
        description.element_alignment(),
    ] {
        expected.extend_from_slice(&value.to_le_bytes());
    }
    expected.extend_from_slice(&[2, 1]);
    expected.extend_from_slice(digest(0x26).as_bytes());
    expected.extend_from_slice(&0_u16.to_le_bytes());
    assert_eq!(obligation.to_canonical_bytes(), expected);
    assert_eq!(obligation.obligation_identity(), sha256(&expected));

    let (request, control_flow) = request_and_control_flow(
        &source,
        &obligation,
        STATIC_VIEW_PROOF_REQUIRED_PROPERTIES_V1.to_vec(),
    );
    let evidence = bind_static_view_proof_evidence_v1(&request, control_flow, obligation).unwrap();
    assert_eq!(
        &evidence.to_canonical_bytes()[..12],
        &[
            STATIC_VIEW_PROOF_EVIDENCE_DOMAIN_V1.as_slice(),
            &STATIC_VIEW_PROOF_VERSION_V1.to_le_bytes(),
            &[0, 0],
        ]
        .concat()
    );
    assert_eq!(
        evidence.evidence_identity(),
        sha256(&evidence.to_canonical_bytes())
    );
    let canonical_evidence = evidence.to_canonical_bytes();
    for bit in 0..canonical_evidence.len() * 8 {
        let mut mutated = canonical_evidence.clone();
        mutated[bit / 8] ^= 1 << (bit % 8);
        assert_ne!(
            sha256(&mutated),
            evidence.evidence_identity(),
            "bit mutation {bit} retained the evidence identity"
        );
    }
}

#[test]
fn every_single_bit_obligation_mutation_changes_content_identity() {
    let source = source("src/kernel.rs");
    let obligation = obligation_with(
        &source,
        StaticViewAccessDescriptionV1::SharedRead,
        digest(0x21),
        0x30,
        0,
    );
    let canonical = obligation.to_canonical_bytes();
    for bit in 0..canonical.len() * 8 {
        let mut mutated = canonical.clone();
        mutated[bit / 8] ^= 1 << (bit % 8);
        assert_ne!(
            sha256(&mutated),
            obligation.obligation_identity(),
            "bit mutation {bit} retained the obligation identity"
        );
    }
}

#[test]
fn caller_epochs_exclusive_leases_and_evidence_are_non_authoritative() {
    let source = source("src/kernel.rs");
    let first = obligation_with(
        &source,
        StaticViewAccessDescriptionV1::ExclusiveWrite,
        digest(0x21),
        0x30,
        0x26,
    );
    let substituted = obligation_with(
        &source,
        StaticViewAccessDescriptionV1::ExclusiveWrite,
        digest(0x21),
        0x31,
        0x27,
    );
    assert_ne!(
        first.obligation_identity(),
        substituted.obligation_identity()
    );
    for obligation in [&first, &substituted] {
        assert!(!obligation.grants_proof_authority());
        assert!(!obligation.grants_runtime_authority());
        assert!(!obligation.authenticates_live_allocation());
        assert!(!obligation.authenticates_exclusive_lease());
        assert!(
            !obligation
                .claimed_lifetime()
                .authenticates_live_allocation()
        );
    }

    let (request, control_flow) = request_and_control_flow(
        &source,
        &first,
        STATIC_VIEW_PROOF_REQUIRED_PROPERTIES_V1.to_vec(),
    );
    let evidence = bind_static_view_proof_evidence_v1(&request, control_flow, first).unwrap();
    assert!(!evidence.grants_proof_authority());
    assert!(!evidence.grants_runtime_authority());
    assert!(!evidence.authenticates_verifier_execution());
    assert!(!evidence.authenticates_global_ledger_namespace());
    assert!(!evidence.authenticates_live_allocation());
    assert!(!evidence.authenticates_exclusive_lease());
}

#[test]
fn lifetime_claims_check_coherence_without_authenticating_liveness() {
    assert_eq!(
        StaticViewLifetimeEpochClaimV1::new(digest(0x30), 20, 10, 12),
        Err(StaticViewProofErrorV1::InvalidClaimedLifetimeEpochRange {
            valid_from: 20,
            valid_through: 10,
        })
    );
    assert_eq!(
        StaticViewLifetimeEpochClaimV1::new(digest(0x30), 10, 20, 21),
        Err(StaticViewProofErrorV1::ClaimedLaunchEpochOutsideLifetime {
            launch_epoch: 21,
            valid_from: 10,
            valid_through: 20,
        })
    );
    assert_eq!(
        StaticViewLifetimeEpochClaimV1::new(Digest::from_bytes([0; 32]), 10, 20, 12),
        Err(StaticViewProofErrorV1::ZeroIdentity {
            field: "claimed allocation epoch"
        })
    );
}

#[test]
fn exclusive_write_claim_requires_symbolic_lease_but_authenticates_nothing() {
    let source = source("src/kernel.rs");
    assert_eq!(
        StaticViewProofObligationV1::new(
            description_with(StaticViewAccessDescriptionV1::ExclusiveWrite, 7),
            &source,
            digest(0x21),
            digest(0x22),
            digest(0x23),
            digest(0x24),
            digest(0x25),
            lifetime(0x30),
            None,
        ),
        Err(StaticViewProofErrorV1::MissingClaimedExclusiveLease)
    );
    assert_eq!(
        StaticViewProofObligationV1::new(
            description_with(StaticViewAccessDescriptionV1::SharedRead, 7),
            &source,
            digest(0x21),
            digest(0x22),
            digest(0x23),
            digest(0x24),
            digest(0x25),
            lifetime(0x30),
            Some(digest(0x26)),
        ),
        Err(StaticViewProofErrorV1::UnexpectedClaimedExclusiveLease)
    );
}

#[test]
fn request_evidence_requires_every_requested_property() {
    let source = source("src/kernel.rs");
    let obligation = obligation_with(
        &source,
        StaticViewAccessDescriptionV1::SharedRead,
        digest(0x21),
        0x30,
        0,
    );
    let (complete_request, control_flow) = request_and_control_flow(
        &source,
        &obligation,
        STATIC_VIEW_PROOF_REQUIRED_PROPERTIES_V1.to_vec(),
    );
    for missing in STATIC_VIEW_PROOF_REQUIRED_PROPERTIES_V1 {
        let properties = STATIC_VIEW_PROOF_REQUIRED_PROPERTIES_V1
            .into_iter()
            .filter(|property| *property != missing)
            .collect();
        let incomplete = request(
            complete_request.target().functional_specification_digest,
            properties,
        );
        assert_eq!(
            bind_static_view_proof_evidence_v1(
                &incomplete,
                control_flow.clone(),
                obligation.clone(),
            ),
            Err(StaticViewProofErrorV1::MissingRequestedProperty { property: missing })
        );
    }
}

#[test]
fn source_layout_launch_region_and_epoch_substitutions_change_the_obligation() {
    let changed_source = source("src/other.rs");
    let source = source("src/kernel.rs");
    let original = obligation_with(
        &source,
        StaticViewAccessDescriptionV1::SharedRead,
        digest(0x21),
        0x30,
        0,
    );
    let substitutions = [
        obligation_with(
            &changed_source,
            StaticViewAccessDescriptionV1::SharedRead,
            digest(0x21),
            0x30,
            0,
        ),
        StaticViewProofObligationV1::new(
            description_with(StaticViewAccessDescriptionV1::SharedRead, 8),
            &source,
            digest(0x21),
            digest(0x22),
            digest(0x23),
            digest(0x24),
            digest(0x25),
            lifetime(0x30),
            None,
        )
        .unwrap(),
        StaticViewProofObligationV1::new(
            description_with(StaticViewAccessDescriptionV1::SharedRead, 7),
            &source,
            digest(0x21),
            digest(0x22),
            digest(0x99),
            digest(0x24),
            digest(0x25),
            lifetime(0x30),
            None,
        )
        .unwrap(),
        StaticViewProofObligationV1::new(
            description_with(StaticViewAccessDescriptionV1::SharedRead, 7),
            &source,
            digest(0x21),
            digest(0x22),
            digest(0x23),
            digest(0x24),
            digest(0x99),
            lifetime(0x31),
            None,
        )
        .unwrap(),
    ];
    for substituted in substitutions {
        assert_ne!(
            original.obligation_identity(),
            substituted.obligation_identity()
        );
    }
}

#[test]
fn exact_target_axes_are_checked_without_granting_authority() {
    let source = source("src/kernel.rs");
    let obligation = obligation_with(
        &source,
        StaticViewAccessDescriptionV1::SharedRead,
        digest(0x99),
        0x30,
        0,
    );
    let base = derive_static_view_functional_specification_digest_v1(&obligation);
    let functional = derive_control_flow_functional_specification_digest_v1(base, &source).unwrap();
    let request = request(
        functional,
        STATIC_VIEW_PROOF_REQUIRED_PROPERTIES_V1.to_vec(),
    );
    let control_flow = bind_control_flow_proof_request_v1(&request, base, source).unwrap();
    assert_eq!(
        bind_static_view_proof_evidence_v1(&request, control_flow, obligation),
        Err(StaticViewProofErrorV1::TargetIdentityMismatch {
            field: "source tree"
        })
    );
}
