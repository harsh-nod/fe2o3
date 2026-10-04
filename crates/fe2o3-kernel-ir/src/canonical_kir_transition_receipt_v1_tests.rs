use super::*;
use crate::{CanonicalKernelIrWorkBudgetV1, Module, VerifiedCanonicalKernelIrModuleV12};

const WORK: usize = 100_000_000;
const STORAGE: usize = 100_000_000;

fn empty() -> Candidate<'static> {
    Candidate {
        functions: &[],
        blocks: &[],
        segments: &[],
        operations: &[],
        definitions: &[],
        definition_outputs: &[],
        uses: &[],
        edges: &[],
        edge_arguments: &[],
    }
}
fn identity() -> VerifiedCanonicalKernelIrIdentityV12 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, _) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &Module::new("transition-codec-fixture"),
            &mut budget,
        )
        .unwrap();
    *owner.canonical().identity()
}

// Independent literal word layout for all variants in the fixed nine-slice order.
const GOLDEN_COUNTS: [u32; 9] = [1, 1, 2, 2, 3, 2, 2, 2, 2];
const GOLDEN_DIGEST: [u8; 32] = [
    0xcc, 0xb9, 0x15, 0x25, 0xb2, 0x2f, 0x65, 0x33, 0xc3, 0xd0, 0x26, 0xa0, 0x8f, 0x5a, 0x8a, 0x09,
    0x96, 0x69, 0x06, 0x46, 0x65, 0xf0, 0xbe, 0x1f, 0x98, 0xcc, 0x3c, 0x3a, 0x3a, 0xea, 0x30, 0x58,
];
const GOLDEN_WORDS: &[u32] = &[
    0, 1, // functions
    1, 3, 0, 2, // blocks
    0, 2, 1, 0, 2, 1, 0, 7, 0, 0, 0, 0, // segments: Some and None
    1, 3, 0, 0, 0, 2, 6, 0, 0, 1, 3, 1, 1, 2, 0, 2, 6, 1, // origins
    0, 0, 0, 0, 4, 0, 1, 1, 0, 2, 0, 5, 1, 0, 2, 0, 2, 6, 1, 1, 1, // definitions
    2, 1, 3, 0, 0, 0, 0, 1, 0, 0, 4, 1, // descendants
    0, 1, 3, 0, 2, 1, 0, 2, 0, 4, 1, 1, 3, 0, 1, 0, 0, 2, 6, 0, // uses
    1, 3, 0, 0, 7, 0, 1, 3, 1, 0, 7, 1, // edges
    1, 3, 0, 0, 0, 7, 0, 2, 1, 3, 1, 1, 0, 7, 1, 3, // edge arguments
];
fn golden(full: bool) -> Vec<u8> {
    let length = if full { 600_u32 } else { 132 };
    let mut bytes = Vec::with_capacity(length as usize);
    bytes.extend_from_slice(b"F2NTR1\0\0");
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(&[0x11; 32]);
    bytes.extend_from_slice(&37_u64.to_le_bytes());
    bytes.extend_from_slice(&[0x22; 32]);
    bytes.extend_from_slice(&91_u64.to_le_bytes());
    for count in if full { GOLDEN_COUNTS } else { [0; 9] } {
        bytes.extend_from_slice(&count.to_le_bytes());
    }
    if full {
        for word in GOLDEN_WORDS {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
    }
    assert_eq!(bytes.len(), length as usize);
    bytes
}
fn block(function: u32, block: u32) -> Block {
    Block {
        function: Function(function),
        block,
    }
}
fn operation(function: u32, block_id: u32, operation: u32) -> Operation {
    Operation {
        block: block(function, block_id),
        operation,
    }
}
fn edge(function: u32, block_id: u32, successor: u32) -> Edge {
    Edge {
        source: block(function, block_id),
        successor,
    }
}
fn fixture_rows() -> Rows {
    let original = Definition::Result {
        operation: operation(0, 2, 6),
        result: 1,
    };
    Rows {
        functions: vec![FunctionRow {
            input: Function(0),
            output: Function(1),
        }],
        blocks: vec![BlockRow {
            output: block(1, 3),
            segments: Range { start: 0, len: 2 },
        }],
        segments: vec![
            Segment {
                input: block(0, 2),
                connector: Some(edge(0, 2, 1)),
            },
            Segment {
                input: block(0, 7),
                connector: None,
            },
        ],
        operations: vec![
            OperationRow {
                output: operation(1, 3, 0),
                origin: Origin::Retained(operation(0, 2, 6)),
            },
            OperationRow {
                output: operation(1, 3, 1),
                origin: Origin::ConstantFrom(original),
            },
        ],
        definitions: vec![
            DefinitionRow {
                input: Definition::FunctionArgument {
                    function: Function(0),
                    argument: 4,
                },
                outputs: Range { start: 0, len: 1 },
            },
            DefinitionRow {
                input: Definition::BlockArgument {
                    block: block(0, 2),
                    argument: 5,
                },
                outputs: Range { start: 1, len: 0 },
            },
            DefinitionRow {
                input: original,
                outputs: Range { start: 1, len: 1 },
            },
        ],
        definition_outputs: vec![
            Descendant {
                output: Definition::Result {
                    operation: operation(1, 3, 0),
                    result: 0,
                },
                kind: DescendantKind::Retained,
            },
            Descendant {
                output: Definition::FunctionArgument {
                    function: Function(1),
                    argument: 4,
                },
                kind: DescendantKind::Substituted,
            },
        ],
        uses: vec![
            UseRow {
                output: Use::OperationOperand {
                    operation: operation(1, 3, 0),
                    operand: 2,
                },
                input: Use::TerminatorOperand {
                    block: block(0, 2),
                    operand: 4,
                },
            },
            UseRow {
                output: Use::TerminatorOperand {
                    block: block(1, 3),
                    operand: 1,
                },
                input: Use::OperationOperand {
                    operation: operation(0, 2, 6),
                    operand: 0,
                },
            },
        ],
        edges: vec![
            EdgeRow {
                output: edge(1, 3, 0),
                input: edge(0, 7, 0),
            },
            EdgeRow {
                output: edge(1, 3, 1),
                input: edge(0, 7, 1),
            },
        ],
        edge_arguments: vec![
            EdgeArgumentRow {
                output: EdgeArgument {
                    edge: edge(1, 3, 0),
                    argument: 0,
                },
                input: EdgeArgument {
                    edge: edge(0, 7, 0),
                    argument: 2,
                },
            },
            EdgeArgumentRow {
                output: EdgeArgument {
                    edge: edge(1, 3, 1),
                    argument: 1,
                },
                input: EdgeArgument {
                    edge: edge(0, 7, 1),
                    argument: 3,
                },
            },
        ],
    }
}
fn assert_rows(actual: Candidate<'_>, expected: Candidate<'_>) {
    assert_eq!(actual.functions, expected.functions);
    assert_eq!(actual.blocks, expected.blocks);
    assert_eq!(actual.segments, expected.segments);
    assert_eq!(actual.operations, expected.operations);
    assert_eq!(actual.definitions, expected.definitions);
    assert_eq!(actual.definition_outputs, expected.definition_outputs);
    assert_eq!(actual.uses, expected.uses);
    assert_eq!(actual.edges, expected.edges);
    assert_eq!(actual.edge_arguments, expected.edge_arguments);
}
fn typed_payload(full: bool) -> usize {
    if !full {
        return 0;
    }
    // Source-derived field-type equation, independent of codec extent()/SIZES.
    size_of::<FunctionRow>()
        + size_of::<BlockRow>()
        + 2 * size_of::<Segment>()
        + 2 * size_of::<OperationRow>()
        + 3 * size_of::<DefinitionRow>()
        + 2 * size_of::<Descendant>()
        + 2 * size_of::<UseRow>()
        + 2 * size_of::<EdgeRow>()
        + 2 * size_of::<EdgeArgumentRow>()
}

#[test]
fn literal_golden_exercises_every_coordinate_origin_and_descendant_tag() {
    assert_eq!(GOLDEN_WORDS.len(), 117);
    let id = identity();
    let bytes = golden(true);
    let expected = fixture_rows();
    let input_floor =
        17 + size_of::<Vec<u8>>() + bytes.capacity() + size_of::<Rows>() + typed_payload(true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(input_floor).unwrap();
    let (decoded, storage) =
        InertCanonicalKirTransitionReceiptV1::decode_with_budget(&bytes, &mut budget).unwrap();
    assert_eq!(budget.storage(), input_floor);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_rows(decoded.candidate(), expected.candidate());
    assert_eq!(decoded.canonical_bytes(), bytes);
    assert_eq!(decoded.input_identity().digest(), &[0x11; 32]);
    assert_eq!(decoded.output_identity().canonical_length(), 91);
    assert_eq!(decoded.checker_policy(), 1);
    assert!(!decoded.grants_authority());
    // Independently fixed SHA-256 of domain + length:u64 + all 600 golden bytes.
    assert_eq!(decoded.digest(), &GOLDEN_DIGEST);
    let (encoded, encoded_storage) =
        InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
            &id,
            &id,
            expected.candidate(),
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(encoded_storage.retained_storage())
        .unwrap();
    assert_eq!(&encoded.canonical_bytes()[96..], &bytes[96..]);
    assert!(encoded.input_identity().matches_verified(&id));
    assert_rows(encoded.candidate(), expected.candidate());
}

#[test]
fn decoder_empty_and_full_exact_one_under_work_and_storage_are_independent() {
    for full in [false, true] {
        let bytes = golden(full);
        let length = if full { 600 } else { 132 };
        let row_count = if full { 17 } else { 0 };
        let range_rows = if full { 4 } else { 0 };
        let retained =
            size_of::<InertCanonicalKirTransitionReceiptV1>() + length + typed_payload(full);
        let floor = 17 + size_of::<Vec<u8>>() + bytes.capacity();
        // Header parse132 + body parse(L-132)+row copiesN + range rowsB+D
        // + copyL + hash(domain + length8 + L) + entry1.
        let required = 1
            + 3 * length
            + row_count
            + range_rows
            + CANONICAL_KIR_TRANSITION_RECEIPT_IDENTITY_DOMAIN_V1.len()
            + 8;
        for exact_work in [false, true] {
            let mut work =
                CanonicalKernelIrWorkBudgetV1::new(7 + required - usize::from(!exact_work));
            let mut budget = Budget::new(&mut work, floor + retained);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(floor).unwrap();
            let result =
                InertCanonicalKirTransitionReceiptV1::decode_with_budget(&bytes, &mut budget);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor + retained);
            assert_eq!(budget.failed_storage(), None);
            if exact_work {
                let (_, receipt) = result.unwrap();
                assert_eq!(receipt.retained_storage(), retained);
                assert_eq!(budget.work(), 7 + required);
                assert_eq!(work.failed_work(), None);
            } else {
                assert!(matches!(
                    result,
                    Err(CanonicalKirTransitionReceiptErrorV1::Resource(
                        Resource::Work(_)
                    ))
                ));
                assert_eq!(budget.work(), 7 + 1 + length + row_count + range_rows);
                assert_eq!(work.failed_work(), Some(7 + required));
            }
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, floor + retained - 1);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            InertCanonicalKirTransitionReceiptV1::decode_with_budget(&bytes, &mut budget),
            Err(CanonicalKirTransitionReceiptErrorV1::Resource(
                Resource::Storage(_)
            ))
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.work(), 7 + 1 + 132);
        assert_eq!(budget.failed_storage(), Some(floor + retained));
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn encoder_empty_and_full_exact_one_under_work_and_storage_are_independent() {
    let id = identity();
    for full in [false, true] {
        let rows = fixture_rows();
        let candidate = if full { rows.candidate() } else { empty() };
        let length = if full { 600 } else { 132 };
        let count = if full { 17 } else { 0 };
        let range_rows = if full { 4 } else { 0 };
        let retained =
            size_of::<InertCanonicalKirTransitionReceiptV1>() + length + typed_payload(full);
        let floor = 17 + size_of::<Rows>() + typed_payload(true);
        // Entry1 + range rowsB+D + encodeL + typed copiesN + hashL/domain/length8.
        let required = 1
            + range_rows
            + 2 * length
            + count
            + CANONICAL_KIR_TRANSITION_RECEIPT_IDENTITY_DOMAIN_V1.len()
            + 8;
        for exact in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(7 + required - usize::from(!exact));
            let mut budget = Budget::new(&mut work, floor + retained);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(floor).unwrap();
            let result = InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
                &id,
                &id,
                candidate,
                &mut budget,
            );
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor + retained);
            assert_eq!(budget.failed_storage(), None);
            if exact {
                let (_, receipt) = result.unwrap();
                assert_eq!(receipt.retained_storage(), retained);
                assert_eq!(budget.work(), 7 + required);
                assert_eq!(work.failed_work(), None);
            } else {
                assert!(matches!(
                    result,
                    Err(CanonicalKirTransitionReceiptErrorV1::Resource(
                        Resource::Work(_)
                    ))
                ));
                assert_eq!(budget.work(), 7 + 1 + range_rows);
                assert_eq!(work.failed_work(), Some(7 + required));
            }
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, floor + retained - 1);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
                &id,
                &id,
                candidate,
                &mut budget
            ),
            Err(CanonicalKirTransitionReceiptErrorV1::Resource(
                Resource::Storage(_)
            ))
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.work(), 7 + 1 + range_rows);
        assert_eq!(budget.failed_storage(), Some(floor + retained));
        assert_eq!(work.failed_work(), None);
    }
}

