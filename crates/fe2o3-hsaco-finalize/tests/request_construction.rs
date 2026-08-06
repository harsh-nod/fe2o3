use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkInputKindClosureV1, LinkInputV1, LinkOptionV1, LinkOutputV1,
    LinkSymbolClosureV1, MAX_WORKER_SYMBOLS, MAX_WORKER_TOOLCHAIN_ID_BYTES, MultiInputLinkPlanV1,
    ProvenanceNodeV1, WorkerInputKindV1, WorkerInputV1, WorkerOptimizationLevelV1, WorkerOptionsV1,
    WorkerOutputConstraintsV1, WorkerProtocolError, WorkerRequestConstructionError,
    WorkerRequestV1, construct_worker_request_v1,
};
use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};

const LLVM_BUILD_ID: &str = "llvmorg-22.0.0-rocm-7.2+0123456789abcdef";

struct Fixture {
    plan: MultiInputLinkPlanV1,
    inputs: Vec<WorkerInputV1>,
    input_kinds: LinkInputKindClosureV1,
}

fn target() -> DeviceTargetV1 {
    DeviceTargetV1::parse("gfx942:xnack-").unwrap()
}

fn other_target() -> DeviceTargetV1 {
    DeviceTargetV1::parse("gfx950").unwrap()
}

fn options() -> WorkerOptionsV1 {
    WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true)
}

fn plan_options() -> Vec<LinkOptionV1> {
    vec![
        LinkOptionV1::new("code-object-version", "6").unwrap(),
        LinkOptionV1::new("opt-level", "2").unwrap(),
        LinkOptionV1::new("strip-debug", "true").unwrap(),
        LinkOptionV1::new("verify-each", "true").unwrap(),
    ]
}

fn closure() -> LinkSymbolClosureV1 {
    LinkSymbolClosureV1::new(
        strings(&["external_add", "kernel_main", "rust_helper"]),
        strings(&["external_add"]),
        strings(&["rust_helper"]),
    )
    .unwrap()
}

fn fixture() -> Fixture {
    fixture_with(target(), plan_options())
}

fn fixture_with(target: DeviceTargetV1, options: Vec<LinkOptionV1>) -> Fixture {
    let mut inputs = vec![
        WorkerInputV1::new(
            WorkerInputKindV1::LlvmBitcode,
            b"canonical bitcode A".to_vec(),
        )
        .unwrap(),
        WorkerInputV1::new(
            WorkerInputKindV1::AmdGpuRelocatable,
            b"canonical object B".to_vec(),
        )
        .unwrap(),
    ];
    inputs.sort_by_key(|input| input.identity());
    let link_inputs: Vec<_> = inputs
        .iter()
        .map(|input| LinkInputV1::new(input.identity(), target))
        .collect();
    let output_identity = ContentIdentityV1::calculate(b"expected linked hsaco bytes");
    let mut provenance: Vec<_> = link_inputs
        .iter()
        .map(|input| ProvenanceNodeV1::new(input.identity(), vec![]).unwrap())
        .collect();
    provenance.push(
        ProvenanceNodeV1::new(
            output_identity,
            link_inputs.iter().map(|input| input.identity()).collect(),
        )
        .unwrap(),
    );
    let plan = MultiInputLinkPlanV1::canonicalized(
        target,
        link_inputs,
        options,
        LinkOutputV1::new(output_identity, target),
        provenance,
    )
    .unwrap();
    let input_kinds =
        LinkInputKindClosureV1::new(&plan, inputs.iter().map(|input| input.kind()).collect())
            .unwrap();
    Fixture {
        plan,
        inputs,
        input_kinds,
    }
}

fn construct(
    fixture: &Fixture,
    inputs: Vec<WorkerInputV1>,
    closure: &LinkSymbolClosureV1,
) -> Result<WorkerRequestV1, WorkerRequestConstructionError> {
    construct_worker_request_v1(
        &fixture.plan,
        LLVM_BUILD_ID,
        target(),
        CodeObjectVersion::V6,
        options(),
        inputs,
        &fixture.input_kinds,
        closure,
        WorkerOutputConstraintsV1::new(fixture.plan.output().identity().byte_len()).unwrap(),
    )
}

