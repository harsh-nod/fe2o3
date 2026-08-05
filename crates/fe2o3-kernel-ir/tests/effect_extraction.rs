use fe2o3_kernel_ir::*;

fn pointer(scalar: ScalarType) -> Type {
    Type::pointer(
        Type::Scalar(scalar),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    )
}

fn function(parameters: Vec<Type>, operations: Vec<Operation>) -> Function {
    let parameter_values = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    Function::definition(
        "kernel_impl",
        Signature::new(parameters, vec![]),
        parameter_values,
        vec![block],
    )
}

fn store(pointer: u32, value: u32, access: MemoryAccess) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(pointer),
            value: ValueId(value),
            access,
        },
    )
}

fn region(
    allocation: AllocationIdentity,
    offset: ByteExpression,
    length: ByteExpression,
) -> MemoryRegion {
    MemoryRegion::new(allocation, AddressSpace::Global, offset, length)
}

fn allocation(id: u32) -> AllocationIdentity {
    AllocationId::new(id).into()
}

fn location(index: usize) -> FunctionOperationLocation {
    FunctionOperationLocation::new(BlockId(0), index)
}

fn invocations(count: u64) -> InvocationRange1d {
    InvocationRange1d::from_count(count).unwrap()
}

fn bind(
    bindings: &mut FunctionEffectBindings,
    operation_index: usize,
    pointer: u32,
    region: MemoryRegion,
    count: u64,
) {
    bindings.bind_pointer_region(ValueId(pointer), region);
    bindings.bind_invocations(location(operation_index), invocations(count));
}

#[test]
fn extracts_real_load_and_store_effects_with_derived_widths() {
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let load = Operation::effect_free(
        ValueDef::new(ValueId(3), Type::F32),
        OperationKind::Load {
            pointer: ValueId(0),
            access,
        },
    );
    let function = function(
        vec![pointer(ScalarType::F32), pointer(ScalarType::F32)],
        vec![load, store(1, 3, access)],
    );
    let mut bindings = FunctionEffectBindings::new();
    bind(
        &mut bindings,
        0,
        0,
        region(
            allocation(1),
            ByteExpression::invocation_affine(0, 4),
            ByteExpression::constant(4),
        ),
        32,
    );
    bind(
        &mut bindings,
        1,
        1,
        region(
            allocation(2),
            ByteExpression::invocation_affine(0, 4),
            ByteExpression::constant(4),
        ),
        32,
    );

    let report = extract_function_region_effects(&function, &bindings);
    assert!(report.extraction_issues.is_empty());
    assert_eq!(report.effects.len(), 2);
    assert_eq!(report.effects[0].effect.kind, RegionEffectKind::Read);
    assert_eq!(report.effects[1].effect.kind, RegionEffectKind::Write);
    assert!(report.effects.iter().all(|effect| {
        effect.effect.access_width == 4 && effect.effect.epoch == SynchronizationEpoch::INITIAL
    }));
    assert!(
        report
            .bounds_obligations
            .iter()
            .all(|obligation| obligation.outcome == BoundsObligationOutcome::Proven)
    );
    assert!(report.race_obligations.iter().all(|obligation| matches!(
        obligation.outcome,
        RaceObligationOutcome::NoConflict(NoConflictReason::DisjointRegions)
    )));
}

#[test]
fn access_width_larger_than_the_bound_region_is_a_bounds_violation() {
    let access = MemoryAccess::new(AddressSpace::Global, 8);
    let function = function(
        vec![pointer(ScalarType::F64), Type::F64],
        vec![store(0, 1, access)],
    );
    let mut bindings = FunctionEffectBindings::new();
    bind(
        &mut bindings,
        0,
        0,
        region(
            allocation(1),
            ByteExpression::constant(0),
            ByteExpression::constant(4),
        ),
        1,
    );

    let report = extract_function_region_effects(&function, &bindings);
    assert_eq!(report.effects[0].effect.access_width, 8);
    assert_eq!(
        report.bounds_obligations[0].outcome,
        BoundsObligationOutcome::Violated(BoundsViolation::Region(
            RegionValidationError::AccessExceedsRegion {
                access_width: 8,
                byte_length: 4,
                invocation_index: 0,
            }
        ))
    );
    assert!(matches!(
        report.race_obligations[0].outcome,
        RaceObligationOutcome::Indeterminate(RaceIndeterminateReason::Analysis(_))
    ));
}

