use super::*;
use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::InvocationReceiptV1;
use std::cell::Cell;

include!("fixture.rs");

#[test]
fn live_reads_preserve_ordinary_failure_and_full_payload_across_two_runs() {
    for global in [false, true] {
        let fixture = fixture(global, 2);
        let ordinary = fixture.ordinary();
        assert!(!ordinary.is_clean());
        let first = fixture.run(Some(&fixture.reads)).unwrap();
        let second = fixture.run(Some(&fixture.reads)).unwrap();
        assert!(first.is_clean());
        assert_eq!(first, second);
        assert_eq!(first.ordinary_report(), &ordinary);
        assert_eq!(first.reads(), Some(fixture.reads.as_slice()));
        assert_eq!(first.obligations.len(), 2);
        assert_eq!(first.coverage.len(), 1);
        for (index, obligation) in first.obligations.iter().enumerate() {
            assert_eq!(obligation.finding, index);
            assert_eq!(obligation.read, Some(index));
            assert_eq!(obligation.live_operation, fixture.reads[index].operation);
        }
    }
}

#[test]
fn complete_empty_roster_has_a_distinct_nominal_payload_from_legacy() {
    let fixture = fixture(false, 0);
    let empty = fixture.run(Some(&[])).unwrap();
    let legacy = fixture.run(None).unwrap();
    assert!(empty.is_clean() && legacy.is_clean());
    assert_eq!(empty.reads(), Some([].as_slice()));
    assert_eq!(legacy.reads(), None);
    assert_eq!(empty.coverage.len(), 1);
    assert!(legacy.coverage.is_empty());
    assert_ne!(empty, legacy);
    let with_reads = self::fixture(false, 2);
    assert_eq!(
        with_reads.run(None).unwrap_err().failure,
        FailureV1::ReadRoster
    );
    assert!(with_reads.run(Some(&[])).is_err());
}

#[test]
fn ordinary_clean_reads_still_require_the_complete_unique_source_roster() {
    let fixture = fixture_with_extent(false, 2, true);
    assert!(fixture.ordinary().is_clean());
    assert!(fixture.run(None).unwrap().is_clean());
    assert!(fixture.run(Some(&fixture.reads)).unwrap().is_clean());
    let mut duplicated = fixture.reads.clone();
    duplicated.push(duplicated[0]);
    for reads in [&[][..], &fixture.reads[..1], &duplicated] {
        let error = fixture.run(Some(reads)).unwrap_err();
        assert_eq!(
            error.failure,
            FailureV1::Coverage {
                selection: 0,
                refusal: RuleRefusalV1::Coordinate
            }
        );
        assert!(error.ordinary_report().unwrap().is_clean());
    }
}

#[test]
fn exact_live_read_roster_rejects_missing_duplicate_and_substituted_coordinates() {
    let fixture = fixture(false, 2);
    for fault in 0..8 {
        let mut reads = fixture.reads.clone();
        match fault {
            0 => {
                reads.pop();
            }
            1 => reads.push(reads[0]),
            2 => reads[1].operation = reads[0].operation,
            3 => reads[0].view = reads[1].view,
            4 => reads[0].index = reads[0].extent,
            5 => reads[0].extent = reads[1].extent,
            6 => reads[0].operation = fixture.occurrences[0].1,
            7 => reads[0].view = fixture.occurrences[0].2,
            _ => unreachable!(),
        }
        let error = fixture.run(Some(&reads)).unwrap_err();
        assert_eq!(
            error.ordinary_report(),
            Some(&fixture.ordinary()),
            "fault {fault}"
        );
        assert!(matches!(error.failure, FailureV1::ReadAssociation { .. }));
    }
    let mut reordered = fixture.reads.clone();
    reordered.reverse();
    let report = fixture.run(Some(&reordered)).unwrap();
    assert_eq!(report.obligations[0].read, Some(1));
    assert_eq!(report.obligations[1].read, Some(0));
}

