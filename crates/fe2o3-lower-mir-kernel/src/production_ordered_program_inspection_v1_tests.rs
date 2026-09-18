//! Inspector tests share the ordinary semantic-owner fixture, not a public raw owner.
use super::*;
use ProductionOrderedProgramInspectionAvailabilityV1 as Availability;
use ProductionOrderedProgramInspectionErrorV1 as InspectionError;
use ProductionOrderedProgramInspectionLimitsV1 as Limits;
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1, CanonicalKirFunctionCoordinateV1,
    CanonicalKirOperationCoordinateV1 as Coordinate, Gfx942OrderedProgramRegistersV1,
    Gfx942OrderedProgramV1, TargetCapability, VerifiedCanonicalKernelIrModuleV17,
};

const WORK: usize = 100_000_000;
const STORAGE: usize = 32 * 1024 * 1024;

fn make_owner(fixture: &Fixture) -> ProductionOrderedProgramPreRankedKirOwnerV17 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    fixture.materialize(&mut budget).unwrap()
}
fn owner_floor(owner: &ProductionOrderedProgramPreRankedKirOwnerV17) -> usize {
    17 + owner.executable_storage().retained_storage() + owner.call_correspondence_storage()
}
fn coordinates(owner: &ProductionOrderedProgramPreRankedKirOwnerV17) -> Coordinate {
    for (function, value) in owner.executable().module().functions.iter().enumerate() {
        for (block, value) in value.body.as_ref().unwrap().blocks.iter().enumerate() {
            for (operation, value) in value.operations.iter().enumerate() {
                if matches!(value.kind, OperationKind::Gfx942OrderedProgram(_)) {
                    return Coordinate {
                        block: CanonicalKirBlockCoordinateV1 {
                            function: CanonicalKirFunctionCoordinateV1(function as u32),
                            block: block as u32,
                        },
                        operation: operation as u32,
                    };
                }
            }
        }
    }
    panic!("fixture region absent")
}

// Rebuild inert semantic input through public constructors, then ordinary SSA,
// launch agreement and normal lowering. No executable Module is supplied here.
fn data_owner(data: [SemanticOperandV1; 3]) -> ProductionOrderedProgramPreRankedKirOwnerV17 {
    let fixture = Fixture {
        registers: [1, 5, 9, 17, 63],
        ..Fixture::default()
    };
    let baseline = fixture
        .request()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let old = &baseline.functions()[0];
    let mut blocks = old.blocks().to_vec();
    let SemanticTerminatorKindV1::Call(call) = blocks[0].terminator().kind() else {
        panic!()
    };
    let mut arguments = data.to_vec();
    arguments.extend_from_slice(&call.arguments()[3..]);
    let call = SemanticDirectCallV1::new_callable(
        call.callee(),
        arguments,
        call.destination().cloned(),
        call.unwind(),
    )
    .unwrap()
    .with_ordered_program_source_v32(call.ordered_program_source_v32().unwrap());
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        blocks[0].source(),
        blocks[0].statements().to_vec(),
        SemanticTerminatorV1::new(
            blocks[0].terminator().source(),
            SemanticTerminatorKindV1::Call(call),
        ),
    )
    .unwrap();
    let root = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        old.locals().to_vec(),
        old.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(old.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        baseline.target(),
        baseline.types().to_vec(),
        baseline.allocations().to_vec(),
        baseline.statics().to_vec(),
        baseline.vtables().to_vec(),
        vec![root],
        baseline.callables().to_vec(),
        baseline.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "ordered_program_root",
            [15; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    ProductionOrderedProgramPreRankedKirOwnerV17::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}
fn u32_literal(value: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value.into(), 4).unwrap()),
    ))
}

