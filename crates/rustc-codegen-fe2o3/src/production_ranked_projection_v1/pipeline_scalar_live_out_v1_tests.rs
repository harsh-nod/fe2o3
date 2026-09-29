// Reuse the original exact semantic type/ABI constructors, not an SSA mock.
use crate::production_ranked_projection_v1::canonical_assertion_facts_v1::*;
include!("canonical_assertion_fixtures_v1_tests.rs");

#[derive(Clone, Copy, Default)]
enum Change {
    #[default]
    None,
    SymbolicSeed,
    SymbolicBound,
    Bypass,
    SideExit,
    Redefine,
    LessEqual,
    Reversed,
    CheckedOverwrite,
    ParallelSeed,
}

fn loop_owner(
    bits: u16,
    seed: u64,
    bound: u64,
    step: u64,
    checked: bool,
    commute: bool,
    change: Change,
) -> ProductionSemanticSsaOwnerV1 {
    wide_loop_owner(bits, seed, bound, step, checked, commute, change, 0)
}

fn wide_loop_owner(
    bits: u16,
    seed: u64,
    bound: u64,
    step: u64,
    checked: bool,
    commute: bool,
    change: Change,
    width: usize,
) -> ProductionSemanticSsaOwnerV1 {
    let literal = |value: u64| typed_constant(A_U64, value.into(), (bits / 8) as u8);
    let induction = || typed_operand(1, A_U64);
    let initial = if matches!(change, Change::SymbolicSeed) {
        typed_operand(4, A_U64)
    } else {
        literal(seed)
    };
    let bound = if matches!(change, Change::SymbolicBound) {
        typed_operand(4, A_U64)
    } else {
        literal(bound)
    };
    let (left, right) = if matches!(change, Change::Reversed) {
        (bound, induction())
    } else {
        (induction(), bound)
    };
    let header = typed_assignment(
        2,
        A_BOOL,
        SemanticRvalueKindV1::Binary {
            operation: if matches!(change, Change::LessEqual) {
                SemanticBinaryOpV1::LessOrEqual
            } else {
                SemanticBinaryOpV1::LessThan
            },
            left,
            right,
        },
    );
    let (left, right) = if commute {
        (literal(step), induction())
    } else {
        (induction(), literal(step))
    };
    let latch = if checked {
        typed_assignment(
            1,
            A_U64,
            SemanticRvalueKindV1::Use(checked_field(5, 0, A_U64)),
        )
    } else {
        typed_assignment(
            1,
            A_U64,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: left.clone(),
                right: right.clone(),
            },
        )
    };
    let producer = if checked {
        vec![typed_assignment(
            5,
            A_CHECKED,
            SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                SemanticCheckedBinaryOpV1::Add,
                left.clone(),
                right.clone(),
            )),
        )]
    } else {
        vec![]
    };
    let producer_end = if checked {
        SemanticTerminatorKindV1::Assert {
            condition: checked_field(5, 1, A_BOOL),
            expected: false,
            message: SemanticAssertMessageV1::Overflow {
                operation: SemanticBinaryOpV1::Add,
                left,
                right,
            },
            target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, 4),
            unwind: SemanticUnwindActionV1::Unreachable,
        }
    } else {
        goto(4)
    };
    let mut exit = Vec::new();
    if matches!(change, Change::Redefine) {
        exit.push(typed_assignment(
            1,
            A_U64,
            SemanticRvalueKindV1::Use(literal(19)),
        ));
    }
    exit.push(typed_assignment(
        3,
        A_U64,
        SemanticRvalueKindV1::Use(induction()),
    ));
    let mut blocks = vec![
        block(
            190,
            vec![typed_assignment(
                1,
                A_U64,
                SemanticRvalueKindV1::Use(initial),
            )],
            if matches!(change, Change::ParallelSeed) {
                zero_switch(4, A_U64, 1, 1)
            } else if matches!(change, Change::Bypass) {
                zero_switch(4, A_U64, 5, 1)
            } else {
                goto(1)
            },
        ),
        block(191, vec![header], zero_switch(2, A_BOOL, 5, 2)),
        block(
            192,
            vec![],
            if matches!(change, Change::SideExit) {
                zero_switch(4, A_U64, 5, 3)
            } else {
                goto(3)
            },
        ),
        block(193, producer, producer_end),
        block(
            194,
            if matches!(change, Change::CheckedOverwrite) {
                vec![
                    typed_assignment(
                        5,
                        A_CHECKED,
                        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                            SemanticCheckedBinaryOpV1::Add,
                            induction(),
                            literal(step),
                        )),
                    ),
                    latch,
                ]
            } else {
                vec![latch]
            },
            goto(1),
        ),
        block(195, exit, goto(6)),
        block(
            196,
            vec![typed_assignment(
                3,
                A_U64,
                SemanticRvalueKindV1::Use(induction()),
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ];
    if width > 0 {
        blocks[2] = block(192, vec![], goto(7));
        for offset in 0..width {
            let mut identity = [0; 32];
            // Fixed blocks use repeated bytes 190..=196. Keep every appended
            // identity after those rows and numerically ordered past 255.
            identity[0] = 197;
            identity[1..5].copy_from_slice(&u32::try_from(offset).unwrap().to_be_bytes());
            blocks.push(
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(identity),
                    SemanticSourceProvenanceV1::unavailable(),
                    vec![],
                    SemanticTerminatorV1::new(
                        SemanticSourceProvenanceV1::unavailable(),
                        goto(if offset + 1 == width {
                            3
                        } else {
                            8 + offset as u32
                        }),
                    ),
                )
                .unwrap(),
            );
        }
    }
    let mut function = assertion_root_with_access(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (A_U64, SemanticLocalRoleV1::Temporary),
            (A_BOOL, SemanticLocalRoleV1::Temporary),
            (A_U64, SemanticLocalRoleV1::Temporary),
            (A_U64, SemanticLocalRoleV1::Argument(0)),
            (A_CHECKED, SemanticLocalRoleV1::Temporary),
        ],
        vec![A_U64],
        blocks,
        false,
    );
    if bits < 32 {
        // GpuKernel is a foreign-classified ABI: the narrow unsigned source
        // argument needs ZeroExtend, unlike the shared fixture's u64 argument.
        let original = function.abi();
        let abi = SemanticFunctionAbiV1::from_rustc(
            original.identity(),
            original.layout_identity(),
            original.canon_abi(),
            original.extern_abi(),
            original.can_unwind(),
            original.c_variadic(),
            original.fixed_count(),
            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                A_U64,
                SemanticAbiPassModeV1::Direct(
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                        SemanticAbiExtensionV1::ZeroExtend,
                        0,
                        None,
                    )
                    .unwrap(),
                ),
            ))],
            original.return_value().clone(),
        )
        .unwrap()
        .with_source_argument_ownership(original.source_argument_ownership().to_vec())
        .unwrap();
        function = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            abi,
            function.locals().to_vec(),
            function.entry(),
            function.blocks().to_vec(),
        )
        .unwrap()
        .with_kernel_entry(function.kernel_entry().unwrap().clone());
    }
    let mut types = assertion_types();
    if bits != 64 {
        let size = u64::from(bits / 8);
        types[A_U64.index() as usize] = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(233)),
            SemanticLayoutIdentityV1::from_sha256(bytes(233)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(size),
                size,
                neutral_scalar_backend_v1(
                    SemanticBackendPrimitiveV1::integer(false, bits, size),
                    (1_u128 << bits) - 1,
                ),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits,
            }),
        );
        let tuple_size = (size + 1).next_multiple_of(size);
        let padding = tuple_size - size - 1;
        types[A_CHECKED.index() as usize] = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(234)),
            SemanticLayoutIdentityV1::from_sha256(bytes(234)),
            SemanticTypeLayoutV1::aggregate(
                Some(tuple_size),
                size,
                SemanticAggregateLayoutV1::new(
                    vec![0, size],
                    if padding == 0 {
                        vec![]
                    } else {
                        vec![SemanticPaddingV1::new(size + 1, padding).unwrap()]
                    },
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![A_U64, A_BOOL]).unwrap()),
        );
    }
    let mut owner = assertion_ssa_functions(types, vec![function]);
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), 0);
    owner
}