#[test]
fn source_domain_must_match_the_actual_read_paths() {
    for global in [false, true] {
        let fixture = fixture(global, 2);
        let mut reads = fixture.reads.clone();
        reads[0].domain = if global {
            Domain::GuardedOutput
        } else {
            Domain::GlobalLaunch
        };
        let error = fixture.run(Some(&reads)).unwrap_err();
        assert_eq!(
            error.failure,
            FailureV1::Coverage {
                selection: 0,
                refusal: RuleRefusalV1::Coordinate
            }
        );
        assert_eq!(error.ordinary_report(), Some(&fixture.ordinary()));
    }
}

#[test]
fn static_out_of_bounds_and_machine_overflow_are_terminal_with_original_diagnostics() {
    for overflow in [false, true] {
        let mut fixture = fixture(false, 2);
        fixture.insert_terminal_read(overflow);
        let ordinary = fixture.ordinary();
        assert!(ordinary.findings().iter().any(|finding| if overflow {
            matches!(
                finding,
                RankedBoundsFindingV1::MachineIntegerOverflow { .. }
            )
        } else {
            matches!(finding, RankedBoundsFindingV1::StaticOutOfBounds { .. })
        }));
        let error = fixture.run(Some(&fixture.reads)).unwrap_err();
        assert!(matches!(error.failure, FailureV1::TerminalFinding { .. }));
        assert_eq!(error.ordinary_report(), Some(&ordinary));
    }
}

#[test]
fn stale_epoch_and_foreign_read_operation_fail_closed() {
    let fixture = fixture(false, 2);
    let foreign = self::fixture(false, 2);
    let mut reads = fixture.reads.clone();
    reads[0].operation = foreign.reads[0].operation;
    assert!(matches!(
        fixture.run(Some(&reads)).unwrap_err().failure,
        FailureV1::ReadAssociation { .. }
    ));
    drop(fixture.occurrences[0].1.deref_mut(&fixture.context));
    let error = fixture.run(Some(&fixture.reads)).unwrap_err();
    assert_eq!(error.failure, FailureV1::Subject);
    assert!(error.ordinary_report().is_none());
}

#[test]
fn association_ignores_diagnostic_text_but_never_access_kind_or_dimension() {
    let fixture = fixture(false, 2);
    let mut report = fixture.run(Some(&fixture.reads)).unwrap();
    for finding in &mut report.ordinary.findings {
        let RankedBoundsFindingV1::UnprovedBound {
            view,
            index,
            extent,
            ..
        } = finding
        else {
            panic!()
        };
        *view = "untrusted alias".into();
        *index = "different diagnostic".into();
        *extent = "same spelling".into();
    }
    associate(
        &report.ordinary,
        &mut report.obligations,
        Some(&fixture.reads),
    )
    .unwrap();
    for fault in 0..3 {
        let mut report = fixture.run(Some(&fixture.reads)).unwrap();
        let RankedBoundsFindingV1::UnprovedBound {
            access, dimension, ..
        } = &mut report.ordinary.findings[0]
        else {
            panic!()
        };
        match fault {
            0 => *access = AccessKindAttr::Write,
            1 => *dimension = 1,
            2 => report.obligations[0].live_operation = fixture.occurrences[0].1,
            _ => unreachable!(),
        }
        assert!(
            associate(
                &report.ordinary,
                &mut report.obligations,
                Some(&fixture.reads)
            )
            .is_err()
        );
    }
}

#[test]
fn non_obligation_findings_cannot_be_conditionalized() {
    for finding in [
        RankedBoundsFindingV1::StructuralVerificationFailed,
        RankedBoundsFindingV1::ResourceLimitExceeded {
            resource: "work",
            limit: 1,
            actual: 2,
        },
        RankedBoundsFindingV1::UnreachableBlock { block: 1 },
        RankedBoundsFindingV1::UnsupportedTerminator {
            block: 0,
            operation: "branch".into(),
        },
        RankedBoundsFindingV1::UnsupportedOperation {
            block: 0,
            operation: 0,
            kind: "opaque".into(),
        },
        RankedBoundsFindingV1::SparseIndexAnalysisFailed {
            detail: "incomplete".into(),
        },
    ] {
        let ordinary = RankedBoundsReportV1 {
            findings: vec![finding],
        };
        assert_eq!(
            associate(&ordinary, &mut [], Some(&[])),
            Err(FailureV1::TerminalFinding { finding: 0 })
        );
    }
}

