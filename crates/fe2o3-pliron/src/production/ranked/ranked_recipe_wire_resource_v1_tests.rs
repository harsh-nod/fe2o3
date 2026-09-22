use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use sha2::Sha256;

const SENTINEL: usize = 37;
// Literal grammar: eleven header tokens, then name, block arguments/count,
// return tag and reserved word. Counter visits only the five body tokens.
const TOKENS: usize = 11 + 5;
const LENGTH: usize = 61;
const HASH_WORK: usize = b"FE2O3/RANKED-RECIPE/V1\0".len() + 8 + LENGTH + 1;
const WRITE_WORK: usize = LENGTH + TOKENS;
const ENCODE_WORK: usize = 2 + 5 + WRITE_WORK + HASH_WORK;
const READ_WORK: usize = 2 + WRITE_WORK + HASH_WORK;
const MATERIALIZE_WORK: usize = 2 + 2 * WRITE_WORK + 2 + 5 + WRITE_WORK + HASH_WORK;
const HASH_STORAGE: usize = size_of::<Sha256>() + size_of::<[u8; 8]>();

fn exact_vector_capacity<T>(count: usize) -> usize {
    let mut values = Vec::<T>::new();
    values.try_reserve_exact(count).unwrap();
    values.capacity()
}
fn input_floor(kernel: &Kernel) -> usize {
    SENTINEL
        + size_of::<Kernel>()
        + kernel.function_name.capacity()
        + kernel.blocks.capacity() * size_of::<Block>()
}
fn decode_header() -> usize {
    size_of::<DecodedRankedRecipeV1>() + size_of::<RankedRecipeStorageV1>()
}
fn view_header() -> usize {
    size_of::<RankedRecipeRefV1<'_>>() + size_of::<RankedRecipeStorageV1>()
}
fn read_scratch() -> usize {
    size_of::<Reader<'_, '_, '_>>() + size_of::<Op>() + size_of::<decode::Parsed<'_>>()
}
fn assert_storage_failure(
    error: E,
    budget: &Budget<'_>,
    floor: usize,
    actual: usize,
    limit: usize,
    work: usize,
    peak: usize,
) {
    let E::Resource(Resource::Storage(error)) = error else {
        panic!("not exact Storage: {error:?}");
    };
    assert_eq!((error.actual(), error.limit()), (actual, limit));
    assert_eq!(
        (budget.storage(), budget.work(), budget.peak_storage()),
        (floor, work, peak)
    );
    assert_eq!(budget.failed_storage(), Some(actual));
}

