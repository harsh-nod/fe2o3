//! Inert projection/codec fixtures only. No checked request, generated-fields
//! owner, execution, imported proof, or CPU-source authority is fabricated here.
use super::*;
use fe2o3_kernel_ir::{BlockId, CanonicalKernelIrWorkBudgetV1 as Work};
use fe2o3_pliron::ProductionRankedValueIdV1;
use fe2o3_proof_contracts::DigestV1;

#[path = "../../fe2o3-kernel-descriptor/tests/support/conditional_invocation.rs"]
mod fixture;
use fixture::Fixture;

fn ample<R>(run: impl FnOnce(&mut Budget<'_>) -> R) -> R {
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    run(&mut budget)
}

fn mixed() -> Fixture {
    let mut f = Fixture::new(2, 3);
    for (i, a) in f.arguments.iter_mut().enumerate() {
        a.source_argument = [19, 4, 1][i];
        a.adjusted_argument = [30, 8, 5][i];
        a.generated_field = [2, 0, 1][i];
        a.semantic_local = [103, 77, 211][i];
        a.semantic_type = [8, 9, 8][i];
    }
    f.roots = vec![[4, 3, 2, 1], [11, 22, 33, 44], [4, 3, 2, 1]];
    for (i, r) in f.reads.iter_mut().enumerate() {
        r.argument = [1, 0, 1][i];
        r.slice = 90 + u32::from(r.argument);
        r.ranked_view = RankedValue::Local(50 + u32::from(r.argument));
        r.ranked_index = RankedValue::BlockArgument {
            block: 8,
            argument: 4,
        };
    }
    f.refresh_premises();
    f
}

#[test]
fn complete_roundtrip_keeps_mixed_ordinals_roots_and_repeated_occurrences() {
    let f = mixed();
    ample(|budget| {
        let entry = budget.storage();
        with_encoded(&f.input(), budget, |view, budget| {
            assert_eq!(*view.subjects(), f.subjects);
            assert_eq!(*view.theorem(), f.theorem);
            assert_eq!(view.output(), f.output);
            assert_eq!(
                view.numerical_domain(),
                ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1
            );
            assert_eq!(view.canonical_bytes(), f.wire());
            let mut charge = |n| budget.charge_work(n);
            let mut roots = view.typed_roots();
            for expected in &f.roots {
                assert_eq!(roots.next(&mut charge).unwrap(), Some(*expected));
            }
            assert_eq!(roots.next(&mut charge).unwrap(), None);
            let mut arguments = view.arguments();
            for expected in &f.arguments {
                assert_eq!(arguments.next(&mut charge).unwrap(), Some(*expected));
            }
            assert_eq!(arguments.next(&mut charge).unwrap(), None);
            let mut reads = view.reads();
            for expected in &f.reads {
                assert_eq!(reads.next(&mut charge).unwrap(), Some(*expected));
            }
            assert_eq!(reads.next(&mut charge).unwrap(), None);
            let mut premises = view.premises();
            for expected in &f.premises {
                assert_eq!(premises.next(&mut charge).unwrap(), Some(*expected));
            }
            assert_eq!(premises.next(&mut charge).unwrap(), None);
        })
        .unwrap();
        assert_eq!(budget.storage(), entry);
    });
}

// A changed fixture may be structurally invalid or encode a different inert
// identity. Neither successful encoding nor a digest is producer authority.
fn changes_contract(change: impl FnOnce(&mut Fixture)) {
    let mut f = mixed();
    let original = ample(|b| with_encoded(&f.input(), b, |v, _| v.identity()).unwrap());
    change(&mut f);
    ample(|b| match with_encoded(&f.input(), b, |v, _| v.identity()) {
        Ok(changed) => assert_ne!(changed, original),
        Err(Error::Codec(_)) => {}
        Err(error) => panic!("unexpected refusal: {error}"),
    });
}

#[test]
fn every_argument_coordinate_is_retained() {
    for case in 0..9 {
        changes_contract(|f| {
            let a = &mut f.arguments[0];
            match case {
                0 => a.canonical_parameter += 1,
                1 => a.source_argument += 1,
                2 => a.adjusted_argument += 1,
                3 => a.semantic_local += 1,
                4 => a.semantic_type += 1,
                5 => a.generated_field += 1,
                6 => a.role = Role::Output,
                7 => a.source_type_identity[0] ^= 1,
                8 => a.device_layout_identity[0] ^= 1,
                _ => unreachable!(),
            }
        });
    }
}

#[test]
fn every_read_and_output_coordinate_is_retained() {
    for case in 0..19 {
        changes_contract(|f| {
            let r = &mut f.reads[0];
            match case {
                0 => r.argument = 0,
                1 => r.canonical.block += 1,
                2 => r.canonical.operation += 100,
                3 => r.slice += 1,
                4 => r.pointer += 1,
                5 => r.index += 1,
                6 => r.value += 1,
                7 => r.ranked.block += 1,
                8 => r.ranked.operation += 100,
                9 => r.ranked_view = RankedValue::Local(123),
                10 => r.ranked_view = RankedValue::Argument(51),
                11 => {
                    r.ranked_index = RankedValue::BlockArgument {
                        block: 9,
                        argument: 4,
                    }
                }
                12 => {
                    r.ranked_index = RankedValue::BlockArgument {
                        block: 8,
                        argument: 5,
                    }
                }
                13 => r.ranked_index = RankedValue::Local(4),
                14 => r.access_domain = Domain::GlobalLaunch,
                15 => r.address_domain = Domain::GuardedOutput,
                16 => r.element_bytes = 8,
                17 => r.alignment = 2,
                18 => f.reads.swap(0, 2),
                _ => unreachable!(),
            }
        });
    }
    for case in 0..10 {
        changes_contract(|f| {
            let o = &mut f.output;
            match case {
                0 => o.argument = 0,
                1 => o.canonical_store.block += 1,
                2 => o.canonical_store.operation += 1,
                3 => o.ranked_store.block += 1,
                4 => o.ranked_store.operation += 2,
                5 => o.ranked_effect.block += 1,
                6 => o.ranked_effect.operation += 2,
                7 => o.element_bytes = 8,
                8 => o.alignment = 2,
                9 => o.address_domain = Domain::GlobalLaunch,
                _ => unreachable!(),
            }
        });
    }
}

#[test]
fn all_subjects_theorem_fields_and_ordered_roots_are_retained() {
    for case in 0..9 {
        changes_contract(|f| {
            let s = &mut f.subjects;
            let digest = match case {
                0 => &mut s.kernel_id,
                1 => &mut s.exact_graph_identity,
                2 => &mut s.aggregate_statement_identity,
                3 => &mut s.source_semantic_identity,
                4 => &mut s.safe_reference_identity,
                5 => &mut s.safe_reference_source_hash,
                6 => &mut s.safe_reference_mir_hash,
                7 => &mut s.kernel_subject_identity,
                8 => &mut s.kernel_mir_hash,
                _ => unreachable!(),
            };
            digest[0] ^= 1;
        });
    }
    for case in 0..8 {
        changes_contract(|f| {
            let t = &mut f.theorem;
            let digest = match case {
                0 => &mut t.statement_identity,
                1 => &mut t.generated_source_identity,
                2 => &mut t.execution_identity,
                3 => &mut t.receipt_identity,
                4 => &mut t.staging_receipt_identity,
                5 => &mut t.staging_obligation_identity,
                6 => &mut t.staging_signer_identity,
                7 => &mut t.staging_execution_identity,
                _ => unreachable!(),
            };
            digest[0] ^= 1;
        });
    }
    for word in 0..4 {
        changes_contract(|f| f.roots[1][word] += 1);
    }
    changes_contract(|f| f.roots.swap(0, 1));
    changes_contract(|f| {
        f.roots.pop();
    });
}

#[test]
fn stale_theorem_preimages_fail_before_callback() {
    for case in 0..7 {
        let mut f = mixed();
        match case {
            0 => f.subjects.aggregate_statement_identity[0] ^= 1,
            1 => f.theorem.generated_source_identity[0] ^= 1,
            2 => f.theorem.staging_receipt_identity[0] ^= 1,
            3 => f.theorem.staging_obligation_identity[0] ^= 1,
            4 => f.theorem.staging_signer_identity[0] ^= 1,
            5 => f.theorem.staging_execution_identity[0] ^= 1,
            6 => f.theorem.statement_identity[0] ^= 1,
            _ => unreachable!(),
        }
        ample(|b| {
            assert!(with_encoded(&f.input(), b, |_, _| panic!("stale theorem accepted")).is_err())
        });
    }
}

#[test]
fn every_premise_field_and_order_is_checked() {
    let original = mixed();
    for index in 0..original.premises.len() {
        for component in 0..4 {
            let mut f = mixed();
            match &mut f.premises[index] {
                Premise::D1Launch => f.premises[index] = Premise::WritableOutput { parameter: 42 },
                Premise::OutputWithinGlobalX { parameter }
                | Premise::WritableOutput { parameter } => *parameter += 1,
                Premise::ReadableInput { parameter, domain } => {
                    if component == 0 {
                        *parameter += 1;
                    } else {
                        *domain = flip(*domain);
                    }
                }
                Premise::SeparateInputOutput { input, output } => {
                    if component == 0 {
                        *input += 1;
                    } else {
                        *output += 1;
                    }
                }
                Premise::RepresentableAddress {
                    parameter,
                    domain,
                    element_bytes,
                    alignment,
                } => match component {
                    0 => *parameter += 1,
                    1 => *domain = flip(*domain),
                    2 => *element_bytes *= 2,
                    3 => *alignment /= 2,
                    _ => unreachable!(),
                },
            }
            ample(|b| {
                assert!(with_encoded(&f.input(), b, |_, _| panic!("substituted premise")).is_err())
            });
        }
    }
    for index in 0..original.premises.len() - 1 {
        let mut f = mixed();
        f.premises.swap(index, index + 1);
        ample(|b| {
            assert!(with_encoded(&f.input(), b, |_, _| panic!("reordered premises")).is_err())
        });
    }
    let mut f = mixed();
    f.reads.remove(2);
    ample(|b| assert!(with_encoded(&f.input(), b, |_, _| panic!("deduplicated reads")).is_err()));
}

fn flip(d: Domain) -> Domain {
    match d {
        Domain::GuardedOutput => Domain::GlobalLaunch,
        Domain::GlobalLaunch => Domain::GuardedOutput,
    }
}

#[test]
fn typed_projection_preserves_all_value_variants_and_premise_fields() {
    use ConditionalTotalViewAddressDomainV1 as D;
    use ProductionConditionalRuntimePremiseV1 as P;
    for (source, expected) in [
        (P::D1Launch, Premise::D1Launch),
        (
            P::OutputWithinGlobalX { parameter: 9 },
            Premise::OutputWithinGlobalX { parameter: 9 },
        ),
        (
            P::WritableOutput { parameter: 8 },
            Premise::WritableOutput { parameter: 8 },
        ),
        (
            P::ReadableInput {
                parameter: 7,
                domain: D::GlobalLaunch,
            },
            Premise::ReadableInput {
                parameter: 7,
                domain: Domain::GlobalLaunch,
            },
        ),
        (
            P::SeparateInputOutput {
                input: 6,
                output: 5,
            },
            Premise::SeparateInputOutput {
                input: 6,
                output: 5,
            },
        ),
        (
            P::RepresentableAddress {
                parameter: 4,
                domain: D::GuardedOutput,
                element_bytes: 8,
                alignment: 2,
            },
            Premise::RepresentableAddress {
                parameter: 4,
                domain: Domain::GuardedOutput,
                element_bytes: 8,
                alignment: 2,
            },
        ),
        (
            P::RepresentableAddress {
                parameter: 3,
                domain: D::GlobalLaunch,
                element_bytes: 4,
                alignment: 1,
            },
            Premise::RepresentableAddress {
                parameter: 3,
                domain: Domain::GlobalLaunch,
                element_bytes: 4,
                alignment: 1,
            },
        ),
    ] {
        assert_eq!(project_premise(source), expected);
    }
    assert_eq!(
        ranked_value(ProductionRankedValueV1::Argument(7)),
        RankedValue::Argument(7)
    );
    assert_eq!(
        ranked_value(ProductionRankedValueV1::BlockArgument {
            block: 9,
            argument: 4
        }),
        RankedValue::BlockArgument {
            block: 9,
            argument: 4
        }
    );
    assert_eq!(
        ranked_value(ProductionRankedValueV1::Local(
            ProductionRankedValueIdV1::new(19)
        )),
        RankedValue::Local(19)
    );
    assert_eq!(
        canonical_location(FunctionOperationLocation::new(BlockId(12), 987)).unwrap(),
        CanonicalLocation {
            block: 12,
            operation: 987
        }
    );
    let f = mixed();
    assert_eq!(
        output_layout(&f.premises, 16).unwrap(),
        (Domain::GuardedOutput, 4, 4)
    );
    assert!(output_layout(&f.premises, 2).is_err());
    assert!(require_argument(&f.arguments, 2, 16, Role::Output).is_ok());
    assert!(require_argument(&f.arguments, 1, 16, Role::Output).is_err());
    assert!(require_argument(&f.arguments, 2, 2, Role::Output).is_err());
    assert!(require_argument(&f.arguments, 2, 16, Role::Input).is_err());
}

fn reference(kind: SafeReferenceKindV2, bytes: [[u8; 32]; 5]) -> FunctionalRefinementSubjectsV2 {
    FunctionalRefinementSubjectsV2::new(
        kind,
        DigestV1::from_untrusted_bytes(bytes[0]),
        DigestV1::from_untrusted_bytes(bytes[1]),
        DigestV1::from_untrusted_bytes(bytes[2]),
        DigestV1::from_untrusted_bytes(bytes[3]),
        DigestV1::from_untrusted_bytes(bytes[4]),
    )
    .unwrap()
}
fn binding(subject: FunctionalRefinementSubjectsV2) -> FunctionalRefinementBindingV2 {
    FunctionalRefinementBindingV2::from_subjects(subject, DigestV1::from_untrusted_bytes([99; 32]))
        .unwrap()
}

#[test]
fn execution_and_staging_must_match_every_reference_subject_and_boundary() {
    let bytes = [[1; 32], [0; 32], [2; 32], [3; 32], [4; 32]];
    let expected = reference(SafeReferenceKindV2::Mir, bytes);
    let boundary = FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir;
    ample(|b| {
        require_subjects(expected, binding(expected), binding(expected), boundary, b).unwrap();
        for index in 0..5 {
            let mut changed = bytes;
            changed[index][0] ^= 8;
            let kind = if index == 1 {
                SafeReferenceKindV2::SourceAndMir
            } else {
                SafeReferenceKindV2::Mir
            };
            let other = reference(kind, changed);
            assert!(
                require_subjects(expected, binding(other), binding(expected), boundary, b).is_err()
            );
            assert!(
                require_subjects(expected, binding(expected), binding(other), boundary, b).is_err()
            );
            assert!(
                require_subjects(other, binding(expected), binding(expected), boundary, b).is_err()
            );
        }
        for wrong in [
            FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
            FunctionalRefinementBoundaryV2::SafeReferenceSourceToKernelMir,
        ] {
            assert!(
                require_subjects(expected, binding(expected), binding(expected), wrong, b).is_err()
            );
        }
        let mut source_bytes = bytes;
        source_bytes[1] = [8; 32];
        let unsupported = reference(SafeReferenceKindV2::SourceAndMir, source_bytes);
        assert!(
            require_subjects(
                unsupported,
                binding(unsupported),
                binding(unsupported),
                boundary,
                b
            )
            .is_err()
        );
    });
}

#[test]
fn bounded_rosters_include_output_only_and_reject_incomplete_maps() {
    assert!(require_counts(1, 1, 0, 4, 0).is_ok());
    assert!(
        require_counts(
            MAX_CONDITIONAL_ROOTS_V1,
            MAX_CONDITIONAL_ARGUMENTS_V1,
            MAX_CONDITIONAL_READS_V1,
            MAX_CONDITIONAL_PREMISES_V1,
            MAX_CONDITIONAL_READS_V1
        )
        .is_ok()
    );
    for counts in [
        (0, 1, 0, 4, 0),
        (MAX_CONDITIONAL_ROOTS_V1 + 1, 1, 0, 4, 0),
        (1, 0, 0, 4, 0),
        (1, MAX_CONDITIONAL_ARGUMENTS_V1 + 1, 0, 4, 0),
        (1, 1, MAX_CONDITIONAL_READS_V1 + 1, 4, 0),
        (1, 3, 3, 13, 2),
        (1, 3, 3, 12, 3),
        (1, 3, 3, 14, 3),
        (1, 1, usize::MAX, 4, 0),
    ] {
        assert!(require_counts(counts.0, counts.1, counts.2, counts.3, counts.4).is_err());
    }
    let f = Fixture::new(0, 0);
    ample(|b| {
        with_encoded(&f.input(), b, |v, _| {
            assert_eq!(v.read_count(), 0);
            assert_eq!(v.argument_count(), 1);
            assert_eq!(v.premise_count(), 4);
        })
        .unwrap()
    });
}

#[test]
fn work_and_storage_denials_precede_callback_and_preserve_history() {
    let f = mixed();
    let needed = ample(|b| {
        with_encoded(&f.input(), b, |_, _| ()).unwrap();
        b.work()
    });
    for limit in [
        0,
        1,
        MAX_CONDITIONAL_INVOCATION_BYTES_V1,
        needed / 2,
        needed - 1,
    ] {
        let mut work = Work::new(limit);
        let mut b = Budget::new(&mut work, ENCODING_STORAGE + 23);
        b.reserve_storage(23).unwrap();
        assert!(with_encoded(&f.input(), &mut b, |_, _| panic!("unpaid encoding")).is_err());
        assert_eq!(b.storage(), 23);
        let denied = b.failed_work();
        assert!(denied.is_some());
        assert!(b.work() <= limit);
        b.charge_work(0).unwrap();
        assert_eq!(b.failed_work(), denied);
    }
    let mut work = Work::new(needed);
    let mut b = Budget::new(&mut work, ENCODING_STORAGE);
    with_encoded(&f.input(), &mut b, |_, _| ()).unwrap();
    assert_eq!(b.work(), needed);
    let mut work = Work::new(needed);
    let mut b = Budget::new(&mut work, ENCODING_STORAGE - 1);
    assert!(matches!(
        with_encoded(&f.input(), &mut b, |_, _| panic!("unpaid storage")),
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(b.work(), 1);
    assert_eq!(b.storage(), 0);
    assert_eq!(b.failed_storage(), Some(ENCODING_STORAGE));
}

#[test]
fn scopes_release_only_their_own_scratch_on_success_error_and_unwind() {
    ample(|b| {
        b.reserve_storage(11).unwrap();
        let account = b.work_ledger_identity_v1();
        let f = mixed();
        let result = with_encoded(&f.input(), b, |_, b| {
            assert!(b.work_ledger_identity_v1() == account);
            b.reserve_storage(17).unwrap();
            Err::<(), _>("consumer error")
        })
        .unwrap();
        assert_eq!(result, Err("consumer error"));
        assert_eq!(b.storage(), 28);
        let work = b.work();
        let failed = with_scratch(b, 50, |b| {
            b.charge_work(7)?;
            Err::<(), _>(Error::Mismatch("producer error"))
        });
        assert!(failed.is_err());
        assert_eq!(b.storage(), 28);
        assert_eq!(b.work(), work + 8);
        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ = with_encoded(&f.input(), b, |_, b| {
                b.reserve_storage(3).unwrap();
                panic!("consumer unwind");
            });
        }));
        assert!(unwind.is_err());
        assert_eq!(b.storage(), 31);
    });
}