#[test]
fn actual_structural_failure_keeps_the_ordinary_report() {
    let mut fixture = fixture(false, 2);
    let entry = fixture.function.get_entry_block(&fixture.context);
    ReturnOp::new(&mut fixture.context)
        .get_operation()
        .insert_at_front(entry, &fixture.context);
    // Identity capture rejects this malformed graph. Exercise the producer's
    // own structural gate with a widened fixture census and the current epoch.
    fixture.census.operations += 1;
    fixture.epoch = current_epoch(&fixture.context).unwrap();
    let error = fixture.run(Some(&fixture.reads)).unwrap_err();
    assert_eq!(error.failure, FailureV1::TerminalFinding { finding: 0 });
    assert_eq!(
        error.ordinary_report().unwrap().findings(),
        &[RankedBoundsFindingV1::StructuralVerificationFailed]
    );
}

#[test]
fn memory_bounds_coverage_arithmetic_is_attributed_to_its_actual_phase() {
    let mut census = fixture(false, 2).census;
    census.identifier_bytes = usize::MAX;
    let error = preflight_live_rule_with_inputs_v1(census, 2, Phase::MemoryBounds).unwrap_err();
    assert_eq!(error.phase, Phase::MemoryBounds);
    let legacy =
        preflight_live_rule_with_inputs_v1(census, 2, Phase::HierarchicalOwnership).unwrap_err();
    assert_eq!(legacy.phase, Phase::HierarchicalOwnership);
}

#[test]
fn typed_capture_preserves_every_terminal_presburger_failure() {
    let fixture = fixture(false, 2);
    for failure in [
        PresburgerFailureV1::InvalidModel {
            detail: "invalid model",
        },
        PresburgerFailureV1::ArithmeticOverflow,
        PresburgerFailureV1::MachineIntegerOverflow {
            bits: 64,
            signed: false,
        },
        PresburgerFailureV1::ResourceLimit {
            limit: 1,
            actual: 2,
        },
    ] {
        let mut capture = ReadCaptureV1::new(fixture.census).unwrap();
        capture.failure(&failure);
        capture.failure(&PresburgerFailureV1::Unsupported {
            detail: "dynamic extent",
        });
        assert_eq!(capture.terminal, Some(failure));
    }
    let mut capture = ReadCaptureV1::new(fixture.census).unwrap();
    capture.failure(&PresburgerFailureV1::Unsupported {
        detail: "dynamic extent",
    });
    assert_eq!(capture.terminal, None);
}

#[test]
fn capture_keeps_one_failure_and_every_later_resource_denial() {
    let fixture = fixture(false, 2);
    let mut capture = ReadCaptureV1::new(fixture.census).unwrap();
    let capacity = capture.obligations.capacity();
    capture.failure(&PresburgerFailureV1::ArithmeticOverflow);
    for actual in 2..130 {
        capture.failure(&PresburgerFailureV1::ResourceLimit { limit: 1, actual });
        assert_eq!(
            capture.terminal,
            Some(PresburgerFailureV1::ArithmeticOverflow)
        );
        assert!(capture.resource_denied);
        assert_eq!(capture.obligations.capacity(), capacity);
        assert!(capture.obligations.is_empty());
    }
}

#[test]
fn real_presburger_resource_fallback_is_terminal_even_without_an_observer() {
    use crate::production_analysis::{
        LivePlironStructuralIdentityProviderV1, PlironStructuralIdentityProviderV1,
    };
    let cap = fe2o3_kernel_analysis::MAX_PRESBURGER_WORK_UNITS_V1;
    let (context, function, _) =
        super::super::observed_presburger_tests::fixture((cap + 1) as u64, cap as u64);
    let census = LivePlironStructuralIdentityProviderV1::new(&context, &function)
        .capture_with_resource_limits_v1(hard())
        .ok()
        .unwrap()
        .input_census;
    let mut manager =
        Manager::new_with_resource_contract(&function, census, Bound::default(), 0, hard())
            .unwrap();
    let base = preflight_v1(census, 0, hard()).unwrap();
    manager
        .admit_retained_resource_upper_bound(Phase::MemoryBounds, base)
        .unwrap();
    let error = run_observed(
        InputV1 {
            context: &context,
            function: &function,
            census,
            epoch: current_epoch(&context).unwrap(),
            reads: Some(&[]),
            occurrences: &[],
        },
        &mut manager,
        (None, None),
    )
    .unwrap_err();
    assert_eq!(
        error.failure,
        FailureV1::Resource(resource("Presburger query work limit"))
    );
    assert!(matches!(
        error.ordinary_report().unwrap().findings(),
        [RankedBoundsFindingV1::UnprovedBound { .. }]
    ));
}