#[test]
fn encoder_exact_success_global_storage_prefix_and_adjacent_work_are_independent() {
    let kernel = tests::empty();
    let floor = input_floor(&kernel);
    let capacity = exact_vector_capacity::<u8>(LENGTH);
    let retained =
        size_of::<InertRankedRecipeBytesV1>() + size_of::<RankedRecipeStorageV1>() + capacity;
    let before_hash = floor + retained + size_of::<Writer<'_, '_>>();
    let peak = before_hash + HASH_STORAGE;
    for work_limit in [ENCODE_WORK, ENCODE_WORK + 1] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, peak);
        budget.reserve_storage(floor).unwrap();
        let (bytes, receipt) = encode_ranked_recipe_v1(&kernel, &mut budget).unwrap();
        assert_eq!(bytes.canonical_bytes(), tests::literal_empty());
        assert_eq!(bytes.bytes.capacity(), capacity);
        assert_eq!(receipt.retained_storage(), retained);
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (ENCODE_WORK, floor, peak)
        );
        budget.reserve_storage(retained).unwrap();
        drop(bytes);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.failed_storage(), None);
    }
    let mut work = Work::new(ENCODE_WORK);
    let mut budget = Budget::new(&mut work, peak - 1);
    budget.reserve_storage(floor).unwrap();
    let error = encode_ranked_recipe_v1(&kernel, &mut budget).unwrap_err();
    assert_storage_failure(
        error,
        &budget,
        floor,
        peak,
        peak - 1,
        2 + 5 + WRITE_WORK,
        before_hash,
    );
    let mut work = Work::new(ENCODE_WORK - 1);
    let mut budget = Budget::new(&mut work, peak);
    budget.reserve_storage(floor).unwrap();
    let Err(E::Resource(Resource::Work(error))) = encode_ranked_recipe_v1(&kernel, &mut budget)
    else {
        panic!("hash must refuse work");
    };
    assert_eq!(
        (error.actual(), error.limit()),
        (ENCODE_WORK, ENCODE_WORK - 1)
    );
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (2 + 5 + WRITE_WORK, floor, peak)
    );
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn real_nested_tensor_counter_work_refusal_preserves_its_typed_wire_cause() {
    let kernel = Kernel {
        function_name: "x".into(),
        argument_count: 0,
        tree_work: 0,
        blocks: vec![Block::new(
            vec![Op::TensorLayout {
                contract: TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
                convergence: TensorConvergenceAttr::UniformSubgroup,
                active_lanes: 64,
                binding: None,
            }],
            Term::Return,
        )],
    };
    // Outer fixed entry2, name/block/op-count3, operation tag/reserved2,
    // tensor entry2: the next original KIR profile token must fail.
    let mut work = Work::new(9);
    let mut budget = Budget::new(&mut work, 4 * tests::FIXTURE_FLOOR);
    budget.reserve_storage(tests::FIXTURE_FLOOR).unwrap();
    let error = encode_ranked_recipe_v1(&kernel, &mut budget).unwrap_err();
    let E::Tensor(TensorLayoutPayloadErrorV12::Encode(
        fe2o3_kernel_ir::KernelIrEncodeError::WorkLimit(child),
    )) = &error
    else {
        panic!("genuine nested profile refusal: {error:?}");
    };
    assert_eq!((child.actual(), child.limit()), (10, 9));
    assert_eq!(budget.work(), 9);
    assert_eq!(budget.storage(), tests::FIXTURE_FLOOR);
    let tensor = error
        .source()
        .unwrap()
        .downcast_ref::<TensorLayoutPayloadErrorV12>()
        .unwrap();
    assert!(
        tensor
            .source()
            .unwrap()
            .downcast_ref::<fe2o3_kernel_ir::KernelIrEncodeError>()
            .is_some()
    );
}

#[test]
fn borrowed_reader_exact_header_hash_and_work_refusals_restore_paid_backing() {
    let bytes = tests::literal_empty();
    let floor = SENTINEL + size_of::<Vec<u8>>() + bytes.capacity();
    let header = view_header() + read_scratch();
    let peak = floor + header + HASH_STORAGE;
    let mut work = Work::new(READ_WORK);
    let mut budget = Budget::new(&mut work, peak);
    budget.reserve_storage(floor).unwrap();
    let (view, receipt) = read_ranked_recipe_v1(&bytes, &mut budget).unwrap();
    assert_eq!(receipt.retained_storage(), view_header());
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (READ_WORK, floor, peak)
    );
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    drop(view);
    budget.release_storage(receipt.retained_storage()).unwrap();
    for (limit, extent, prefix_work, prefix_peak) in [
        (floor + header - 1, floor + header, 2, floor),
        (peak - 1, peak, 2 + WRITE_WORK, floor + header),
    ] {
        let mut work = Work::new(READ_WORK);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        let error = read_ranked_recipe_v1(&bytes, &mut budget).unwrap_err();
        assert_storage_failure(
            error,
            &budget,
            floor,
            extent,
            limit,
            prefix_work,
            prefix_peak,
        );
    }
    let mut work = Work::new(READ_WORK - 1);
    let mut budget = Budget::new(&mut work, peak);
    budget.reserve_storage(floor).unwrap();
    let Err(E::Resource(Resource::Work(error))) = read_ranked_recipe_v1(&bytes, &mut budget) else {
        panic!("exact hash work");
    };
    assert_eq!((error.actual(), error.limit()), (READ_WORK, READ_WORK - 1));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (2 + WRITE_WORK, floor, peak)
    );
}

