use fe2o3_kernel_analysis::{
    AcceptedMachineOpcodeV1, FinalizedEntryPointV1, FinalizedMachineFunctionV1,
    FinalizedMachineOperationV1, MACHINE_EFFECT_ANALYSIS_INPUT_DOMAIN_V1,
    MACHINE_EFFECT_EVIDENCE_DOMAIN_V1, MAX_MACHINE_EFFECT_ANALYSIS_INPUT_BYTES_V1,
    MAX_MACHINE_EFFECT_CALL_EDGES_V1, MAX_MACHINE_EFFECT_EFFECTS_V1,
    MAX_MACHINE_EFFECT_EVIDENCE_BYTES_V1, MAX_MACHINE_EFFECT_OPERATIONS_V1, MachineAddressSpaceV1,
    MachineAnalyzerIdentityV1, MachineCallTargetV1, MachineDescriptorIdentityV1,
    MachineEffectAnalysisBasisV1, MachineEffectAnalysisErrorV1, MachineEffectAnalysisInputV1,
    MachineEffectBindingsV1, MachineEffectDecodeErrorV1, MachineEffectEvidenceDecodeErrorV1,
    MachineEffectEvidenceV1, MachineEffectInputErrorV1, MachineEffectKindV1, MachineEffectV1,
    MachineEntryPointIdV1, MachineFunctionIdV1, MachineKernelIdentityV1, MachinePayloadIdentityV1,
    MachineRecursionBoundV1, MachineTargetIdentityV1, MachineTargetV1, MachineToolchainIdentityV1,
    analyze_gfx942_machine_effects_v1,
};

const ENTRY: MachineEntryPointIdV1 = MachineEntryPointIdV1(1);
const ROOT: MachineFunctionIdV1 = MachineFunctionIdV1(10);
const HELPER: MachineFunctionIdV1 = MachineFunctionIdV1(20);

fn bindings() -> MachineEffectBindingsV1 {
    bindings_from(
        [
            [0x11; 32], [0x22; 32], [0x33; 32], [0x44; 32], [0x55; 32], [0x66; 32],
        ],
        4_096,
    )
    .unwrap()
}

fn bindings_from(
    digests: [[u8; 32]; 6],
    payload_len: u64,
) -> Result<MachineEffectBindingsV1, MachineEffectInputErrorV1> {
    MachineEffectBindingsV1::new(
        MachineTargetV1::Gfx942,
        MachineTargetIdentityV1::from_sha256_bytes(digests[0]),
        MachineToolchainIdentityV1::from_sha256_bytes(digests[1]),
        MachineAnalyzerIdentityV1::from_sha256_bytes(digests[2]),
        MachineKernelIdentityV1::from_sha256_bytes(digests[3]),
        MachinePayloadIdentityV1::from_parts(digests[4], payload_len),
        MachineDescriptorIdentityV1::from_sha256_bytes(digests[5]),
    )
}

fn entry() -> FinalizedEntryPointV1 {
    FinalizedEntryPointV1::new(ENTRY, "alpha", ROOT)
}

fn helper_operations() -> Vec<FinalizedMachineOperationV1> {
    vec![
        FinalizedMachineOperationV1::AddressDerivation {
            address_space: MachineAddressSpaceV1::Global,
            address_id: 7,
            base_argument: 0,
            index_scale: 4,
            constant_offset: 0,
        },
        FinalizedMachineOperationV1::Read {
            address_space: MachineAddressSpaceV1::Global,
            address_id: 7,
            byte_width: 4,
        },
        FinalizedMachineOperationV1::AddressDerivation {
            address_space: MachineAddressSpaceV1::Global,
            address_id: 8,
            base_argument: 1,
            index_scale: 4,
            constant_offset: 16,
        },
        FinalizedMachineOperationV1::Write {
            address_space: MachineAddressSpaceV1::Global,
            address_id: 8,
            byte_width: 4,
        },
        FinalizedMachineOperationV1::NoEffect(AcceptedMachineOpcodeV1::Return),
    ]
}

fn root_function(calls: Vec<MachineCallTargetV1>) -> FinalizedMachineFunctionV1 {
    FinalizedMachineFunctionV1::new(
        ROOT,
        calls,
        vec![
            FinalizedMachineOperationV1::NoEffect(AcceptedMachineOpcodeV1::IntegerAlu),
            FinalizedMachineOperationV1::NoEffect(AcceptedMachineOpcodeV1::Return),
        ],
    )
}

fn helper_function(operations: Vec<FinalizedMachineOperationV1>) -> FinalizedMachineFunctionV1 {
    FinalizedMachineFunctionV1::new(HELPER, Vec::new(), operations)
}

