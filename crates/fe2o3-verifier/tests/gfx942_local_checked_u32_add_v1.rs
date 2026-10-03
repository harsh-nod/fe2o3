#![cfg(target_os = "linux")]

use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineAnalysisExecutionV1, AuthenticatedPhysicalMachineEffectLimitsV1,
    AuthenticatedPhysicalMachineEffectWorkerV1, GFX942_ORDINARY_SGPR_COUNT_V1,
    Gfx942ReachingDefinitionV1, Gfx942ScalarIntegerStateV1, Gfx942U32OriginV1,
    PhysicalMachineEffectBudgetV1, PhysicalMachineEffectEntryRequestV1,
    inspect_physical_machine_effect_worker_candidate_v1,
};
use fe2o3_kernel_ir::decode_module_v8;
use fe2o3_verifier::{
    Gfx942CapturedMovPrefixCheckedU32AddErrorV1, Gfx942LocalCheckedU32AddErrorV1,
    Gfx942LocalMovPrefixCheckedU32AddErrorV1, check_gfx942_captured_mov_prefix_checked_u32_add_v1,
    check_gfx942_local_checked_u32_add_v1, check_gfx942_local_mov_prefix_checked_u32_add_v1,
};
use std::{path::Path, time::Duration};

#[path = "support/local_checked_u32_add.rs"]
mod source_fixture;

const SYNTHETIC_S_ADD_PAYLOAD: [u8; 8] = [0x23, 0x81, 0x00, 0x80, 0x00, 0x00, 0x81, 0xbf];
const SYNTHETIC_MOV_ADD_PAYLOAD: [u8; 12] = [
    0x24, 0x00, 0x85, 0xbe, 0x05, 0x81, 0x00, 0x80, 0x00, 0x00, 0x81, 0xbf,
];

fn test_worker_execution(
    symbol: &str,
    payload: Vec<u8>,
) -> AuthenticatedPhysicalMachineAnalysisExecutionV1 {
    // Authenticates execution of this TEST binary, not the production LLVM analyzer.
    let path = Path::new(env!(
        "CARGO_BIN_EXE_fe2o3-verifier-machine-effect-worker-fixture"
    ));
    let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        Duration::from_secs(30),
        1024 * 1024,
        16 * 1024,
    )
    .unwrap();
    let candidate = inspect_physical_machine_effect_worker_candidate_v1(path, limits).unwrap();
    let worker =
        AuthenticatedPhysicalMachineEffectWorkerV1::open(path, candidate.policy(), limits).unwrap();
    worker
        .analyze(
            payload,
            vec![
                PhysicalMachineEffectEntryRequestV1::new(
                    symbol,
                    PhysicalMachineEffectBudgetV1::new(0, 0, 0, 1, 0),
                )
                .unwrap(),
            ],
            limits,
        )
        .unwrap()
}

#[test]
fn public_entry_borrows_exact_source_and_authenticated_test_worker_owners() {
    let inputs = source_fixture::source_inputs();
    let module = decode_module_v8(inputs.kernel_ir().canonical_bytes()).unwrap();
    let symbol = module
        .functions
        .iter()
        .find(|f| f.body.is_some())
        .unwrap()
        .id
        .as_str();
    let execution = test_worker_execution(symbol, SYNTHETIC_S_ADD_PAYLOAD.to_vec());
    let obligation = check_gfx942_local_checked_u32_add_v1(&inputs, &execution, 0, 0).unwrap();
    assert!(std::ptr::eq(obligation.inputs(), &inputs));
    assert!(std::ptr::eq(obligation.execution(), &execution));
    assert_eq!(obligation.function_symbol(), symbol);
    assert_eq!(obligation.literal(), 1);
    assert_eq!(
        obligation.anchor(),
        inputs.semantic_u32_induction_kir_anchors()[0]
    );
    let hypotheses = obligation.unresolved_entry_relation();
    assert_eq!(hypotheses.semantic_local, 1);
    assert_eq!(hypotheses.machine_source_sgpr, 35);
    assert_eq!(hypotheses.machine_instruction_offset, 0);
    assert_eq!(
        obligation.machine_input_definition(),
        Gfx942ReachingDefinitionV1::LiveIn
    );
    let conditional = obligation.conditional_results();
    assert_eq!(conditional.semantic_result_local, 4);
    assert_eq!(
        conditional.kernel_ir_value,
        obligation.anchor().value_result()
    );
    assert_eq!(
        conditional.kernel_ir_overflow,
        obligation.anchor().overflow_result()
    );
    assert_eq!(conditional.machine_destination_sgpr, 0);
    assert_eq!(
        obligation.machine_operation().encoding(),
        &SYNTHETIC_S_ADD_PAYLOAD[..4]
    );
    // No fixture supplies a MIR-local/SSA/SGPR equality or source-to-payload derivation.
    // The public return is a borrowed conditional obligation, not an accepted authority.
}