#[test]
fn payload_equality_compares_coordinates_domains_facts_and_original_findings() {
    let fixture = fixture(false, 2);
    let original = fixture.run(Some(&fixture.reads)).unwrap();
    for fault in 0..7 {
        let mut altered = fixture.run(Some(&fixture.reads)).unwrap();
        match fault {
            0 => altered.reads.as_mut().unwrap()[0].domain = Domain::GlobalLaunch,
            1 => altered.reads.as_mut().unwrap()[0].extent = fixture.reads[1].extent,
            2 => altered.obligations[0].read = Some(1),
            3 => altered.coverage[0].1.normal_exits[0] += 1,
            4 => altered.epoch += 1,
            5 => altered.context_address += 1,
            6 => altered.ordinary.findings.clear(),
            _ => unreachable!(),
        }
        assert_ne!(original, altered, "fault {fault}");
    }
}

#[test]
fn fixed_stage_preflight_has_exact_work_storage_and_overflow_boundaries() {
    let fixture = fixture(false, 2);
    let bound = preflight_v1(fixture.census, fixture.reads.len(), hard()).unwrap();
    assert_eq!(
        bound.retained_storage_upper_bound(),
        bound.peak_storage_upper_bound()
    );
    for (work_short, storage_short) in [(0, 0), (1, 0), (0, 1)] {
        let result = preflight_v1(
            fixture.census,
            fixture.reads.len(),
            Limits::new(
                bound.work_upper_bound() - work_short,
                bound.peak_storage_upper_bound() - storage_short,
            ),
        );
        if work_short == 0 && storage_short == 0 {
            assert_eq!(result, Ok(bound));
        } else {
            assert_eq!(result.unwrap_err().phase, Phase::MemoryBounds);
        }
    }
    assert_eq!(
        preflight_v1(fixture.census, usize::MAX, hard())
            .unwrap_err()
            .phase,
        Phase::MemoryBounds
    );
    let mut census = fixture.census;
    census.ownership_contracts = usize::MAX;
    assert_eq!(
        preflight_v1(census, 2, hard()).unwrap_err().phase,
        Phase::MemoryBounds
    );
}

#[test]
fn observed_query_admission_uses_memory_bounds_and_preserves_denied_prefix() {
    let fixture = fixture(false, 2);
    let base = preflight_v1(fixture.census, 2, hard()).unwrap();
    let query = preflight_live_rule_with_inputs_v1(fixture.census, 2, Phase::MemoryBounds).unwrap();
    let stage = base
        .checked_then_retain(query, Phase::MemoryBounds)
        .unwrap();
    let initial = fixture.manager(hard()).resource_upper_bound();
    let total = initial
        .checked_then_retain(stage, Phase::MemoryBounds)
        .unwrap();
    for (work_short, storage_short) in [(0, 0), (1, 0), (0, 1)] {
        let limits = Limits::new(
            total.work_upper_bound() - work_short,
            total.peak_storage_upper_bound() - storage_short,
        );
        let mut manager = fixture.manager(limits);
        let mut receipt = InvocationReceiptV1::new(initial, limits).unwrap();
        let phase = receipt.phase(Phase::MemoryBounds, 0).unwrap();
        let observer = phase.observer(&Ok);
        observer
            .require(
                manager
                    .remaining_resource_limits(Phase::MemoryBounds)
                    .unwrap(),
                Phase::MemoryBounds,
                Ok(base),
            )
            .unwrap();
        manager
            .admit_retained_resource_upper_bound(Phase::MemoryBounds, base)
            .unwrap();
        let fixed_prefix = manager.resource_upper_bound();
        let additional = Cell::new(Bound::default());
        let outcome = observer.with_projection(
            &|local| base.checked_then_retain(local, Phase::MemoryBounds),
            |extra| {
                run_observed(
                    fixture.input(Some(&fixture.reads)),
                    &mut manager,
                    (Some(&observer), Some((extra, &additional))),
                )
            },
        );
        drop(observer);
        if work_short == 0 && storage_short == 0 {
            let report = outcome.unwrap();
            assert_eq!(report.resource_upper_bound(), stage);
            assert_eq!(additional.get(), query);
            assert_eq!(manager.resource_upper_bound(), total);
            phase.commit(stage).unwrap();
            assert_eq!(receipt.complete(), Ok(stage));
        } else {
            let error = outcome.unwrap_err();
            let FailureV1::Resource(limit) = error.failure else {
                panic!("{error:?}")
            };
            assert_eq!(limit.phase, Phase::MemoryBounds);
            assert_eq!(error.ordinary_report(), Some(&fixture.ordinary()));
            assert_eq!(manager.resource_upper_bound(), fixed_prefix);
            assert_eq!(additional.get(), Bound::default());
            drop(phase);
            assert_eq!(receipt.snapshot().first_denial, Some(limit));
            assert_eq!(
                receipt.snapshot().current.work_upper_bound(),
                base.work_upper_bound()
            );
        }
    }
}