fn inductions(owner: &ProductionSemanticSsaOwnerV1) -> R<Vec<ProjectedUniformInductionV1>> {
    let function = function(owner);
    let types = owner.source_semantic().types();
    let proof = SemanticAssertProofsV1::new(types, function)?;
    let mut arguments = vec![None; function.locals().len()];
    let mut origins = vec![None; function.locals().len()];
    for (index, local) in function.locals().iter().enumerate() {
        if let SemanticLocalRoleV1::Argument(argument) = local.role() {
            origins[index] = Some(argument);
        }
    }
    project_uniform_inductions_with_multi_entry_v1(
        owner.source_semantic().callables(),
        types,
        function,
        &vec![None; function.locals().len()],
        &origins,
        &proof.definition_counts,
        &mut arguments,
        &mut 0,
        &mut Vec::new(),
        &mut 0,
        None,
    )
}

fn projected(
    index: &Index<'_>,
    facts: &mut dyn ProjectedAssertionFactsV1,
    owner: &ProductionSemanticSsaOwnerV1,
    inductions: &[ProjectedUniformInductionV1],
    use_site: ScalarAssignmentSiteV1,
    initial_work: usize,
) -> R<Vec<ProductionRankedOperationV1>> {
    let function = function(owner);
    let types = owner.source_semantic().types();
    let mut operations = Vec::new();
    let value = PipelineScalarProjectorV1 {
        types,
        function,
        index_values: &vec![None; function.locals().len()],
        runtime_index_arguments: &mut vec![None; function.locals().len()],
        next_runtime_argument: &mut 0,
        uniform_inductions: inductions,
        entry_operations: &mut operations,
        next_value: &mut 0,
        assertion_proofs: SemanticAssertProofsV1::new(types, function)?,
        scalar_ssa: Some((index, facts)),
        work: initial_work,
    }
    .project_operand(&typed_operand(1, A_U64), use_site, &mut HashSet::new())?;
    assert!(matches!(
        value,
        ProjectedPipelineScalarV1::Value(ProductionRankedValueV1::Local(_))
    ));
    Ok(operations)
}