#[test]
fn materialization_exact_new_owned_extent_and_successful_prefix_roster() {
    let bytes = tests::literal_empty();
    let mut setup_work = Work::new(100_000);
    let mut setup = Budget::new(&mut setup_work, 100_000);
    setup
        .reserve_storage(SENTINEL + size_of::<Vec<u8>>() + bytes.capacity())
        .unwrap();
    let (view, view_storage) = read_ranked_recipe_v1(&bytes, &mut setup).unwrap();
    setup
        .reserve_storage(view_storage.retained_storage())
        .unwrap();
    let floor = SENTINEL + size_of::<Vec<u8>>() + bytes.capacity() + view_header();
    let blocks = exact_vector_capacity::<Block>(1) * size_of::<Block>();
    let name = String::from("k").capacity();
    let retained = decode_header() + blocks + name;
    let prefix = floor + decode_header() + read_scratch();
    // These are independently composed successful reservations, not measured failed-run totals.
    let reservations = [
        (prefix, 2),
        (prefix + blocks, 2 + WRITE_WORK + 49 + 12),
        (prefix + blocks + blocks + 1, 2 + 2 * WRITE_WORK + 2),
        (
            prefix + blocks + name + size_of::<Writer<'_, '_>>(),
            2 + 2 * WRITE_WORK + 2,
        ),
        (
            prefix + blocks + name + size_of::<Writer<'_, '_>>() + HASH_STORAGE,
            2 + 2 * WRITE_WORK + 2 + 5 + WRITE_WORK,
        ),
    ];
    let peak = reservations.iter().map(|row| row.0).max().unwrap();
    let mut work = Work::new(MATERIALIZE_WORK);
    let mut budget = Budget::new(&mut work, peak);
    budget.reserve_storage(floor).unwrap();
    let (decoded, receipt) = materialize_ranked_recipe_v1(&view, &mut budget).unwrap();
    assert_eq!(decoded.kernel(), &tests::empty());
    assert_eq!(receipt.retained_storage(), retained);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (MATERIALIZE_WORK, floor, peak)
    );
    budget.reserve_storage(retained).unwrap();
    drop(decoded);
    budget.release_storage(retained).unwrap();

    let first = reservations
        .iter()
        .position(|row| row.0 > peak - 1)
        .unwrap();
    let prefix_peak = reservations[..first]
        .iter()
        .map(|row| row.0)
        .max()
        .unwrap_or(floor);
    let mut work = Work::new(MATERIALIZE_WORK);
    let mut budget = Budget::new(&mut work, peak - 1);
    budget.reserve_storage(floor).unwrap();
    let error = materialize_ranked_recipe_v1(&view, &mut budget).unwrap_err();
    assert_storage_failure(
        error,
        &budget,
        floor,
        reservations[first].0,
        peak - 1,
        reservations[first].1,
        prefix_peak,
    );
    let mut work = Work::new(MATERIALIZE_WORK - 1);
    let mut budget = Budget::new(&mut work, peak);
    budget.reserve_storage(floor).unwrap();
    let Err(E::Resource(Resource::Work(error))) = materialize_ranked_recipe_v1(&view, &mut budget)
    else {
        panic!("materialized identity work");
    };
    assert_eq!(
        (error.actual(), error.limit()),
        (MATERIALIZE_WORK, MATERIALIZE_WORK - 1)
    );
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (MATERIALIZE_WORK - HASH_WORK, floor, peak)
    );
}

#[test]
fn actual_operation_tree_and_recursive_counts_are_checked_before_allocation() {
    let count = 65_533usize;
    let mut bytes = tests::literal_empty();
    bytes.truncate(57);
    // Lie in the header; actual syntax must hit the tree bound before census comparison.
    bytes[28..32].copy_from_slice(&0u32.to_le_bytes());
    bytes[53..57].copy_from_slice(&(count as u32).to_le_bytes());
    for _ in 0..count {
        bytes.extend_from_slice(&[25, 0, 0, 0, 1, 0, 1, 0, 1, 0]);
    }
    bytes.extend_from_slice(&[11, 0, 0, 0]);
    let length = bytes.len() as u64;
    bytes[16..24].copy_from_slice(&length.to_le_bytes());
    let floor = SENTINEL + size_of::<Vec<u8>>() + bytes.capacity();
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, floor + view_header() + read_scratch());
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        read_ranked_recipe_v1(&bytes, &mut budget),
        Err(E::Limit {
            field: "operation tree",
            actual: 131_073,
            limit: 131_072
        })
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(
        budget.peak_storage(),
        floor + view_header() + read_scratch()
    );
}

