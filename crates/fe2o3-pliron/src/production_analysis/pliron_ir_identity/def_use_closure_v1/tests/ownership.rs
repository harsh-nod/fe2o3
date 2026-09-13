use super::*;
use crate::production_analysis::pliron_pass_contract::{
    PlironPassPreservationErrorV1, begin_production_pliron_pass_contract_session_v1,
};

fn public_failure(
    context: &Context,
    function: &FuncOp,
    code: &'static str,
) -> PlironIrIdentityErrorV1 {
    reset_trace();
    let error = derive_pliron_ir_structural_identity_v1(context, function).unwrap_err();
    assert_eq!(error.code(), code);
    assert_eq!(observed().full_verifications, 0);
    let session = begin_production_pliron_pass_contract_session_v1(
        LivePlironStructuralIdentityProviderV1::new(context, function),
    )
    .err()
    .expect("closed ownership is required for a session");
    assert!(
        matches!(session, PlironPassPreservationErrorV1::IdentityUnavailable { source_code, .. } if source_code == code)
    );
    assert_eq!(observed().full_verifications, 0);
    error
}

#[test]
fn foreign_results_and_arguments_reject_before_definition_or_use_scans() {
    for argument in [false, true] {
        for mistyped in [false, true] {
            let context = &mut setup();
            let local = fanout(context, 1);
            let index = IndexType::get(context).into();
            let ty = if mistyped {
                UnitType::get(context).into()
            } else {
                index
            };
            let foreign = function(context, vec![ty]);
            let foreign_entry = foreign.get_entry_block(context);
            let value = if argument {
                foreign_entry.deref(context).get_argument(0)
            } else {
                // A raw result type isolates ownership precedence even when
                // the foreign operation itself has an invalid result type.
                let op = Operation::new(
                    context,
                    IndexConstantOp::get_concrete_op_info(),
                    vec![ty],
                    vec![],
                    vec![],
                    0,
                );
                op.insert_at_back(foreign_entry, context);
                op.deref(context).get_result(0)
            };
            let scan = prescan(context, &local).unwrap();
            Operation::replace_operand(scan.operations[0][1], context, 0, value);
            let error = public_failure(context, &local, "FE2O3-PRESERVE-003");
            assert!(matches!(
                error,
                PlironIrIdentityErrorV1::ExternalOperand {
                    location: PlironPreserveLocationV1::Operation {
                        block: 0,
                        operation: 1,
                        ..
                    },
                    operand: 0,
                    ..
                }
            ));
            assert_eq!(observed().definition_rosters, 0);
            assert_eq!(observed().value_use_vectors, 0);
            assert_eq!(observed().successor_use_vectors, 0);
        }
    }
}

#[test]
fn foreign_successor_keeps_exact_ordinal() {
    use dialect_kernel::IndexLessThanBranchArgsOp;
    for ordinal in [0, 1] {
        let context = &mut setup();
        let index = IndexType::get(context).into();
        let local = function(context, vec![index]);
        let entry = local.get_entry_block(context);
        let argument = entry.deref(context).get_argument(0);
        let join = BasicBlock::new(context, None, vec![index]);
        join.insert_at_back(local.get_region(context), context);
        let branch = IndexLessThanBranchArgsOp::new(
            context,
            argument,
            argument,
            vec![argument],
            vec![argument],
            join,
            join,
        );
        append(context, entry, branch);
        let ret = ReturnOp::new(context);
        append(context, join, ret);
        assert!(derive_pliron_ir_structural_identity_v1(context, &local).is_ok());
        let foreign = function(context, vec![index]);
        Operation::replace_successor(
            branch.get_operation(),
            context,
            ordinal,
            foreign.get_entry_block(context),
        );
        let error = public_failure(context, &local, "FE2O3-PRESERVE-003");
        assert!(matches!(error, PlironIrIdentityErrorV1::ExternalSuccessor {
            location: PlironPreserveLocationV1::Operation { block: 0, operation: 0, .. }, successor
        } if successor == ordinal));
        assert_eq!(observed().value_use_vectors, 0);
        assert_eq!(observed().successor_use_vectors, 0);
    }
}

