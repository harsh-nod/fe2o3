#![cfg(test)]
//! Inert components only: no fabricated Request, execution, collector or proof.
use super::*;
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

#[path = "nominal_field_tests.rs"]
mod nominal_field_tests;

#[allow(dead_code)]
#[path = "../../../fe2o3-kernel-descriptor/tests/support/conditional_invocation_v2.rs"]
mod fixture;

#[test]
fn scratch_exact_and_one_short_do_not_enter_unpaid_work() {
    for exact in [false, true] {
        let mut work = Work::new(10);
        let needed = 17 + 23 + account::HEADER;
        let mut budget = Budget::new(&mut work, needed - usize::from(!exact));
        budget.reserve_storage(17).unwrap();
        let entered = Cell::new(false);
        let result = account::scope(&mut budget, 23, |budget| {
            entered.set(true);
            budget.charge_work(3)?;
            Ok(())
        });
        assert_eq!(result.is_ok(), exact);
        assert_eq!(entered.get(), exact);
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.work(), if exact { 3 } else { 0 });
        assert_eq!(budget.failed_storage().is_some(), !exact);
    }
}

#[test]
fn scratch_preserves_opaque_child_reservations_on_ok_and_err() {
    for error in [false, true] {
        let mut work = Work::new(10);
        let mut budget = Budget::new(&mut work, 17 + account::HEADER + 23 + 11);
        budget.reserve_storage(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = account::scope(&mut budget, 23, |budget| {
            budget.reserve_storage(11)?;
            budget.charge_work(7)?;
            if error {
                Err(E::Mismatch("child"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result.is_err(), error);
        assert_eq!((budget.storage(), budget.work()), (28, 7));
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn scratch_drops_locals_before_release_on_unwind_without_refunding_child() {
    struct Mark<'a>(&'a Cell<bool>);
    impl Drop for Mark<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, 17 + account::HEADER + 23 + 11);
    budget.reserve_storage(17).unwrap();
    let dropped = Cell::new(false);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        account::scope(&mut budget, 23, |budget| {
            let _mark = Mark(&dropped);
            budget.reserve_storage(11)?;
            budget.charge_work(7)?;
            panic!("component unwind");
        })
    }));
    assert!(result.is_err() && dropped.get());
    assert_eq!((budget.storage(), budget.work()), (28, 7));
}

#[test]
fn scratch_floor_theft_is_terminal_without_cleanup_refund() {
    let mut work = Work::new(10);
    let extent = 17 + account::HEADER + 23;
    let mut budget = Budget::new(&mut work, extent);
    budget.reserve_storage(17).unwrap();
    assert!(matches!(
        account::scope(&mut budget, 23, |budget| {
            budget.release_storage(1)?;
            Ok(())
        }),
        Err(E::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), extent - 1);
}

#[test]
fn scratch_does_not_refund_a_foreign_account() {
    let mut work = Work::new(10);
    let mut other_work = Work::new(10);
    let extent = 17 + account::HEADER + 23;
    let mut budget = Budget::new(&mut work, extent + 10);
    let mut other = Budget::new(&mut other_work, extent + 10);
    budget.reserve_storage(17).unwrap();
    other.reserve_storage(extent + 1).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    assert!(matches!(
        account::scope(&mut budget, 23, |budget| {
            std::mem::swap(budget, &mut other);
            Err(E::Mismatch("child"))
        }),
        Err(E::Resource(Resource::Accounting))
    ));
    assert_eq!((budget.storage(), other.storage()), (extent + 1, extent));
    assert!(other.work_ledger_identity_v1() == ledger);
}

#[test]
fn scratch_keeps_denial_history_and_overflow_never_enters() {
    let mut work = Work::new(10);
    let mut budget = Budget::new(&mut work, account::HEADER + 23);
    assert!(matches!(
        account::scope(&mut budget, 23, |budget| {
            budget.reserve_storage(1)?;
            Ok(())
        }),
        Err(E::Resource(Resource::Storage(_)))
    ));
    assert_eq!(budget.storage(), 0);
    assert!(budget.failed_storage().is_some());
    assert!(matches!(
        account::scope(&mut budget, usize::MAX, |_| {
            panic!("overflow callback");
        }),
        Err(E::Resource(Resource::Arithmetic))
    ));
}

#[test]
fn counts_reject_omissions_excess_and_overflow() {
    assert!(rows::counts(1, 3, 2, 10).is_ok());
    for (r, a, n, p) in [
        (0, 3, 2, 10),
        (1, 0, 2, 10),
        (1, 3, 2, 9),
        (MAX_CONDITIONAL_ROOTS_V1 + 1, 3, 2, 10),
        (1, MAX_CONDITIONAL_ARGUMENTS_V1 + 1, 2, 10),
        (1, 3, usize::MAX, 10),
    ] {
        assert!(rows::counts(r, a, n, p).is_err());
    }
}

#[test]
fn all_v2_theorem_fields_and_subjects_are_compared_not_only_commitment() {
    let f = fixture::Fixture::new(2, 2);
    let wire = fixture::wire(&f);
    let contract = decode_conditional_invocation_contract_v2(&wire, &mut fixture::free).unwrap();
    for field in 0..9 {
        let mut changed = *contract.theorem();
        let digest = match field {
            0 => &mut changed.statement_identity,
            1 => &mut changed.generated_source_identity,
            2 => &mut changed.execution_identity,
            3 => &mut changed.receipt_identity,
            4 => &mut changed.staging_receipt_identity,
            5 => &mut changed.staging_obligation_identity,
            6 => &mut changed.staging_signer_identity,
            7 => &mut changed.staging_execution_identity,
            _ => &mut changed.cpu_input_commitment,
        };
        digest[0] ^= 1;
        let mut work = Work::new(10000);
        let mut budget = Budget::new(&mut work, 0);
        assert!(
            rows::compare_header(&contract, contract.subjects(), &changed, &mut budget).is_err()
        );
    }
    for field in 0..9 {
        let mut changed = *contract.subjects();
        let digest = match field {
            0 => &mut changed.kernel_id,
            1 => &mut changed.exact_graph_identity,
            2 => &mut changed.aggregate_statement_identity,
            3 => &mut changed.source_semantic_identity,
            4 => &mut changed.safe_reference_identity,
            5 => &mut changed.safe_reference_source_hash,
            6 => &mut changed.safe_reference_mir_hash,
            7 => &mut changed.kernel_subject_identity,
            _ => &mut changed.kernel_mir_hash,
        };
        digest[0] ^= 1;
        let mut work = Work::new(10000);
        let mut budget = Budget::new(&mut work, 0);
        assert!(
            rows::compare_header(&contract, &changed, contract.theorem(), &mut budget).is_err()
        );
    }
}

#[test]
fn header_work_exact_and_one_short_use_original_debit() {
    let f = fixture::Fixture::new(2, 2);
    let wire = fixture::wire(&f);
    let contract = decode_conditional_invocation_contract_v2(&wire, &mut fixture::free).unwrap();
    let debit = 2 * size_of::<ConditionalSubjectsV1>() + 2 * size_of::<ConditionalTheoremV2>() + 1;
    for exact in [false, true] {
        let mut work = Work::new(debit + 7 - usize::from(!exact));
        let mut budget = Budget::new(&mut work, 17);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(17).unwrap();
        let result = rows::compare_header(
            &contract,
            contract.subjects(),
            contract.theorem(),
            &mut budget,
        );
        assert_eq!(result.is_ok(), exact);
        assert_eq!(budget.work(), if exact { debit + 7 } else { 7 });
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn charged_wire_roots_require_complete_order() {
    let mut f = fixture::Fixture::new(2, 2);
    f.roots.push([55, 66, 77, 88]);
    let wire = fixture::wire(&f);
    let contract = decode_conditional_invocation_contract_v2(&wire, &mut fixture::free).unwrap();
    let mut work = Work::new(10000);
    let mut budget = Budget::new(&mut work, 0);
    rows::ordered_roots(&contract, &f.roots, &mut budget).unwrap();
    assert!(rows::ordered_roots(&contract, &f.roots[..1], &mut budget).is_err());
    f.roots.swap(0, 1);
    assert!(rows::ordered_roots(&contract, &f.roots, &mut budget).is_err());
}

#[test]
fn wire_valid_header_substitution_still_disagrees_with_original_content() {
    let original = fixture::Fixture::new(2, 2);
    let original_wire = fixture::wire(&original);
    let original_contract =
        decode_conditional_invocation_contract_v2(&original_wire, &mut fixture::free).unwrap();
    let mut substituted = fixture::Fixture::new(2, 2);
    substituted.theorem.generated_source_identity[0] ^= 1;
    // The fixture recomputes the inert statement: this is valid V2 framing,
    // not a malformed wire rejected before the comparison under test.
    let wire = fixture::wire(&substituted);
    let contract = decode_conditional_invocation_contract_v2(&wire, &mut fixture::free).unwrap();
    let mut work = Work::new(10000);
    let mut budget = Budget::new(&mut work, 0);
    assert!(
        rows::compare_header(
            &contract,
            original_contract.subjects(),
            original_contract.theorem(),
            &mut budget
        )
        .is_err()
    );
}

#[test]
fn ordered_premises_preserve_every_variant_and_complete_roster() {
    use fe2o3_kernel_ir::ConditionalTotalViewAddressDomainV1 as Domain;
    use fe2o3_pliron::ProductionConditionalRuntimePremiseV1 as Live;
    let f = fixture::Fixture::new(2, 2);
    let wire = fixture::wire(&f);
    let contract = decode_conditional_invocation_contract_v2(&wire, &mut fixture::free).unwrap();
    let domain = |d| match d {
        ConditionalAddressDomainV1::GuardedOutput => Domain::GuardedOutput,
        ConditionalAddressDomainV1::GlobalLaunch => Domain::GlobalLaunch,
    };
    let mut expected: Vec<_> = f
        .premises
        .iter()
        .map(|p| match *p {
            ConditionalRuntimePremiseV1::D1Launch => Live::D1Launch,
            ConditionalRuntimePremiseV1::OutputWithinGlobalX { parameter } => {
                Live::OutputWithinGlobalX { parameter }
            }
            ConditionalRuntimePremiseV1::WritableOutput { parameter } => {
                Live::WritableOutput { parameter }
            }
            ConditionalRuntimePremiseV1::ReadableInput {
                parameter,
                domain: d,
            } => Live::ReadableInput {
                parameter,
                domain: domain(d),
            },
            ConditionalRuntimePremiseV1::SeparateInputOutput { input, output } => {
                Live::SeparateInputOutput { input, output }
            }
            ConditionalRuntimePremiseV1::RepresentableAddress {
                parameter,
                domain: d,
                element_bytes,
                alignment,
            } => Live::RepresentableAddress {
                parameter,
                domain: domain(d),
                element_bytes,
                alignment,
            },
        })
        .collect();
    let mut work = Work::new(100000);
    let mut budget = Budget::new(&mut work, 0);
    rows::ordered_premises(&contract, &expected, &mut budget).unwrap();
    assert!(
        rows::ordered_premises(&contract, &expected[..expected.len() - 1], &mut budget).is_err()
    );
    expected.swap(4, 7);
    assert!(rows::ordered_premises(&contract, &expected, &mut budget).is_err());
    expected.swap(4, 7);
    expected[3] = Live::RepresentableAddress {
        parameter: f.arguments[2].canonical_parameter,
        domain: Domain::GuardedOutput,
        element_bytes: 8,
        alignment: 4,
    };
    assert!(rows::ordered_premises(&contract, &expected, &mut budget).is_err());
}

#[test]
fn argument_queries_do_not_confuse_parameter_source_or_role() {
    let mut f = fixture::Fixture::new(2, 2);
    for (i, row) in f.arguments.iter_mut().enumerate() {
        row.adjusted_argument = i as u32;
    }
    let wire = fixture::wire(&f);
    let contract = decode_conditional_invocation_contract_v2(&wire, &mut fixture::free).unwrap();
    let mut work = Work::new(10000);
    let mut budget = Budget::new(&mut work, 0);
    for (i, expected) in f.arguments.iter().enumerate() {
        let row = rows::argument(
            &contract,
            i as u16,
            expected.canonical_parameter,
            expected.role,
            &mut budget,
        )
        .unwrap();
        assert_eq!(
            (
                row.source_argument,
                row.adjusted_argument,
                row.generated_field
            ),
            (i as u32, i as u32, i as u16)
        );
        assert!(rows::argument(&contract, i as u16, i as u32, expected.role, &mut budget).is_err());
    }
    assert!(
        rows::argument(
            &contract,
            0,
            f.arguments[0].canonical_parameter,
            ConditionalArgumentRoleV1::Output,
            &mut budget
        )
        .is_err()
    );
    assert!(
        rows::argument(
            &contract,
            3,
            0,
            ConditionalArgumentRoleV1::Input,
            &mut budget
        )
        .is_err()
    );
}

#[test]
fn every_read_field_is_checked_and_equal_parameter_does_not_deduplicate() {
    let f = fixture::Fixture::new(1, 2);
    let actual = f.reads[0];
    let mut work = Work::new(10000);
    let mut budget = Budget::new(&mut work, 0);
    rows::same_read(actual, actual, &mut budget).unwrap();
    assert_eq!(actual.argument, f.reads[1].argument);
    assert!(rows::same_read(actual, f.reads[1], &mut budget).is_err());
    for field in 0..17 {
        let mut changed = actual;
        match field {
            0 => changed.argument += 1,
            1 => changed.canonical.block += 1,
            2 => changed.canonical.operation += 1,
            3 => changed.slice += 1,
            4 => changed.pointer += 1,
            5 => changed.index += 1,
            6 => changed.value += 1,
            7 => changed.ranked.block += 1,
            8 => changed.ranked.operation += 1,
            9 => changed.ranked_view = ConditionalRankedValueV1::Local(17),
            10 => changed.ranked_index = ConditionalRankedValueV1::Argument(17),
            11 => changed.access_domain = ConditionalAddressDomainV1::GlobalLaunch,
            12 => changed.address_domain = ConditionalAddressDomainV1::GuardedOutput,
            13 => changed.element_bytes += 1,
            14 => changed.alignment += 1,
            15 => {
                changed.ranked_view = ConditionalRankedValueV1::BlockArgument {
                    block: 1,
                    argument: 2,
                }
            }
            _ => {
                changed.ranked_index = ConditionalRankedValueV1::BlockArgument {
                    block: 2,
                    argument: 1,
                }
            }
        }
        assert_ne!(actual, changed);
        assert!(rows::same_read(actual, changed, &mut budget).is_err());
    }
}

#[test]
fn conditional_slice_roles_reject_pointer_and_scalar_lookalikes() {
    use ConditionalArgumentRoleV1::{Input, Output};
    use ScalarTypeV1::F32;
    use SourceTypeDescriptorV3::*;
    assert!(nominal::role_matches(
        SharedSlice(F32),
        AccessMode::ReadOnly,
        Input
    ));
    assert!(nominal::role_matches(
        DisjointSlice(F32),
        AccessMode::ReadWrite,
        Output
    ));
    for kind in [Scalar(F32), GlobalMutPointer(F32), SharedSlice(F32)] {
        assert!(!nominal::role_matches(kind, AccessMode::ReadWrite, Output));
    }
    assert!(!nominal::role_matches(
        DisjointSlice(F32),
        AccessMode::ReadOnly,
        Input
    ));
    assert!(!nominal::role_matches(
        SharedSlice(F32),
        AccessMode::ReadWrite,
        Input
    ));
}

#[test]
fn layout_alignment_is_checked_without_overflow_or_zero_division() {
    assert_eq!(nominal::align(17, 8).unwrap(), 24);
    assert_eq!(nominal::align(16, 8).unwrap(), 16);
    assert!(matches!(
        nominal::align(u32::MAX, 8),
        Err(Resource::Arithmetic)
    ));
    for alignment in [0, 3] {
        assert!(matches!(
            nominal::align(1, alignment),
            Err(Resource::Accounting)
        ));
    }
}

#[test]
fn nominal_shape_checks_actual_element_space_and_access() {
    use SourceTypeDescriptorV3 as S;
    use fe2o3_kernel_ir::{AccessMode as A, AddressSpace as Space, ScalarType as Scalar, Type};
    let f32_type = Type::Scalar(Scalar::F32);
    let shared = Type::slice(f32_type.clone(), Space::Global, A::ReadOnly);
    let private = Type::slice(f32_type.clone(), Space::Private, A::ReadOnly);
    assert!(!nominal::shape(
        S::SharedSlice(ScalarTypeV1::F32),
        &private,
        AccessMode::ReadOnly
    ));
    assert!(nominal::shape(
        S::SharedSlice(ScalarTypeV1::F32),
        &shared,
        AccessMode::ReadOnly
    ));
    assert!(!nominal::shape(
        S::SharedSlice(ScalarTypeV1::F64),
        &shared,
        AccessMode::ReadOnly
    ));
    assert!(!nominal::shape(
        S::DisjointSlice(ScalarTypeV1::F32),
        &shared,
        AccessMode::ReadWrite
    ));
    let writable = Type::slice(f32_type.clone(), Space::Global, A::WriteOnly);
    assert!(nominal::shape(
        S::DisjointSlice(ScalarTypeV1::F32),
        &writable,
        AccessMode::WriteOnly
    ));
    assert!(!nominal::shape(
        S::DisjointSlice(ScalarTypeV1::F32),
        &writable,
        AccessMode::ReadWrite
    ));
    let pointer = Type::pointer(f32_type, Space::Global, A::ReadWrite);
    assert!(!nominal::shape(
        S::DisjointSlice(ScalarTypeV1::F32),
        &pointer,
        AccessMode::ReadWrite
    ));
    assert!(nominal::shape(
        S::GlobalMutPointer(ScalarTypeV1::F32),
        &pointer,
        AccessMode::ReadWrite
    ));
    assert!(!nominal::shape(
        S::Usize,
        &Type::Scalar(Scalar::I64),
        AccessMode::ByValue
    ));
}
