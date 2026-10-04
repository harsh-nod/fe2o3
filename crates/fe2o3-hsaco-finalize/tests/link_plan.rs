use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkInputV1, LinkOptionV1, LinkOutputV1, LinkPlanError, MAX_LINK_INPUTS,
    MAX_LINK_OPTION_NAME_BYTES, MAX_LINK_OPTION_VALUE_BYTES, MAX_LINK_OPTIONS,
    MAX_LINK_PROVENANCE_EDGES, MAX_LINK_PROVENANCE_NODES, MultiInputLinkPlanV1, ProvenanceNodeV1,
};
use fe2o3_kernel_descriptor::DeviceTargetV1;

fn id(byte: u8) -> ContentIdentityV1 {
    ContentIdentityV1::from_parts([byte; 32], u64::from(byte) + 1)
}

fn target() -> DeviceTargetV1 {
    DeviceTargetV1::parse("gfx942:xnack-").unwrap()
}

fn other_target() -> DeviceTargetV1 {
    DeviceTargetV1::parse("gfx950").unwrap()
}

fn input(byte: u8) -> LinkInputV1 {
    LinkInputV1::new(id(byte), target())
}

fn node(byte: u8, parents: &[u8]) -> ProvenanceNodeV1 {
    ProvenanceNodeV1::new(id(byte), parents.iter().copied().map(id).collect()).unwrap()
}

fn standard_parts() -> (
    Vec<LinkInputV1>,
    Vec<LinkOptionV1>,
    LinkOutputV1,
    Vec<ProvenanceNodeV1>,
) {
    (
        vec![input(10), input(20)],
        vec![
            LinkOptionV1::new("code-object-version", "6").unwrap(),
            LinkOptionV1::new("strip-debug", "true").unwrap(),
        ],
        LinkOutputV1::new(id(30), target()),
        vec![
            node(1, &[]),
            node(2, &[]),
            node(10, &[1]),
            node(20, &[2]),
            node(30, &[10, 20]),
        ],
    )
}

fn standard_plan() -> MultiInputLinkPlanV1 {
    let (inputs, options, output, provenance) = standard_parts();
    MultiInputLinkPlanV1::new(target(), inputs, options, output, provenance).unwrap()
}

#[test]
fn constructs_bounded_closed_multi_input_plan() {
    let plan = standard_plan();
    assert_eq!(plan.target(), target());
    assert_eq!(plan.inputs().len(), 2);
    assert_eq!(plan.options()[0].name(), "code-object-version");
    assert_eq!(plan.options()[0].value(), "6");
    assert_eq!(plan.output().identity(), id(30));
    assert_eq!(plan.provenance().len(), 5);
    assert_eq!(plan.identity().as_bytes().len(), 32);
    assert!(!plan.canonical_bytes().is_empty());
    assert!(!plan.grants_launch_authority());
}

#[test]
fn preserves_the_existing_single_input_use_case() {
    let plan = MultiInputLinkPlanV1::new(
        target(),
        vec![input(10)],
        vec![],
        LinkOutputV1::new(id(30), target()),
        vec![node(1, &[]), node(10, &[1]), node(30, &[10])],
    )
    .unwrap();
    assert_eq!(plan.inputs(), &[input(10)]);
}

#[test]
fn canonicalization_is_deterministic_across_set_order() {
    let (inputs, options, output, provenance) = standard_parts();
    let expected = MultiInputLinkPlanV1::canonicalized(
        target(),
        inputs.clone(),
        options.clone(),
        output,
        provenance.clone(),
    )
    .unwrap();

    let mut reversed_inputs = inputs;
    reversed_inputs.reverse();
    let mut reversed_options = options;
    reversed_options.reverse();
    let mut reversed_provenance = provenance;
    reversed_provenance.reverse();
    for entry in &mut reversed_provenance {
        let mut parents = entry.parents().to_vec();
        parents.reverse();
        *entry = ProvenanceNodeV1::new(entry.identity(), parents).unwrap();
    }
    let reordered = MultiInputLinkPlanV1::canonicalized(
        target(),
        reversed_inputs,
        reversed_options,
        output,
        reversed_provenance,
    )
    .unwrap();

    assert_eq!(expected, reordered);
    assert_eq!(expected.canonical_bytes(), reordered.canonical_bytes());
    assert_eq!(expected.identity(), reordered.identity());
}