fn assert_constant(operations: &[ProductionRankedOperationV1], expected: u64) {
    assert!(
        matches!(operations, [ProductionRankedOperationV1::IndexConstant { value, .. }] if *value == expected)
    );
}

#[test]
fn live_out_uses_exact_phi_for_zero_iterations_overshoot_and_narrow_unsigned_types() {
    for (bits, seed, bound, step, expected) in [
        (64, 0, 4, 1, 4),
        (64, 9, 4, 3, 9),
        (64, 4, 4, 1, 4),
        (64, 2, 10, 3, 11),
        (8, 248, 250, 3, 251),
        (16, 0, 7, 4, 8),
    ] {
        let owner = loop_owner(bits, seed, bound, step, false, false, Change::None);
        let inductions = inductions(&owner).unwrap();
        assert_eq!(inductions.len(), 1);
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(FLOOR).unwrap();
        let completed = Cell::new(false);
        with_index(
            source(&owner),
            function(&owner),
            &mut Facts(&mut budget),
            |index, facts| {
                for use_site in [site(5, 0), site(6, 0)] {
                    assert_constant(
                        &projected(index, facts, &owner, &inductions, use_site, 0)?,
                        expected,
                    );
                }
                completed.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert!(completed.get());
        assert_eq!(budget.storage(), FLOOR);
        owner.verify_replay().unwrap();
    }
}

#[test]
fn live_out_accepts_original_checked_add_in_both_operand_orders() {
    for commute in [false, true] {
        let owner = loop_owner(64, 1, 8, 3, true, commute, Change::None);
        let inductions = inductions(&owner).unwrap();
        assert_eq!(inductions.len(), 1);
        assert!(matches!(
            inductions[0].source_progress.update,
            ProjectedSourceInductionUpdateV1::Checked { .. }
        ));
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let completed = Cell::new(false);
        with_index(
            source(&owner),
            function(&owner),
            &mut Facts(&mut budget),
            |index, facts| {
                assert_constant(
                    &projected(index, facts, &owner, &inductions, site(6, 0), 0)?,
                    10,
                );
                completed.set(true);
                Ok(())
            },
        )
        .unwrap();
        assert!(completed.get());
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn live_out_redefinition_resolves_to_its_actual_post_loop_assignment() {
    let owner = loop_owner(64, 0, 4, 1, false, false, Change::Redefine);
    // The original induction builder deliberately rejects a third definition.
    // Even a retained, earlier candidate cannot override the exact SSA use.
    let original = loop_owner(64, 0, 4, 1, false, false, Change::None);
    let candidates = inductions(&original).unwrap();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let completed = Cell::new(false);
    with_index(
        source(&owner),
        function(&owner),
        &mut Facts(&mut budget),
        |index, facts| {
            assert_constant(
                &projected(index, facts, &owner, &candidates, site(5, 1), 0)?,
                19,
            );
            assert_constant(
                &projected(index, facts, &owner, &candidates, site(6, 0), 0)?,
                19,
            );
            completed.set(true);
            Ok(())
        },
    )
    .unwrap();
    assert!(completed.get());
    assert_eq!(budget.storage(), 0);
}

#[test]
fn live_out_keeps_symbolic_operands_and_wrapping_recurrences_refused() {
    for (seed, bound, step, change, reason) in [
        (
            0,
            4,
            1,
            Change::SymbolicSeed,
            "a pipeline live-out requires a literal original initializer",
        ),
        (
            0,
            4,
            1,
            Change::SymbolicBound,
            "a pipeline live-out requires a literal unsigned bound",
        ),
        (
            250,
            255,
            3,
            Change::None,
            "a pipeline live-out may wrap its original unsigned type",
        ),
    ] {
        let owner = loop_owner(8, seed, bound, step, false, false, change);
        let candidates = inductions(&owner).unwrap();
        assert_eq!(candidates.len(), 1);
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let completed = Cell::new(false);
        let result = with_index(
            source(&owner),
            function(&owner),
            &mut Facts(&mut budget),
            |index, facts| {
                incomplete(
                    projected(index, facts, &owner, &candidates, site(6, 0), 0),
                    reason,
                );
                completed.set(true);
                Ok(())
            },
        );
        assert!(completed.get());
        incomplete(result, reason);
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn live_out_does_not_create_candidates_for_changed_comparisons_zero_step_or_side_exits() {
    for (step, change) in [
        (0, Change::None),
        (1, Change::LessEqual),
        (1, Change::Reversed),
        (1, Change::SideExit),
    ] {
        let owner = loop_owner(64, 0, 4, step, false, false, change);
        let result = inductions(&owner);
        assert!(
            matches!(result,Ok(ref rows) if rows.is_empty())
                || matches!(
                    result,
                    Err(ProductionRankedProjectionErrorV1::Incomplete(_))
                )
        );
    }
}

#[test]
fn live_out_bypass_and_changed_phi_coordinates_cannot_select_an_incoming_value() {
    let original = loop_owner(64, 0, 4, 1, false, false, Change::None);
    for mode in 0..4 {
        let owner = loop_owner(
            64,
            0,
            4,
            1,
            false,
            false,
            if mode == 0 {
                Change::Bypass
            } else {
                Change::None
            },
        );
        let mut candidates = inductions(&original).unwrap();
        match mode {
            1 => candidates[0].header = 2,
            2 => candidates[0].latch = 3,
            3 => candidates[0].source_progress.latch_statement = 1,
            _ => {}
        }
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let completed = Cell::new(false);
        let result = with_index(
            source(&owner),
            function(&owner),
            &mut Facts(&mut budget),
            |index, facts| {
                assert!(matches!(
                    projected(index, facts, &owner, &candidates, site(6, 0), 0),
                    Err(ProductionRankedProjectionErrorV1::Incomplete(_))
                ));
                completed.set(true);
                Ok(())
            },
        );
        assert!(completed.get());
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Incomplete(_))
        ));
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn live_out_uses_the_existing_shared_sixty_four_node_projector_cap() {
    let owner = loop_owner(64, 0, 4, 1, false, false, Change::None);
    let candidates = inductions(&owner).unwrap();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let completed = Cell::new(false);
    let result = with_index(
        source(&owner),
        function(&owner),
        &mut Facts(&mut budget),
        |index, facts| {
            assert_eq!(MAX_PIPELINE_SCALAR_NODES_V1, 64);
            incomplete(
                projected(index, facts, &owner, &candidates, site(6, 0), 63),
                "a pipeline live-out alias proof exceeded the existing scalar node cap",
            );
            completed.set(true);
            Ok(())
        },
    );
    assert!(completed.get());
    incomplete(
        result,
        "a pipeline live-out alias proof exceeded the existing scalar node cap",
    );
    assert_eq!(budget.storage(), 0);
}

fn measured_live_out(
    owner: &ProductionSemanticSsaOwnerV1,
    candidates: &[ProjectedUniformInductionV1],
    work_limit: usize,
    storage_limit: usize,
) -> (R<()>, usize, usize, bool) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let completed = Cell::new(false);
    let result = with_index(
        source(owner),
        function(owner),
        &mut Facts(&mut budget),
        |index, facts| {
            assert_constant(
                &projected(index, facts, owner, candidates, site(6, 0), 0)?,
                4,
            );
            completed.set(true);
            Ok(())
        },
    );
    assert_eq!(budget.storage(), FLOOR);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        completed.get(),
    )
}

#[test]
fn live_out_shared_ledger_exact_and_one_short_boundaries_retire_all_new_backing() {
    let owner = loop_owner(64, 0, 4, 1, false, false, Change::None);
    let candidates = inductions(&owner).unwrap();
    let (result, work, peak, completed) =
        measured_live_out(&owner, &candidates, 1_000_000, 1_000_000);
    result.unwrap();
    assert!(completed);
    let (result, actual, actual_peak, completed) =
        measured_live_out(&owner, &candidates, work, peak);
    result.unwrap();
    assert!(completed);
    assert_eq!((actual, actual_peak), (work, peak));
    for (work_limit, storage_limit, is_work) in [(work - 1, peak, true), (work, peak - 1, false)] {
        let (result, _, _, completed) =
            measured_live_out(&owner, &candidates, work_limit, storage_limit);
        assert!(!completed);
        match result {
            Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Work(error)),
            )) if is_work => assert_eq!(error.limit(), work_limit),
            Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Storage(error)),
            )) if !is_work => assert_eq!(error.limit(), storage_limit),
            _ => panic!("live-out boundary did not return the exact resource category"),
        }
    }
}

