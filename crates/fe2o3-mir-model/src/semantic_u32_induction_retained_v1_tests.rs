// Original fixture definitions below are copied byte-for-byte; new controls follow.
use super::*;
use crate::semantic_mir_v1::{
    InertSemanticMirRequestV1, SemanticAbiExtensionV1, SemanticAbiIdentityV1,
    SemanticAbiPassModeV1, SemanticAbiRegularAttributesV1, SemanticAbiValueAttributesV1,
    SemanticAbiValueV1, SemanticAggregateLayoutV1, SemanticAggregateTypeV1,
    SemanticBackendPrimitiveV1, SemanticBackendReprV1, SemanticBackendScalarV1, SemanticCanonAbiV1,
    SemanticConstGenericArgumentsIdentityV1, SemanticConstantV1, SemanticConstantValueV1,
    SemanticControlFlowEdgeV1, SemanticEdgeRoleV1, SemanticFunctionAbiV1, SemanticFunctionRoleV1,
    SemanticGenericTypeArgumentsIdentityV1, SemanticItemDefinitionIdentityV1,
    SemanticLayoutIdentityV1, SemanticLocalDeclV1, SemanticMirLimitsV1,
    SemanticMonomorphizationIdentityV1, SemanticPaddingV1, SemanticRvalueV1,
    SemanticScalarValidityRangeV1, SemanticSourceProvenanceV1, SemanticStatementV1,
    SemanticSwitchTargetV1, SemanticSwitchTargetsV1, SemanticTargetDataLayoutV1,
    SemanticTerminatorV1, SemanticTypeIdentityV1, SemanticTypeLayoutV1,
};

const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const CHECKED_U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const BOUND: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(1);
const INDUCTION: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(2);
const PREDICATE: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(3);
const CHECKED_RESULT: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(4);

#[derive(Clone, Copy)]
struct Shape {
    step: u128,
    expected_overflow: bool,
    bound_is_argument: bool,
    extra_induction_definition: bool,
    alias_induction: bool,
    guard_snapshot: bool,
    guard_snapshot_extra_use: bool,
    mutate_assert_operands: bool,
    identity_seed: u8,
    dead_predecessor: bool,
    dead_definitions: bool,
}

impl Default for Shape {
    fn default() -> Self {
        Self {
            step: 1,
            expected_overflow: false,
            bound_is_argument: true,
            extra_induction_definition: false,
            alias_induction: false,
            guard_snapshot: false,
            guard_snapshot_extra_use: false,
            mutate_assert_operands: false,
            identity_seed: 0,
            dead_predecessor: false,
            dead_definitions: false,
        }
    }
}

fn identity(seed: u8, tag: u8) -> [u8; 32] {
    [seed.wrapping_add(tag); 32]
}

fn scalar_layout(
    size: u64,
    alignment: u64,
    primitive: SemanticBackendPrimitiveV1,
    maximum: u128,
) -> SemanticTypeLayoutV1 {
    SemanticTypeLayoutV1::new_with_backend_repr(
        Some(size),
        alignment,
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            primitive,
            SemanticScalarValidityRangeV1::new(0, maximum),
        )),
        false,
    )
    .unwrap()
}

fn types(seed: u8) -> Vec<SemanticTypeDeclV1> {
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(seed, 1)),
            SemanticLayoutIdentityV1::from_sha256(identity(seed, 2)),
            scalar_layout(
                4,
                4,
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                u128::from(u32::MAX),
            ),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(seed, 3)),
            SemanticLayoutIdentityV1::from_sha256(identity(seed, 4)),
            scalar_layout(1, 1, SemanticBackendPrimitiveV1::integer(false, 8, 1), 1),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(identity(seed, 5)),
            SemanticLayoutIdentityV1::from_sha256(identity(seed, 6)),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                4,
                SemanticAggregateLayoutV1::new(
                    vec![0, 4],
                    vec![SemanticPaddingV1::new(5, 3).unwrap()],
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, BOOL]).unwrap()),
        ),
    ]
}

fn direct_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn place(local: SemanticLocalIdV1, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(local, vec![], ty).unwrap()
}