#[test]
fn canonical_encoding_and_identity_have_stable_golden_values() {
    let plan = standard_plan();
    assert_eq!(
        to_hex(plan.identity().as_bytes()),
        "12ded4c2f2117fd0c68970c25ed4de274c4e5472e1e14a4e5e293ca13f044dad"
    );
    assert_eq!(plan.canonical_bytes().len(), 618);
}

#[test]
fn verifies_expected_output_digest_and_length() {
    let bytes = b"linked hsaco";
    let output_identity = ContentIdentityV1::calculate(bytes);
    // The calculated output digest may sort before the fixed test identities.
    let mut provenance = vec![
        node(1, &[]),
        node(10, &[1]),
        ProvenanceNodeV1::new(output_identity, vec![id(10)]).unwrap(),
    ];
    provenance.sort_by_key(ProvenanceNodeV1::identity);
    let plan = MultiInputLinkPlanV1::new(
        target(),
        vec![input(10)],
        vec![],
        LinkOutputV1::new(output_identity, target()),
        provenance,
    )
    .unwrap();

    assert!(plan.verify_output_bytes(bytes).is_ok());
    assert_eq!(
        plan.verify_output_bytes(b"linked hsacp"),
        Err(LinkPlanError::OutputIdentityMismatch)
    );
    assert_eq!(
        plan.verify_output_bytes(b"linked hsaco!"),
        Err(LinkPlanError::OutputIdentityMismatch)
    );
}

#[test]
fn rejects_empty_too_many_and_noncanonical_inputs() {
    let (_, options, output, provenance) = standard_parts();
    assert_eq!(
        MultiInputLinkPlanV1::new(target(), vec![], options, output, provenance),
        Err(LinkPlanError::NoInputs)
    );

    let too_many = vec![input(10); MAX_LINK_INPUTS + 1];
    assert_eq!(
        MultiInputLinkPlanV1::new(target(), too_many, vec![], output, vec![]),
        Err(LinkPlanError::TooManyInputs)
    );

    let (mut inputs, options, output, provenance) = standard_parts();
    inputs.reverse();
    assert_eq!(
        MultiInputLinkPlanV1::new(target(), inputs, options, output, provenance),
        Err(LinkPlanError::NonCanonicalOrder("link inputs"))
    );
}

#[test]
fn rejects_duplicate_and_conflicting_input_identities() {
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            vec![input(10), input(10)],
            vec![],
            LinkOutputV1::new(id(30), target()),
            vec![],
        ),
        Err(LinkPlanError::DuplicateInput(id(10)))
    );

    let first = ContentIdentityV1::from_parts([10; 32], 11);
    let second = ContentIdentityV1::from_parts([10; 32], 12);
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            vec![
                LinkInputV1::new(first, target()),
                LinkInputV1::new(second, target()),
            ],
            vec![],
            LinkOutputV1::new(id(30), target()),
            vec![],
        ),
        Err(LinkPlanError::ConflictingContentLength([10; 32]))
    );
}

#[test]
fn rejects_empty_and_oversized_content() {
    let empty = ContentIdentityV1::from_parts([1; 32], 0);
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            vec![LinkInputV1::new(empty, target())],
            vec![],
            LinkOutputV1::new(id(30), target()),
            vec![],
        ),
        Err(LinkPlanError::EmptyContent)
    );

    let oversized = ContentIdentityV1::from_parts([1; 32], MAX_HSACO_BYTES as u64 + 1);
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            vec![LinkInputV1::new(oversized, target())],
            vec![],
            LinkOutputV1::new(id(30), target()),
            vec![],
        ),
        Err(LinkPlanError::ContentTooLarge)
    );
}

#[test]
fn rejects_input_and_output_target_mismatches() {
    let (mut inputs, options, output, provenance) = standard_parts();
    inputs[0] = LinkInputV1::new(inputs[0].identity(), other_target());
    assert_eq!(
        MultiInputLinkPlanV1::new(target(), inputs, options, output, provenance),
        Err(LinkPlanError::InputTargetMismatch(id(10)))
    );

    let (inputs, options, _, provenance) = standard_parts();
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            inputs,
            options,
            LinkOutputV1::new(id(30), other_target()),
            provenance,
        ),
        Err(LinkPlanError::OutputTargetMismatch)
    );
}