fn accepted_effects() -> Vec<MachineEffectV1> {
    vec![
        MachineEffectV1::new(
            ENTRY,
            HELPER,
            0,
            MachineEffectKindV1::GlobalAddressDerivation {
                address_id: 7,
                base_argument: 0,
                index_scale: 4,
                constant_offset: 0,
            },
        ),
        MachineEffectV1::new(
            ENTRY,
            HELPER,
            1,
            MachineEffectKindV1::GlobalRead {
                address_id: 7,
                byte_width: 4,
            },
        ),
        MachineEffectV1::new(
            ENTRY,
            HELPER,
            2,
            MachineEffectKindV1::GlobalAddressDerivation {
                address_id: 8,
                base_argument: 1,
                index_scale: 4,
                constant_offset: 16,
            },
        ),
        MachineEffectV1::new(
            ENTRY,
            HELPER,
            3,
            MachineEffectKindV1::GlobalWrite {
                address_id: 8,
                byte_width: 4,
            },
        ),
    ]
}

fn input() -> MachineEffectAnalysisInputV1 {
    MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![
            helper_function(helper_operations()),
            root_function(vec![MachineCallTargetV1::Direct(HELPER)]),
        ],
        Vec::new(),
        accepted_effects(),
    )
    .unwrap()
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn append_test_bindings(output: &mut Vec<u8>) {
    let bindings = bindings();
    push_u16(output, 1);
    output.extend_from_slice(&bindings.target_identity().as_bytes());
    output.extend_from_slice(&bindings.toolchain_identity().as_bytes());
    output.extend_from_slice(&bindings.analyzer_identity().as_bytes());
    output.extend_from_slice(&bindings.kernel_identity().as_bytes());
    output.extend_from_slice(&bindings.payload_identity().sha256());
    output.extend_from_slice(&bindings.payload_identity().byte_len().to_le_bytes());
    output.extend_from_slice(&bindings.descriptor_identity().as_bytes());
}

fn aggregate_budget_wire(call_count: usize, operation_count: usize) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(MACHINE_EFFECT_ANALYSIS_INPUT_DOMAIN_V1);
    push_u32(&mut output, 0);
    push_u32(&mut output, 0);
    append_test_bindings(&mut output);
    push_u32(&mut output, 1);
    push_u32(&mut output, 2);
    push_u32(&mut output, 0);
    push_u32(&mut output, 0);

    push_u32(&mut output, ENTRY.0);
    push_u32(&mut output, ROOT.0);
    push_u16(&mut output, 1);
    output.push(b'k');

    push_u32(&mut output, ROOT.0);
    push_u32(&mut output, call_count as u32);
    push_u32(&mut output, operation_count as u32);
    for _ in 0..call_count {
        push_u16(&mut output, 1);
        push_u32(&mut output, HELPER.0);
    }
    for _ in 0..operation_count {
        push_u16(&mut output, 4);
        push_u16(&mut output, 1);
    }

    push_u32(&mut output, HELPER.0);
    push_u32(&mut output, if call_count != 0 { 1 } else { 0 });
    push_u32(&mut output, if operation_count != 0 { 1 } else { 0 });
    if call_count != 0 {
        push_u16(&mut output, 1);
        push_u32(&mut output, ROOT.0);
    }
    if operation_count != 0 {
        push_u16(&mut output, 4);
        push_u16(&mut output, 4);
    }

    let length = output.len() as u32;
    let length_offset = MACHINE_EFFECT_ANALYSIS_INPUT_DOMAIN_V1.len();
    output[length_offset..length_offset + 4].copy_from_slice(&length.to_le_bytes());
    output
}

#[test]
fn derives_canonical_effects_for_a_closed_gfx942_graph() {
    let input = input();
    let evidence = analyze_gfx942_machine_effects_v1(&input).unwrap();

    assert_eq!(evidence.bindings(), bindings());
    assert_eq!(evidence.input_identity(), input.identity());
    assert_eq!(evidence.entry_points(), &[entry()]);
    assert_eq!(evidence.effects(), accepted_effects());
    assert_eq!(evidence.schema_version(), 1);
    assert_eq!(
        evidence.analysis_basis(),
        MachineEffectAnalysisBasisV1::UnauthenticatedCallerSuppliedFinalizedMechanics
    );
    assert!(
        evidence
            .canonical_bytes()
            .starts_with(MACHINE_EFFECT_EVIDENCE_DOMAIN_V1)
    );
    assert_eq!(
        evidence.identity().byte_len(),
        evidence.canonical_bytes().len() as u64
    );
    assert!(!evidence.authenticates_extractor());
    assert!(!evidence.authenticates_compiler());
    assert!(!evidence.establishes_payload_refinement());
    assert!(!evidence.contains_general_isa_disassembly());
    assert!(!evidence.grants_load_authority());
    assert!(!evidence.grants_launch_authority());
}