#[test]
fn touching_regions_are_disjoint_but_partial_overlap_conflicts() {
    let access = MemoryAccess::new(AddressSpace::Global, 1);
    let function = function(
        vec![
            pointer(ScalarType::U32),
            pointer(ScalarType::U32),
            Type::Scalar(ScalarType::U32),
        ],
        vec![store(0, 2, access), store(1, 2, access)],
    );

    let report_for_second_offset = |second_offset| {
        let mut bindings = FunctionEffectBindings::new();
        bind(
            &mut bindings,
            0,
            0,
            region(
                allocation(1),
                ByteExpression::constant(0),
                ByteExpression::constant(4),
            ),
            2,
        );
        bind(
            &mut bindings,
            1,
            1,
            region(
                allocation(1),
                ByteExpression::constant(second_offset),
                ByteExpression::constant(4),
            ),
            2,
        );
        extract_function_region_effects(&function, &bindings)
    };

    let touching = report_for_second_offset(4);
    assert_eq!(
        touching.race_obligations[1].outcome,
        RaceObligationOutcome::NoConflict(NoConflictReason::DisjointRegions)
    );

    let overlapping = report_for_second_offset(3);
    assert_eq!(
        overlapping.race_obligations[1].outcome,
        RaceObligationOutcome::Conflict(ConflictReason::OverlappingNonAtomicWrite)
    );
}

#[test]
fn reports_conflicts_between_distinct_invocations_of_one_operation() {
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let function = function(
        vec![pointer(ScalarType::U32), Type::Scalar(ScalarType::U32)],
        vec![store(0, 1, access)],
    );
    let mut bindings = FunctionEffectBindings::new();
    bind(
        &mut bindings,
        0,
        0,
        region(
            allocation(1),
            ByteExpression::constant(0),
            ByteExpression::constant(4),
        ),
        64,
    );

    let report = extract_function_region_effects(&function, &bindings);
    assert_eq!(
        report.race_obligations,
        vec![RaceObligation {
            left: location(0),
            right: location(0),
            outcome: RaceObligationOutcome::Conflict(ConflictReason::OverlappingNonAtomicWrite),
        }]
    );
}

#[test]
fn unknown_and_missing_regions_remain_indeterminate() {
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let function = function(
        vec![pointer(ScalarType::U32), Type::Scalar(ScalarType::U32)],
        vec![store(0, 1, access)],
    );

    let mut unknown_bindings = FunctionEffectBindings::new();
    bind(
        &mut unknown_bindings,
        0,
        0,
        region(
            AllocationIdentity::Unknown,
            ByteExpression::constant(0),
            ByteExpression::constant(4),
        ),
        8,
    );
    let unknown = extract_function_region_effects(&function, &unknown_bindings);
    assert_eq!(
        unknown.bounds_obligations[0].outcome,
        BoundsObligationOutcome::Indeterminate(BoundsIndeterminateReason::UnknownAllocation)
    );
    assert!(matches!(
        unknown.race_obligations[0].outcome,
        RaceObligationOutcome::Indeterminate(RaceIndeterminateReason::Conflict(
            ConflictIndeterminateReason::Region(RegionIndeterminateReason::UnknownAllocation)
        ))
    ));

    let mut missing_bindings = FunctionEffectBindings::new();
    missing_bindings.bind_invocations(location(0), invocations(8));
    let missing = extract_function_region_effects(&function, &missing_bindings);
    assert_eq!(
        missing.bounds_obligations[0].outcome,
        BoundsObligationOutcome::Indeterminate(BoundsIndeterminateReason::MissingPointerRegion {
            pointer: ValueId(0)
        })
    );
    assert!(matches!(
        missing.race_obligations[0].outcome,
        RaceObligationOutcome::Indeterminate(RaceIndeterminateReason::Conflict(
            ConflictIndeterminateReason::Region(RegionIndeterminateReason::UnknownAllocation)
        ))
    ));
}

#[test]
fn unbounded_regions_and_address_space_mismatches_fail_closed() {
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let function = function(
        vec![pointer(ScalarType::U32), Type::Scalar(ScalarType::U32)],
        vec![store(0, 1, access)],
    );

    let mut unbounded = FunctionEffectBindings::new();
    bind(
        &mut unbounded,
        0,
        0,
        region(
            allocation(1),
            ByteExpression::Unbounded,
            ByteExpression::constant(4),
        ),
        8,
    );
    let unbounded = extract_function_region_effects(&function, &unbounded);
    assert_eq!(
        unbounded.bounds_obligations[0].outcome,
        BoundsObligationOutcome::Indeterminate(BoundsIndeterminateReason::UnboundedRegion)
    );
    assert!(matches!(
        unbounded.race_obligations[0].outcome,
        RaceObligationOutcome::Indeterminate(RaceIndeterminateReason::Analysis(_))
    ));

    let mut mismatch = FunctionEffectBindings::new();
    let workgroup_region = MemoryRegion::new(
        allocation(1),
        AddressSpace::Workgroup,
        ByteExpression::constant(0),
        ByteExpression::constant(4),
    );
    bind(&mut mismatch, 0, 0, workgroup_region, 1);
    let mismatch = extract_function_region_effects(&function, &mismatch);
    assert_eq!(
        mismatch.bounds_obligations[0].outcome,
        BoundsObligationOutcome::Violated(BoundsViolation::AddressSpaceMismatch {
            region: AddressSpace::Workgroup,
            access: AddressSpace::Global,
        })
    );
}

