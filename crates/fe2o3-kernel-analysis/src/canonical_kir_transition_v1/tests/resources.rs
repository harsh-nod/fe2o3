use super::*;

fn scratch(a: &Inventory<'_>, b: &Inventory<'_>) -> usize {
    // Independent roster equation: two function maps; five input block maps
    // (owner/position/incoming-head/pending/selected-edge); output heads/tails; bidirectional
    // operation maps; output anchors; input parents; three input edge maps.
    let words = a.functions().len()
        + b.functions().len()
        + 5 * a.blocks().len()
        + 2 * b.blocks().len()
        + a.operations().len()
        + b.operations().len()
        + a.definitions().len()
        + b.definitions().len()
        + 3 * a.edges().len();
    size_of::<State<'_, '_, '_, '_>>()
        + words * size_of::<usize>()
        + a.definitions().len() * size_of::<Option<Literal>>()
        + a.blocks().len()
        + b.definitions().len()
}

#[test]
fn literal_empty_and_single_block_work_boundaries_preserve_prefix_history() {
    // Empty "x": entry/state 2, metadata 1+2+1, one fixed-point visit 1 = 7.
    // Unit helper "f": additionally 10 initialized cells, signature 7,
    // block install 8, output parameter-roster visit 1, reachability 3,
    // phi block visit 1, connector framing 2, block coverage 1,
    // terminator 1, ordered roster 1, selected-edge refresh 1 = 43 total.
    for (original, required) in [
        (Module::new("x"), 7),
        (
            module(
                vec![],
                vec![],
                vec![],
                vec![returning(4_000_000_000, vec![], &[])],
            ),
            43,
        ),
    ] {
        inspect(
            original.clone(),
            original,
            |a, _| Rows::identity(a),
            |a, b, rows, floor| {
                let prefix = 17;
                let peak = floor
                    + size_of::<CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>>()
                    + scratch(a, b);
                for allowance in [required, required - 1] {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(prefix + allowance);
                    let mut budget = Budget::new(&mut work, peak);
                    budget.charge_work(prefix).unwrap();
                    budget.reserve_storage(floor).unwrap();
                    let checked =
                        check_canonical_kir_transition_v1(a, b, rows.candidate(), &mut budget);
                    if allowance == required {
                        assert!(checked.is_ok());
                        assert_eq!(budget.work(), prefix + required);
                    } else {
                        assert!(matches!(checked, Err(Error::Resource(Resource::Work(_)))));
                        assert_eq!(budget.work(), prefix + required - 1);
                    }
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(budget.peak_storage(), peak);
                }
            },
        );
    }
}

#[test]
fn nonempty_exact_and_one_under_peak_own_inverse_queue_and_value_tables() {
    let mut entry = BasicBlock::new(BlockId(900));
    entry.operations.push(constant(7, Constant::U32(91)));
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(77),
        then_arguments: vec![ValueId(7)],
        else_target: BlockId(77),
        else_arguments: vec![ValueId(7)],
    });
    let mut join = returning(77, vec![], &[11]);
    join.parameters.push(ValueDef::new(ValueId(11), U32));
    let original = module(vec![Type::BOOL], vec![U32], vec![3], vec![entry, join]);
    inspect(
        original.clone(),
        original,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            assert_eq!(
                (
                    a.blocks().len(),
                    a.edges().len(),
                    a.definitions().len(),
                    a.uses().len()
                ),
                (2, 2, 3, 4)
            );
            let header = size_of::<CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>>();
            let peak = floor + header + scratch(a, b);
            for limit in [peak, peak - 1] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                let mut budget = Budget::new(&mut work, limit);
                budget.charge_work(19).unwrap();
                budget.reserve_storage(floor).unwrap();
                let result = check_canonical_kir_transition_v1(a, b, rows.candidate(), &mut budget);
                if limit == peak {
                    assert!(result.is_ok());
                    assert_eq!(budget.peak_storage(), peak);
                    assert_eq!(budget.failed_storage(), None);
                } else {
                    assert!(
                        matches!(result, Err(Error::Resource(Resource::Storage(error))) if error.actual() == peak && error.limit() == limit)
                    );
                    assert_eq!(budget.failed_storage(), Some(peak));
                    assert!(budget.peak_storage() < peak);
                }
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() >= 21);
            }
        },
    );
}

#[test]
fn failure_does_not_release_borrowed_owners_and_success_transfers_only_view() {
    let original = module(vec![], vec![], vec![], vec![returning(5, vec![], &[])]);
    inspect(
        original.clone(),
        original,
        |a, _| Rows::identity(a),
        |a, b, rows, floor| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(17 + 80);
            let mut budget = Budget::new(&mut work, floor + LIMIT);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(floor).unwrap();
            {
                let (_, receipt) =
                    check_canonical_kir_transition_v1(a, b, rows.candidate(), &mut budget).unwrap();
                assert_eq!(
                    receipt.retained_storage(),
                    size_of::<CheckedCanonicalKirTransitionV1<'_, '_, '_, '_>>()
                );
            }
            // Same single-block census as the independent 43-work oracle above.
            let accepted = 17 + 43;
            assert_eq!(budget.work(), accepted);
            let old_peak = budget.peak_storage();
            rows.segments[0].connector = Some(edge(0, 0));
            assert!(matches!(
                check_canonical_kir_transition_v1(a, b, rows.candidate(), &mut budget),
                Err(Error::Rule("block chain connector framing"))
            ));
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > accepted);
            assert_eq!(budget.peak_storage(), old_peak);
        },
    );
}

#[test]
fn private_call_whole_entry_pinned_transition_layout_equivalence_premises() {
    // Pinned 64-bit nightly only; not a portable repr(Rust) layout guarantee.
    fn same<A, B>() {
        assert_eq!(size_of::<A>(), size_of::<B>());
        assert_eq!(std::mem::align_of::<A>(), std::mem::align_of::<B>());
    }
    assert_eq!((size_of::<usize>(), std::mem::align_of::<usize>()), (8, 8));
    type LiteralShape = (fe2o3_kernel_ir::ScalarType, u128);
    type StateShape = (
        &'static Inventory<'static>,
        &'static Inventory<'static>,
        fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'static>,
        [Vec<usize>; 16],
        [Vec<u8>; 2],
        Vec<Option<LiteralShape>>,
    );
    same::<Literal, LiteralShape>();
    same::<Option<Literal>, Option<LiteralShape>>();
    same::<State<'_, '_, '_, '_>, StateShape>();
}