#[test]
fn canonical_round_trips_bind_all_identities_and_payload_length() {
    let input = input();
    let decoded = MachineEffectAnalysisInputV1::decode_canonical(input.canonical_bytes()).unwrap();
    assert_eq!(decoded, input);
    assert_eq!(
        decoded.bindings().target_identity(),
        bindings().target_identity()
    );
    assert_eq!(
        decoded.bindings().toolchain_identity(),
        bindings().toolchain_identity()
    );
    assert_eq!(
        decoded.bindings().analyzer_identity(),
        bindings().analyzer_identity()
    );
    assert_eq!(
        decoded.bindings().kernel_identity(),
        bindings().kernel_identity()
    );
    assert_eq!(
        decoded.bindings().payload_identity(),
        bindings().payload_identity()
    );
    assert_eq!(
        decoded.bindings().descriptor_identity(),
        bindings().descriptor_identity()
    );
    assert_eq!(decoded.bindings().payload_identity().byte_len(), 4_096);

    let evidence = analyze_gfx942_machine_effects_v1(&input).unwrap();
    let decoded =
        MachineEffectEvidenceV1::decode_canonical_for(&input, evidence.canonical_bytes()).unwrap();
    assert_eq!(decoded, evidence);
}

#[test]
fn bindings_reject_reserved_zero_identities_and_empty_payloads() {
    let fields = [
        "target identity",
        "toolchain identity",
        "analyzer identity",
        "kernel identity",
        "payload identity",
        "descriptor identity",
    ];
    for (index, field) in fields.into_iter().enumerate() {
        let mut digests = [[0x7a; 32]; 6];
        digests[index] = [0; 32];
        assert_eq!(
            bindings_from(digests, 1),
            Err(MachineEffectInputErrorV1::ZeroDigestIdentity { field })
        );
    }
    assert_eq!(
        bindings_from([[0x7a; 32]; 6], 0),
        Err(MachineEffectInputErrorV1::ZeroPayloadLength)
    );
}

#[test]
fn decoders_reject_reserved_zero_binding_identities() {
    const TARGET_IDENTITY_OFFSET_IN_BINDINGS: usize = 2;

    let input = input();
    let input_bindings = MACHINE_EFFECT_ANALYSIS_INPUT_DOMAIN_V1.len() + 4 + 4;
    let mut input_bytes = input.canonical_bytes().to_vec();
    input_bytes[input_bindings + TARGET_IDENTITY_OFFSET_IN_BINDINGS
        ..input_bindings + TARGET_IDENTITY_OFFSET_IN_BINDINGS + 32]
        .fill(0);
    assert_eq!(
        MachineEffectAnalysisInputV1::decode_canonical(&input_bytes),
        Err(MachineEffectDecodeErrorV1::InvalidInput(Box::new(
            MachineEffectInputErrorV1::ZeroDigestIdentity {
                field: "target identity",
            },
        )))
    );

    let evidence = analyze_gfx942_machine_effects_v1(&input).unwrap();
    let evidence_bindings = MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len() + 4 + 4;
    let mut evidence_bytes = evidence.canonical_bytes().to_vec();
    evidence_bytes[evidence_bindings + TARGET_IDENTITY_OFFSET_IN_BINDINGS
        ..evidence_bindings + TARGET_IDENTITY_OFFSET_IN_BINDINGS + 32]
        .fill(0);
    assert_eq!(
        MachineEffectEvidenceV1::decode_canonical_for(&input, &evidence_bytes),
        Err(MachineEffectEvidenceDecodeErrorV1::InvalidBindings(
            Box::new(MachineEffectInputErrorV1::ZeroDigestIdentity {
                field: "target identity",
            }),
        ))
    );
}

#[test]
fn canonicalizes_function_and_edge_summary_order() {
    let leaf = MachineFunctionIdV1(30);
    let first = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![
            FinalizedMachineFunctionV1::new(leaf, Vec::new(), Vec::new()),
            helper_function(Vec::new()),
            root_function(vec![
                MachineCallTargetV1::Direct(leaf),
                MachineCallTargetV1::Direct(HELPER),
            ]),
        ],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let second = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![
            root_function(vec![
                MachineCallTargetV1::Direct(HELPER),
                MachineCallTargetV1::Direct(leaf),
            ]),
            helper_function(Vec::new()),
            FinalizedMachineFunctionV1::new(leaf, Vec::new(), Vec::new()),
        ],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(first.identity(), second.identity());
}

