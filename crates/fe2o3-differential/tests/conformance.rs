use fe2o3_differential::{
    AccessKind, AtomicOperation, AtomicScope, AtomicSpec, BackendOutcome, CompileRejection,
    ConformanceOutcome, CopyNonoverlappingSpec, HardwareUnavailable, IntegerSwitchSpec, LayoutSpec,
    MemoryAccess, MemoryOrdering, ObligationSpec, PointerDistanceSpec, ReferenceOutcome,
    ScalarLayout, SemanticCase, SemanticFeature, SemanticObservation, SemanticSpec,
    VolatileOperation, VolatileSpec, classify_semantic_outcome, evaluate_semantic_case,
};

fn case(feature: SemanticFeature, specification: SemanticSpec) -> SemanticCase {
    SemanticCase::new(7, 3, feature, specification).unwrap()
}

#[test]
fn cpu_reference_covers_pointer_and_memory_contracts() {
    let pointer = case(
        SemanticFeature::PointerDistance,
        SemanticSpec::PointerDistance(PointerDistanceSpec {
            allocation_bytes: 64,
            from_offset: 40,
            to_offset: 8,
            element_bytes: 8,
            same_allocation: true,
            signed: true,
        }),
    );
    assert_eq!(
        evaluate_semantic_case(&pointer),
        ReferenceOutcome::Execution(SemanticObservation::Scalar(-4))
    );

    let volatile = case(
        SemanticFeature::VolatileMemory,
        SemanticSpec::Volatile(VolatileSpec {
            words: vec![1, 2, 3],
            index: 1,
            byte_alignment: 4,
            readable: true,
            writable: true,
            operation: VolatileOperation::Store(-9),
        }),
    );
    assert_eq!(
        evaluate_semantic_case(&volatile),
        ReferenceOutcome::Execution(SemanticObservation::Words(vec![1, -9, 3]))
    );

    let copy = case(
        SemanticFeature::CopyNonoverlapping,
        SemanticSpec::CopyNonoverlapping(CopyNonoverlappingSpec {
            words: vec![10, 11, 12, 13, 0, 0, 0, 0],
            source: 1,
            destination: 5,
            count: 3,
        }),
    );
    assert_eq!(
        evaluate_semantic_case(&copy),
        ReferenceOutcome::Execution(SemanticObservation::Words(vec![
            10, 11, 12, 13, 0, 11, 12, 13,
        ]))
    );
}

#[test]
fn cpu_reference_covers_layout_switch_and_atomics() {
    let aggregate = case(
        SemanticFeature::RustLayout,
        SemanticSpec::Layout(LayoutSpec::Aggregate {
            fields: vec![
                ScalarLayout {
                    size: 1,
                    alignment: 1,
                },
                ScalarLayout {
                    size: 8,
                    alignment: 8,
                },
                ScalarLayout {
                    size: 2,
                    alignment: 2,
                },
            ],
        }),
    );
    assert_eq!(
        evaluate_semantic_case(&aggregate),
        ReferenceOutcome::Execution(SemanticObservation::Layout {
            size: 24,
            alignment: 8,
            offsets: vec![0, 8, 16],
        })
    );

    let tagged_enum = case(
        SemanticFeature::RustLayout,
        SemanticSpec::Layout(LayoutSpec::TaggedEnum {
            tag: ScalarLayout {
                size: 1,
                alignment: 1,
            },
            payloads: vec![
                ScalarLayout {
                    size: 4,
                    alignment: 4,
                },
                ScalarLayout {
                    size: 8,
                    alignment: 8,
                },
            ],
        }),
    );
    assert_eq!(
        evaluate_semantic_case(&tagged_enum),
        ReferenceOutcome::Execution(SemanticObservation::Layout {
            size: 16,
            alignment: 8,
            offsets: vec![0, 8],
        })
    );

    let switch = case(
        SemanticFeature::IntegerSwitch,
        SemanticSpec::IntegerSwitch(IntegerSwitchSpec {
            selector: -4,
            arms: vec![(0, 10), (-4, 20), (9, 30)],
            default: 40,
        }),
    );
    assert_eq!(
        evaluate_semantic_case(&switch),
        ReferenceOutcome::Execution(SemanticObservation::Switch {
            arm: Some(1),
            value: 20,
        })
    );

    let atomics = case(
        SemanticFeature::AtomicScopes,
        SemanticSpec::Atomics(AtomicSpec {
            initial: 5,
            scope: AtomicScope::Device,
            operations: vec![
                AtomicOperation::FetchAdd {
                    value: 3,
                    ordering: MemoryOrdering::AcquireRelease,
                },
                AtomicOperation::CompareExchange {
                    current: 8,
                    new: -2,
                    success: MemoryOrdering::SequentiallyConsistent,
                    failure: MemoryOrdering::Acquire,
                },
                AtomicOperation::Load {
                    ordering: MemoryOrdering::Relaxed,
                },
            ],
        }),
    );
    assert_eq!(
        evaluate_semantic_case(&atomics),
        ReferenceOutcome::Execution(SemanticObservation::Atomic {
            observed: vec![5, 8, -2],
            final_value: -2,
        })
    );
}