#[test]
fn public_entry_rejects_substituted_sites_and_encoding_without_consuming_owners() {
    let inputs = source_fixture::source_inputs();
    let module = decode_module_v8(inputs.kernel_ir().canonical_bytes()).unwrap();
    let symbol = module
        .functions
        .iter()
        .find(|f| f.body.is_some())
        .unwrap()
        .id
        .as_str();
    let execution = test_worker_execution(symbol, SYNTHETIC_S_ADD_PAYLOAD.to_vec());
    assert!(matches!(
        check_gfx942_local_checked_u32_add_v1(&inputs, &execution, 1, 0),
        Err(Gfx942LocalCheckedU32AddErrorV1::Anchor)
    ));
    assert!(matches!(
        check_gfx942_local_checked_u32_add_v1(&inputs, &execution, 0, 4),
        Err(Gfx942LocalCheckedU32AddErrorV1::MachineInstruction(_))
    ));
    assert!(check_gfx942_local_checked_u32_add_v1(&inputs, &execution, 0, 0).is_ok());
    let other = test_worker_execution("unrelated_symbol", SYNTHETIC_S_ADD_PAYLOAD.to_vec());
    assert!(matches!(
        check_gfx942_local_checked_u32_add_v1(&inputs, &other, 0, 0),
        Err(Gfx942LocalCheckedU32AddErrorV1::MachineBinding)
    ));
    let mut changed = SYNTHETIC_S_ADD_PAYLOAD.to_vec();
    changed[1] = 130;
    let changed = test_worker_execution(symbol, changed);
    assert!(matches!(
        check_gfx942_local_checked_u32_add_v1(&inputs, &changed, 0, 0),
        Err(Gfx942LocalCheckedU32AddErrorV1::MachineInstruction(_))
    ));
}

#[test]
fn public_mov_prefix_entry_retains_owners_and_composes_actual_state() {
    let inputs = source_fixture::source_inputs();
    let module = decode_module_v8(inputs.kernel_ir().canonical_bytes()).unwrap();
    let symbol = module
        .functions
        .iter()
        .find(|function| function.body.is_some())
        .unwrap()
        .id
        .as_str();
    let execution = test_worker_execution(symbol, SYNTHETIC_MOV_ADD_PAYLOAD.to_vec());
    let obligation =
        check_gfx942_local_mov_prefix_checked_u32_add_v1(&inputs, &execution, 0, 0, 4).unwrap();
    assert!(std::ptr::eq(obligation.inputs(), &inputs));
    assert!(std::ptr::eq(obligation.execution(), &execution));
    assert_eq!(obligation.function_symbol(), symbol);
    assert_eq!(obligation.literal(), 1);
    assert_eq!(
        obligation.anchor(),
        inputs.semantic_u32_induction_kir_anchors()[0]
    );
    let entry = obligation.unresolved_entry_relation();
    assert_eq!(entry.semantic_local, 1);
    assert_eq!(entry.machine_source_sgpr, 36);
    assert_eq!(entry.machine_instruction_offset, 0);
    assert_eq!(
        obligation.machine_input_definition(),
        Gfx942ReachingDefinitionV1::LiveIn
    );
    let span = obligation.machine_span();
    assert_eq!(
        (span.first_offset(), span.add_offset(), span.end_offset()),
        (0, 4, 8)
    );
    assert_eq!(span.moves().len(), 1);
    assert_eq!(
        span.terminal_origins(),
        [
            Gfx942U32OriginV1::EntrySgpr(36),
            Gfx942U32OriginV1::Constant(1)
        ]
    );
    assert_eq!(span.moves()[0].encoding(), &SYNTHETIC_MOV_ADD_PAYLOAD[..4]);
    assert_eq!(
        span.terminal_add().encoding(),
        &SYNTHETIC_MOV_ADD_PAYLOAD[4..8]
    );
    assert_eq!(obligation.conditional_results().machine_destination_sgpr, 0);
    assert!(!span.grants_launch_authority());
    assert!(!span.establishes_compiler_refinement());
    for value in [0u32, 1, u32::MAX] {
        for incoming_scc in [false, true] {
            let mut before = [17; GFX942_ORDINARY_SGPR_COUNT_V1];
            before[36] = value;
            let mut state = Gfx942ScalarIntegerStateV1::new(before, incoming_scc);
            let result = span.execute(&mut state);
            let expected = value.overflowing_add(1);
            let mut frame = before;
            frame[5] = value;
            frame[0] = expected.0;
            assert_eq!((result.value, result.scc), expected);
            assert_eq!(state.registers(), &frame);
            assert_eq!(state.scc(), expected.1);
        }
    }
    // Executing a supplied projected state does not establish source-entry equality.
    // The old single-instruction checker is deliberately not widened to accept MOVs.
    assert!(matches!(
        check_gfx942_local_checked_u32_add_v1(&inputs, &execution, 0, 4),
        Err(Gfx942LocalCheckedU32AddErrorV1::UnsupportedMachineDefinition)
    ));
}