#[test]
fn rejects_missing_entries_and_missing_entry_functions() {
    assert_eq!(
        MachineEffectAnalysisInputV1::new(
            bindings(),
            Vec::new(),
            vec![root_function(Vec::new())],
            Vec::new(),
            Vec::new(),
        ),
        Err(MachineEffectInputErrorV1::MissingEntryPoints)
    );

    assert_eq!(
        MachineEffectAnalysisInputV1::new(
            bindings(),
            vec![entry()],
            vec![helper_function(Vec::new())],
            Vec::new(),
            Vec::new(),
        ),
        Err(MachineEffectInputErrorV1::MissingEntryFunction {
            entry: ENTRY,
            function: ROOT,
        })
    );
}

#[test]
fn rejects_unknown_and_duplicate_direct_call_edges() {
    let unknown = MachineFunctionIdV1(99);
    assert_eq!(
        MachineEffectAnalysisInputV1::new(
            bindings(),
            vec![entry()],
            vec![root_function(vec![MachineCallTargetV1::Direct(unknown)])],
            Vec::new(),
            Vec::new(),
        ),
        Err(MachineEffectInputErrorV1::UnknownDirectCallee {
            caller: ROOT,
            callee: unknown,
        })
    );

    assert_eq!(
        MachineEffectAnalysisInputV1::new(
            bindings(),
            vec![entry()],
            vec![
                root_function(vec![
                    MachineCallTargetV1::Direct(HELPER),
                    MachineCallTargetV1::Direct(HELPER),
                ]),
                helper_function(Vec::new()),
            ],
            Vec::new(),
            Vec::new(),
        ),
        Err(MachineEffectInputErrorV1::DuplicateCallEdge {
            caller: ROOT,
            callee: HELPER,
        })
    );
}

#[test]
fn rejects_indirect_calls_before_effect_analysis() {
    let input = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![root_function(vec![MachineCallTargetV1::Indirect])],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        analyze_gfx942_machine_effects_v1(&input),
        Err(MachineEffectAnalysisErrorV1::IndirectCall { function: ROOT })
    );
}

#[test]
fn recursion_requires_explicit_complete_bounds() {
    let recursive_root = || {
        root_function(vec![
            MachineCallTargetV1::Direct(HELPER),
            MachineCallTargetV1::Direct(ROOT),
        ])
    };
    let unbounded = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![recursive_root(), helper_function(helper_operations())],
        Vec::new(),
        accepted_effects(),
    )
    .unwrap();
    assert_eq!(
        analyze_gfx942_machine_effects_v1(&unbounded),
        Err(MachineEffectAnalysisErrorV1::UnboundedRecursion { function: ROOT })
    );

    let bounded = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![recursive_root(), helper_function(helper_operations())],
        vec![MachineRecursionBoundV1::new(ROOT, 8)],
        accepted_effects(),
    )
    .unwrap();
    assert!(analyze_gfx942_machine_effects_v1(&bounded).is_ok());
}

#[test]
fn scc_marks_every_member_across_finished_dfs_subtrees() {
    let third = MachineFunctionIdV1(30);
    let functions = || {
        vec![
            root_function(vec![
                MachineCallTargetV1::Direct(HELPER),
                MachineCallTargetV1::Direct(third),
            ]),
            FinalizedMachineFunctionV1::new(
                HELPER,
                vec![MachineCallTargetV1::Direct(ROOT)],
                vec![FinalizedMachineOperationV1::NoEffect(
                    AcceptedMachineOpcodeV1::Return,
                )],
            ),
            FinalizedMachineFunctionV1::new(
                third,
                vec![MachineCallTargetV1::Direct(HELPER)],
                vec![FinalizedMachineOperationV1::NoEffect(
                    AcceptedMachineOpcodeV1::Return,
                )],
            ),
        ]
    };
    let missing = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        functions(),
        vec![
            MachineRecursionBoundV1::new(ROOT, 8),
            MachineRecursionBoundV1::new(HELPER, 8),
        ],
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        analyze_gfx942_machine_effects_v1(&missing),
        Err(MachineEffectAnalysisErrorV1::UnboundedRecursion { function: third })
    );

    let complete = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        functions(),
        vec![
            MachineRecursionBoundV1::new(ROOT, 8),
            MachineRecursionBoundV1::new(HELPER, 8),
            MachineRecursionBoundV1::new(third, 8),
        ],
        Vec::new(),
    )
    .unwrap();
    assert!(analyze_gfx942_machine_effects_v1(&complete).is_ok());
}

