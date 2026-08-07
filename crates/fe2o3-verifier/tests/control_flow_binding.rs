use fe2o3_rustc_front::{
    ControlFlowContractV1, ControlFlowNodeIdV1, ControlFlowNodeKindV1, ControlFlowNodeV1,
    FrontendIntegerSwitchCaseV1, FrontendIntegerSwitchTypeV1, FrontendSourceSpanV1,
    encode_control_flow_contract_v1,
};
use fe2o3_verifier::{
    Configuration, ControlFlowBindingErrorV1, ControlFlowClaimsV1,
    ControlFlowIntegerSwitchCaseClaimV1, ControlFlowIntegerSwitchClaimV1, ControlFlowLoopClaimV1,
    CorrelationId, Digest, ProofProperty, ProofRequestV1, ProofTargetIdentity,
    VerificationModelIdentity, bind_control_flow_proof_request_v1,
    derive_control_flow_functional_specification_digest_v1, reconcile_control_flow_source_v1,
};

fn digest(seed: u8) -> Digest {
    Digest::from_bytes([seed; 32])
}

fn span(line: u32) -> FrontendSourceSpanV1 {
    FrontendSourceSpanV1::new("src/kernel.rs", line, 1, line, 8).unwrap()
}

fn source_contract(max_iterations: u32) -> (Vec<u8>, Vec<u8>) {
    let id = ControlFlowNodeIdV1::new;
    let switch = ControlFlowNodeKindV1::integer_switch(
        FrontendIntegerSwitchTypeV1::new(32, true).unwrap(),
        vec![
            FrontendIntegerSwitchCaseV1::from_signed(1, id(4)),
            FrontendIntegerSwitchCaseV1::from_signed(0, id(3)),
        ],
        id(3),
    )
    .unwrap();
    let contract = ControlFlowContractV1::new(
        id(0),
        vec![
            ControlFlowNodeV1::new(
                id(0),
                span(10),
                ControlFlowNodeKindV1::Entry { target: id(1) },
            ),
            ControlFlowNodeV1::new(
                id(1),
                span(11),
                ControlFlowNodeKindV1::Loop {
                    max_iterations,
                    body: id(2),
                    exit: id(5),
                },
            ),
            ControlFlowNodeV1::new(id(2), span(12), switch),
            ControlFlowNodeV1::new(
                id(3),
                span(13),
                ControlFlowNodeKindV1::Continue {
                    loop_header: id(1),
                    target: id(1),
                },
            ),
            ControlFlowNodeV1::new(
                id(4),
                span(14),
                ControlFlowNodeKindV1::Break {
                    loop_header: id(1),
                    target: id(5),
                },
            ),
            ControlFlowNodeV1::new(id(5), span(15), ControlFlowNodeKindV1::Exit),
        ],
    )
    .unwrap();
    let cfg_identity = contract.cfg_identity().as_bytes().to_vec();
    (
        encode_control_flow_contract_v1(&contract).unwrap(),
        cfg_identity,
    )
}

fn switch_claim(node_id: u32) -> ControlFlowIntegerSwitchClaimV1 {
    ControlFlowIntegerSwitchClaimV1::new(
        node_id,
        32,
        true,
        vec![
            ControlFlowIntegerSwitchCaseClaimV1::new(1, 4),
            ControlFlowIntegerSwitchCaseClaimV1::new(0, 3),
        ],
        3,
    )
    .unwrap()
}

fn claims(max_iterations: u32) -> ControlFlowClaimsV1 {
    ControlFlowClaimsV1::new(
        vec![ControlFlowLoopClaimV1::new(1, max_iterations).unwrap()],
        vec![switch_claim(2)],
    )
    .unwrap()
}

fn target(functional_specification_digest: Digest) -> ProofTargetIdentity {
    ProofTargetIdentity {
        kernel_id: digest(1),
        instance_digest: digest(2),
        source_tree_digest: digest(3),
        crate_graph_digest: digest(4),
        executable_digest: digest(5),
        environment_digest: digest(6),
        artifact_selection_digest: digest(7),
        artifact_contract_digest: digest(8),
        memory_contract_digest: digest(9),
        effects_contract_digest: digest(10),
        type_layout_digest: digest(11),
        capability_semantics_digest: digest(12),
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
        VerificationModelIdentity::new("control-flow-model-v1", digest(14)).unwrap(),
        properties,
        vec![],
    )
    .unwrap()
}

fn required_properties() -> Vec<ProofProperty> {
    vec![ProofProperty::Bounds, ProofProperty::FunctionalCorrectness]
}

