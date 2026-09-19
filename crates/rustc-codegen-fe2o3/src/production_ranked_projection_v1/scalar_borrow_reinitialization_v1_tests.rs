use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAtomicOrderingV1, SemanticAtomicScopeV1, SemanticMemoryStoreV1,
    SemanticTargetDataLayoutV1,
};

// Eligibility-only fragments exercise the census, not an admitted Rust program.
// The separate source-owner test below materializes and projects a genuine owner.
struct Fragment {
    types: Vec<SemanticTypeDeclV1>,
    function: SemanticFunctionDeclV1,
    target: SemanticTargetDataLayoutV1,
    root: u32,
    alias: u32,
    scalar: SemanticTypeIdV1,
    bytes: u8,
}

fn fragment(signed: bool, bits: u16) -> Fragment {
    let (source, _, _) = borrow_source_fixture();
    let semantic = source.semantic_ssa().source_semantic();
    let seed = &semantic.functions()[0];
    let mut types = semantic.types().to_vec();
    let scalar = SemanticTypeIdV1::from_index(types.len() as u32);
    let size = u64::from(bits / 8);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(246)),
        SemanticLayoutIdentityV1::from_sha256(bytes(246)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            size,
            neutral_scalar_backend_v1(
                SemanticBackendPrimitiveV1::integer(signed, bits, size),
                (1_u128 << bits) - 1,
            ),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
    ));
    let reference = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(neutral_pointer_type_v1(
        247,
        scalar,
        SemanticPointerKindV1::Reference,
        SemanticMutabilityV1::Mutable,
        1,
    ));
    let mut locals = seed.locals().to_vec();
    let root = locals.len() as u32;
    let alias = root + 1;
    let output = root + 2;
    for (tag, ty) in [(246, scalar), (247, reference), (248, scalar)] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(tag)),
            ty,
            SemanticLocalRoleV1::Temporary,
            SemanticSourceProvenanceV1::unavailable(),
        ));
    }
    let read = typed_assignment(
        output,
        scalar,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(alias),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, scalar)
                        .unwrap(),
                ],
                scalar,
            )
            .unwrap(),
        )),
    );
    let statements = vec![
        typed_assignment(
            root,
            scalar,
            SemanticRvalueKindV1::Use(typed_constant(scalar, 7, bits as u8 / 8)),
        ),
        typed_assignment(
            root,
            scalar,
            SemanticRvalueKindV1::Use(typed_constant(scalar, 9, bits as u8 / 8)),
        ),
        typed_assignment(
            alias,
            reference,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: typed_place(root, scalar),
            },
        ),
        read.clone(),
        read,
    ];
    let mut blocks = seed.blocks().to_vec();
    for (index, body) in blocks.iter_mut().enumerate() {
        *body = SemanticBasicBlockV1::new(
            body.identity(),
            body.source(),
            if index == 1 {
                statements.clone()
            } else {
                vec![]
            },
            body.terminator().clone(),
        )
        .unwrap();
    }
    Fragment {
        types,
        function: borrow_rebuild_function(seed, locals, blocks),
        target: semantic.target(),
        root,
        alias,
        scalar,
        bytes: bits as u8 / 8,
    }
}