#[test]
fn foreign_and_unattached_scalar_users_reject_before_user_roster_scan() {
    for argument in [false, true] {
        for attached in [false, true] {
            for fits_local_count in [false, true] {
                let context = &mut setup();
                let index = IndexType::get(context).into();
                let local = function(context, if argument { vec![index] } else { vec![] });
                let entry = local.get_entry_block(context);
                let exported = if argument {
                    entry.deref(context).get_argument(0)
                } else {
                    let constant = IndexConstantOp::new(context, 7);
                    let value = constant.result(context);
                    append(context, entry, constant);
                    value
                };
                if fits_local_count {
                    let other = IndexConstantOp::new(context, 9);
                    let value = other.result(context);
                    append(context, entry, other);
                    let use_other =
                        IndexBinaryOp::new(context, IndexBinaryKindAttr::Add, value, value);
                    append(context, entry, use_other);
                }
                let ret = ReturnOp::new(context);
                append(context, entry, ret);
                let foreign = function(context, vec![index]);
                let foreign_entry = foreign.get_entry_block(context);
                let value = foreign_entry.deref(context).get_argument(0);
                let mut operands = vec![value; 257];
                operands[256] = exported;
                let user = Operation::new(
                    context,
                    IndexBinaryOp::get_concrete_op_info(),
                    vec![index],
                    operands,
                    vec![],
                    0,
                );
                if attached {
                    user.insert_at_back(foreign_entry, context);
                }
                let error = public_failure(context, &local, "FE2O3-PRESERVE-000");
                let expected = if fits_local_count {
                    "SSA use leaves this function"
                } else {
                    "SSA uses exceed this function's operand roster"
                };
                assert!(
                    matches!(error, PlironIrIdentityErrorV1::StructuralVerificationFailed { detail } if detail == expected)
                );
                assert_eq!(
                    observed().value_use_vectors,
                    2 * usize::from(fits_local_count)
                );
                assert_eq!(observed().user_rosters, 0);
            }
        }
    }
}

#[test]
fn foreign_and_unattached_predecessors_reject_before_user_roster_scan() {
    for attached in [false, true] {
        for fits_local_count in [false, true] {
            let context = &mut setup();
            let local = function(context, vec![]);
            let entry = local.get_entry_block(context);
            let exit = if fits_local_count {
                let join = BasicBlock::new(context, None, vec![]);
                join.insert_at_back(local.get_region(context), context);
                let branch = BranchOp::new(context, join);
                append(context, entry, branch);
                join
            } else {
                entry
            };
            let ret = ReturnOp::new(context);
            append(context, exit, ret);
            let foreign = function(context, vec![]);
            let foreign_entry = foreign.get_entry_block(context);
            let mut successors = vec![foreign_entry; 257];
            successors[256] = entry;
            let user = Operation::new(
                context,
                BranchOp::get_concrete_op_info(),
                vec![],
                vec![],
                successors,
                0,
            );
            if attached {
                user.insert_at_back(foreign_entry, context);
            }
            let error = public_failure(context, &local, "FE2O3-PRESERVE-000");
            let expected = if fits_local_count {
                "successor use leaves this function"
            } else {
                "block uses exceed this function's successor roster"
            };
            assert!(
                matches!(error, PlironIrIdentityErrorV1::StructuralVerificationFailed { detail } if detail == expected)
            );
            assert_eq!(
                observed().successor_use_vectors,
                2 * usize::from(fits_local_count)
            );
            assert_eq!(observed().user_rosters, 0);
        }
    }
}

#[test]
fn local_type_errors_still_reach_the_structural_verifier() {
    let context = &mut setup();
    let local = function(context, vec![UnitType::get(context).into()]);
    let entry = local.get_entry_block(context);
    let index = IndexType::get(context).into();
    let join = BasicBlock::new(context, None, vec![index]);
    join.insert_at_back(local.get_region(context), context);
    let value = entry.deref(context).get_argument(0);
    let branch = BranchArgsOp::new(context, vec![value], join);
    append(context, entry, branch);
    let ret = ReturnOp::new(context);
    append(context, join, ret);
    reset_trace();
    let error = derive_pliron_ir_structural_identity_v1(context, &local).unwrap_err();
    assert_eq!(error.code(), "FE2O3-PRESERVE-000");
    assert_eq!(observed().full_verifications, 1);
}

#[test]
fn failure_diagnostics_are_admitted_after_transient_indexes_retire() {
    let context = &mut setup();
    let local = fanout(context, 1);
    let scan = prescan(context, &local).unwrap();
    let foreign = IndexConstantOp::new(context, 9);
    Operation::replace_operand(scan.operations[0][1], context, 0, foreign.result(context));
    let mut budget = Budget::new(hard()).unwrap();
    let failure = check_inner(context, &local, &scan, &mut budget).unwrap_err();
    assert!(matches!(failure, Failure::Operand { .. }));
    let prefix_work = budget.work;
    budget.admit_failure(&failure).unwrap();
    let work = budget.work;
    let peak = budget.peak;
    assert_eq!(
        work - prefix_work,
        64 + 4 * MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1
    );
    assert_eq!(budget.base, FRAME_CELLS + budget.incoming);
    assert!(matches!(
        check(
            context,
            &local,
            &scan,
            ProductionAnalysisResourceLimitsV1::new(work, peak)
        ),
        Err(Failure::Operand { .. })
    ));
    assert_resource(
        check(
            context,
            &local,
            &scan,
            ProductionAnalysisResourceLimitsV1::new(work - 1, peak),
        ),
        "work upper bound",
    );
    assert_resource(
        check(
            context,
            &local,
            &scan,
            ProductionAnalysisResourceLimitsV1::new(work, peak - 1),
        ),
        "peak storage upper bound",
    );
}
