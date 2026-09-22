use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::Sha256;

pub(super) const FIXTURE_FLOOR: usize = 1024 * 1024;
pub(super) fn empty() -> Kernel {
    Kernel::new("k", 0, vec![Block::new(vec![], Term::Return)]).unwrap()
}
pub(super) fn literal_empty() -> Vec<u8> {
    // Independently specified header and one ordered empty return block.
    let mut bytes = b"F2RKR1\0\0".to_vec();
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&48u32.to_le_bytes());
    bytes.extend_from_slice(&61u64.to_le_bytes());
    for count in [1u32, 0, 0, 0, 1, 0] {
        bytes.extend_from_slice(&count.to_le_bytes());
    }
    bytes.push(b'k');
    bytes.extend_from_slice(&[0; 8]);
    bytes.extend_from_slice(&[11, 0, 0, 0]);
    assert_eq!(bytes.len(), 61);
    bytes
}
pub(super) fn roundtrip(kernel: &Kernel) -> DecodedRankedRecipeV1 {
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(FIXTURE_FLOOR).unwrap();
    let (bytes, bytes_storage) = encode_ranked_recipe_v1(kernel, &mut budget).unwrap();
    budget
        .reserve_storage(bytes_storage.retained_storage())
        .unwrap();
    let (view, view_storage) = read_ranked_recipe_v1(bytes.canonical_bytes(), &mut budget).unwrap();
    budget
        .reserve_storage(view_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let (decoded, decoded_storage) = materialize_ranked_recipe_v1(&view, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget
        .reserve_storage(decoded_storage.retained_storage())
        .unwrap();
    assert_eq!(decoded.kernel(), kernel);
    assert_eq!(decoded.identity(), view.identity());
    drop(view);
    budget
        .release_storage(view_storage.retained_storage())
        .unwrap();
    drop(bytes);
    budget
        .release_storage(bytes_storage.retained_storage())
        .unwrap();
    // Ownership leaves this test helper; no later work uses this ledger.
    decoded
}

#[test]
fn literal_wire_and_domain_length_identity_are_independent() {
    let expected = literal_empty();
    let kernel = empty();
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget
        .reserve_storage(
            size_of::<Kernel>()
                + kernel.function_name.capacity()
                + kernel.blocks.capacity() * size_of::<Block>(),
        )
        .unwrap();
    let (bytes, storage) = encode_ranked_recipe_v1(&kernel, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(bytes.canonical_bytes(), expected);
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/RANKED-RECIPE/V1\0");
    hash.update(61u64.to_le_bytes());
    hash.update(&expected);
    let expected_digest: [u8; 32] = hash.finalize().into();
    assert_eq!(bytes.identity(), expected_digest);
    let changed = Sha256::digest(&expected);
    assert_ne!(bytes.identity().as_slice(), changed.as_slice());
    let decoded = roundtrip(&kernel);
    assert_eq!(decoded.kernel.tree_work, 9);
}

#[test]
fn all_truncations_header_domains_reserved_tags_and_trailing_bytes_refuse() {
    let bytes = literal_empty();
    for end in 0..bytes.len() {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(FIXTURE_FLOOR / 32).unwrap();
        assert!(
            read_ranked_recipe_v1(&bytes[..end], &mut budget).is_err(),
            "end {end}"
        );
    }
    for offset in [0, 8, 10, 12, 16, 24, 28, 32, 36, 40, 44, 57, 59] {
        let mut hostile = bytes.clone();
        hostile[offset] ^= 0xff;
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, 100_000);
        budget.reserve_storage(4096).unwrap();
        assert!(
            read_ranked_recipe_v1(&hostile, &mut budget).is_err(),
            "offset {offset}"
        );
        assert_eq!(budget.storage(), 4096);
    }
    let mut hostile = bytes.clone();
    hostile.push(0);
    let length = hostile.len() as u64;
    hostile[16..24].copy_from_slice(&length.to_le_bytes());
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    assert!(matches!(
        read_ranked_recipe_v1(&hostile, &mut budget),
        Err(E::Length)
    ));
    let mut invalid_utf8 = bytes;
    invalid_utf8[48] = 0xff;
    assert!(matches!(
        read_ranked_recipe_v1(&invalid_utf8, &mut budget),
        Err(E::Utf8)
    ));
}

#[test]
fn framing_is_not_constructor_normal_form_and_normalization_is_rejected() {
    let raw = Kernel {
        function_name: "normalized".into(),
        argument_count: 0,
        tree_work: 0,
        blocks: vec![Block::new(
            vec![Op::View {
                result: Id::new(0),
                element_width: 32,
                writable: false,
                shape: vec![8],
                dynamic_extents: vec![],
                allocation_origin: 1,
                noalias_class: 1,
            }],
            Term::Return,
        )],
    };
    let normalized =
        Kernel::new(raw.function_name(), raw.argument_count, raw.blocks.clone()).unwrap();
    assert!(matches!(
        normalized.blocks[0].operations[0],
        Op::ViewInSpace {
            memory_space: MemorySpaceAttr::Global,
            ..
        }
    ));
    assert_ne!(raw.blocks, normalized.blocks);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 4 * FIXTURE_FLOOR);
    budget.reserve_storage(FIXTURE_FLOOR).unwrap();
    let (bytes, storage) = encode_ranked_recipe_v1(&raw, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (view, view_storage) = read_ranked_recipe_v1(bytes.canonical_bytes(), &mut budget).unwrap();
    budget
        .reserve_storage(view_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    assert!(matches!(
        materialize_ranked_recipe_v1(&view, &mut budget),
        Err(E::NonCanonical)
    ));
    assert_eq!(budget.storage(), floor);
    roundtrip(&normalized);
}

#[test]
fn syntax_valid_invalid_kernel_cannot_be_materialized() {
    let mut bytes = literal_empty();
    bytes[48] = b'!';
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    budget.reserve_storage(4096).unwrap();
    let (view, receipt) = read_ranked_recipe_v1(&bytes, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert!(matches!(
        materialize_ranked_recipe_v1(&view, &mut budget),
        Err(E::Constructor(
            ProductionRankedKernelErrorV1::InvalidFunctionName(_)
        ))
    ));
}

#[test]
fn value_coordinates_root_order_and_complete_recipe_fields_survive() {
    let kernel = Kernel::new(
        "ordered",
        1,
        vec![
            Block::new(
                vec![Op::IndexUnknown { result: Id::new(0) }],
                Term::AnalysisSplitArgs {
                    control_dependencies: vec![Value::Argument(0)],
                    first_arguments: vec![Value::Local(Id::new(0))],
                    second_arguments: vec![],
                    first_block: 2,
                    second_block: 1,
                },
            ),
            Block::new(vec![], Term::Return),
            Block::with_index_arguments(
                1,
                vec![],
                Term::IndexLessThan {
                    lhs: Value::BlockArgument {
                        block: 2,
                        argument: 0,
                    },
                    rhs: Value::Argument(0),
                    true_block: 1,
                    false_block: 1,
                },
            ),
        ],
    )
    .unwrap();
    roundtrip(&kernel);
    let mut changed = kernel.clone();
    changed.blocks.swap(1, 2);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 4 * FIXTURE_FLOOR);
    budget.reserve_storage(FIXTURE_FLOOR).unwrap();
    let (first, first_storage) = encode_ranked_recipe_v1(&kernel, &mut budget).unwrap();
    budget
        .reserve_storage(first_storage.retained_storage())
        .unwrap();
    let (second, second_storage) = encode_ranked_recipe_v1(&changed, &mut budget).unwrap();
    budget
        .reserve_storage(second_storage.retained_storage())
        .unwrap();
    assert_ne!(first.canonical_bytes(), second.canonical_bytes());
    assert_ne!(first.identity(), second.identity());
}

#[test]
fn admitted_argument_maximum_and_outer_block_limit_keep_distinct_bounds() {
    let kernel = Kernel::new("sixty_four", 64, vec![Block::new(vec![], Term::Return)]).unwrap();
    roundtrip(&kernel);
    for (offset, value, field, maximum) in [
        (36, 65u32, "arguments", 64usize),
        (24, 1025u32, "blocks", 1024usize),
    ] {
        let mut bytes = literal_empty();
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, FIXTURE_FLOOR);
        assert!(matches!(read_ranked_recipe_v1(&bytes, &mut budget),
            Err(E::Limit { field: actual_field, actual, limit })
                if (actual_field, actual, limit) == (field, value as usize, maximum)));
    }
}

#[test]
fn manual_mixed_recipe_keeps_all_scalar_and_receipt_fields_in_the_wire() {
    let scalar = Scalar::Integer {
        signed: true,
        bits: 16,
    };
    let proof = variant_tests::proof(0x5a);
    let kernel = Kernel::new(
        "proof_fields",
        0,
        vec![Block::new(
            vec![
                Op::SemanticExpression {
                    result: Id::new(0),
                    expression: Expr::Constant {
                        scalar,
                        bits: 0xffff,
                    },
                    numerical_contract: Numerical::ExactBitVectorOperatorCongruence,
                },
                Op::SemanticExpression {
                    result: Id::new(1),
                    expression: Expr::Symbol { scalar, symbol: 3 },
                    numerical_contract: Numerical::ExactBitVectorOperatorCongruence,
                },
                Op::RequireAuthenticatedReferenceEquivalent {
                    actual: Value::Local(Id::new(0)),
                    expected: Value::Local(Id::new(1)),
                    proof,
                },
            ],
            Term::Return,
        )],
    )
    .unwrap();
    let mut expected = b"F2RKR1\0\0".to_vec();
    expected.extend_from_slice(&[1, 0, 0, 0, 48, 0, 0, 0]);
    expected.extend_from_slice(&[0; 8]);
    for count in [1u32, 3, 2, 0, 12, 0] {
        expected.extend_from_slice(&count.to_le_bytes());
    }
    expected.extend_from_slice(b"proof_fields");
    expected.extend_from_slice(&[0, 0, 0, 0, 3, 0, 0, 0]);
    expected.extend_from_slice(&[31, 0, 0, 0, 0, 0, 0, 0, 2, 0, 2, 0, 1, 16, 0]);
    expected.extend_from_slice(&0xffffu64.to_le_bytes());
    expected.extend_from_slice(&[1, 0]);
    expected.extend_from_slice(&[
        31, 0, 0, 0, 1, 0, 0, 0, 1, 0, 3, 0, 0, 0, 2, 0, 1, 16, 0, 1, 0,
    ]);
    expected.extend_from_slice(&[34, 0, 0, 0, 3, 0, 0, 0, 0, 0, 3, 0, 1, 0, 0, 0]);
    expected.extend_from_slice(&[0x5a; 32]);
    expected.extend_from_slice(&[1, 0]);
    for byte in [1, 2, 3, 4, 5, 6] {
        expected.extend_from_slice(&[byte; 32]);
    }
    expected.extend_from_slice(&[11, 0, 0, 0]);
    let length = expected.len() as u64;
    expected[16..24].copy_from_slice(&length.to_le_bytes());
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, FIXTURE_FLOOR * 4);
    budget.reserve_storage(FIXTURE_FLOOR).unwrap();
    let (encoded, receipt) = encode_ranked_recipe_v1(&kernel, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(encoded.canonical_bytes(), expected);
    roundtrip(&kernel);
    // Every strict prefix of this mixed body must fail, not just the smallest fixture.
    for length in 0..expected.len() {
        assert!(read_ranked_recipe_v1(&expected[..length], &mut budget).is_err());
    }
}

#[test]
fn scalar_numerical_absent_source_and_optional_fields_have_closed_literal_syntax() {
    for (expected, bytes) in [
        (Scalar::Bool, vec![1, 0]),
        (
            Scalar::Integer {
                signed: false,
                bits: 64,
            },
            vec![2, 0, 0, 64, 0],
        ),
        (
            Scalar::Integer {
                signed: true,
                bits: 8,
            },
            vec![2, 0, 1, 8, 0],
        ),
        (Scalar::Float { bits: 32 }, vec![3, 0, 32, 0]),
    ] {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, FIXTURE_FLOOR);
        let mut reader = Reader::new(&bytes, true, &mut budget);
        assert_eq!(reader.scalar().unwrap(), expected);
        assert_eq!(reader.position, bytes.len());
        drop(reader);
        let mut writer = Writer::owned(bytes.len(), &mut budget).unwrap();
        writer.scalar(expected).unwrap();
        assert_eq!(writer.into_bytes().unwrap(), bytes);
    }
    let mut error_bounded = vec![4, 0];
    error_bounded.extend_from_slice(&0.25f64.to_bits().to_le_bytes());
    error_bounded.extend_from_slice(&0.5f64.to_bits().to_le_bytes());
    for (expected, bytes) in [
        (Numerical::ExactBitVectorOperatorCongruence, vec![1, 0]),
        (
            Numerical::ExactIeee754OperatorCongruence {
                rounding: ProductionIeeeRoundingModeV2::TowardNegative,
                exceptional_values: ProductionIeeeExceptionalValuePolicyV2::CanonicalNan,
            },
            vec![2, 0, 4, 0, 2, 0],
        ),
        (Numerical::Relaxed, vec![3, 0]),
        (
            Numerical::ErrorBounded {
                absolute_error_f64_bits: 0.25f64.to_bits(),
                relative_error_f64_bits: 0.5f64.to_bits(),
            },
            error_bounded,
        ),
    ] {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, FIXTURE_FLOOR);
        let mut reader = Reader::new(&bytes, true, &mut budget);
        assert_eq!(reader.numerical().unwrap(), expected);
        drop(reader);
        let mut writer = Writer::owned(bytes.len(), &mut budget).unwrap();
        writer.numerical(expected).unwrap();
        assert_eq!(writer.into_bytes().unwrap(), bytes);
    }
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, FIXTURE_FLOOR);
    for bytes in [[2u8], [255]] {
        let mut reader = Reader::new(&bytes, true, &mut budget);
        assert!(matches!(
            reader.boolean(),
            Err(E::Tag { field: "bool", .. })
        ));
        let mut reader = Reader::new(&bytes, true, &mut budget);
        assert!(matches!(
            reader.cooperative(),
            Err(E::Tag { field: "bool", .. })
        ));
    }
    let mut reader = Reader::new(&[0], true, &mut budget);
    assert_eq!(reader.cooperative().unwrap(), None);
    let mut absent = vec![2, 0];
    for byte in [1, 0, 3, 4, 5] {
        absent.extend_from_slice(&[byte; 32]);
    }
    let mut reader = Reader::new(&absent, true, &mut budget);
    let subjects = reader.subjects().unwrap().unwrap();
    drop(reader);
    assert_eq!(
        subjects.safe_reference_kind(),
        fe2o3_functional_proof::SafeReferenceKindV2::Mir
    );
    assert_eq!(subjects.safe_reference_source_hash(), DigestV1::ZERO);
    absent[34] = 1;
    let mut reader = Reader::new(&absent, false, &mut budget);
    assert!(reader.subjects().unwrap().is_none());
    drop(reader);
    let mut reader = Reader::new(&absent, true, &mut budget);
    assert!(matches!(
        reader.subjects(),
        Err(E::Subjects(
            fe2o3_functional_proof::FunctionalRefinementImportErrorV2::NonCanonicalAbsentSourceHash
        ))
    ));
}