struct CountedFacts<'a, 'w> {
    inner: Facts<'a, 'w>,
    charged: &'a Cell<usize>,
    reserves: &'a Cell<usize>,
}
impl ProjectedAssertionFactsV1 for CountedFacts<'_, '_> {
    fn charge_private_array_work(&mut self, amount: usize) -> R<()> {
        self.inner.charge_private_array_work(amount)?;
        self.charged.set(self.charged.get() + amount);
        Ok(())
    }
    fn helper_value_ledger_v1(&self) -> R<(usize, Ledger)> {
        self.inner.helper_value_ledger_v1()
    }
    fn scalar_private_storage_v1(&self) -> R<usize> {
        self.inner.scalar_private_storage_v1()
    }
    fn reserve_scalar_private_storage_v1(&mut self, amount: usize) -> R<()> {
        self.inner.reserve_scalar_private_storage_v1(amount)?;
        self.reserves.set(self.reserves.get() + amount);
        Ok(())
    }
    fn release_scalar_private_storage_v1(&mut self, amount: usize) -> R<()> {
        self.inner.release_scalar_private_storage_v1(amount)
    }
    fn private_array_initializer_count(&mut self, _: usize, _: usize) -> R<Option<u64>> {
        Ok(None)
    }
    fn is_materialized_block(&mut self, _: usize) -> R<bool> {
        Ok(true)
    }
    fn condition(
        &mut self,
        _: usize,
        _: bool,
        _: SemanticBlockIdV1,
    ) -> R<ProjectedAssertionConditionV1> {
        Ok(ProjectedAssertionConditionV1::Dynamic)
    }
}

