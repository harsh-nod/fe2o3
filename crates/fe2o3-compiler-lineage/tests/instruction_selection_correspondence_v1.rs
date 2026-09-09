use fe2o3_compiler_lineage::{
    CompilerBranchDivergenceV1, CompilerInstructionSelectionCorrespondenceErrorV1,
    CompilerInstructionSelectionCorrespondencePartsV1,
    CompilerInstructionSelectionCorrespondenceV1, CompilerLlvmBlockV1,
    CompilerLlvmOperationCoordinateV1, CompilerLlvmOperationKindV1, CompilerLlvmOperationV1,
    CompilerLlvmValueTypeV1, CompilerMachineRegisterClassV1, CompilerMachineRegisterV1,
    CompilerMachineValueLocationV1, CompilerMemoryEffectKindV1, CompilerMemoryEffectV1,
    CompilerMemoryOrderingV1, CompilerMemoryScopeV1, CompilerNumericalContractV1,
    CompilerPhiEdgeTransportV1, CompilerPhiInputV1, ExactCompilerStageContentIdentityV1,
    MachineRefinementArchitectureV1, PostLlvmStageCustodyV1,
    check_compiler_emitted_instruction_selection_correspondence_v1,
    check_exact_post_llvm_stage_contents_v1,
};

const POST: &[u8] = b"BC\xc0\xde-post-optimization";
const OBJECT: &[u8] = b"generated-object";
const HSACO: &[u8] = b"final-hsaco";