#[test]
fn exact_four_mib_syntax_is_admitted_and_one_extra_byte_is_typed_refusal() {
    const LENGTH: usize = 4 * 1024 * 1024;
    const DEPENDENCIES: usize = (LENGTH - 88) / 6;
    let mut bytes = b"F2RKR1\0\0".to_vec();
    bytes.extend_from_slice(&[1, 0, 0, 0, 48, 0, 0, 0]);
    bytes.extend_from_slice(&(LENGTH as u64).to_le_bytes());
    for count in [2u32, 0, 0, 1, 4, 0] {
        bytes.extend_from_slice(&count.to_le_bytes());
    }
    bytes.extend_from_slice(b"full");
    bytes.extend_from_slice(&[0; 8]);
    bytes.extend_from_slice(&[5, 0, 0, 0]);
    bytes.extend_from_slice(&(DEPENDENCIES as u32).to_le_bytes());
    for _ in 0..DEPENDENCIES {
        bytes.extend_from_slice(&[1, 0, 0, 0, 0, 0]);
    }
    bytes.extend_from_slice(&[1, 0, 0, 0, 1, 0, 0, 0]);
    bytes.extend_from_slice(&[0; 8]);
    bytes.extend_from_slice(&[11, 0, 0, 0]);
    assert_eq!(bytes.len(), LENGTH);
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 16 * FIXTURE_FLOOR);
    budget
        .reserve_storage(size_of::<Vec<u8>>() + bytes.capacity())
        .unwrap();
    let (view, receipt) = read_ranked_recipe_v1(&bytes, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!((view.block_count(), view.operation_count()), (2, 0));
    drop(view);
    budget.release_storage(receipt.retained_storage()).unwrap();
    // New host fixture backing is a separate prepaid input, not a returned recipe owner.
    let mut extra = bytes.clone();
    extra.push(0);
    budget
        .reserve_storage(size_of::<Vec<u8>>() + extra.capacity())
        .unwrap();
    assert!(matches!(read_ranked_recipe_v1(&extra, &mut budget),
        Err(E::Limit { field: "recipe bytes", actual, limit }) if actual == LENGTH + 1 && limit == LENGTH));
}