#[test]
fn rejects_extraneous_and_invalid_recursion_bounds() {
    let extraneous = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![root_function(Vec::new())],
        vec![MachineRecursionBoundV1::new(ROOT, 1)],
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        analyze_gfx942_machine_effects_v1(&extraneous),
        Err(MachineEffectAnalysisErrorV1::ExtraneousRecursionBound { function: ROOT })
    );

    assert_eq!(
        MachineEffectAnalysisInputV1::new(
            bindings(),
            vec![entry()],
            vec![root_function(vec![MachineCallTargetV1::Direct(ROOT)])],
            vec![MachineRecursionBoundV1::new(ROOT, 0)],
            Vec::new(),
        ),
        Err(MachineEffectInputErrorV1::InvalidRecursionDepth {
            function: ROOT,
            depth: 0,
        })
    );
}

#[test]
fn rejects_effect_expansion_and_duplicate_policy_effects() {
    let mut policy = accepted_effects();
    let expanded = policy.pop().unwrap();
    let input = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![
            root_function(vec![MachineCallTargetV1::Direct(HELPER)]),
            helper_function(helper_operations()),
        ],
        Vec::new(),
        policy,
    )
    .unwrap();
    assert_eq!(
        analyze_gfx942_machine_effects_v1(&input),
        Err(MachineEffectAnalysisErrorV1::EffectExpansion {
            effect: expanded.clone(),
        })
    );

    let duplicate = MachineEffectV1::new(
        ENTRY,
        ROOT,
        0,
        MachineEffectKindV1::GlobalWrite {
            address_id: 1,
            byte_width: 4,
        },
    );
    assert_eq!(
        MachineEffectAnalysisInputV1::new(
            bindings(),
            vec![entry()],
            vec![root_function(Vec::new())],
            Vec::new(),
            vec![duplicate.clone(), duplicate.clone()],
        ),
        Err(MachineEffectInputErrorV1::DuplicateEffect(duplicate))
    );
}

#[test]
fn rejects_unsupported_opcodes_address_spaces_and_targets() {
    let unsupported_opcode = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![FinalizedMachineFunctionV1::new(
            ROOT,
            Vec::new(),
            vec![FinalizedMachineOperationV1::UnsupportedOpcode(0xbeef)],
        )],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        analyze_gfx942_machine_effects_v1(&unsupported_opcode),
        Err(MachineEffectAnalysisErrorV1::UnsupportedOpcode {
            function: ROOT,
            operation_index: 0,
            opcode: 0xbeef,
        })
    );

    let unsupported_space = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![FinalizedMachineFunctionV1::new(
            ROOT,
            Vec::new(),
            vec![FinalizedMachineOperationV1::AddressDerivation {
                address_space: MachineAddressSpaceV1::Workgroup,
                address_id: 1,
                base_argument: 0,
                index_scale: 4,
                constant_offset: 0,
            }],
        )],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        analyze_gfx942_machine_effects_v1(&unsupported_space),
        Err(MachineEffectAnalysisErrorV1::UnsupportedAddressSpace {
            function: ROOT,
            operation_index: 0,
            address_space: MachineAddressSpaceV1::Workgroup,
        })
    );

    let unsupported_bindings = MachineEffectBindingsV1::new(
        MachineTargetV1::Unsupported(99),
        bindings().target_identity(),
        bindings().toolchain_identity(),
        bindings().analyzer_identity(),
        bindings().kernel_identity(),
        bindings().payload_identity(),
        bindings().descriptor_identity(),
    )
    .unwrap();
    let unsupported_target = MachineEffectAnalysisInputV1::new(
        unsupported_bindings,
        vec![entry()],
        vec![root_function(Vec::new())],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        analyze_gfx942_machine_effects_v1(&unsupported_target),
        Err(MachineEffectAnalysisErrorV1::UnsupportedTarget(
            MachineTargetV1::Unsupported(99)
        ))
    );
}