fn run(
    fragment: &Fragment,
    work_limit: usize,
    storage_limit: usize,
) -> (R<[bool; 2]>, usize, usize) {
    let mut work = W::new(work_limit);
    let mut budget = B::new(&mut work, storage_limit);
    budget.reserve_storage(19).unwrap();
    let provenance = local_provenance_v1(&fragment.types, &fragment.function).unwrap();
    let result = scope(
        &fragment.types,
        &fragment.function,
        fragment.target,
        &mut BorrowMeter {
            budget: &mut budget,
        },
        |census, facts| {
            let Some(census) = census else {
                return Ok([false; 2]);
            };
            let mut result = [false; 2];
            let mut reads = 0;
            for (statement, row) in fragment.function.blocks()[1]
                .statements()
                .iter()
                .enumerate()
            {
                let SemanticStatementKindV1::Assign(assignment) = row.kind() else {
                    continue;
                };
                let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) =
                    assignment.value().kind()
                else {
                    continue;
                };
                if place.local().index() != fragment.alias || place.projections().is_empty() {
                    continue;
                }
                assert!(reads < result.len());
                result[reads] = census
                    .resolve(
                        &fragment.function,
                        &fragment.types,
                        fragment.target,
                        ProjectedSemanticAccessSiteV1 {
                            block: 1,
                            statement: Some(statement),
                        },
                        place,
                        AccessKindAttr::Read,
                        None,
                        provenance.allocation_provenance[fragment.alias as usize],
                        facts,
                    )?
                    .is_some();
                reads += 1;
            }
            assert_eq!(reads, 2);
            Ok(result)
        },
    );
    assert_eq!(budget.storage(), 19);
    (result, budget.work(), budget.peak_storage())
}

type R<T> = Result<T, ProductionRankedProjectionErrorV1>;

fn replace(fragment: &mut Fragment, statements: Vec<SemanticStatementV1>) {
    fragment.function = borrow_replace_statements(&fragment.function, 1, statements);
}

fn write(fragment: &Fragment, value: u128) -> SemanticStatementV1 {
    typed_assignment(
        fragment.root,
        fragment.scalar,
        SemanticRvalueKindV1::Use(typed_constant(fragment.scalar, value, fragment.bytes)),
    )
}

fn store(
    fragment: &Fragment,
    volatility: SemanticVolatilityV1,
    atomic: Option<SemanticAtomicAccessV1>,
) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
        typed_place(fragment.root, fragment.scalar),
        typed_constant(fragment.scalar, 11, fragment.bytes),
        volatility,
        atomic,
    )))
}

#[test]
fn scalar_borrow_repeated_preborrow_writes_accept_all_fixed_integer_widths_and_distinct_values() {
    for signed in [false, true] {
        for bits in [8, 16, 32, 64] {
            let mut fixture = fragment(signed, bits);
            let mut statements = fixture.function.blocks()[1].statements().to_vec();
            statements.insert(2, store(&fixture, SemanticVolatilityV1::NonVolatile, None));
            statements.insert(
                0,
                statement(SemanticStatementKindV1::StorageLive(
                    SemanticLocalIdV1::from_index(fixture.root),
                )),
            );
            replace(&mut fixture, statements);
            assert_eq!(run(&fixture, usize::MAX, usize::MAX).0.unwrap(), [true; 2]);
        }
    }
}

#[test]
fn scalar_borrow_reinitialization_after_kill_or_restart_never_reopens_the_epoch() {
    for sequence in [
        vec![false],
        vec![true],
        vec![false, true],
        vec![false, true, false],
    ] {
        let mut fixture = fragment(false, 32);
        let mut statements = fixture.function.blocks()[1].statements().to_vec();
        let local = SemanticLocalIdV1::from_index(fixture.root);
        for live in sequence.into_iter().rev() {
            statements.insert(
                1,
                statement(if live {
                    SemanticStatementKindV1::StorageLive(local)
                } else {
                    SemanticStatementKindV1::StorageDead(local)
                }),
            );
        }
        replace(&mut fixture, statements);
        assert_eq!(run(&fixture, usize::MAX, usize::MAX).0.unwrap(), [false; 2]);
    }
    let mut fixture = fragment(false, 32);
    let mut statements = fixture.function.blocks()[1].statements().to_vec();
    statements.insert(
        0,
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(fixture.root),
        )),
    );
    replace(&mut fixture, statements);
    assert_eq!(run(&fixture, usize::MAX, usize::MAX).0.unwrap(), [false; 2]);
}