#[test]
fn unsafe_contracts_have_specific_compile_rejections() {
    let fixtures = [
        (
            case(
                SemanticFeature::PointerDistance,
                SemanticSpec::PointerDistance(PointerDistanceSpec {
                    allocation_bytes: 32,
                    from_offset: 0,
                    to_offset: 8,
                    element_bytes: 4,
                    same_allocation: false,
                    signed: true,
                }),
            ),
            CompileRejection::PointerProvenance,
        ),
        (
            case(
                SemanticFeature::CopyNonoverlapping,
                SemanticSpec::CopyNonoverlapping(CopyNonoverlappingSpec {
                    words: vec![1, 2, 3, 4],
                    source: 0,
                    destination: 1,
                    count: 3,
                }),
            ),
            CompileRejection::CopyOverlap,
        ),
        (
            case(
                SemanticFeature::RustLayout,
                SemanticSpec::Layout(LayoutSpec::NicheEnum {
                    payload: ScalarLayout {
                        size: 8,
                        alignment: 8,
                    },
                }),
            ),
            CompileRejection::UnsupportedNicheLayout,
        ),
        (
            case(
                SemanticFeature::IntegerSwitch,
                SemanticSpec::IntegerSwitch(IntegerSwitchSpec {
                    selector: 1,
                    arms: vec![(1, 2), (1, 3)],
                    default: 4,
                }),
            ),
            CompileRejection::DuplicateSwitchValue,
        ),
        (
            case(
                SemanticFeature::AtomicScopes,
                SemanticSpec::Atomics(AtomicSpec {
                    initial: 0,
                    scope: AtomicScope::System,
                    operations: vec![AtomicOperation::Load {
                        ordering: MemoryOrdering::Relaxed,
                    }],
                }),
            ),
            CompileRejection::UnsupportedAtomicScope,
        ),
        (
            case(
                SemanticFeature::BoundsAndRaces,
                SemanticSpec::Obligation(ObligationSpec::Race {
                    allocation_words: 4,
                    accesses: vec![
                        MemoryAccess {
                            lane: 0,
                            index: 2,
                            kind: AccessKind::Write,
                            atomic: false,
                        },
                        MemoryAccess {
                            lane: 1,
                            index: 2,
                            kind: AccessKind::Read,
                            atomic: false,
                        },
                    ],
                }),
            ),
            CompileRejection::RaceObligation,
        ),
    ];

    for (case, reason) in fixtures {
        assert_eq!(
            evaluate_semantic_case(&case),
            ReferenceOutcome::CompileRejection(reason)
        );
    }
}

#[test]
fn outcomes_are_fail_closed_and_hardware_unavailability_is_not_pass() {
    let accepted = case(
        SemanticFeature::BoundsAndRaces,
        SemanticSpec::Obligation(ObligationSpec::Bounds {
            length: 4,
            index: 3,
        }),
    );
    let expected = SemanticObservation::ObligationsSatisfied;
    assert_eq!(
        classify_semantic_outcome(&accepted, BackendOutcome::Execution(expected)),
        ConformanceOutcome::SupportedPass
    );
    assert!(matches!(
        classify_semantic_outcome(
            &accepted,
            BackendOutcome::CompileRejection(CompileRejection::BoundsObligation)
        ),
        ConformanceOutcome::SemanticMismatch(_)
    ));
    assert_eq!(
        classify_semantic_outcome(
            &accepted,
            BackendOutcome::HardwareUnavailable(HardwareUnavailable::NoCompatibleDevice)
        ),
        ConformanceOutcome::HardwareUnavailable(HardwareUnavailable::NoCompatibleDevice)
    );

    let rejected = case(
        SemanticFeature::BoundsAndRaces,
        SemanticSpec::Obligation(ObligationSpec::Bounds {
            length: 4,
            index: 4,
        }),
    );
    assert_eq!(
        classify_semantic_outcome(
            &rejected,
            BackendOutcome::CompileRejection(CompileRejection::BoundsObligation)
        ),
        ConformanceOutcome::ExpectedCompileRejection
    );
    assert!(matches!(
        classify_semantic_outcome(
            &rejected,
            BackendOutcome::CompileRejection(CompileRejection::RaceObligation)
        ),
        ConformanceOutcome::SemanticMismatch(_)
    ));
}