#[test]
fn typed_sources_preserve_actual_resource_objects_and_nonresource_diagnostics() {
    let mut work = Work::new(0);
    let child = work.charge_work(1).unwrap_err();
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 0);
    let storage = budget.reserve_storage(1).unwrap_err();
    for resource in [
        Resource::Work(child),
        storage,
        Resource::Accounting,
        Resource::Allocation,
        Resource::Arithmetic,
    ] {
        let error = E::Resource(resource);
        let actual = error.source().unwrap().downcast_ref::<Resource>().unwrap();
        let E::Resource(expected) = &error else {
            unreachable!()
        };
        assert!(std::ptr::eq(actual, expected));
    }
    let error = E::Tensor(TensorLayoutPayloadErrorV12::Resource(Resource::Accounting));
    assert!(
        error
            .source()
            .unwrap()
            .source()
            .unwrap()
            .downcast_ref::<Resource>()
            .is_some()
    );
    for error in [
        E::Length,
        E::Header,
        E::Reserved,
        E::Utf8,
        E::NonCanonical,
        E::Panicked,
        E::Tag {
            field: "operation",
            tag: 0,
        },
        E::Limit {
            field: "count",
            actual: 2,
            limit: 1,
        },
    ] {
        assert!(error.source().is_none());
    }
}

#[test]
fn internal_scope_drops_failed_outputs_before_restoring_the_entry_floor() {
    use std::cell::Cell;
    struct Probe<'a>(&'a Cell<bool>);
    impl Drop for Probe<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let dropped = Cell::new(false);
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 1000);
    budget.reserve_storage(SENTINEL).unwrap();
    let result = scope::<()>(&mut budget, |budget| {
        budget.reserve_storage(13)?;
        let _probe = Probe(&dropped);
        panic!("private panic path");
    });
    assert!(matches!(result, Err(E::Panicked)));
    assert!(dropped.get());
    assert_eq!(budget.storage(), SENTINEL);
    assert_eq!(budget.work(), 2);
}

#[derive(Default)]
struct PrefixOracle {
    work: usize,
    storage: usize,
    peak: usize,
    boxes: usize,
    selected: Option<(usize, usize, usize)>,
}
impl PrefixOracle {
    fn work(&mut self, amount: usize) {
        self.work += amount;
    }
    fn field(&mut self, bytes: usize) {
        self.work(bytes + 1);
    }
    fn reserve(&mut self, amount: usize, frame: bool) {
        let actual = self.storage + amount;
        if frame && self.boxes != 0 && actual > self.peak && self.selected.is_none() {
            self.selected = Some((actual, self.work, self.peak));
        }
        self.storage = actual;
        self.peak = self.peak.max(actual);
    }
    fn release(&mut self, amount: usize) {
        self.storage -= amount;
    }
    fn expression(&mut self, depth: usize, owned: bool) {
        let frame = size_of::<([Option<Expr>; 3], Expr, usize, usize)>();
        self.reserve(frame, owned);
        self.work(1);
        self.field(2); // expression tag
        if depth == 0 {
            self.field(2);
            self.field(1);
            self.field(2); // u8 scalar
            self.field(8); // constant bits
        } else {
            self.field(2); // binary operation
            self.field(2);
            self.field(1);
            self.field(2); // u8 scalar
            self.field(2); // wrapping
            for _ in 0..2 {
                self.expression(depth - 1, owned);
                if owned {
                    self.reserve(size_of::<Expr>(), false);
                    self.boxes += 1;
                }
            }
        }
        self.release(frame);
    }
    fn parse(&mut self, depth: usize, owned: bool) {
        for bytes in [8, 2, 2, 4, 8, 4, 4, 4, 4, 4, 4, 6] {
            self.field(bytes);
        }
        if owned {
            self.reserve(
                exact_vector_capacity::<Block>(1) * size_of::<Block>(),
                false,
            );
        }
        self.field(4);
        self.field(4);
        if owned {
            self.reserve(exact_vector_capacity::<Op>(1) * size_of::<Op>(), false);
        }
        self.field(2);
        self.field(2);
        self.field(4);
        self.expression(depth, owned);
        self.field(2); // numerical contract
        self.field(2);
        self.field(2); // return and reserved
    }
}