#[test]
fn canonical_claims_bind_exact_source_cfg_and_request() {
    let (source, cfg) = source_contract(4);
    let source_binding = reconcile_control_flow_source_v1(&source, &cfg, claims(4)).unwrap();
    let base = digest(13);
    let functional =
        derive_control_flow_functional_specification_digest_v1(base, &source_binding).unwrap();
    let request = request(functional, required_properties());
    let binding =
        bind_control_flow_proof_request_v1(&request, base, source_binding.clone()).unwrap();

    assert_eq!(binding.source(), &source_binding);
    assert_eq!(binding.functional_specification_digest(), functional);
    assert_eq!(binding.target(), request.target());
    assert_ne!(binding.request_digest(), Digest::from_bytes([0; 32]));
    assert_ne!(binding.binding_identity(), binding.request_digest());
    assert!(!binding.grants_proof_authority());
    assert!(!binding.source().grants_compiler_authority());

    let mut expected = Vec::new();
    expected.extend_from_slice(b"FE2CFCL\0");
    expected.extend_from_slice(&1_u16.to_le_bytes());
    expected.extend_from_slice(&0_u16.to_le_bytes());
    expected.extend_from_slice(&1_u16.to_le_bytes());
    expected.extend_from_slice(&1_u16.to_le_bytes());
    expected.extend_from_slice(&1_u32.to_le_bytes());
    expected.extend_from_slice(&4_u32.to_le_bytes());
    expected.extend_from_slice(&2_u32.to_le_bytes());
    expected.extend_from_slice(&32_u16.to_le_bytes());
    expected.extend_from_slice(&[1, 0]);
    expected.extend_from_slice(&2_u16.to_le_bytes());
    expected.extend_from_slice(&0_u16.to_le_bytes());
    expected.extend_from_slice(&3_u32.to_le_bytes());
    expected.extend_from_slice(&0_u128.to_le_bytes());
    expected.extend_from_slice(&3_u32.to_le_bytes());
    expected.extend_from_slice(&1_u128.to_le_bytes());
    expected.extend_from_slice(&4_u32.to_le_bytes());
    assert_eq!(binding.source().claims().to_canonical_bytes(), expected);
}

#[test]
fn stale_or_missing_cfg_identity_fails_closed() {
    let (source, cfg) = source_contract(4);
    assert_eq!(
        reconcile_control_flow_source_v1(&source, &[], claims(4)),
        Err(ControlFlowBindingErrorV1::CfgIdentityMismatch)
    );

    let mut stale = cfg;
    let last = stale.len() - 1;
    stale[last] ^= 1;
    assert_eq!(
        reconcile_control_flow_source_v1(&source, &stale, claims(4)),
        Err(ControlFlowBindingErrorV1::CfgIdentityMismatch)
    );
    assert!(matches!(
        reconcile_control_flow_source_v1(&source[..source.len() - 1], &stale, claims(4)),
        Err(ControlFlowBindingErrorV1::SourceContract(_))
    ));
}

#[test]
fn missing_extra_and_stale_loop_claims_are_distinct() {
    let (source, cfg) = source_contract(4);
    let missing = ControlFlowClaimsV1::new(vec![], vec![switch_claim(2)]).unwrap();
    assert_eq!(
        reconcile_control_flow_source_v1(&source, &cfg, missing),
        Err(ControlFlowBindingErrorV1::MissingLoopClaim { node_id: 1 })
    );

    let extra = ControlFlowClaimsV1::new(
        vec![
            ControlFlowLoopClaimV1::new(1, 4).unwrap(),
            ControlFlowLoopClaimV1::new(6, 1).unwrap(),
        ],
        vec![switch_claim(2)],
    )
    .unwrap();
    assert_eq!(
        reconcile_control_flow_source_v1(&source, &cfg, extra),
        Err(ControlFlowBindingErrorV1::UnexpectedLoopClaim { node_id: 6 })
    );

    assert_eq!(
        reconcile_control_flow_source_v1(&source, &cfg, claims(5)),
        Err(ControlFlowBindingErrorV1::LoopClaimMismatch { node_id: 1 })
    );
}