#[test]
fn constructs_a_deterministic_plan_bound_request_and_round_trips() {
    let fixture = fixture();
    let closure = closure();
    let first = construct(&fixture, fixture.inputs.clone(), &closure).unwrap();
    let second = construct(&fixture, fixture.inputs.clone(), &closure).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.target(), fixture.plan.target());
    assert_eq!(first.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(first.options(), options());
    assert_eq!(first.inputs(), fixture.inputs);
    assert_eq!(first.required_symbols(), closure.required_symbols());
    assert_eq!(first.expected_defined_symbols(), closure.required_symbols());
    assert_eq!(
        first.output_constraints().max_bytes(),
        fixture.plan.output().identity().byte_len()
    );
    assert_ne!(first.request_id(), &[0; 32]);
    assert!(!closure.grants_link_authority());
    assert!(!closure.grants_launch_authority());

    let decoded = WorkerRequestV1::decode(first.canonical_bytes()).unwrap();
    assert_eq!(decoded, first);
    for length in 0..first.canonical_bytes().len() {
        assert!(
            WorkerRequestV1::decode(&first.canonical_bytes()[..length]).is_err(),
            "accepted plan-bound request prefix {length}"
        );
    }
}

#[test]
fn input_mutation_truncation_permutation_and_substitution_fail_closed() {
    let fixture = fixture();
    let closure = closure();

    let mut mutated = fixture.inputs.clone();
    let mut bytes = mutated[0].bytes().to_vec();
    bytes[0] ^= 1;
    mutated[0] = WorkerInputV1::new(mutated[0].kind(), bytes).unwrap();
    assert!(matches!(
        construct(&fixture, mutated, &closure),
        Err(WorkerRequestConstructionError::InputIdentityMismatch { .. })
    ));

    let mut truncated = fixture.inputs.clone();
    let mut bytes = truncated[0].bytes().to_vec();
    bytes.pop();
    truncated[0] = WorkerInputV1::new(truncated[0].kind(), bytes).unwrap();
    assert!(matches!(
        construct(&fixture, truncated, &closure),
        Err(WorkerRequestConstructionError::InputIdentityMismatch { .. })
    ));

    let mut permuted = fixture.inputs.clone();
    permuted.reverse();
    assert!(matches!(
        construct(&fixture, permuted, &closure),
        Err(WorkerRequestConstructionError::InputIdentityMismatch { index: 0, .. })
    ));

    let duplicated = vec![fixture.inputs[0].clone(), fixture.inputs[0].clone()];
    assert!(matches!(
        construct(&fixture, duplicated, &closure),
        Err(WorkerRequestConstructionError::InputIdentityMismatch { .. })
    ));

    assert_eq!(
        construct(&fixture, fixture.inputs[..1].to_vec(), &closure),
        Err(WorkerRequestConstructionError::InputCountMismatch {
            planned: 2,
            provided: 1
        })
    );
    let mut extra = fixture.inputs.clone();
    extra.push(
        WorkerInputV1::new(
            WorkerInputKindV1::LlvmBitcode,
            b"unreferenced input".to_vec(),
        )
        .unwrap(),
    );
    assert_eq!(
        construct(&fixture, extra, &closure),
        Err(WorkerRequestConstructionError::InputCountMismatch {
            planned: 2,
            provided: 3
        })
    );

    let mut changed = fixture.inputs[0].bytes().to_vec();
    changed[0] ^= 1;
    assert_eq!(
        WorkerInputV1::from_declared(
            fixture.inputs[0].kind(),
            fixture.inputs[0].identity(),
            changed
        ),
        Err(WorkerProtocolError::ContentIdentityMismatch)
    );
}

#[test]
fn closure_rejects_empty_noncanonical_duplicate_and_conflicting_symbols() {
    assert_eq!(
        LinkSymbolClosureV1::new(vec![], vec![], vec![]),
        Err(WorkerRequestConstructionError::EmptySymbolClosure)
    );
    assert_eq!(
        LinkSymbolClosureV1::new(strings(&["z", "a"]), vec![], vec![]),
        Err(WorkerRequestConstructionError::InvalidRequiredSymbols(
            WorkerProtocolError::NonCanonicalSymbols
        ))
    );
    assert_eq!(
        LinkSymbolClosureV1::new(strings(&["a", "a"]), vec![], vec![]),
        Err(WorkerRequestConstructionError::InvalidRequiredSymbols(
            WorkerProtocolError::DuplicateSymbol
        ))
    );
    assert_eq!(
        LinkSymbolClosureV1::new(strings(&["a"]), strings(&["missing"]), vec![]),
        Err(WorkerRequestConstructionError::UnreferencedImport(
            "missing".to_owned()
        ))
    );
    assert_eq!(
        LinkSymbolClosureV1::new(strings(&["a"]), vec![], strings(&["missing"])),
        Err(WorkerRequestConstructionError::UnreferencedExport(
            "missing".to_owned()
        ))
    );
    assert_eq!(
        LinkSymbolClosureV1::new(strings(&["a"]), strings(&["a"]), strings(&["a"])),
        Err(WorkerRequestConstructionError::ConflictingSymbolRole(
            "a".to_owned()
        ))
    );
    assert_eq!(
        LinkSymbolClosureV1::new(strings(&["a"]), strings(&["bad symbol"]), vec![]),
        Err(WorkerRequestConstructionError::InvalidImportSymbols(
            WorkerProtocolError::InvalidSymbol
        ))
    );
}