fn reject(bytes: &[u8]) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = 17 + bytes.len();
    budget.reserve_storage(floor).unwrap();
    assert!(InertCanonicalKirTransitionReceiptV1::decode_with_budget(bytes, &mut budget).is_err());
    assert_eq!(budget.storage(), floor);
}
fn set_word(bytes: &mut [u8], offset: usize, word: u32) {
    bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
}

#[test]
fn malformed_header_counts_every_truncation_and_trailing_data_fail_closed() {
    let bytes = golden(true);
    for end in 0..bytes.len() {
        reject(&bytes[..end]);
    }
    for offset in [0, 8, 10] {
        let mut changed = bytes.clone();
        changed[offset] ^= 0x80;
        reject(&changed);
    }
    for offset in [12, 96, 100, 104, 108, 112, 116, 120, 124, 128] {
        let mut changed = bytes.clone();
        set_word(&mut changed, offset, u32::MAX);
        reject(&changed);
    }
    for offset in [48, 88] {
        let mut changed = bytes.clone();
        changed[offset..offset + 8].fill(0);
        reject(&changed);
    }
    let mut changed = bytes.clone();
    changed.push(0);
    reject(&changed);
    set_word(&mut changed, 12, 601);
    reject(&changed);
}

#[test]
fn unknown_tags_nonzero_padding_and_nonpartitioning_ranges_are_rejected() {
    let bytes = golden(true);
    // Section starts independently summed from the literal nine row widths:
    // functions132 blocks140 segments156 operations204 definitions276
    // descendants360 uses408 edges488 edge-arguments536. No production offset
    // helper is used.
    const SEGMENTS: usize = 132 + 8 + 16;
    const OPS: usize = SEGMENTS + 2 * 24;
    const DEFS: usize = OPS + 2 * 36;
    const DESC: usize = DEFS + 3 * 28;
    const USES: usize = DESC + 2 * 24;
    for offset in [
        SEGMENTS + 8,
        OPS + 12,
        DEFS,
        DEFS + 28,
        DEFS + 56,
        DESC + 20,
        USES,
        USES + 20,
    ] {
        let mut changed = bytes.clone();
        set_word(&mut changed, offset, 9);
        reject(&changed);
    }
    for offset in [
        SEGMENTS + 24 + 12,
        OPS + 28,
        DEFS + 8,
        DEFS + 12,
        DEFS + 28 + 12,
        USES + 20 + 12,
        USES + 40 + 12,
    ] {
        let mut changed = bytes.clone();
        set_word(&mut changed, offset, 1);
        reject(&changed);
    }
    for (offset, value) in [
        (140 + 8, 1),
        (140 + 12, 0),
        (140 + 12, 1),
        (DEFS + 20, 1),
        (DEFS + 28 + 20, 0),
        (DEFS + 56 + 24, u32::MAX),
    ] {
        let mut changed = bytes.clone();
        set_word(&mut changed, offset, value);
        reject(&changed);
    }
    let id = identity();
    let mut rows = fixture_rows();
    rows.definitions[1].outputs.start = 0;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(17 + size_of::<Rows>() + typed_payload(true))
        .unwrap();
    let floor = budget.storage();
    assert!(matches!(
        InertCanonicalKirTransitionReceiptV1::from_candidate_with_budget(
            &id,
            &id,
            rows.candidate(),
            &mut budget
        ),
        Err(CanonicalKirTransitionReceiptErrorV1::RangePartition)
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), floor);
}

#[test]
fn wire_bound_and_identity_are_inert_not_graph_admission() {
    assert_eq!(
        MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1,
        4 * 1024 * 1024
    );
    let id = identity();
    let bytes = golden(true);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (receipt, _) =
        InertCanonicalKirTransitionReceiptV1::decode_with_budget(&bytes, &mut budget).unwrap();
    assert!(!receipt.input_identity().matches_verified(&id));
    assert!(!receipt.output_identity().matches_verified(&id));
    assert!(!receipt.grants_authority());
}

#[test]
fn over_bound_frame_rejects_before_header_work_or_owned_allocation() {
    let bytes = vec![0; 4 * 1024 * 1024 + 1];
    let floor = 17 + size_of::<Vec<u8>>() + bytes.capacity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
    let mut budget = Budget::new(&mut work, floor);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        InertCanonicalKirTransitionReceiptV1::decode_with_budget(&bytes, &mut budget),
        Err(CanonicalKirTransitionReceiptErrorV1::Limit)
    ));
    assert_eq!(budget.work(), 8);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), floor);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(work.failed_work(), None);
}