fn effect(
    kind: CompilerMemoryEffectKindV1,
    ordering: CompilerMemoryOrderingV1,
    scope: CompilerMemoryScopeV1,
) -> CompilerMemoryEffectV1 {
    CompilerMemoryEffectV1::new(kind, 1, 4, 4, ordering, scope, false).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn operation(
    block: u32,
    ordinal: u32,
    kind: CompilerLlvmOperationKindV1,
    result: Option<u32>,
    operands: &[u32],
    phi: &[CompilerPhiInputV1],
    successors: &[u32],
    divergence: CompilerBranchDivergenceV1,
    memory: CompilerMemoryEffectV1,
    numerical: CompilerNumericalContractV1,
    offsets: &[u64],
) -> CompilerLlvmOperationV1 {
    let phi_result = CompilerMachineRegisterV1::new(
        CompilerMachineRegisterClassV1::Vector,
        result.unwrap_or_default() as u16,
    );
    let phi_transports = phi
        .iter()
        .map(|input| {
            CompilerPhiEdgeTransportV1::new(
                input.predecessor(),
                input.value(),
                CompilerMachineValueLocationV1::Register(phi_result),
                phi_result,
                None,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    CompilerLlvmOperationV1::new(
        CompilerLlvmOperationCoordinateV1::new(0, block, ordinal),
        100 + ordinal as u16,
        kind,
        0,
        result,
        result.map_or(CompilerLlvmValueTypeV1::Void, |_| {
            CompilerLlvmValueTypeV1::Float(32)
        }),
        operands,
        phi,
        phi_transports,
        successors,
        divergence,
        memory,
        numerical,
        offsets,
        [],
        None,
    )
    .unwrap()
}

fn parts() -> CompilerInstructionSelectionCorrespondencePartsV1 {
    let blocks = vec![
        CompilerLlvmBlockV1::new(0, 0, [1]).unwrap(),
        CompilerLlvmBlockV1::new(0, 1, [2, 3]).unwrap(),
        CompilerLlvmBlockV1::new(0, 2, [1]).unwrap(),
        CompilerLlvmBlockV1::new(0, 3, []).unwrap(),
    ];
    let operations = vec![
        operation(
            0,
            0,
            CompilerLlvmOperationKindV1::Constant,
            Some(0),
            &[],
            &[],
            &[],
            CompilerBranchDivergenceV1::None,
            CompilerMemoryEffectV1::none(),
            CompilerNumericalContractV1::Exact,
            &[],
        ),
        operation(
            0,
            1,
            CompilerLlvmOperationKindV1::Branch,
            None,
            &[],
            &[],
            &[1],
            CompilerBranchDivergenceV1::None,
            CompilerMemoryEffectV1::none(),
            CompilerNumericalContractV1::Exact,
            &[100],
        ),
        operation(
            1,
            0,
            CompilerLlvmOperationKindV1::Phi,
            Some(1),
            &[],
            &[CompilerPhiInputV1::new(0, 0), CompilerPhiInputV1::new(2, 3)],
            &[],
            CompilerBranchDivergenceV1::None,
            CompilerMemoryEffectV1::none(),
            CompilerNumericalContractV1::Exact,
            &[],
        ),
        operation(
            1,
            1,
            CompilerLlvmOperationKindV1::Load,
            Some(2),
            &[1],
            &[],
            &[],
            CompilerBranchDivergenceV1::None,
            effect(
                CompilerMemoryEffectKindV1::Read,
                CompilerMemoryOrderingV1::NotAtomic,
                CompilerMemoryScopeV1::None,
            ),
            CompilerNumericalContractV1::Exact,
            &[104],
        ),
        operation(
            1,
            2,
            CompilerLlvmOperationKindV1::ConditionalBranch,
            None,
            &[2],
            &[],
            &[2, 3],
            CompilerBranchDivergenceV1::Divergent,
            CompilerMemoryEffectV1::none(),
            CompilerNumericalContractV1::Exact,
            &[108, 112],
        ),
        operation(
            2,
            0,
            CompilerLlvmOperationKindV1::FloatAdd,
            Some(3),
            &[2, 1],
            &[],
            &[],
            CompilerBranchDivergenceV1::None,
            CompilerMemoryEffectV1::none(),
            CompilerNumericalContractV1::IeeeBinary32,
            &[116],
        ),
        operation(
            2,
            1,
            CompilerLlvmOperationKindV1::Store,
            None,
            &[1, 3],
            &[],
            &[],
            CompilerBranchDivergenceV1::None,
            effect(
                CompilerMemoryEffectKindV1::Write,
                CompilerMemoryOrderingV1::NotAtomic,
                CompilerMemoryScopeV1::None,
            ),
            CompilerNumericalContractV1::Exact,
            &[120],
        ),
        operation(
            2,
            2,
            CompilerLlvmOperationKindV1::Branch,
            None,
            &[],
            &[],
            &[1],
            CompilerBranchDivergenceV1::None,
            CompilerMemoryEffectV1::none(),
            CompilerNumericalContractV1::Exact,
            &[124],
        ),
        operation(
            3,
            0,
            CompilerLlvmOperationKindV1::Return,
            None,
            &[],
            &[],
            &[],
            CompilerBranchDivergenceV1::None,
            CompilerMemoryEffectV1::none(),
            CompilerNumericalContractV1::Exact,
            &[128],
        ),
    ];
    CompilerInstructionSelectionCorrespondencePartsV1 {
        architecture: MachineRefinementArchitectureV1::AmdGcn,
        compiler_occurrence_identity: [1; 32],
        worker_request_identity: [2; 32],
        worker_response_identity: [3; 32],
        post_optimization_bitcode: ExactCompilerStageContentIdentityV1::calculate(POST).unwrap(),
        generated_object: ExactCompilerStageContentIdentityV1::calculate(OBJECT).unwrap(),
        final_code_object: ExactCompilerStageContentIdentityV1::calculate(HSACO).unwrap(),
        blocks: blocks.into_boxed_slice(),
        operations: operations.into_boxed_slice(),
    }
}

#[test]
fn canonical_round_trip_retains_cfg_phi_backedge_memory_and_machine_offsets() {
    let transcript = CompilerInstructionSelectionCorrespondenceV1::from_parts(parts()).unwrap();
    let decoded = CompilerInstructionSelectionCorrespondenceV1::decode_canonical(
        transcript.canonical_bytes(),
    )
    .unwrap();
    assert_eq!(decoded, transcript);
    assert_eq!(decoded.blocks().len(), 4);
    assert_eq!(decoded.operations().len(), 9);
    assert_eq!(decoded.operations()[2].phi_inputs().len(), 2);
    assert_eq!(decoded.operations()[4].machine_offsets(), [108, 112]);
    assert!(!decoded.proves_instruction_selection_semantics());
}

#[test]
fn operation_reorder_and_omission_fail_closed() {
    let mut reordered = parts();
    reordered.operations.swap(0, 1);
    assert_eq!(
        CompilerInstructionSelectionCorrespondenceV1::from_parts(reordered).unwrap_err(),
        CompilerInstructionSelectionCorrespondenceErrorV1::NonCanonical
    );

    let mut omitted = parts();
    omitted.operations = omitted.operations[..omitted.operations.len() - 1]
        .to_vec()
        .into_boxed_slice();
    assert_eq!(
        CompilerInstructionSelectionCorrespondenceV1::from_parts(omitted).unwrap_err(),
        CompilerInstructionSelectionCorrespondenceErrorV1::InvalidControlFlow
    );
}

#[test]
fn phi_predecessor_and_machine_offset_substitution_fail_closed() {
    let mut phi = parts();
    phi.operations[2] = operation(
        1,
        0,
        CompilerLlvmOperationKindV1::Phi,
        Some(1),
        &[],
        &[CompilerPhiInputV1::new(0, 0), CompilerPhiInputV1::new(3, 3)],
        &[],
        CompilerBranchDivergenceV1::None,
        CompilerMemoryEffectV1::none(),
        CompilerNumericalContractV1::Exact,
        &[],
    );
    assert_eq!(
        CompilerInstructionSelectionCorrespondenceV1::from_parts(phi).unwrap_err(),
        CompilerInstructionSelectionCorrespondenceErrorV1::InvalidControlFlow
    );

    let mut duplicate = parts();
    duplicate.operations[5] = operation(
        2,
        0,
        CompilerLlvmOperationKindV1::FloatAdd,
        Some(3),
        &[2, 1],
        &[],
        &[],
        CompilerBranchDivergenceV1::None,
        CompilerMemoryEffectV1::none(),
        CompilerNumericalContractV1::IeeeBinary32,
        &[104],
    );
    assert_eq!(
        CompilerInstructionSelectionCorrespondenceV1::from_parts(duplicate).unwrap_err(),
        CompilerInstructionSelectionCorrespondenceErrorV1::InvalidMachineMapping
    );
}

#[test]
fn phi_edge_transport_rejects_predecessor_location_and_move_substitutions() {
    let result = CompilerMachineRegisterV1::new(CompilerMachineRegisterClassV1::Vector, 1);
    let incoming = CompilerMachineValueLocationV1::Register(result);
    let transport = CompilerPhiEdgeTransportV1::new(0, 0, incoming, result, None).unwrap();
    let build = |phi_inputs: &[CompilerPhiInputV1],
                 transports: &[CompilerPhiEdgeTransportV1],
                 offsets: &[u64]| {
        CompilerLlvmOperationV1::new(
            CompilerLlvmOperationCoordinateV1::new(0, 1, 0),
            100,
            CompilerLlvmOperationKindV1::Phi,
            0,
            Some(1),
            CompilerLlvmValueTypeV1::Float(32),
            [],
            phi_inputs,
            transports,
            [],
            CompilerBranchDivergenceV1::None,
            CompilerMemoryEffectV1::none(),
            CompilerNumericalContractV1::Exact,
            offsets,
            [],
            None,
        )
    };

    assert_eq!(
        build(&[CompilerPhiInputV1::new(2, 0)], &[transport], &[]).unwrap_err(),
        CompilerInstructionSelectionCorrespondenceErrorV1::InvalidOperation
    );

    let other = CompilerMachineRegisterV1::new(CompilerMachineRegisterClassV1::Vector, 2);
    assert_eq!(
        CompilerPhiEdgeTransportV1::new(
            0,
            0,
            CompilerMachineValueLocationV1::Register(other),
            result,
            None,
        )
        .unwrap_err(),
        CompilerInstructionSelectionCorrespondenceErrorV1::InvalidValueBinding
    );

    let moved = CompilerPhiEdgeTransportV1::new(
        0,
        0,
        CompilerMachineValueLocationV1::Register(other),
        result,
        Some(80),
    )
    .unwrap();
    assert_eq!(
        build(&[CompilerPhiInputV1::new(0, 0)], &[moved], &[]).unwrap_err(),
        CompilerInstructionSelectionCorrespondenceErrorV1::InvalidValueBinding
    );
    assert!(build(&[CompilerPhiInputV1::new(0, 0)], &[moved], &[80]).is_ok());
}

#[test]
fn unknown_operation_tag_and_trailing_bytes_are_rejected() {
    let transcript = CompilerInstructionSelectionCorrespondenceV1::from_parts(parts()).unwrap();
    let mut unknown = transcript.canonical_bytes().to_vec();
    let marker = 100_u16.to_le_bytes();
    let opcode = unknown
        .windows(marker.len())
        .position(|window| window == marker)
        .unwrap();
    unknown[opcode + marker.len()] = u8::MAX;
    assert_eq!(
        CompilerInstructionSelectionCorrespondenceV1::decode_canonical(&unknown).unwrap_err(),
        CompilerInstructionSelectionCorrespondenceErrorV1::UnknownTag
    );

    let mut trailing = transcript.canonical_bytes().to_vec();
    trailing.push(0);
    assert_eq!(
        CompilerInstructionSelectionCorrespondenceV1::decode_canonical(&trailing).unwrap_err(),
        CompilerInstructionSelectionCorrespondenceErrorV1::NonCanonical
    );
}

#[test]
fn checked_boundary_requires_exact_occurrence_and_all_three_artifacts() {
    let record = PostLlvmStageCustodyV1::from_exact_stage_bytes(
        b"lowered llvm",
        b"BC\xc0\xde-pre",
        POST,
        OBJECT,
        HSACO,
        "llvm-build",
        [],
    )
    .unwrap();
    let stages = check_exact_post_llvm_stage_contents_v1(
        record,
        b"lowered llvm".as_slice(),
        b"BC\xc0\xde-pre".as_slice(),
        POST,
        OBJECT,
        HSACO,
    )
    .unwrap();
    let transcript = CompilerInstructionSelectionCorrespondenceV1::from_parts(parts()).unwrap();
    let checked = check_compiler_emitted_instruction_selection_correspondence_v1(
        &stages,
        transcript.canonical_bytes(),
        [1; 32],
        [2; 32],
        [3; 32],
    )
    .unwrap();
    assert!(checked.retains_required_machine_refinement_input());
    assert!(!checked.proves_target_instruction_semantics());

    assert_eq!(
        check_compiler_emitted_instruction_selection_correspondence_v1(
            &stages,
            transcript.canonical_bytes(),
            [9; 32],
            [2; 32],
            [3; 32],
        )
        .unwrap_err(),
        CompilerInstructionSelectionCorrespondenceErrorV1::OccurrenceMismatch
    );
}