#[test]
fn rejects_output_identity_aliasing_an_input() {
    let (inputs, options, _, provenance) = standard_parts();
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            inputs,
            options,
            LinkOutputV1::new(id(10), target()),
            provenance,
        ),
        Err(LinkPlanError::OutputAliasesInput)
    );
}

#[test]
fn validates_option_text_and_bounds() {
    assert_eq!(
        LinkOptionV1::new("", "1"),
        Err(LinkPlanError::EmptyOptionName)
    );
    assert_eq!(
        LinkOptionV1::new("A", "1"),
        Err(LinkPlanError::InvalidOptionName)
    );
    assert_eq!(
        LinkOptionV1::new("bad option", "1"),
        Err(LinkPlanError::InvalidOptionName)
    );
    assert_eq!(
        LinkOptionV1::new("x".repeat(MAX_LINK_OPTION_NAME_BYTES + 1), "1"),
        Err(LinkPlanError::OptionNameTooLong)
    );
    assert_eq!(
        LinkOptionV1::new("x", "two words"),
        Err(LinkPlanError::InvalidOptionValue)
    );
    assert_eq!(
        LinkOptionV1::new("x", "v".repeat(MAX_LINK_OPTION_VALUE_BYTES + 1)),
        Err(LinkPlanError::OptionValueTooLong)
    );
}

#[test]
fn rejects_duplicate_conflicting_and_noncanonical_options() {
    let (inputs, _, output, provenance) = standard_parts();
    let option = LinkOptionV1::new("opt-level", "2").unwrap();
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            inputs.clone(),
            vec![option.clone(), option],
            output,
            provenance.clone(),
        ),
        Err(LinkPlanError::DuplicateOption("opt-level".to_owned()))
    );
    assert_eq!(
        MultiInputLinkPlanV1::canonicalized(
            target(),
            inputs.clone(),
            vec![
                LinkOptionV1::new("opt-level", "2").unwrap(),
                LinkOptionV1::new("opt-level", "3").unwrap(),
            ],
            output,
            provenance.clone(),
        ),
        Err(LinkPlanError::ConflictingOption("opt-level".to_owned()))
    );
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            inputs,
            vec![
                LinkOptionV1::new("strip-debug", "true").unwrap(),
                LinkOptionV1::new("opt-level", "2").unwrap(),
            ],
            output,
            provenance,
        ),
        Err(LinkPlanError::NonCanonicalOrder("link options"))
    );
}

#[test]
fn rejects_too_many_options() {
    let (inputs, _, output, provenance) = standard_parts();
    let options = (0..=MAX_LINK_OPTIONS)
        .map(|index| LinkOptionV1::new(format!("x{index:02}"), "1").unwrap())
        .collect();
    assert_eq!(
        MultiInputLinkPlanV1::new(target(), inputs, options, output, provenance),
        Err(LinkPlanError::TooManyOptions)
    );
}

#[test]
fn requires_input_and_output_provenance_nodes() {
    let (inputs, options, output, mut provenance) = standard_parts();
    provenance.retain(|node| node.identity() != id(10));
    assert_eq!(
        MultiInputLinkPlanV1::new(target(), inputs, options, output, provenance),
        Err(LinkPlanError::MissingProvenanceNode(id(10)))
    );

    let (inputs, options, output, mut provenance) = standard_parts();
    provenance.retain(|node| node.identity() != id(30));
    assert_eq!(
        MultiInputLinkPlanV1::new(target(), inputs, options, output, provenance),
        Err(LinkPlanError::MissingProvenanceNode(id(30)))
    );
}

#[test]
fn output_provenance_must_name_exactly_all_inputs() {
    let (inputs, options, output, mut provenance) = standard_parts();
    *provenance.last_mut().unwrap() = node(30, &[10]);
    assert_eq!(
        MultiInputLinkPlanV1::new(target(), inputs, options, output, provenance),
        Err(LinkPlanError::OutputParentsMismatch)
    );
}

#[test]
fn rejects_unknown_duplicate_and_noncanonical_parents() {
    let (inputs, options, output, mut provenance) = standard_parts();
    provenance[2] = node(10, &[3]);
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            inputs.clone(),
            options.clone(),
            output,
            provenance,
        ),
        Err(LinkPlanError::UnknownProvenanceParent(id(3)))
    );

    let (.., mut provenance) = standard_parts();
    provenance[2] = node(10, &[1, 1]);
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            inputs.clone(),
            options.clone(),
            output,
            provenance,
        ),
        Err(LinkPlanError::DuplicateProvenanceParent(id(1)))
    );

    let (.., mut provenance) = standard_parts();
    provenance[4] = node(30, &[20, 10]);
    assert_eq!(
        MultiInputLinkPlanV1::new(target(), inputs, options, output, provenance),
        Err(LinkPlanError::NonCanonicalOrder("provenance parents"))
    );
}