#[test]
fn enclosing_generated_fields_style_scope_returns_payload_unreserved() {
    let f = mixed();
    ample(|b| {
        b.reserve_storage(29).unwrap();
        let mut length = 0;
        let owned = b
            .with_prepaid_scope::<_, Error>(29, 1, 1, 13, |b| {
                with_encoded(&f.input(), b, |view, b| -> Result<Vec<u8>, Error> {
                    length = view.canonical_bytes().len();
                    b.reserve_storage(length)?;
                    b.charge_work(length)?;
                    Ok(view.canonical_bytes().to_vec())
                })?
            })
            .unwrap();
        assert_eq!(b.storage(), 29);
        // Primary retention must perform this reservation before storing owned.
        b.reserve_storage(owned.capacity()).unwrap();
        assert_eq!(owned.len(), length);
        assert_eq!(owned, f.wire());
        assert_eq!(b.storage(), 29 + owned.capacity());
    });
}

#[test]
fn foreign_ledger_and_damaged_floor_are_rejected_without_foreign_release() {
    let mut first_work = Work::new(100);
    let mut second_work = Work::new(100);
    let mut first = Budget::new(&mut first_work, 100);
    let mut second = Budget::new(&mut second_work, 100);
    second.reserve_storage(77).unwrap();
    let result = with_scratch(&mut first, 20, |b| {
        std::mem::swap(b, &mut second);
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(first.storage(), 77);
    assert_eq!(second.storage(), 20);
    let result = with_scratch(&mut first, 20, |b| {
        b.release_storage(1)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(first.storage(), 96);
}