#[test]
fn public_mov_prefix_entry_rejects_substituted_intervals_and_trace_disagreement() {
    let inputs = source_fixture::source_inputs();
    let module = decode_module_v8(inputs.kernel_ir().canonical_bytes()).unwrap();
    let symbol = module
        .functions
        .iter()
        .find(|function| function.body.is_some())
        .unwrap()
        .id
        .as_str();
    let execution = test_worker_execution(symbol, SYNTHETIC_MOV_ADD_PAYLOAD.to_vec());
    assert!(matches!(
        check_gfx942_local_mov_prefix_checked_u32_add_v1(&inputs, &execution, 1, 0, 4),
        Err(Gfx942LocalMovPrefixCheckedU32AddErrorV1::Source(
            Gfx942LocalCheckedU32AddErrorV1::Anchor
        ))
    ));
    for (first, last) in [(2, 4), (0, 8), (4, 0), (0, 6)] {
        assert!(matches!(
            check_gfx942_local_mov_prefix_checked_u32_add_v1(&inputs, &execution, 0, first, last),
            Err(Gfx942LocalMovPrefixCheckedU32AddErrorV1::Span(_))
        ));
    }
    let other = test_worker_execution("unrelated_symbol", SYNTHETIC_MOV_ADD_PAYLOAD.to_vec());
    assert!(matches!(
        check_gfx942_local_mov_prefix_checked_u32_add_v1(&inputs, &other, 0, 0, 4),
        Err(Gfx942LocalMovPrefixCheckedU32AddErrorV1::MachineBinding)
    ));
    for index in [2, 5] {
        let mut changed = SYNTHETIC_MOV_ADD_PAYLOAD.to_vec();
        changed[index] ^= 1;
        let changed = test_worker_execution(symbol, changed);
        assert!(matches!(
            check_gfx942_local_mov_prefix_checked_u32_add_v1(&inputs, &changed, 0, 0, 4),
            Err(Gfx942LocalMovPrefixCheckedU32AddErrorV1::Span(_))
        ));
    }
    assert!(check_gfx942_local_mov_prefix_checked_u32_add_v1(&inputs, &execution, 0, 0, 4).is_ok());
}

#[test]
fn captured_public_entry_retains_same_compiler_owner_through_formal_admission() {
    let ordinary = source_fixture::source_inputs();
    let (inputs, formal) = source_fixture::captured_source_inputs(0);
    let owner = formal.semantic_kir();
    formal.verify_equivalence().unwrap();
    assert_eq!(
        ordinary.semantic_mir().canonical_encoding(),
        inputs.semantic_mir().canonical_encoding()
    );
    assert_eq!(
        ordinary.kernel_ir().canonical_bytes(),
        inputs.kernel_ir().canonical_bytes()
    );
    assert_eq!(
        ordinary.correspondence().canonical_bytes(),
        inputs.correspondence().canonical_bytes()
    );
    assert_eq!(
        ordinary.formal_memory().canonical_bytes(),
        inputs.formal_memory().canonical_bytes()
    );
    let capture = owner.checked_u32_add_capture_v1().unwrap();
    let execution = test_worker_execution(
        capture.kernel_ir_function(),
        SYNTHETIC_MOV_ADD_PAYLOAD.to_vec(),
    );
    let joined =
        check_gfx942_captured_mov_prefix_checked_u32_add_v1(&inputs, &execution, capture, 0, 0, 4)
            .unwrap();
    assert!(std::ptr::eq(joined.capture().owner(), owner));
    assert!(std::ptr::eq(joined.local_obligation().inputs(), &inputs));
    assert!(std::ptr::eq(
        joined.local_obligation().execution(),
        &execution
    ));
    assert_eq!(joined.capture().lhs_local().index(), 1);
    assert_eq!(joined.capture().tuple_local().index(), 4);
    assert_eq!(
        joined.capture().use_event().checked_add(1),
        Some(joined.capture().define_event())
    );
    assert_ne!(joined.capture().lhs_ssa(), joined.capture().tuple_ssa());
    let local = joined.local_obligation();
    assert_eq!(
        local.unresolved_entry_relation().kernel_ir_value,
        joined.capture().operand().0
    );
    assert_eq!(local.unresolved_entry_relation().machine_source_sgpr, 36);
    assert_eq!(
        local.unresolved_entry_relation().machine_instruction_offset,
        0
    );
    assert_eq!(
        local.conditional_results().kernel_ir_value,
        joined.capture().value().0
    );
    assert_eq!(
        local.conditional_results().kernel_ir_overflow,
        joined.capture().overflow().0
    );
    assert!(!local.machine_span().grants_launch_authority());
    assert!(!inputs.authenticates_compiler_origin());
    // Capture supplies genuine emission coordinates, not source runtime values.
    assert_eq!(local.unresolved_entry_relation().semantic_local, 1);
}