#[test]
fn rejects_duplicate_and_noncanonical_provenance_nodes() {
    let (inputs, options, output, mut provenance) = standard_parts();
    provenance.insert(1, node(1, &[]));
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            inputs.clone(),
            options.clone(),
            output,
            provenance,
        ),
        Err(LinkPlanError::DuplicateProvenanceNode(id(1)))
    );

    let (.., mut provenance) = standard_parts();
    provenance.swap(0, 1);
    assert_eq!(
        MultiInputLinkPlanV1::new(target(), inputs, options, output, provenance),
        Err(LinkPlanError::NonCanonicalOrder("provenance nodes"))
    );
}

#[test]
fn rejects_cycles_and_orphan_provenance() {
    let (inputs, options, output, mut provenance) = standard_parts();
    provenance[2] = node(10, &[1, 30]);
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            inputs.clone(),
            options.clone(),
            output,
            provenance,
        ),
        Err(LinkPlanError::ProvenanceCycle(id(30)))
    );

    let (.., mut provenance) = standard_parts();
    provenance.insert(2, node(3, &[]));
    assert_eq!(
        MultiInputLinkPlanV1::new(target(), inputs, options, output, provenance),
        Err(LinkPlanError::OrphanProvenanceNode(id(3)))
    );
}

#[test]
fn enforces_provenance_node_and_edge_bounds() {
    let (inputs, options, output, _) = standard_parts();
    let too_many_nodes = (0..=MAX_LINK_PROVENANCE_NODES)
        .map(|index| {
            let byte = (index % 250 + 1) as u8;
            node(byte, &[])
        })
        .collect();
    assert_eq!(
        MultiInputLinkPlanV1::new(target(), inputs, options, output, too_many_nodes),
        Err(LinkPlanError::TooManyProvenanceNodes)
    );

    assert_eq!(
        ProvenanceNodeV1::new(id(30), vec![id(1); MAX_LINK_PROVENANCE_EDGES + 1]),
        Err(LinkPlanError::TooManyProvenanceEdges)
    );

    let half_plus_one = MAX_LINK_PROVENANCE_EDGES / 2 + 1;
    let parents: Vec<_> = (0..half_plus_one).map(numbered_id).collect();
    let aggregate_overflow = vec![
        ProvenanceNodeV1::new(numbered_id(3000), parents.clone()).unwrap(),
        ProvenanceNodeV1::new(numbered_id(3001), parents).unwrap(),
    ];
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            vec![input(10)],
            vec![],
            LinkOutputV1::new(id(30), target()),
            aggregate_overflow,
        ),
        Err(LinkPlanError::TooManyProvenanceEdges)
    );
}

#[test]
fn accepts_a_maximum_depth_provenance_dag_without_recursion() {
    let mut provenance = Vec::with_capacity(MAX_LINK_PROVENANCE_NODES);
    provenance.push(ProvenanceNodeV1::new(numbered_id(0), vec![]).unwrap());
    for index in 1..MAX_LINK_PROVENANCE_NODES {
        provenance
            .push(ProvenanceNodeV1::new(numbered_id(index), vec![numbered_id(index - 1)]).unwrap());
    }
    let input_identity = numbered_id(MAX_LINK_PROVENANCE_NODES - 2);
    let output_identity = numbered_id(MAX_LINK_PROVENANCE_NODES - 1);

    let plan = MultiInputLinkPlanV1::new(
        target(),
        vec![LinkInputV1::new(input_identity, target())],
        vec![],
        LinkOutputV1::new(output_identity, target()),
        provenance,
    )
    .unwrap();
    assert_eq!(plan.provenance().len(), MAX_LINK_PROVENANCE_NODES);
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn numbered_id(index: usize) -> ContentIdentityV1 {
    let mut digest = [0; 32];
    digest[..8].copy_from_slice(&(index as u64).to_be_bytes());
    ContentIdentityV1::from_parts(digest, 1)
}