#[test]
fn missing_extra_and_stale_switch_claims_are_distinct() {
    let (source, cfg) = source_contract(4);
    let loop_claim = ControlFlowLoopClaimV1::new(1, 4).unwrap();
    let missing = ControlFlowClaimsV1::new(vec![loop_claim], vec![]).unwrap();
    assert_eq!(
        reconcile_control_flow_source_v1(&source, &cfg, missing),
        Err(ControlFlowBindingErrorV1::MissingIntegerSwitchClaim { node_id: 2 })
    );

    let extra =
        ControlFlowClaimsV1::new(vec![loop_claim], vec![switch_claim(2), switch_claim(6)]).unwrap();
    assert_eq!(
        reconcile_control_flow_source_v1(&source, &cfg, extra),
        Err(ControlFlowBindingErrorV1::UnexpectedIntegerSwitchClaim { node_id: 6 })
    );

    let changed = ControlFlowIntegerSwitchClaimV1::new(
        2,
        32,
        true,
        vec![
            ControlFlowIntegerSwitchCaseClaimV1::new(0, 3),
            ControlFlowIntegerSwitchCaseClaimV1::new(1, 3),
        ],
        3,
    )
    .unwrap();
    let changed = ControlFlowClaimsV1::new(vec![loop_claim], vec![changed]).unwrap();
    assert_eq!(
        reconcile_control_flow_source_v1(&source, &cfg, changed),
        Err(ControlFlowBindingErrorV1::IntegerSwitchClaimMismatch { node_id: 2 })
    );
}

#[test]
fn invalid_or_unbounded_claim_sets_are_rejected() {
    assert_eq!(
        ControlFlowLoopClaimV1::new(1, 0),
        Err(ControlFlowBindingErrorV1::ZeroLoopBound { node_id: 1 })
    );
    assert_eq!(
        ControlFlowIntegerSwitchClaimV1::new(2, 24, false, vec![], 3),
        Err(ControlFlowBindingErrorV1::UnsupportedIntegerWidth {
            node_id: 2,
            width: 24,
        })
    );
    assert_eq!(
        ControlFlowIntegerSwitchClaimV1::new(
            2,
            8,
            false,
            vec![ControlFlowIntegerSwitchCaseClaimV1::new(256, 3)],
            3,
        ),
        Err(ControlFlowBindingErrorV1::IntegerCaseOutOfRange {
            node_id: 2,
            bits: 256,
        })
    );
    assert_eq!(
        ControlFlowIntegerSwitchClaimV1::new(
            2,
            8,
            false,
            vec![
                ControlFlowIntegerSwitchCaseClaimV1::new(1, 3),
                ControlFlowIntegerSwitchCaseClaimV1::new(1, 4),
            ],
            3,
        ),
        Err(ControlFlowBindingErrorV1::DuplicateIntegerCase {
            node_id: 2,
            bits: 1,
        })
    );

    let loops = (0..=fe2o3_contracts::MAX_SOURCE_LOOPS_V1)
        .map(|node| ControlFlowLoopClaimV1::new(u32::from(node), 1).unwrap())
        .collect();
    assert_eq!(
        ControlFlowClaimsV1::new(loops, vec![]),
        Err(ControlFlowBindingErrorV1::TooManyClaims {
            field: "loop claims",
            max: fe2o3_contracts::MAX_SOURCE_LOOPS_V1 as usize,
        })
    );
}

#[test]
fn proof_request_must_commit_the_exact_source_and_properties() {
    let (source, cfg) = source_contract(4);
    let source_binding = reconcile_control_flow_source_v1(&source, &cfg, claims(4)).unwrap();
    let base = digest(13);
    let functional =
        derive_control_flow_functional_specification_digest_v1(base, &source_binding).unwrap();

    let missing_bounds = request(functional, vec![ProofProperty::FunctionalCorrectness]);
    assert_eq!(
        bind_control_flow_proof_request_v1(&missing_bounds, base, source_binding.clone()),
        Err(ControlFlowBindingErrorV1::MissingProofProperty {
            property: ProofProperty::Bounds,
        })
    );
    let stale = request(digest(99), required_properties());
    assert_eq!(
        bind_control_flow_proof_request_v1(&stale, base, source_binding),
        Err(ControlFlowBindingErrorV1::FunctionalSpecificationMismatch)
    );
}

#[test]
fn every_single_bit_source_mutation_loses_the_original_request_binding() {
    let (source, cfg) = source_contract(4);
    let base = digest(13);
    let original_source = reconcile_control_flow_source_v1(&source, &cfg, claims(4)).unwrap();
    let functional =
        derive_control_flow_functional_specification_digest_v1(base, &original_source).unwrap();
    let request = request(functional, required_properties());

    for bit in 0..source.len() * 8 {
        let mut mutated = source.clone();
        mutated[bit / 8] ^= 1 << (bit % 8);
        if let Ok(mutated_source) = reconcile_control_flow_source_v1(&mutated, &cfg, claims(4)) {
            assert!(
                bind_control_flow_proof_request_v1(&request, base, mutated_source).is_err(),
                "source mutation at bit {bit} retained the original request binding"
            );
        }
    }
}