#[test]
fn closure_bounds_and_direction_are_bound_into_request_identity() {
    let too_many: Vec<_> = (0..=MAX_WORKER_SYMBOLS)
        .map(|index| format!("symbol_{index:04}"))
        .collect();
    assert_eq!(
        LinkSymbolClosureV1::new(too_many, vec![], vec![]),
        Err(WorkerRequestConstructionError::InvalidRequiredSymbols(
            WorkerProtocolError::TooManySymbols
        ))
    );

    let fixture = fixture();
    let imported = LinkSymbolClosureV1::new(
        strings(&["kernel_main", "shared_helper"]),
        strings(&["shared_helper"]),
        vec![],
    )
    .unwrap();
    let exported = LinkSymbolClosureV1::new(
        strings(&["kernel_main", "shared_helper"]),
        vec![],
        strings(&["shared_helper"]),
    )
    .unwrap();
    assert_ne!(imported.identity(), exported.identity());

    let imported_request = construct(&fixture, fixture.inputs.clone(), &imported).unwrap();
    let exported_request = construct(&fixture, fixture.inputs.clone(), &exported).unwrap();
    assert_eq!(
        imported_request.required_symbols(),
        exported_request.required_symbols()
    );
    assert_ne!(imported_request.request_id(), exported_request.request_id());
    assert_ne!(imported_request.identity(), exported_request.identity());
}

#[test]
fn target_code_object_and_worker_options_must_exactly_match_the_plan() {
    let fixture = fixture();
    let closure = closure();
    let output =
        WorkerOutputConstraintsV1::new(fixture.plan.output().identity().byte_len()).unwrap();

    assert_eq!(
        construct_worker_request_v1(
            &fixture.plan,
            LLVM_BUILD_ID,
            other_target(),
            CodeObjectVersion::V6,
            options(),
            fixture.inputs.clone(),
            &fixture.input_kinds,
            &closure,
            output.clone()
        ),
        Err(WorkerRequestConstructionError::TargetMismatch)
    );
    assert_eq!(
        construct_worker_request_v1(
            &fixture.plan,
            LLVM_BUILD_ID,
            target(),
            CodeObjectVersion::V5,
            options(),
            fixture.inputs.clone(),
            &fixture.input_kinds,
            &closure,
            output.clone()
        ),
        Err(WorkerRequestConstructionError::CodeObjectVersionMismatch {
            planned: CodeObjectVersion::V6,
            requested: CodeObjectVersion::V5
        })
    );
    let wrong_options = WorkerOptionsV1::new(WorkerOptimizationLevelV1::O3, true, true);
    assert!(matches!(
        construct_worker_request_v1(
            &fixture.plan,
            LLVM_BUILD_ID,
            target(),
            CodeObjectVersion::V6,
            wrong_options,
            fixture.inputs.clone(),
            &fixture.input_kinds,
            &closure,
            output
        ),
        Err(WorkerRequestConstructionError::OptionsMismatch { .. })
    ));
}