#[test]
fn straight_line_v1_rejects_branch_around_and_nonterminal_return() {
    let branch_around = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![FinalizedMachineFunctionV1::new(
            ROOT,
            Vec::new(),
            vec![
                FinalizedMachineOperationV1::AddressDerivation {
                    address_space: MachineAddressSpaceV1::Global,
                    address_id: 1,
                    base_argument: 0,
                    index_scale: 4,
                    constant_offset: 0,
                },
                FinalizedMachineOperationV1::NoEffect(AcceptedMachineOpcodeV1::ControlFlow),
                FinalizedMachineOperationV1::Read {
                    address_space: MachineAddressSpaceV1::Global,
                    address_id: 1,
                    byte_width: 4,
                },
                FinalizedMachineOperationV1::NoEffect(AcceptedMachineOpcodeV1::Return),
            ],
        )],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        analyze_gfx942_machine_effects_v1(&branch_around),
        Err(MachineEffectAnalysisErrorV1::UnsupportedControlFlow {
            function: ROOT,
            operation_index: 1,
        })
    );

    let nonterminal_return = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![FinalizedMachineFunctionV1::new(
            ROOT,
            Vec::new(),
            vec![
                FinalizedMachineOperationV1::NoEffect(AcceptedMachineOpcodeV1::Return),
                FinalizedMachineOperationV1::NoEffect(AcceptedMachineOpcodeV1::IntegerAlu),
            ],
        )],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        analyze_gfx942_machine_effects_v1(&nonterminal_return),
        Err(MachineEffectAnalysisErrorV1::NonTerminalReturn {
            function: ROOT,
            operation_index: 0,
        })
    );
}

#[test]
fn straight_line_v1_rejects_access_before_late_derivation() {
    let input = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![FinalizedMachineFunctionV1::new(
            ROOT,
            Vec::new(),
            vec![
                FinalizedMachineOperationV1::Read {
                    address_space: MachineAddressSpaceV1::Global,
                    address_id: 1,
                    byte_width: 4,
                },
                FinalizedMachineOperationV1::AddressDerivation {
                    address_space: MachineAddressSpaceV1::Global,
                    address_id: 1,
                    base_argument: 0,
                    index_scale: 4,
                    constant_offset: 0,
                },
                FinalizedMachineOperationV1::NoEffect(AcceptedMachineOpcodeV1::Return),
            ],
        )],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        analyze_gfx942_machine_effects_v1(&input),
        Err(MachineEffectAnalysisErrorV1::UnknownAddress {
            function: ROOT,
            operation_index: 0,
            address_id: 1,
        })
    );
}

