use fe2o3_kernel_analysis::{
    AcceptedMachineOpcodeV1, FinalizedEntryPointV1, FinalizedMachineFunctionV1,
    FinalizedMachineOperationV1, MACHINE_EFFECT_ANALYSIS_INPUT_DOMAIN_V1,
    MACHINE_EFFECT_EVIDENCE_DOMAIN_V1, MAX_MACHINE_EFFECT_ANALYSIS_INPUT_BYTES_V1,
    MAX_MACHINE_EFFECT_EVIDENCE_BYTES_V1, MachineAddressSpaceV1, MachineAnalyzerIdentityV1,
    MachineCallTargetV1, MachineDescriptorIdentityV1, MachineEffectAnalysisBasisV1,
    MachineEffectAnalysisErrorV1, MachineEffectAnalysisInputV1, MachineEffectBindingsV1,
    MachineEffectDecodeErrorV1, MachineEffectEvidenceDecodeErrorV1, MachineEffectEvidenceV1,
    MachineEffectInputErrorV1, MachineEffectKindV1, MachineEffectV1, MachineEntryPointIdV1,
    MachineFunctionIdV1, MachineKernelIdentityV1, MachinePayloadIdentityV1,
    MachineRecursionBoundV1, MachineTargetIdentityV1, MachineTargetV1, MachineToolchainIdentityV1,
    analyze_gfx942_machine_effects_v1,
};

const ENTRY: MachineEntryPointIdV1 = MachineEntryPointIdV1(1);
const ROOT: MachineFunctionIdV1 = MachineFunctionIdV1(10);
const HELPER: MachineFunctionIdV1 = MachineFunctionIdV1(20);

fn bindings() -> MachineEffectBindingsV1 {
    MachineEffectBindingsV1::new(
        MachineTargetV1::Gfx942,
        MachineTargetIdentityV1::from_sha256_bytes([0x11; 32]),
        MachineToolchainIdentityV1::from_sha256_bytes([0x22; 32]),
        MachineAnalyzerIdentityV1::from_sha256_bytes([0x33; 32]),
        MachineKernelIdentityV1::from_sha256_bytes([0x44; 32]),
        MachinePayloadIdentityV1::from_parts([0x55; 32], 4_096),
        MachineDescriptorIdentityV1::from_sha256_bytes([0x66; 32]),
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
            FinalizedMachineOperationV1::NoEffect(AcceptedMachineOpcodeV1::ControlFlow),
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
    );
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