#[test]
fn incomplete_invocation_mapping_and_overflow_fail_closed() {
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let function = function(
        vec![pointer(ScalarType::U32), Type::Scalar(ScalarType::U32)],
        vec![store(0, 1, access)],
    );
    let overflowing_region = region(
        allocation(1),
        ByteExpression::constant(u64::MAX - 3),
        ByteExpression::constant(4),
    );

    let mut incomplete = FunctionEffectBindings::new();
    incomplete.bind_pointer_region(ValueId(0), overflowing_region.clone());
    let incomplete = extract_function_region_effects(&function, &incomplete);
    assert_eq!(
        incomplete.bounds_obligations[0].outcome,
        BoundsObligationOutcome::Indeterminate(BoundsIndeterminateReason::MissingInvocationMapping)
    );
    assert_eq!(
        incomplete.race_obligations[0].outcome,
        RaceObligationOutcome::Indeterminate(RaceIndeterminateReason::MissingInvocationMapping {
            location: location(0)
        })
    );

    let mut overflowing = FunctionEffectBindings::new();
    bind(&mut overflowing, 0, 0, overflowing_region, 1);
    let overflowing = extract_function_region_effects(&function, &overflowing);
    assert!(matches!(
        overflowing.bounds_obligations[0].outcome,
        BoundsObligationOutcome::Violated(BoundsViolation::Region(
            RegionValidationError::RegionEndOverflow { .. }
        ))
    ));
    assert!(matches!(
        overflowing.race_obligations[0].outcome,
        RaceObligationOutcome::Indeterminate(RaceIndeterminateReason::Analysis(_))
    ));
}

#[test]
fn narrow_atomic_scope_cannot_discharge_a_cross_invocation_obligation() {
    let access = MemoryAccess::new(AddressSpace::Global, 4);
    let atomic = Operation::effect_free(
        ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        OperationKind::Atomic(Atomic {
            kind: AtomicKind::Add,
            pointer: ValueId(0),
            value: Some(ValueId(1)),
            compare: None,
            access,
            scope: SynchronizationScope::Workgroup,
            ordering: MemoryOrdering::Relaxed,
            failure_ordering: None,
        }),
    );
    let function = function(
        vec![pointer(ScalarType::U32), Type::Scalar(ScalarType::U32)],
        vec![atomic],
    );
    let mut bindings = FunctionEffectBindings::new();
    bind(
        &mut bindings,
        0,
        0,
        region(
            allocation(1),
            ByteExpression::constant(0),
            ByteExpression::constant(4),
        ),
        8,
    );

    let report = extract_function_region_effects(&function, &bindings);
    assert_eq!(
        report.bounds_obligations[0].outcome,
        BoundsObligationOutcome::Proven
    );
    assert_eq!(
        report.race_obligations[0].outcome,
        RaceObligationOutcome::Indeterminate(RaceIndeterminateReason::Conflict(
            ConflictIndeterminateReason::AtomicScopeCoverageNotEstablished {
                scope: SynchronizationScope::Workgroup
            }
        ))
    );
}

#[test]
fn unsupported_widths_and_calls_are_reported_as_incomplete() {
    let access = MemoryAccess::new(AddressSpace::Global, 8);
    let call = Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new("unknown_helper"),
            arguments: vec![],
        },
    );
    let function = function(
        vec![pointer(ScalarType::Index), Type::INDEX],
        vec![store(0, 1, access), call],
    );
    let mut bindings = FunctionEffectBindings::new();
    bind(
        &mut bindings,
        0,
        0,
        region(
            allocation(1),
            ByteExpression::constant(0),
            ByteExpression::constant(8),
        ),
        4,
    );

    let report = extract_function_region_effects(&function, &bindings);
    assert_eq!(report.effects, vec![]);
    assert_eq!(
        report.bounds_obligations[0].outcome,
        BoundsObligationOutcome::Indeterminate(BoundsIndeterminateReason::AccessWidthUnavailable)
    );
    assert_eq!(
        report.race_obligations[0].outcome,
        RaceObligationOutcome::Indeterminate(RaceIndeterminateReason::EffectUnavailable {
            location: location(0)
        })
    );
    assert_eq!(
        report.extraction_issues,
        vec![EffectExtractionIssue::CallEffectsUnavailable {
            location: location(1),
            callee: FunctionId::new("unknown_helper"),
        }]
    );
}

#[test]
fn declarations_have_an_explicit_extraction_issue() {
    let function = Function::declaration("external", Signature::new(vec![], vec![]));
    let report = extract_function_region_effects(&function, &FunctionEffectBindings::new());
    assert_eq!(
        report.extraction_issues,
        vec![EffectExtractionIssue::FunctionDeclaration]
    );
    assert!(report.effects.is_empty());
    assert!(report.bounds_obligations.is_empty());
    assert!(report.race_obligations.is_empty());
}