#[test]
fn inspector_four_normal_owner_modes_retain_exact_plan_and_no_authority() {
    for consume_result in [false, true] {
        for old_marker in [false, true] {
            let owner = make_owner(&Fixture {
                consume_result,
                old_marker,
                ..Fixture::default()
            });
            let before = owner.executable().canonical().canonical_bytes().to_vec();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
            let floor = owner_floor(&owner);
            budget.reserve_storage(floor).unwrap();
            let (view, receipt) = owner
                .inspect_ordered_program_v1(
                    owner.executable().canonical().identity(),
                    None,
                    Limits::default(),
                    &mut budget,
                )
                .unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(receipt.retained_storage(), std::mem::size_of_val(&view));
            assert!(receipt.retained_storage() <= 1024);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(
                view.canonical_identity(),
                owner.executable().canonical().identity()
            );
            assert_eq!(
                view.semantic_sha256(),
                owner.correspondence().semantic_sha256()
            );
            assert_eq!(view.coordinate(), coordinates(&owner));
            assert_eq!(view.semantic_function(), ROOT);
            assert_eq!(view.semantic_block().index(), u32::from(old_marker));
            assert_eq!(
                view.kernel_ir_block(),
                view.terminator_span().kernel_ir_block()
            );
            assert_eq!(
                view.ordered_program().source().frontend_unit,
                source().frontend_unit()
            );
            assert_eq!(
                view.ordered_program().source().function,
                *source().function().as_bytes()
            );
            assert_eq!(
                view.ordered_program().source().contract,
                source().contract()
            );
            assert_eq!(
                view.ordered_program().source().statement,
                source().statement()
            );
            assert_eq!(view.ordered_program().registers().scratch(), 32);
            assert_eq!(view.ordered_program().registers().output(), 33);
            assert_eq!(view.ordered_program().registers().inputs(), [34, 35, 36]);
            assert_eq!(view.ordered_program().registers().vgpr_high_water(), 37);
            assert_eq!(view.declared_program().count(), 3);
            assert_eq!(
                &view.declared_program().descriptors()[..3],
                &[0x0085, 0x0133, 0x019d]
            );
            assert_eq!(
                view.source_provenance(),
                SemanticSourceProvenanceV1::unavailable()
            );
            assert_eq!(view.source_launch(), &owner.source_launch().roots()[0]);
            assert_eq!(view.declared_target(), "gfx942:xnack-");
            assert_eq!(view.declared_wave_width(), WaveWidth::Wave64);
            assert_eq!(view.declared_program(), view.ordered_program().program());
            assert_eq!(
                view.source_association(),
                Availability::RetainedSemanticCorrespondence
            );
            for unavailable in [
                view.physical_values(),
                view.final_artifact(),
                view.source_insertion(),
            ] {
                assert_eq!(unavailable, Availability::Unavailable);
            }
            assert!(!view.authenticates_source());
            assert!(!view.grants_artifact_or_launch_authority());
            assert!(!view.grants_proof_or_resume_authority());
            assert!(!view.can_materialize_helper());
            let actual = &owner.executable().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks[view.coordinate().block.block as usize]
                .operations[view.coordinate().operation as usize];
            assert_eq!(view.ordered_program().result(), actual.results[0].id);
            drop(view);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(owner.executable().canonical().canonical_bytes(), before);
        }
    }
}