#[test]
fn rejects_duplicate_unknown_and_invalid_address_mechanics() {
    let cases = [
        (
            vec![
                FinalizedMachineOperationV1::AddressDerivation {
                    address_space: MachineAddressSpaceV1::Global,
                    address_id: 1,
                    base_argument: 0,
                    index_scale: 4,
                    constant_offset: 0,
                },
                FinalizedMachineOperationV1::AddressDerivation {
                    address_space: MachineAddressSpaceV1::Global,
                    address_id: 1,
                    base_argument: 1,
                    index_scale: 4,
                    constant_offset: 0,
                },
            ],
            MachineEffectAnalysisErrorV1::DuplicateAddress {
                function: ROOT,
                address_id: 1,
            },
        ),
        (
            vec![FinalizedMachineOperationV1::Read {
                address_space: MachineAddressSpaceV1::Global,
                address_id: 9,
                byte_width: 4,
            }],
            MachineEffectAnalysisErrorV1::UnknownAddress {
                function: ROOT,
                operation_index: 0,
                address_id: 9,
            },
        ),
        (
            vec![FinalizedMachineOperationV1::AddressDerivation {
                address_space: MachineAddressSpaceV1::Global,
                address_id: 1,
                base_argument: 0,
                index_scale: 0,
                constant_offset: 0,
            }],
            MachineEffectAnalysisErrorV1::ZeroIndexScale {
                function: ROOT,
                operation_index: 0,
            },
        ),
        (
            vec![
                FinalizedMachineOperationV1::AddressDerivation {
                    address_space: MachineAddressSpaceV1::Global,
                    address_id: 1,
                    base_argument: 0,
                    index_scale: 4,
                    constant_offset: 0,
                },
                FinalizedMachineOperationV1::Write {
                    address_space: MachineAddressSpaceV1::Global,
                    address_id: 1,
                    byte_width: 0,
                },
            ],
            MachineEffectAnalysisErrorV1::ZeroAccessWidth {
                function: ROOT,
                operation_index: 1,
            },
        ),
    ];

    for (operations, expected) in cases {
        let input = MachineEffectAnalysisInputV1::new(
            bindings(),
            vec![entry()],
            vec![FinalizedMachineFunctionV1::new(
                ROOT,
                Vec::new(),
                operations,
            )],
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(analyze_gfx942_machine_effects_v1(&input), Err(expected));
    }
}

#[test]
fn rejects_unreachable_function_records() {
    let input = MachineEffectAnalysisInputV1::new(
        bindings(),
        vec![entry()],
        vec![root_function(Vec::new()), helper_function(Vec::new())],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    assert_eq!(
        analyze_gfx942_machine_effects_v1(&input),
        Err(MachineEffectAnalysisErrorV1::UnreachableFunction { function: HELPER })
    );
}

#[test]
fn input_decoder_rejects_trailing_truncated_and_oversized_records() {
    let input = input();
    let mut trailing = input.canonical_bytes().to_vec();
    trailing.push(0);
    assert_eq!(
        MachineEffectAnalysisInputV1::decode_canonical(&trailing),
        Err(MachineEffectDecodeErrorV1::DeclaredLengthMismatch {
            declared: input.canonical_bytes().len(),
            actual: trailing.len(),
        })
    );

    let truncated = &input.canonical_bytes()[..MACHINE_EFFECT_ANALYSIS_INPUT_DOMAIN_V1.len() - 1];
    assert_eq!(
        MachineEffectAnalysisInputV1::decode_canonical(truncated),
        Err(MachineEffectDecodeErrorV1::Truncated)
    );

    let oversized = vec![0; MAX_MACHINE_EFFECT_ANALYSIS_INPUT_BYTES_V1 + 1];
    assert_eq!(
        MachineEffectAnalysisInputV1::decode_canonical(&oversized),
        Err(MachineEffectDecodeErrorV1::TooLarge {
            actual: oversized.len(),
            maximum: MAX_MACHINE_EFFECT_ANALYSIS_INPUT_BYTES_V1,
        })
    );
}

#[test]
fn decoders_preflight_truncated_max_count_records() {
    const BINDINGS_BYTES: usize = 2 + 32 + 32 + 32 + 32 + 32 + 8 + 32;

    let input = input();
    let mut input_bytes = input.canonical_bytes().to_vec();
    let input_counts = MACHINE_EFFECT_ANALYSIS_INPUT_DOMAIN_V1.len() + 4 + 4 + BINDINGS_BYTES;
    input_bytes[input_counts + 12..input_counts + 16]
        .copy_from_slice(&(MAX_MACHINE_EFFECT_EFFECTS_V1 as u32).to_le_bytes());
    assert_eq!(
        MachineEffectAnalysisInputV1::decode_canonical(&input_bytes),
        Err(MachineEffectDecodeErrorV1::Truncated)
    );

    let evidence = analyze_gfx942_machine_effects_v1(&input).unwrap();
    let mut evidence_bytes = evidence.canonical_bytes().to_vec();
    let evidence_counts = MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len() + 4 + 4 + BINDINGS_BYTES + 32 + 8;
    evidence_bytes[evidence_counts + 4..evidence_counts + 8]
        .copy_from_slice(&(MAX_MACHINE_EFFECT_EFFECTS_V1 as u32).to_le_bytes());
    assert_eq!(
        MachineEffectEvidenceV1::decode_canonical_for(&input, &evidence_bytes),
        Err(MachineEffectEvidenceDecodeErrorV1::Truncated)
    );
}

#[test]
fn input_decoder_rejects_aggregate_call_and_operation_budget_overflow() {
    let calls = aggregate_budget_wire(MAX_MACHINE_EFFECT_CALL_EDGES_V1, 0);
    assert_eq!(
        MachineEffectAnalysisInputV1::decode_canonical(&calls),
        Err(MachineEffectDecodeErrorV1::InvalidInput(Box::new(
            MachineEffectInputErrorV1::CountBoundExceeded {
                field: "call edges",
                actual: MAX_MACHINE_EFFECT_CALL_EDGES_V1 + 1,
                maximum: MAX_MACHINE_EFFECT_CALL_EDGES_V1,
            }
        )))
    );

    let operations = aggregate_budget_wire(0, MAX_MACHINE_EFFECT_OPERATIONS_V1);
    assert_eq!(
        MachineEffectAnalysisInputV1::decode_canonical(&operations),
        Err(MachineEffectDecodeErrorV1::InvalidInput(Box::new(
            MachineEffectInputErrorV1::CountBoundExceeded {
                field: "operations",
                actual: MAX_MACHINE_EFFECT_OPERATIONS_V1 + 1,
                maximum: MAX_MACHINE_EFFECT_OPERATIONS_V1,
            }
        )))
    );
}

#[test]
fn evidence_decoder_rejects_identity_and_effect_mutation() {
    let input = input();
    let evidence = analyze_gfx942_machine_effects_v1(&input).unwrap();

    let bindings_offset = MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len() + 4 + 4;
    for relative_offset in [2, 34, 66, 98, 130, 162, 170] {
        let mut identity_mutation = evidence.canonical_bytes().to_vec();
        identity_mutation[bindings_offset + relative_offset] ^= 0x80;
        assert_eq!(
            MachineEffectEvidenceV1::decode_canonical_for(&input, &identity_mutation),
            Err(MachineEffectEvidenceDecodeErrorV1::IdentityBindingMismatch)
        );
    }

    let mut input_identity_mutation = evidence.canonical_bytes().to_vec();
    input_identity_mutation[bindings_offset + 202] ^= 0x80;
    assert_eq!(
        MachineEffectEvidenceV1::decode_canonical_for(&input, &input_identity_mutation),
        Err(MachineEffectEvidenceDecodeErrorV1::InputIdentityMismatch)
    );

    let mut identity_before_counts = evidence.canonical_bytes().to_vec();
    identity_before_counts[bindings_offset + 2] ^= 0x80;
    let evidence_counts = bindings_offset + 202 + 32 + 8;
    identity_before_counts[evidence_counts + 4..evidence_counts + 8]
        .copy_from_slice(&(MAX_MACHINE_EFFECT_EFFECTS_V1 as u32).to_le_bytes());
    assert_eq!(
        MachineEffectEvidenceV1::decode_canonical_for(&input, &identity_before_counts),
        Err(MachineEffectEvidenceDecodeErrorV1::IdentityBindingMismatch)
    );

    let mut effect_mutation = evidence.canonical_bytes().to_vec();
    let last = effect_mutation.last_mut().unwrap();
    *last ^= 1;
    assert_eq!(
        MachineEffectEvidenceV1::decode_canonical_for(&input, &effect_mutation),
        Err(MachineEffectEvidenceDecodeErrorV1::EffectEvidenceMismatch)
    );
}

#[test]
fn evidence_decoder_rejects_trailing_and_oversized_records() {
    let input = input();
    let evidence = analyze_gfx942_machine_effects_v1(&input).unwrap();
    let mut trailing = evidence.canonical_bytes().to_vec();
    trailing.push(0);
    assert_eq!(
        MachineEffectEvidenceV1::decode_canonical_for(&input, &trailing),
        Err(MachineEffectEvidenceDecodeErrorV1::DeclaredLengthMismatch {
            declared: evidence.canonical_bytes().len(),
            actual: trailing.len(),
        })
    );

    let oversized = vec![0; MAX_MACHINE_EFFECT_EVIDENCE_BYTES_V1 + 1];
    assert_eq!(
        MachineEffectEvidenceV1::decode_canonical_for(&input, &oversized),
        Err(MachineEffectEvidenceDecodeErrorV1::TooLarge {
            actual: oversized.len(),
            maximum: MAX_MACHINE_EFFECT_EVIDENCE_BYTES_V1,
        })
    );
}

#[test]
fn record_count_bounds_fail_before_allocation() {
    let entries = (0..=32)
        .map(|id| FinalizedEntryPointV1::new(MachineEntryPointIdV1(id), format!("k{id}"), ROOT))
        .collect();
    assert!(matches!(
        MachineEffectAnalysisInputV1::new(
            bindings(),
            entries,
            vec![root_function(Vec::new())],
            Vec::new(),
            Vec::new(),
        ),
        Err(MachineEffectInputErrorV1::CountBoundExceeded {
            field: "entry points",
            actual: 33,
            maximum: 32,
        })
    ));
}

#[test]
fn entry_closure_effect_multiplication_is_preflight_bounded() {
    let entries = (1..=32)
        .map(|id| FinalizedEntryPointV1::new(MachineEntryPointIdV1(id), format!("k{id}"), ROOT))
        .collect();
    let mut operations = (0..513)
        .map(
            |address_id| FinalizedMachineOperationV1::AddressDerivation {
                address_space: MachineAddressSpaceV1::Global,
                address_id,
                base_argument: 0,
                index_scale: 1,
                constant_offset: 0,
            },
        )
        .collect::<Vec<_>>();
    operations.push(FinalizedMachineOperationV1::NoEffect(
        AcceptedMachineOpcodeV1::Return,
    ));
    let input = MachineEffectAnalysisInputV1::new(
        bindings(),
        entries,
        vec![FinalizedMachineFunctionV1::new(
            ROOT,
            Vec::new(),
            operations,
        )],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();

    assert_eq!(
        analyze_gfx942_machine_effects_v1(&input),
        Err(MachineEffectAnalysisErrorV1::EffectCountBoundExceeded {
            actual: 32 * 513,
            maximum: MAX_MACHINE_EFFECT_EFFECTS_V1,
        })
    );
}