#[test]
fn unsupported_missing_and_malformed_plan_options_fail_closed() {
    let cases = [
        (
            vec![LinkOptionV1::new("opt-level", "2").unwrap()],
            WorkerRequestConstructionError::MissingCodeObjectVersion,
        ),
        (
            vec![LinkOptionV1::new("code-object-version", "7").unwrap()],
            WorkerRequestConstructionError::InvalidCodeObjectVersion("7".to_owned()),
        ),
        (
            vec![
                LinkOptionV1::new("code-object-version", "6").unwrap(),
                LinkOptionV1::new("plugin", "enabled").unwrap(),
            ],
            WorkerRequestConstructionError::UnsupportedLinkOption("plugin".to_owned()),
        ),
        (
            vec![
                LinkOptionV1::new("code-object-version", "6").unwrap(),
                LinkOptionV1::new("opt-level", "fast").unwrap(),
            ],
            WorkerRequestConstructionError::InvalidLinkOptionValue {
                name: "opt-level".to_owned(),
                value: "fast".to_owned(),
            },
        ),
        (
            vec![
                LinkOptionV1::new("code-object-version", "6").unwrap(),
                LinkOptionV1::new("verify-each", "yes").unwrap(),
            ],
            WorkerRequestConstructionError::InvalidLinkOptionValue {
                name: "verify-each".to_owned(),
                value: "yes".to_owned(),
            },
        ),
    ];

    for (plan_options, expected) in cases {
        let fixture = fixture_with(target(), plan_options);
        assert_eq!(
            construct(&fixture, fixture.inputs.clone(), &closure()),
            Err(expected)
        );
    }
}

#[test]
fn output_and_public_text_bounds_are_checked_before_execution() {
    let fixture = fixture();
    let closure = closure();
    let wrong_output = WorkerOutputConstraintsV1::new(1).unwrap();
    assert_eq!(
        construct_worker_request_v1(
            &fixture.plan,
            LLVM_BUILD_ID,
            target(),
            CodeObjectVersion::V6,
            options(),
            fixture.inputs.clone(),
            &fixture.input_kinds,
            &closure,
            wrong_output
        ),
        Err(WorkerRequestConstructionError::OutputBoundMismatch {
            planned: fixture.plan.output().identity().byte_len(),
            requested: 1
        })
    );

    assert_eq!(
        construct_worker_request_v1(
            &fixture.plan,
            "x".repeat(MAX_WORKER_TOOLCHAIN_ID_BYTES + 1),
            target(),
            CodeObjectVersion::V6,
            options(),
            fixture.inputs.clone(),
            &fixture.input_kinds,
            &closure,
            WorkerOutputConstraintsV1::new(fixture.plan.output().identity().byte_len()).unwrap()
        ),
        Err(WorkerRequestConstructionError::WorkerProtocol(
            WorkerProtocolError::InvalidText("LLVM build identity")
        ))
    );
}

#[test]
fn input_kind_swap_fails_against_the_plan_bound_kind_closure() {
    let fixture = fixture();
    let closure = closure();
    let mut changed_kind = fixture.inputs.clone();
    let replacement_kind = match changed_kind[0].kind() {
        WorkerInputKindV1::LlvmBitcode => WorkerInputKindV1::AmdGpuRelocatable,
        WorkerInputKindV1::AmdGpuRelocatable => WorkerInputKindV1::LlvmBitcode,
    };
    changed_kind[0] = WorkerInputV1::from_declared(
        replacement_kind,
        changed_kind[0].identity(),
        changed_kind[0].bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(
        construct(&fixture, changed_kind, &closure),
        Err(WorkerRequestConstructionError::InputKindMismatch {
            index: 0,
            planned: fixture.input_kinds.kinds()[0],
            provided: replacement_kind,
        })
    );
    assert!(!fixture.input_kinds.grants_link_authority());
    assert!(!fixture.input_kinds.grants_launch_authority());
}

#[test]
fn input_kind_closure_rejects_wrong_count_and_different_plan() {
    let fixture = fixture();
    assert_eq!(
        LinkInputKindClosureV1::new(&fixture.plan, vec![WorkerInputKindV1::LlvmBitcode]),
        Err(WorkerRequestConstructionError::InputKindCountMismatch {
            planned: 2,
            declared: 1,
        })
    );

    let other = fixture_with(
        target(),
        vec![
            LinkOptionV1::new("code-object-version", "6").unwrap(),
            LinkOptionV1::new("opt-level", "3").unwrap(),
            LinkOptionV1::new("strip-debug", "true").unwrap(),
            LinkOptionV1::new("verify-each", "true").unwrap(),
        ],
    );
    assert_eq!(
        construct_worker_request_v1(
            &fixture.plan,
            LLVM_BUILD_ID,
            target(),
            CodeObjectVersion::V6,
            options(),
            fixture.inputs.clone(),
            &other.input_kinds,
            &closure(),
            WorkerOutputConstraintsV1::new(fixture.plan.output().identity().byte_len()).unwrap(),
        ),
        Err(WorkerRequestConstructionError::InputKindPlanMismatch {
            planned: fixture.plan.identity(),
            declared: other.plan.identity(),
        })
    );
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}