#[test]
fn inspector_genuine_constants_repeated_values_and_nondefault_bindings() {
    for (data, constants, repeated) in [
        ([u32_literal(0x1234), input(2), input(3)], 1, false),
        ([u32_literal(1), u32_literal(2), u32_literal(3)], 3, false),
        ([input(1), input(1), input(1)], 0, true),
    ] {
        let owner = data_owner(data);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(owner_floor(&owner)).unwrap();
        let (view, _) = owner
            .inspect_ordered_program_v1(
                owner.executable().canonical().identity(),
                Some(coordinates(&owner)),
                Limits::default(),
                &mut budget,
            )
            .unwrap();
        assert_eq!(view.ordered_program().registers().inputs(), [9, 17, 63]);
        assert_eq!(view.ordered_program().registers().vgpr_high_water(), 64);
        assert_eq!(view.terminator_span().operation_count(), constants + 1);
        assert_eq!(
            view.coordinate().operation,
            view.terminator_span().first_operation_ordinal() + constants
        );
        if repeated {
            assert_eq!(
                view.ordered_program().inputs()[0],
                view.ordered_program().inputs()[1]
            );
            assert_eq!(
                view.ordered_program().inputs()[1],
                view.ordered_program().inputs()[2]
            );
        }
        let body = owner.executable().module().functions[0]
            .body
            .as_ref()
            .unwrap();
        let block = &body.blocks[view.coordinate().block.block as usize];
        for operation in &block.operations[view.terminator_span().first_operation_ordinal() as usize
            ..view.coordinate().operation as usize]
        {
            assert!(matches!(
                operation.kind,
                OperationKind::Constant(Constant::U32(_))
            ));
        }
    }
}