#[test]
fn captured_public_entry_rejects_substitutions_without_consuming_owners() {
    let (inputs, formal) = source_fixture::captured_source_inputs(0);
    let (foreign_inputs, foreign_formal) = source_fixture::captured_source_inputs(1);
    let owner = formal.semantic_kir();
    let capture = || owner.checked_u32_add_capture_v1().unwrap();
    let execution = test_worker_execution(
        capture().kernel_ir_function(),
        SYNTHETIC_MOV_ADD_PAYLOAD.to_vec(),
    );
    assert!(matches!(
        check_gfx942_captured_mov_prefix_checked_u32_add_v1(
            &foreign_inputs,
            &execution,
            capture(),
            0,
            0,
            4
        ),
        Err(Gfx942CapturedMovPrefixCheckedU32AddErrorV1::SemanticOwner)
    ));
    assert!(matches!(
        check_gfx942_captured_mov_prefix_checked_u32_add_v1(
            &inputs,
            &execution,
            foreign_formal
                .semantic_kir()
                .checked_u32_add_capture_v1()
                .unwrap(),
            0,
            0,
            4,
        ),
        Err(Gfx942CapturedMovPrefixCheckedU32AddErrorV1::SemanticOwner)
    ));
    for (anchor, first, last) in [(1, 0, 4), (0, 2, 4), (0, 0, 8)] {
        assert!(matches!(
            check_gfx942_captured_mov_prefix_checked_u32_add_v1(
                &inputs,
                &execution,
                capture(),
                anchor,
                first,
                last
            ),
            Err(Gfx942CapturedMovPrefixCheckedU32AddErrorV1::Local(_))
        ));
    }
    let mut changed = SYNTHETIC_MOV_ADD_PAYLOAD.to_vec();
    changed[5] ^= 1;
    let changed = test_worker_execution(capture().kernel_ir_function(), changed);
    assert!(matches!(
        check_gfx942_captured_mov_prefix_checked_u32_add_v1(&inputs, &changed, capture(), 0, 0, 4),
        Err(Gfx942CapturedMovPrefixCheckedU32AddErrorV1::Local(_))
    ));
    assert!(
        check_gfx942_captured_mov_prefix_checked_u32_add_v1(
            &inputs,
            &execution,
            capture(),
            0,
            0,
            4
        )
        .is_ok()
    );
    formal.verify_equivalence().unwrap();
}

#[test]
fn equal_content_compiler_captures_keep_their_distinct_owner_borrows() {
    let (inputs, first) = source_fixture::captured_source_inputs(0);
    let (_, second) = source_fixture::captured_source_inputs(0);
    let capture = second.semantic_kir().checked_u32_add_capture_v1().unwrap();
    let execution = test_worker_execution(
        capture.kernel_ir_function(),
        SYNTHETIC_MOV_ADD_PAYLOAD.to_vec(),
    );
    let joined =
        check_gfx942_captured_mov_prefix_checked_u32_add_v1(&inputs, &execution, capture, 0, 0, 4)
            .unwrap();
    assert!(std::ptr::eq(
        joined.capture().owner(),
        second.semantic_kir()
    ));
    assert!(!std::ptr::eq(
        joined.capture().owner(),
        first.semantic_kir()
    ));
    assert!(std::ptr::eq(joined.local_obligation().inputs(), &inputs));
    assert!(!inputs.authenticates_compiler_origin());
}