#[test]
fn additional_observations_retain_every_admission_before_a_later_refusal() {
    let mut fixture = fixture(false, 2);
    let output = fixture.occurrences[0].2;
    let extra_contract = OwnershipContractOp::new(
        &mut fixture.context,
        output,
        OwnershipCoverageAttr::TotalView,
        OwnershipPartitionAttr::ExactSets,
    )
    .unwrap();
    extra_contract
        .get_operation()
        .insert_before(&fixture.context, fixture.occurrences[0].1);
    fixture.refresh();
    // Fault injection into the private test descriptor, after a successful
    // query. A real pipeline subject separately authenticates its selections.
    let mut occurrences = [fixture.occurrences[0]; 2];
    occurrences[1].1 = fixture.reads[0].operation;
    let base = preflight_v1(fixture.census, 2, hard()).unwrap();
    let query = preflight_live_rule_with_inputs_v1(fixture.census, 2, Phase::MemoryBounds).unwrap();
    let twice = query
        .checked_then_retain(query, Phase::MemoryBounds)
        .unwrap();
    let complete = base
        .checked_then_retain(twice, Phase::MemoryBounds)
        .unwrap();
    let mut manager = fixture.manager(hard());
    let initial = manager.resource_upper_bound();
    let mut receipt = InvocationReceiptV1::new(initial, hard()).unwrap();
    let phase = receipt.phase(Phase::MemoryBounds, 0).unwrap();
    let observer = phase.observer(&Ok);
    observer
        .require(
            manager
                .remaining_resource_limits(Phase::MemoryBounds)
                .unwrap(),
            Phase::MemoryBounds,
            Ok(base),
        )
        .unwrap();
    manager
        .admit_retained_resource_upper_bound(Phase::MemoryBounds, base)
        .unwrap();
    let additional = Cell::new(Bound::default());
    let mut input = fixture.input(Some(&fixture.reads));
    input.occurrences = &occurrences;
    let error = observer
        .with_projection(
            &|local| base.checked_then_retain(local, Phase::MemoryBounds),
            |extra| {
                run_observed(
                    input,
                    &mut manager,
                    (Some(&observer), Some((extra, &additional))),
                )
            },
        )
        .unwrap_err();
    assert_eq!(
        error.failure,
        FailureV1::Coverage {
            selection: 1,
            refusal: RuleRefusalV1::Coordinate
        }
    );
    assert_eq!(error.ordinary_report(), Some(&fixture.ordinary()));
    assert_eq!(additional.get(), twice);
    assert_eq!(
        manager.resource_upper_bound(),
        initial
            .checked_then_retain(complete, Phase::MemoryBounds)
            .unwrap()
    );
    drop(observer);
    drop(phase);
    let observed = receipt.snapshot();
    assert_eq!(observed.first_denial, None);
    assert_eq!(
        observed.current.work_upper_bound(),
        complete.work_upper_bound()
    );
    assert_eq!(
        observed.current.peak_storage_upper_bound(),
        complete.peak_storage_upper_bound()
    );
    assert_eq!(observed.committed, observed.current);
}