fn field(local: SemanticLocalIdV1, field: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        local,
        vec![
            crate::semantic_mir_v1::SemanticProjectionV1::new(
                SemanticProjectionKindV1::Field(field),
                ty,
            )
            .unwrap(),
        ],
        ty,
    )
    .unwrap()
}

fn copy(local: SemanticLocalIdV1, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}

fn constant(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}

fn assignment(
    destination: SemanticPlaceV1,
    ty: SemanticTypeIdV1,
    kind: SemanticRvalueKindV1,
) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(
        crate::semantic_mir_v1::SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(ty, kind),
        ),
    ))
}

fn block(
    seed: u8,
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(identity(seed, tag)),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

fn admitted(shape: Shape) -> AdmittedInertSemanticMirV1 {
    let seed = shape.identity_seed;
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity(seed, tag)),
            ty,
            role,
            SemanticSourceProvenanceV1::unavailable(),
        )
    };
    let mut locals = vec![
        local(20, U32, SemanticLocalRoleV1::Return),
        local(
            21,
            U32,
            if shape.bound_is_argument {
                SemanticLocalRoleV1::Argument(0)
            } else {
                SemanticLocalRoleV1::Temporary
            },
        ),
        local(22, U32, SemanticLocalRoleV1::Temporary),
        local(23, BOOL, SemanticLocalRoleV1::Temporary),
        local(24, CHECKED_U32, SemanticLocalRoleV1::Temporary),
    ];
    let alias = if shape.alias_induction || shape.guard_snapshot || shape.guard_snapshot_extra_use {
        let alias = SemanticLocalIdV1::from_index(locals.len() as u32);
        locals.push(local(25, U32, SemanticLocalRoleV1::Temporary));
        Some(alias)
    } else {
        None
    };
    let snapshot_sink = if shape.guard_snapshot_extra_use {
        let sink = SemanticLocalIdV1::from_index(locals.len() as u32);
        locals.push(local(26, U32, SemanticLocalRoleV1::Temporary));
        Some(sink)
    } else {
        None
    };

    let mut preheader = vec![assignment(
        place(INDUCTION, U32),
        U32,
        SemanticRvalueKindV1::Use(constant(0)),
    )];
    if shape.extra_induction_definition {
        preheader.push(assignment(
            place(INDUCTION, U32),
            U32,
            SemanticRvalueKindV1::Use(constant(0)),
        ));
    }
    if shape.alias_induction {
        let alias = alias.expect("stale alias local");
        preheader.push(assignment(
            place(alias, U32),
            U32,
            SemanticRvalueKindV1::Use(copy(INDUCTION, U32)),
        ));
    }
    let mut header = Vec::new();
    if shape.guard_snapshot || shape.guard_snapshot_extra_use {
        let alias = alias.expect("header snapshot local");
        header.push(assignment(
            place(alias, U32),
            U32,
            SemanticRvalueKindV1::Use(copy(INDUCTION, U32)),
        ));
        if let Some(sink) = snapshot_sink {
            header.push(assignment(
                place(sink, U32),
                U32,
                SemanticRvalueKindV1::Use(copy(alias, U32)),
            ));
        }
    }
    let guard_induction = alias.filter(|_| {
        shape.alias_induction || shape.guard_snapshot || shape.guard_snapshot_extra_use
    });
    let guard = assignment(
        place(PREDICATE, BOOL),
        BOOL,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left: copy(guard_induction.unwrap_or(INDUCTION), U32),
            right: copy(BOUND, U32),
        },
    );
    header.push(guard);
    let checked = assignment(
        place(CHECKED_RESULT, CHECKED_U32),
        CHECKED_U32,
        SemanticRvalueKindV1::CheckedBinary(
            crate::semantic_mir_v1::SemanticCheckedBinaryRvalueV1::new(
                SemanticCheckedBinaryOpV1::Add,
                copy(INDUCTION, U32),
                constant(shape.step),
            ),
        ),
    );
    let asserted_right = if shape.mutate_assert_operands {
        constant(shape.step.saturating_add(1))
    } else {
        constant(shape.step)
    };
    let mut blocks = vec![
        block(
            seed,
            30,
            preheader,
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(
            seed,
            31,
            header,
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: copy(PREDICATE, BOOL),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 4),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(
            seed,
            32,
            vec![checked],
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Copy(field(CHECKED_RESULT, 1, BOOL)),
                expected: shape.expected_overflow,
                message: SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Add,
                    left: copy(INDUCTION, U32),
                    right: asserted_right,
                },
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 3),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(
            seed,
            33,
            vec![assignment(
                place(INDUCTION, U32),
                U32,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(CHECKED_RESULT, 0, U32))),
            )],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(seed, 34, vec![], SemanticTerminatorKindV1::Return),
    ];
    if shape.dead_predecessor || shape.dead_definitions {
        let statements = if shape.dead_definitions {
            vec![
                assignment(
                    place(INDUCTION, U32),
                    U32,
                    SemanticRvalueKindV1::Use(constant(99)),
                ),
                assignment(
                    place(BOUND, U32),
                    U32,
                    SemanticRvalueKindV1::Use(copy(INDUCTION, U32)),
                ),
                assignment(
                    place(CHECKED_RESULT, CHECKED_U32),
                    CHECKED_U32,
                    SemanticRvalueKindV1::CheckedBinary(
                        crate::semantic_mir_v1::SemanticCheckedBinaryRvalueV1::new(
                            SemanticCheckedBinaryOpV1::Add,
                            copy(INDUCTION, U32),
                            constant(2),
                        ),
                    ),
                ),
                assignment(
                    field(CHECKED_RESULT, 0, U32),
                    U32,
                    SemanticRvalueKindV1::Use(constant(9)),
                ),
            ]
        } else {
            vec![]
        };
        blocks.push(block(
            seed,
            35,
            statements,
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ));
    }
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256(identity(seed, 40)),
        SemanticLayoutIdentityV1::from_sha256(identity(seed, 41)),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![direct_value(U32)],
        direct_value(U32),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(identity(seed, 42)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(identity(seed, 43)),
        SemanticMonomorphizationIdentityV1::from_sha256(identity(seed, 44)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(identity(seed, 45)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(identity(seed, 46)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(
            seed, 47,
        ))),
        types(seed),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn report(admitted: &AdmittedInertSemanticMirV1) -> SemanticU32InductionNoOverflowReportV1 {
    analyze_semantic_u32_induction_no_overflow_v1(admitted, SemanticFunctionIdV1::from_index(0))
        .unwrap()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Denied {
    Work,
    Storage,
    Injected,
}
#[derive(Default)]
struct Meter {
    work: usize,
    storage: usize,
    work_calls: usize,
    storage_calls: usize,
    work_limit: Option<usize>,
    storage_limit: Option<usize>,
    deny_work: Option<usize>,
    deny_storage: Option<usize>,
    panic_work: Option<usize>,
    panic_storage: Option<usize>,
    failed: Option<Denied>,
    after_denial: usize,
    first_storage: Option<usize>,
}
impl SemanticU32InductionBoundSnapshotMeterV1 for Meter {
    type Error = Denied;
    fn charge_work(&mut self, amount: usize) -> Result<(), Denied> {
        if let Some(e) = self.failed {
            self.after_denial += 1;
            return Err(e);
        }
        self.work_calls += 1;
        if self.panic_work == Some(self.work_calls) {
            std::panic::panic_any(());
        }
        let next = self.work.checked_add(amount).unwrap();
        let error = if self.deny_work == Some(self.work_calls) {
            Some(Denied::Injected)
        } else if self.work_limit.is_some_and(|limit| next > limit) {
            Some(Denied::Work)
        } else {
            None
        };
        if let Some(e) = error {
            self.failed = Some(e);
            return Err(e);
        }
        self.work = next;
        Ok(())
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Denied> {
        if let Some(e) = self.failed {
            self.after_denial += 1;
            return Err(e);
        }
        self.storage_calls += 1;
        self.first_storage.get_or_insert(amount);
        if self.panic_storage == Some(self.storage_calls) {
            std::panic::panic_any(());
        }
        let next = self.storage.checked_add(amount).unwrap();
        let error = if self.deny_storage == Some(self.storage_calls) {
            Some(Denied::Injected)
        } else if self.storage_limit.is_some_and(|limit| next > limit) {
            Some(Denied::Storage)
        } else {
            None
        };
        if let Some(e) = error {
            self.failed = Some(e);
            return Err(e);
        }
        self.storage = next;
        Ok(())
    }
}
fn prepare<'s>(
    owner: &mut RetainedSemanticU32InductionV1<'s, Denied>,
    source: &'s AdmittedInertSemanticMirV1,
    meter: &mut Meter,
) -> Result<(), SemanticU32InductionRetainedFailureV1> {
    owner.prepare_into(
        source,
        SemanticFunctionIdV1::from_index(0),
        SemanticU32InductionAnalysisLimitsV1::default(),
        meter,
    )
}
fn state<E>(owner: &RetainedSemanticU32InductionV1<'_, E>) -> Vec<(usize, usize, usize)> {
    // Test inspection allocations are outside component meter claims.
    fn row<T>(v: &Vec<T>) -> (usize, usize, usize) {
        (v.as_ptr() as usize, v.len(), v.capacity())
    }
    let p = &owner.payload;
    let mut rows = vec![
        row(&p.graph.successors),
        row(&p.graph.predecessors),
        row(&p.inventory.definitions),
        row(&p.inventory.address_or_projection_hazard),
        row(&p.inventory.direct_copy_alias),
        row(&p.inventory.use_counts),
        row(&p.inventory.checked_additions),
        row(&p.reaches),
        row(&p.certificates),
    ];
    for v in &p.graph.successors {
        rows.push(row(v));
    }
    for v in &p.graph.predecessors {
        rows.push(row(v));
    }
    for reach in &p.reaches {
        rows.push(row(&reach.visited));
        rows.push(row(&reach.pending));
    }
    rows
}
#[test]
fn complete_reports_match_original_and_strict_metered_oracles() {
    for shape in [
        Shape::default(),
        Shape {
            guard_snapshot: true,
            ..Shape::default()
        },
        Shape {
            step: 2,
            ..Shape::default()
        },
        Shape {
            expected_overflow: true,
            ..Shape::default()
        },
        Shape {
            step: 0,
            ..Shape::default()
        },
        Shape {
            extra_induction_definition: true,
            ..Shape::default()
        },
        Shape {
            alias_induction: true,
            ..Shape::default()
        },
        Shape {
            guard_snapshot: true,
            guard_snapshot_extra_use: true,
            ..Shape::default()
        },
        Shape {
            mutate_assert_operands: true,
            ..Shape::default()
        },
        Shape {
            identity_seed: 17,
            ..Shape::default()
        },
    ] {
        let source = admitted(shape);
        let expected = report(&source);
        let mut original_meter = Meter::default();
        assert_eq!(
            analyze_semantic_u32_induction_no_overflow_with_meter_v1(
                &source,
                SemanticFunctionIdV1::from_index(0),
                SemanticU32InductionAnalysisLimitsV1::default(),
                &mut original_meter
            )
            .unwrap(),
            expected
        );
        let mut owner = RetainedSemanticU32InductionV1::new();
        let mut meter = Meter::default();
        prepare(&mut owner, &source, &mut meter).unwrap();
        let actual = owner
            .completed_for(&source, SemanticFunctionIdV1::from_index(0))
            .unwrap();
        assert_eq!(actual, &expected);
        assert_eq!(owner.work_units_observed(), expected.work_units());
        assert!(!actual.grants_authority() && !actual.authorizes_compiler_transform());
        assert!(owner.failure().is_none());
        assert!(meter.work >= original_meter.work);
        assert!(meter.storage > original_meter.storage);
        assert!(owner.payload.reaches.len() >= 1);
        assert_eq!(
            owner.payload.inventory.checked_additions.len(),
            expected.checked_additions_examined()
        );
        if shape.step == 1
            && !shape.expected_overflow
            && !shape.alias_induction
            && !shape.extra_induction_definition
            && !shape.guard_snapshot_extra_use
            && !shape.mutate_assert_operands
        {
            assert!(!actual.certificates().is_empty());
            assert!(owner.payload.reaches.len() > 1);
        }
    }
}
#[test]
fn original_local_limits_absent_source_and_unreachable_errors_are_exact() {
    let source = admitted(Shape::default());
    let function = SemanticFunctionIdV1::from_index(0);
    let work = report(&source).work_units();
    for limits in [
        SemanticU32InductionAnalysisLimitsV1::new(0, 10),
        SemanticU32InductionAnalysisLimitsV1::new(work - 1, 10),
        SemanticU32InductionAnalysisLimitsV1::new(work, 0),
        SemanticU32InductionAnalysisLimitsV1::new(MAX_SEMANTIC_U32_INDUCTION_WORK_V1 + 1, 10),
        SemanticU32InductionAnalysisLimitsV1::new(
            work,
            MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1 + 1,
        ),
    ] {
        let expected =
            analyze_semantic_u32_induction_no_overflow_with_limits_v1(&source, function, limits)
                .unwrap_err();
        let mut owner = RetainedSemanticU32InductionV1::new();
        assert!(
            owner
                .prepare_into(&source, function, limits, &mut Meter::default())
                .is_err()
        );
        assert_eq!(owner.failure(), Some(&Saved::Analysis(expected)));
        assert!(owner.completed_for(&source, function).is_err());
    }
    let absent = SemanticFunctionIdV1::from_index(99);
    let limits = SemanticU32InductionAnalysisLimitsV1::new(usize::MAX, usize::MAX);
    let expected =
        analyze_semantic_u32_induction_no_overflow_with_limits_v1(&source, absent, limits)
            .unwrap_err();
    let mut owner = RetainedSemanticU32InductionV1::new();
    assert!(
        owner
            .prepare_into(&source, absent, limits, &mut Meter::default())
            .is_err()
    );
    assert_eq!(owner.failure(), Some(&Saved::Analysis(expected)));
    let dead = admitted(Shape {
        dead_predecessor: true,
        ..Shape::default()
    });
    let expected = analyze_semantic_u32_induction_no_overflow_v1(&dead, function).unwrap_err();
    let mut owner = RetainedSemanticU32InductionV1::new();
    assert!(prepare(&mut owner, &dead, &mut Meter::default()).is_err());
    assert_eq!(owner.failure(), Some(&Saved::Analysis(expected)));
    assert_eq!(
        owner.payload.graph.successors.len(),
        dead.functions()[0].blocks().len()
    );
    assert_eq!(owner.payload.reaches.len(), 1);
    assert!(owner.payload.inventory.definitions.is_empty());
}
#[test]
fn exact_external_limits_and_each_one_short_preserve_original_prefix() {
    let source = admitted(Shape::default());
    let mut measured = Meter::default();
    let mut baseline = RetainedSemanticU32InductionV1::new();
    prepare(&mut baseline, &source, &mut measured).unwrap();
    for short in [None, Some(Denied::Work), Some(Denied::Storage)] {
        let mut meter = Meter {
            work: 7,
            storage: 11,
            work_limit: Some(7 + measured.work - usize::from(short == Some(Denied::Work))),
            storage_limit: Some(
                11 + measured.storage - usize::from(short == Some(Denied::Storage)),
            ),
            ..Default::default()
        };
        let mut owner = RetainedSemanticU32InductionV1::new();
        let result = prepare(&mut owner, &source, &mut meter);
        if let Some(e) = short {
            assert!(result.is_err());
            assert_eq!(owner.failure(), Some(&Saved::Meter(e)));
            assert!(
                owner
                    .completed_for(&source, SemanticFunctionIdV1::from_index(0))
                    .is_err()
            );
        } else {
            result.unwrap();
            assert_eq!(
                (meter.work, meter.storage),
                (7 + measured.work, 11 + measured.storage)
            );
        }
        assert!(meter.work >= 7 && meter.storage >= 11);
        assert_eq!(meter.after_denial, 0);
    }
}
#[test]
fn every_resource_call_denial_retains_physical_partial_owners_and_forbids_retry() {
    let source = admitted(Shape::default());
    let mut measured = Meter::default();
    prepare(
        &mut RetainedSemanticU32InductionV1::new(),
        &source,
        &mut measured,
    )
    .unwrap();
    let mut saw_graph = false;
    let mut saw_inventory = false;
    let mut saw_dominance = false;
    let mut saw_certificates = false;
    for storage in [false, true] {
        for call in 1..=if storage {
            measured.storage_calls
        } else {
            measured.work_calls
        } {
            let mut meter = Meter {
                work: 7,
                storage: 11,
                deny_work: (!storage).then_some(call),
                deny_storage: storage.then_some(call),
                ..Default::default()
            };
            let mut owner = RetainedSemanticU32InductionV1::new();
            assert!(prepare(&mut owner, &source, &mut meter).is_err());
            assert_eq!(owner.failure(), Some(&Saved::Meter(Denied::Injected)));
            assert_eq!(meter.after_denial, 0);
            saw_graph |= owner
                .payload
                .graph
                .successors
                .iter()
                .any(|v| v.capacity() > 0);
            saw_inventory |= owner.payload.inventory.definitions.capacity() > 0;
            saw_dominance |= owner.payload.reaches.len() > 1;
            saw_certificates |= owner.payload.certificates.capacity() > 0;
            let before = state(&owner);
            let ledger = (
                meter.work,
                meter.storage,
                meter.work_calls,
                meter.storage_calls,
            );
            let semantic_work = owner.work_units_observed();
            assert!(prepare(&mut owner, &source, &mut meter).is_err());
            assert_eq!(state(&owner), before);
            assert_eq!(owner.work_units_observed(), semantic_work);
            assert_eq!(
                (
                    meter.work,
                    meter.storage,
                    meter.work_calls,
                    meter.storage_calls
                ),
                ledger
            );
            drop(owner); // Original caller alone may refund after its postflight.
            assert_eq!(
                (
                    meter.work,
                    meter.storage,
                    meter.work_calls,
                    meter.storage_calls
                ),
                ledger
            );
        }
    }
    assert!(saw_graph && saw_inventory && saw_dominance && saw_certificates);
}
#[test]
fn every_injected_unwind_keeps_partial_payload_and_saved_semantic_counter() {
    let source = admitted(Shape::default());
    let mut measured = Meter::default();
    prepare(
        &mut RetainedSemanticU32InductionV1::new(),
        &source,
        &mut measured,
    )
    .unwrap();
    let mut saw_late = false;
    for storage in [false, true] {
        for call in 1..=if storage {
            measured.storage_calls
        } else {
            measured.work_calls
        } {
            let mut meter = Meter {
                work: 7,
                storage: 11,
                panic_work: (!storage).then_some(call),
                panic_storage: storage.then_some(call),
                ..Default::default()
            };
            let mut owner = RetainedSemanticU32InductionV1::new();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                prepare(&mut owner, &source, &mut meter)
            }));
            assert!(outcome.is_err());
            drop(outcome); // Panic payload conversion is not a retained-payload claim.
            assert_eq!(owner.phase, Phase::Terminal);
            assert!(owner.payload.report.is_none());
            if owner.payload.reaches.len() > 1 {
                saw_late = true;
                assert!(owner.work_units_observed() > 0);
                assert!(!owner.payload.inventory.definitions.is_empty());
            }
            let before = state(&owner);
            let credits = (
                meter.work,
                meter.storage,
                meter.work_calls,
                meter.storage_calls,
            );
            assert!(prepare(&mut owner, &source, &mut meter).is_err());
            assert_eq!(state(&owner), before);
            drop(owner);
            assert_eq!(
                (
                    meter.work,
                    meter.storage,
                    meter.work_calls,
                    meter.storage_calls
                ),
                credits
            );
        }
    }
    assert!(saw_late);
}
#[test]
fn completed_loan_rejects_equal_detached_source_and_wrong_function() {
    let source = admitted(Shape::default());
    let detached = admitted(Shape::default());
    assert_eq!(source.semantic_sha256(), detached.semantic_sha256());
    let mut meter = Meter::default();
    let mut owner = RetainedSemanticU32InductionV1::new();
    prepare(&mut owner, &source, &mut meter).unwrap();
    assert!(
        owner
            .completed_for(&source, SemanticFunctionIdV1::from_index(0))
            .is_ok()
    );
    assert!(
        owner
            .completed_for(&detached, SemanticFunctionIdV1::from_index(0))
            .is_err()
    );
    assert!(
        owner
            .completed_for(&source, SemanticFunctionIdV1::from_index(1))
            .is_err()
    );
    let credits = (meter.work, meter.storage);
    let before = state(&owner);
    assert!(prepare(&mut owner, &source, &mut meter).is_err());
    assert_eq!(state(&owner), before);
    assert_eq!((meter.work, meter.storage), credits);
}
#[test]
fn retained_reachability_matches_all_original_complete_graph_queries() {
    let source = admitted(Shape::default());
    let mut old_work = WorkBudgetV1::new(MAX_SEMANTIC_U32_INDUCTION_WORK_V1);
    let graph =
        SemanticCfgV1::analyze(&source.functions()[0], false, None, &mut old_work, &mut 0).unwrap();
    let mut reaches = Vec::new();
    let mut meter = Meter::default();
    let mut failure = None;
    for avoided in [None, Some(0), Some(1), Some(2), Some(3), Some(4)] {
        let mut old = WorkBudgetV1::new(MAX_SEMANTIC_U32_INDUCTION_WORK_V1);
        let expected = graph.reachable_avoiding(avoided, &mut old).unwrap();
        let mut adapter = Adapter {
            original: &mut meter,
            failure: &mut failure,
        };
        let mut actual = WorkBudgetV1 {
            used: 0,
            limit: MAX_SEMANTIC_U32_INDUCTION_WORK_V1,
            meter: Some(&mut adapter),
        };
        assert_eq!(
            reachable_retained(&graph, avoided, &mut reaches, &mut actual).unwrap(),
            expected
        );
        assert_eq!(actual.used, old.used);
        assert_eq!(reaches.last().unwrap().avoided, avoided);
    }
    assert_eq!(reaches.len(), 6);
    assert!(reaches[1].pending.is_empty());
    assert_eq!(reaches[1].pending.capacity(), 0);
}
#[test]
fn box_admission_denial_cannot_consume_attached_certificate_vector() {
    let source = admitted(Shape::default());
    let certificate = report(&source).certificates()[0];
    let mut payload = Payload::new();
    payload.certificates.try_reserve_exact(3).unwrap();
    payload.certificates.push(certificate); // Independent helper prestate, not paid driver coverage.
    let before = (
        payload.certificates.as_ptr(),
        payload.certificates.len(),
        payload.certificates.capacity(),
    );
    let mut meter = Meter {
        storage_limit: Some(0),
        ..Default::default()
    };
    let mut failure = None;
    let mut adapter = Adapter {
        original: &mut meter,
        failure: &mut failure,
    };
    let mut budget = WorkBudgetV1 {
        used: 0,
        limit: 1000,
        meter: Some(&mut adapter),
    };
    assert!(prepay_box(&payload.certificates, &mut budget).is_err());
    assert_eq!(
        (
            payload.certificates.as_ptr(),
            payload.certificates.len(),
            payload.certificates.capacity()
        ),
        before
    );
    assert_eq!(payload.certificates, vec![certificate]);
}
#[test]
fn full_noncopy_meter_error_stays_owned_until_component_drop() {
    use std::{cell::Cell, rc::Rc};
    struct OwnedError {
        dropped: Rc<Cell<usize>>,
        bytes: Box<[u8; 64]>,
    }
    impl Drop for OwnedError {
        fn drop(&mut self) {
            self.dropped.set(self.dropped.get() + 1);
        }
    }
    struct ErrorMeter {
        error: Option<OwnedError>,
    }
    impl SemanticU32InductionBoundSnapshotMeterV1 for ErrorMeter {
        type Error = OwnedError;
        fn charge_work(&mut self, _: usize) -> Result<(), OwnedError> {
            Ok(())
        }
        fn reserve_storage(&mut self, _: usize) -> Result<(), OwnedError> {
            Err(self.error.take().unwrap())
        }
    }
    let source = admitted(Shape::default());
    let dropped = Rc::new(Cell::new(0));
    // Opaque error payload is preowned by this test meter, not funded by the component header.
    let mut meter = ErrorMeter {
        error: Some(OwnedError {
            dropped: dropped.clone(),
            bytes: Box::new([23; 64]),
        }),
    };
    let mut owner = RetainedSemanticU32InductionV1::new();
    assert!(
        owner
            .prepare_into(
                &source,
                SemanticFunctionIdV1::from_index(0),
                SemanticU32InductionAnalysisLimitsV1::default(),
                &mut meter
            )
            .is_err()
    );
    assert!(
        matches!(owner.failure(), Some(Saved::Meter(error)) if error.bytes.as_ref() == &[23;64])
    );
    assert_eq!(dropped.get(), 0);
    drop(meter);
    assert_eq!(dropped.get(), 0);
    drop(owner);
    assert_eq!(dropped.get(), 1);
}
#[test]
fn prior_sticky_denial_and_typed_frames_are_separate_from_original_donor() {
    let source = admitted(Shape::default());
    let rows = retained_frame_rows::<Meter>();
    assert_eq!(rows.len(), RETAINED_FRAME_ROWS);
    assert_eq!(
        retained_frame::<Meter>().unwrap(),
        strict_resources::frame_storage_v1::<Meter>().unwrap() + rows.iter().sum::<usize>()
    );
    for failed in [Denied::Work, Denied::Storage] {
        let mut meter = Meter {
            work: 7,
            storage: 11,
            failed: Some(failed),
            ..Default::default()
        };
        let mut owner = RetainedSemanticU32InductionV1::new();
        assert!(prepare(&mut owner, &source, &mut meter).is_err());
        assert_eq!(owner.failure(), Some(&Saved::Meter(failed)));
        assert_eq!((meter.work, meter.storage, meter.after_denial), (7, 11, 1));
        assert!(owner.payload.graph.successors.is_empty());
    }
}
#[test]
fn external_observer_error_or_panic_does_not_drop_completed_payload_early() {
    let source = admitted(Shape::default());
    for panic in [false, true] {
        let mut owner = RetainedSemanticU32InductionV1::new();
        let mut meter = Meter::default();
        prepare(&mut owner, &source, &mut meter).unwrap();
        let before = state(&owner);
        let credits = (meter.work, meter.storage);
        let outcome =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), ()> {
                let report = owner
                    .completed_for(&source, SemanticFunctionIdV1::from_index(0))
                    .unwrap();
                assert!(!report.certificates().is_empty());
                if panic {
                    std::panic::panic_any(());
                }
                Err(())
            }));
        assert!(matches!(&outcome, Ok(Err(()))) || outcome.is_err());
        drop(outcome);
        assert_eq!(state(&owner), before);
        assert_eq!((meter.work, meter.storage), credits);
        drop(owner);
        assert_eq!((meter.work, meter.storage), credits);
    }
}
#[test]
fn source_contract_has_no_refund_and_retains_original_stage_order() {
    let source = include_str!("semantic_u32_induction_retained_v1.rs");
    for forbidden in [
        "release_storage(",
        "catch_unwind(",
        "unsafe {",
        "WorkBudgetV1::new(",
    ] {
        assert!(
            !source.contains(forbidden),
            "unexpected component operation {forbidden}"
        );
    }
    let body = &source[source.find("impl Payload {\n    fn prepare(").unwrap()..];
    let mut previous = 0;
    for stage in [
        "graph_into(declaration",
        "inventory_into(declaration",
        "&mut self.certificates,",
        "for candidate in &self.inventory.checked_additions",
        "prepay_box(&self.certificates",
        "std::mem::take(&mut self.certificates)",
        "self.report = Some",
    ] {
        let index = body.find(stage).unwrap();
        assert!(index >= previous);
        previous = index;
    }
    assert_eq!(
        source
            .matches("dominates_retained(graph, reaches, ")
            .count(),
        4
    );
}