#[test]
fn live_out_repeated_queries_reuse_exit_proof_without_allocating_or_traversing_the_cfg() {
    for width in [0, 16, 256] {
        let owner = wide_loop_owner(64, 0, 4, 1, false, false, Change::None, width);
        let candidates = inductions(&owner).unwrap();
        let function = function(&owner);
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let charged = Cell::new(0);
        let reserves = Cell::new(0);
        let mut facts = CountedFacts {
            inner: Facts(&mut budget),
            charged: &charged,
            reserves: &reserves,
        };
        let completed = Cell::new(false);
        // The concrete facts counters are observed through the common callback,
        // so the original budget remains uniquely borrowed throughout.
        with_index(source(&owner), function, &mut facts, |index, facts| {
            let start = charged.get();
            assert_constant(
                &projected(index, facts, &owner, &candidates, site(6, 0), 0)?,
                4,
            );
            let first = charged.get() - start;
            assert!(first >= 6 * width);
            let storage = facts.scalar_private_storage_v1()?;
            let reserved = reserves.get();
            for _ in 0..128 {
                let start = charged.get();
                assert_constant(
                    &projected(index, facts, &owner, &candidates, site(6, 0), 0)?,
                    4,
                );
                let query = charged.get() - start;
                // Three exact-use event binary searches, two incoming rows,
                // constant validation, and the existing loop membership search.
                assert!(
                    query <= 256 + (usize::BITS - (width + 7).leading_zeros()) as usize,
                    "query {query}, width {width}"
                );
                assert_eq!(facts.scalar_private_storage_v1()?, storage);
                assert_eq!(reserves.get(), reserved);
            }
            completed.set(true);
            Ok(())
        })
        .unwrap();
        assert!(completed.get());
        // Every repeated query is bounded by the exact event binary searches,
        // two incoming rows, two affine leaves, and fixed validation, not a DFS.
        assert_eq!(facts.scalar_private_storage_v1().unwrap(), 0);
    }
}

#[test]
fn live_out_rejects_same_typed_checked_tuple_overwrite_and_parallel_seed_edges() {
    for overwrite in [false, true] {
        let original = loop_owner(64, 0, 4, 1, overwrite, false, Change::None);
        let owner = loop_owner(
            64,
            0,
            4,
            1,
            overwrite,
            false,
            if overwrite {
                Change::CheckedOverwrite
            } else {
                Change::ParallelSeed
            },
        );
        let mut candidates = inductions(&original).unwrap();
        if overwrite {
            candidates[0].source_progress.latch_statement = 1;
        }
        let reason = if overwrite {
            "a pipeline live-out checked tuple no longer has its exact unique producer"
        } else {
            "a pipeline live-out has multiple or changed initializers"
        };
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let completed = Cell::new(false);
        let result = with_index(
            source(&owner),
            function(&owner),
            &mut Facts(&mut budget),
            |index, facts| {
                incomplete(
                    projected(index, facts, &owner, &candidates, site(6, 0), 0),
                    reason,
                );
                completed.set(true);
                Ok(())
            },
        );
        assert!(completed.get());
        incomplete(result, reason);
        assert_eq!(budget.storage(), 0);
    }
}