#[test]
fn independent_successful_nested_prefix_refuses_the_exact_owned_expression_reservation() {
    fn expression(depth: usize) -> Expr {
        let scalar = Scalar::Integer {
            signed: false,
            bits: 8,
        };
        if depth == 0 {
            Expr::Constant { scalar, bits: 1 }
        } else {
            Expr::Binary {
                operation: ProductionSemanticBinaryOpV2::BitAnd,
                scalar,
                overflow: ProductionOverflowContractV2::Wrapping,
                lhs: Box::new(expression(depth - 1)),
                rhs: Box::new(expression(depth - 1)),
            }
        }
    }
    let kernel = Kernel::new(
        "nested",
        0,
        vec![Block::new(
            vec![Op::SemanticExpression {
                result: Id::new(0),
                expression: expression(6),
                numerical_contract: Numerical::ExactBitVectorOperatorCongruence,
            }],
            Term::Return,
        )],
    )
    .unwrap();
    let mut setup_work = Work::new(100_000_000);
    let mut setup = Budget::new(&mut setup_work, 8 * tests::FIXTURE_FLOOR);
    setup.reserve_storage(tests::FIXTURE_FLOOR).unwrap();
    let (bytes, backing) = encode_ranked_recipe_v1(&kernel, &mut setup).unwrap();
    setup.reserve_storage(backing.retained_storage()).unwrap();
    let (view, query) = read_ranked_recipe_v1(bytes.canonical_bytes(), &mut setup).unwrap();
    setup.reserve_storage(query.retained_storage()).unwrap();
    let backing_extent = size_of::<InertRankedRecipeBytesV1>()
        + size_of::<RankedRecipeStorageV1>()
        + bytes.bytes.capacity();
    assert_eq!(backing.retained_storage(), backing_extent);
    assert_eq!(query.retained_storage(), view_header());
    let floor = tests::FIXTURE_FLOOR + backing_extent + view_header();
    let mut oracle = PrefixOracle {
        storage: floor,
        peak: floor,
        ..PrefixOracle::default()
    };
    oracle.work(2);
    oracle.reserve(decode_header() + read_scratch(), false);
    oracle.parse(6, false);
    oracle.parse(6, true);
    let (requested, prefix_work, prefix_peak) = oracle
        .selected
        .expect("owned frames eventually retain decoded child boxes");
    assert!(prefix_peak < requested);
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, requested - 1);
    budget.reserve_storage(floor).unwrap();
    let error = materialize_ranked_recipe_v1(&view, &mut budget).unwrap_err();
    assert_storage_failure(
        error,
        &budget,
        floor,
        requested,
        requested - 1,
        prefix_work,
        prefix_peak,
    );
    assert!(oracle.boxes > 0);
    // Reference visits literal grammar and typed extents only. It never invokes
    // production parse/encode/materialize to obtain expected work or peak.
}

#[test]
fn checked_extent_and_allocator_refusals_remain_typed_without_limit_changes() {
    assert!(matches!(
        primitives::add(usize::MAX, 1),
        Err(E::Resource(Resource::Arithmetic))
    ));
    assert!(matches!(
        primitives::product(usize::MAX, 2),
        Err(E::Resource(Resource::Arithmetic))
    ));
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let result = scope(&mut budget, |budget| {
        let (values, storage) = primitives::vector::<u8>(usize::MAX, budget)?;
        Ok((values, storage))
    });
    assert!(matches!(result, Err(E::Resource(Resource::Allocation))));
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.peak_storage(), usize::MAX);
    assert_eq!(budget.work(), 2);
    assert_eq!(budget.failed_storage(), None);
}