#[test]
fn scalar_borrow_repeat_does_not_waive_rhs_move_alias_escape_or_postborrow_writes() {
    for case in 0..8 {
        let mut fixture = fragment(false, 32);
        let mut statements = fixture.function.blocks()[1].statements().to_vec();
        match case {
            0 | 1 => {
                statements[1] = typed_assignment(
                    fixture.root,
                    fixture.scalar,
                    SemanticRvalueKindV1::Use(if case == 0 {
                        SemanticOperandV1::Copy(typed_place(fixture.root, fixture.scalar))
                    } else {
                        SemanticOperandV1::Move(typed_place(fixture.root, fixture.scalar))
                    }),
                )
            }
            2 => statements.insert(3, write(&fixture, 13)),
            3 => statements.insert(3, store(&fixture, SemanticVolatilityV1::NonVolatile, None)),
            4 => statements[1] = store(&fixture, SemanticVolatilityV1::Volatile, None),
            5 => {
                statements[1] = store(
                    &fixture,
                    SemanticVolatilityV1::NonVolatile,
                    Some(SemanticAtomicAccessV1::new(
                        SemanticAtomicOrderingV1::Relaxed,
                        SemanticAtomicScopeV1::SingleThread,
                    )),
                )
            }
            6 => {
                let ty = fixture.function.locals()[fixture.alias as usize].ty();
                statements.insert(
                    3,
                    typed_assignment(
                        fixture.alias,
                        ty,
                        SemanticRvalueKindV1::Use(typed_operand(fixture.alias, ty)),
                    ),
                );
            }
            7 => {
                statements[1] =
                    statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(fixture.root),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Dereference,
                                    fixture.scalar,
                                )
                                .unwrap(),
                            ],
                            fixture.scalar,
                        )
                        .unwrap(),
                        typed_constant(fixture.scalar, 11, fixture.bytes),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )))
            }
            _ => unreachable!(),
        }
        replace(&mut fixture, statements);
        assert_eq!(
            run(&fixture, usize::MAX, usize::MAX).0.unwrap(),
            [false; 2],
            "case {case}"
        );
    }
}

#[test]
fn scalar_borrow_repeated_initialization_in_another_block_is_not_a_waiver() {
    let mut fixture = fragment(false, 32);
    fixture.function = borrow_replace_statements(&fixture.function, 0, vec![write(&fixture, 3)]);
    assert_eq!(run(&fixture, usize::MAX, usize::MAX).0.unwrap(), [false; 2]);
}

#[test]
fn scalar_borrow_repeated_initialization_exact_resources_and_cleanup() {
    let fixture = fragment(false, 32);
    let (result, work, peak) = run(&fixture, usize::MAX, usize::MAX);
    assert_eq!(result.unwrap(), [true; 2]);
    let (exact, exact_work, _) = run(&fixture, work, peak);
    assert_eq!(exact.unwrap(), [true; 2]);
    assert_eq!(exact_work, work);
    assert!(matches!(
        run(&fixture, work - 1, peak).0,
        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
            canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(_)
        ))
    ));
    assert!(matches!(
        run(&fixture, work, peak - 1).0,
        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
            canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(_)
        ))
    ));
}

#[test]
fn scalar_borrow_repeated_source_initialization_materializes_and_passes_normal_projector() {
    for initializations in [&[7, 7][..], &[7, 9, 7][..]] {
        let (source, inputs, _) = borrow_source_fixture_with_initializations(initializations);
        let repeated = source
            .executable()
            .module()
            .functions
            .iter()
            .flat_map(|function| function.body.iter())
            .flat_map(|body| &body.blocks)
            .any(|block| {
                let mut stores = std::collections::BTreeMap::new();
                for operation in &block.operations {
                    if let fe2o3_kernel_ir::OperationKind::Store {
                        pointer, access, ..
                    } = operation.kind
                        && access.address_space == fe2o3_kernel_ir::AddressSpace::Private
                    {
                        *stores.entry(pointer).or_insert(0_usize) += 1;
                    }
                }
                stores.values().any(|count| *count >= initializations.len())
            });
        assert!(
            repeated,
            "actual source-emitted same-pointer stores must survive"
        );
        let program = project_and_verify_ranked_materialized_semantic_mir_v1(
            source,
            &inputs,
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
        )
        .unwrap();
        assert!(program.all_kernel_checks_are_clean());
        assert!(!program.roots[0].access_sources.is_empty());
    }
}