#[test]
fn inspector_refuses_stale_digest_length_and_other_coordinates() {
    let owner = make_owner(&Fixture {
        old_marker: true,
        ..Fixture::default()
    });
    for foreign in [
        make_owner(&Fixture::default()),
        make_owner(&Fixture {
            old_marker: true,
            registers: [1, 5, 9, 17, 63],
            ..Fixture::default()
        }),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let floor = owner_floor(&owner);
        budget.reserve_storage(floor).unwrap();
        assert_eq!(
            owner
                .inspect_ordered_program_v1(
                    foreign.executable().canonical().identity(),
                    None,
                    Limits::default(),
                    &mut budget
                )
                .unwrap_err(),
            InspectionError::StaleIdentity
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), 37);
    }
    let actual = coordinates(&owner);
    for requested in [
        Coordinate {
            block: CanonicalKirBlockCoordinateV1 {
                function: CanonicalKirFunctionCoordinateV1(0),
                block: 0,
            },
            operation: 0,
        },
        Coordinate {
            block: CanonicalKirBlockCoordinateV1 {
                function: CanonicalKirFunctionCoordinateV1(1),
                ..actual.block
            },
            ..actual
        },
        Coordinate {
            block: CanonicalKirBlockCoordinateV1 {
                block: u32::MAX,
                ..actual.block
            },
            ..actual
        },
        Coordinate {
            operation: u32::MAX,
            ..actual
        },
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let floor = owner_floor(&owner);
        budget.reserve_storage(floor).unwrap();
        assert_eq!(
            owner
                .inspect_ordered_program_v1(
                    owner.executable().canonical().identity(),
                    Some(requested),
                    Limits::default(),
                    &mut budget
                )
                .unwrap_err(),
            InspectionError::WrongCoordinate
        );
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn inspector_rejects_missing_ambiguous_overflow_and_foreign_correspondence() {
    for corruption in 0..9 {
        let mut owner = make_owner(&Fixture::default());
        let spans = &mut owner.correspondence.terminator_operation_spans;
        let selected = spans
            .iter()
            .position(|row| row.operation_count != 0)
            .unwrap();
        match corruption {
            0 => *spans = Box::new([]),
            1 => {
                let mut rows = spans.to_vec();
                rows.push(rows[selected]);
                *spans = rows.into_boxed_slice();
            }
            2 => spans[selected].kernel_ir_block = BlockId(900),
            3 => spans[selected].semantic_block = SemanticBlockIdV1::from_index(900),
            4 => {
                spans[selected].first_operation_ordinal = u32::MAX;
                spans[selected].operation_count = 2;
            }
            5 => spans[selected].operation_count = u32::MAX,
            6 => {
                owner.correspondence.lowered_functions[0].semantic_function =
                    SemanticFunctionIdV1::from_index(900)
            }
            7 => {
                let row = owner.correspondence.lowered_functions[0].clone();
                owner.correspondence.lowered_functions = vec![row.clone(), row].into_boxed_slice();
            }
            8 => spans[selected].correspondence_owner = SemanticFunctionIdV1::from_index(900),
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let floor = owner_floor(&owner);
        budget.reserve_storage(floor).unwrap();
        assert_eq!(
            owner
                .inspect_ordered_program_v1(
                    owner.executable().canonical().identity(),
                    None,
                    Limits::default(),
                    &mut budget
                )
                .unwrap_err(),
            InspectionError::InvalidCorrespondence,
            "case {corruption}"
        );
        assert_eq!(budget.storage(), floor);
    }
}

// Deliberate test-only corruption of the private composite. Reverification keeps
// canonical custody real; it does not legitimize the substituted source join.
fn change_executable(
    owner: &mut ProductionOrderedProgramPreRankedKirOwnerV17,
    change: impl FnOnce(&mut Module),
) {
    let mut module = owner.executable().module().clone();
    change(&mut module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    let (executable, receipt) =
        VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
            &module,
            &mut budget,
        )
        .unwrap();
    owner.executable = executable;
    owner.executable_storage = receipt;
}

#[test]
fn inspector_rejects_substituted_source_or_physical_plan() {
    for wrong_source in [false, true] {
        let mut owner = make_owner(&Fixture::default());
        change_executable(&mut owner, |module| {
            for operation in &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations {
                if let OperationKind::Gfx942OrderedProgram(region) = operation.kind {
                    let mut source = region.source();
                    let registers = if wrong_source {
                        source.contract = [177; 32];
                        region.registers()
                    } else {
                        Gfx942OrderedProgramRegistersV1::new(1, 5, [9, 17, 63]).unwrap()
                    };
                    operation.kind = OperationKind::Gfx942OrderedProgram(
                        Gfx942OrderedProgramV1::new(
                            source,
                            registers,
                            *region.inputs(),
                            *region.program(),
                        )
                        .unwrap(),
                    );
                }
            }
        });
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let floor = owner_floor(&owner);
        budget.reserve_storage(floor).unwrap();
        assert_eq!(
            owner
                .inspect_ordered_program_v1(
                    owner.executable().canonical().identity(),
                    None,
                    Limits::default(),
                    &mut budget
                )
                .unwrap_err(),
            InspectionError::InvalidCorrespondence
        );
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn inspector_rejects_reverified_program_count_or_descriptor_substitution() {
    for change_count in [false, true] {
        let mut owner = make_owner(&Fixture::default());
        change_executable(&mut owner, |module| {
            for operation in &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations {
                if let OperationKind::Gfx942OrderedProgram(payload) = &operation.kind {
                    let mut descriptors = *payload.program().descriptors();
                    let count = if change_count {
                        // Same output, but an additional retained dead move.
                        4
                    } else {
                        // OR rather than AND in the middle step; same count.
                        descriptors[1] = 0x0134;
                        3
                    };
                    let replacement =
                        fe2o3_kernel_ir::Gfx942U32ProgramV1::from_descriptors(count, descriptors)
                            .unwrap();
                    operation.kind = OperationKind::Gfx942OrderedProgram(
                        Gfx942OrderedProgramV1::new(
                            payload.source(),
                            payload.registers(),
                            *payload.inputs(),
                            replacement,
                        )
                        .unwrap(),
                    );
                }
            }
        });
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let floor = owner_floor(&owner);
        budget.reserve_storage(floor).unwrap();
        // Use the genuine replacement canonical identity: failure must come
        // from source/program correspondence, not a stale expected digest.
        assert_eq!(
            owner
                .inspect_ordered_program_v1(
                    owner.executable().canonical().identity(),
                    None,
                    Limits::default(),
                    &mut budget
                )
                .unwrap_err(),
            InspectionError::InvalidCorrespondence
        );
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn inspector_reports_exact_one_and_sixteen_step_programs() {
    for count in [1, 16] {
        let program = bounded_program(count);
        let owner = make_owner(&Fixture {
            program,
            ..Fixture::default()
        });
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let floor = owner_floor(&owner);
        budget.reserve_storage(floor).unwrap();
        let (view, receipt) = owner
            .inspect_ordered_program_v1(
                owner.executable().canonical().identity(),
                Some(coordinates(&owner)),
                Limits::default(),
                &mut budget,
            )
            .unwrap();
        assert_eq!(view.declared_program().count(), count);
        assert_eq!(view.declared_program().descriptors(), program.descriptors());
        assert_eq!(view.ordered_program().program(), view.declared_program());
        assert_eq!(view.physical_values(), Availability::Unavailable);
        assert!(!view.authenticates_source());
        assert_eq!(receipt.retained_storage(), std::mem::size_of_val(&view));
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn inspector_refuses_missing_wave_wrong_target_and_wrong_region_roster() {
    for corruption in 0..8 {
        let mut owner = make_owner(&Fixture::default());
        change_executable(&mut owner, |module| {
            if corruption < 6 {
                let declarations = match corruption / 2 {
                    0 => &mut module.required_capabilities,
                    1 => &mut module.kernels[0].required_capabilities,
                    2 => &mut module.functions[0].required_capabilities,
                    _ => unreachable!(),
                };
                if corruption % 2 == 0 {
                    declarations.remove(&TargetCapability::WaveWidth(WaveWidth::Wave64));
                } else {
                    declarations.insert(TargetCapability::Extension {
                        namespace: fe2o3_kernel_ir::AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.into(),
                        name: "gfx950:xnack-".into(),
                    });
                }
            } else {
                let operations =
                    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
                let index = operations
                    .iter()
                    .position(|operation| {
                        matches!(operation.kind, OperationKind::Gfx942OrderedProgram(_))
                    })
                    .unwrap();
                if corruption == 6 {
                    let OperationKind::Gfx942OrderedProgram(region) = operations[index].kind else {
                        unreachable!()
                    };
                    operations[index].kind = OperationKind::Binary {
                        op: BinaryOp::BitXor,
                        lhs: region.inputs()[0],
                        rhs: region.inputs()[1],
                    };
                } else {
                    let mut second = operations[index].clone();
                    second.results[0].id = ValueId(900);
                    operations.push(second);
                }
            }
        });
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let floor = owner_floor(&owner);
        budget.reserve_storage(floor).unwrap();
        let error = owner
            .inspect_ordered_program_v1(
                owner.executable().canonical().identity(),
                None,
                Limits::default(),
                &mut budget,
            )
            .unwrap_err();
        assert_eq!(
            error,
            if corruption < 6 {
                InspectionError::UnsupportedProfile
            } else {
                InspectionError::MissingOrAmbiguousRegion
            },
            "case {corruption}"
        );
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn inspector_prerequisite_rejects_conflicting_waves_and_wrong_actual_input_type() {
    // These graphs cannot reach inspection: preserve the existing canonical
    // verifier's earlier rejection instead of manufacturing an unchecked owner.
    for corruption in 0..4 {
        let owner = make_owner(&Fixture::default());
        let mut module = owner.executable().module().clone();
        match corruption {
            0 => {
                module
                    .required_capabilities
                    .insert(TargetCapability::WaveWidth(WaveWidth::Wave32));
            }
            1 => {
                module.kernels[0]
                    .required_capabilities
                    .insert(TargetCapability::WaveWidth(WaveWidth::Wave32));
            }
            2 => {
                module.functions[0]
                    .required_capabilities
                    .insert(TargetCapability::WaveWidth(WaveWidth::Wave32));
            }
            3 => {
                module.functions[0].signature.parameters[0] = Type::F32;
            }
            _ => unreachable!(),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(17).unwrap();
        assert!(
            matches!(
                VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
                    &module,
                    &mut budget
                ),
                Err(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV17::Verification(_))
            ),
            "case {corruption}"
        );
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn inspector_does_not_attribute_an_adjacent_old_marker_to_the_region() {
    let mut owner = make_owner(&Fixture {
        old_marker: true,
        ..Fixture::default()
    });
    let row = owner
        .correspondence
        .terminator_operation_spans
        .iter_mut()
        .find(|row| row.semantic_block.index() == 1)
        .unwrap();
    row.semantic_block = SemanticBlockIdV1::from_index(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    let floor = owner_floor(&owner);
    budget.reserve_storage(floor).unwrap();
    assert_eq!(
        owner
            .inspect_ordered_program_v1(
                owner.executable().canonical().identity(),
                None,
                Limits::default(),
                &mut budget
            )
            .unwrap_err(),
        InspectionError::InvalidCorrespondence
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn inspector_raw_block_ids_are_not_roster_ordinals() {
    let mut owner = make_owner(&Fixture::default());
    change_executable(&mut owner, |module| {
        for block in &mut module.functions[0].body.as_mut().unwrap().blocks {
            block.id.0 += 500;
            if let Some(Terminator::Branch { target, .. }) = &mut block.terminator {
                target.0 += 500;
            }
        }
    });
    for span in &mut owner.correspondence.terminator_operation_spans {
        span.kernel_ir_block.0 += 500;
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(owner_floor(&owner)).unwrap();
    let (view, _) = owner
        .inspect_ordered_program_v1(
            owner.executable().canonical().identity(),
            None,
            Limits::default(),
            &mut budget,
        )
        .unwrap();
    assert_eq!(view.coordinate().block.block, 0);
    assert_eq!(view.kernel_ir_block(), BlockId(500));
    let wrong = Coordinate {
        block: CanonicalKirBlockCoordinateV1 {
            block: 500,
            ..view.coordinate().block
        },
        ..view.coordinate()
    };
    assert_eq!(
        owner
            .inspect_ordered_program_v1(
                owner.executable().canonical().identity(),
                Some(wrong),
                Limits::default(),
                &mut budget
            )
            .unwrap_err(),
        InspectionError::WrongCoordinate
    );
}

// An independent sum of the documented schedule, never inferred from a query's
// observed work. The variable contributions come from the immutable input only.
fn expected_work(owner: &ProductionOrderedProgramPreRankedKirOwnerV17) -> usize {
    let module = owner.executable().module();
    let function = &module.functions[0];
    let body = function.body.as_ref().unwrap();
    let operations: usize = body.blocks.iter().map(|b| b.operations.len()).sum();
    let definitions = body.parameters.len()
        + body
            .blocks
            .iter()
            .map(|b| {
                b.parameters.len() + b.operations.iter().map(|o| o.results.len()).sum::<usize>()
            })
            .sum::<usize>();
    let declarations: usize = [
        &module.required_capabilities,
        &module.kernels[0].required_capabilities,
        &function.required_capabilities,
    ]
    .into_iter()
    .flat_map(|s| s.iter())
    .map(|c| {
        4 + match c {
            TargetCapability::Extension { namespace, name } => {
                2 * (namespace.len() + name.len()) + 128
            }
            _ => 0,
        }
    })
    .sum();
    let mapped_names: usize = owner
        .correspondence
        .lowered_functions()
        .iter()
        .map(|row| row.kernel_ir_function().as_str().len() + function.id.as_str().len() + 1)
        .sum();
    let coordinate = coordinates(owner);
    let raw = body.blocks[coordinate.block.block as usize].id;
    let span = owner
        .correspondence
        .terminator_operation_spans()
        .iter()
        .find(|span| {
            span.kernel_ir_block() == raw
                && span.first_operation_ordinal() <= coordinate.operation
                && coordinate.operation < span.first_operation_ordinal() + span.operation_count()
        })
        .unwrap();
    4 + 33
        + 112
        + 40
        + module.functions.len()
        + body.blocks.len()
        + operations
        + function.id.as_str().len()
        + module.kernels[0].entry.as_str().len()
        + 1
        + declarations
        + 1
        + 7 * definitions
        + body.blocks.len()
        + operations
        + 4096
        + mapped_names
        + 3
        + 9 * owner.correspondence.terminator_operation_spans().len()
        + 4096
        + span.operation_count() as usize
        + 4
}

#[test]
fn inspector_exact_work_local_global_and_storage_boundaries() {
    let owner = make_owner(&Fixture::default());
    let expected = expected_work(&owner);
    let floor = owner_floor(&owner);
    let bytes = std::mem::size_of::<ProductionOrderedProgramInspectionV1<'_>>();
    for (work_limit, local, storage, expected_error) in [
        (expected, expected, floor + bytes, 0),
        (expected - 1, expected, floor + bytes, 1),
        (expected, expected - 1, floor + bytes, 2),
        (expected, expected, floor + bytes - 1, 3),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage);
        budget.reserve_storage(floor).unwrap();
        let result = owner.inspect_ordered_program_v1(
            owner.executable().canonical().identity(),
            None,
            Limits { max_work: local },
            &mut budget,
        );
        match expected_error {
            0 => {
                assert!(result.is_ok());
                assert_eq!(budget.work(), expected);
                assert_eq!(budget.peak_storage(), floor + bytes);
            }
            1 => assert!(matches!(
                result,
                Err(InspectionError::Resource(ArgumentResourceV1::Work(_)))
            )),
            2 => assert!(matches!(result, Err(InspectionError::WorkLimit { .. }))),
            3 => {
                assert!(matches!(
                    result,
                    Err(InspectionError::Resource(ArgumentResourceV1::Storage(_)))
                ));
                assert_eq!(budget.failed_storage(), Some(floor + bytes));
            }
            _ => unreachable!(),
        }
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn inspector_cumulative_second_query_and_prior_failures_are_preserved() {
    let owner = make_owner(&Fixture::default());
    let expected = expected_work(&owner);
    let floor = owner_floor(&owner);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(2 * expected - 1);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let (view, receipt) = owner
        .inspect_ordered_program_v1(
            owner.executable().canonical().identity(),
            None,
            Limits::default(),
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    drop(view);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert!(matches!(
        owner.inspect_ordered_program_v1(
            owner.executable().canonical().identity(),
            None,
            Limits::default(),
            &mut budget
        ),
        Err(InspectionError::Resource(ArgumentResourceV1::Work(_)))
    ));
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() >= expected);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    {
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(budget.charge_work(WORK + 1).is_err());
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let first_failure = budget.failed_storage();
        let (view, _) = owner
            .inspect_ordered_program_v1(
                owner.executable().canonical().identity(),
                None,
                Limits::default(),
                &mut budget,
            )
            .unwrap();
        drop(view);
        assert_eq!(budget.failed_storage(), first_failure);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), expected);
    }
    assert_eq!(work.failed_work(), Some(WORK + 1));
}

#[test]
fn inspector_invalid_limits_and_missing_input_receipts_fail_without_floor_loss() {
    let owner = make_owner(&Fixture::default());
    for max_work in [
        0,
        MAX_PRODUCTION_ORDERED_PROGRAM_INSPECTION_WORK_V1 + 1,
        usize::MAX,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(17).unwrap();
        assert_eq!(
            owner
                .inspect_ordered_program_v1(
                    owner.executable().canonical().identity(),
                    None,
                    Limits { max_work },
                    &mut budget
                )
                .unwrap_err(),
            InspectionError::InvalidLimit
        );
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.work(), 0);
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(17).unwrap();
    assert_eq!(
        owner
            .inspect_ordered_program_v1(
                owner.executable().canonical().identity(),
                None,
                Limits::default(),
                &mut budget
            )
            .unwrap_err(),
        InspectionError::Resource(ArgumentResourceV1::Accounting)
    );
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.work(), 4);
}
