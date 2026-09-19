//! Synthetic canonical fixtures only: no authenticated Rust/HIR correspondence.

use super::*;
use crate::multilevel_authoring_v1::AuthoringOperationCoordinateV1;
use fe2o3_kernel_ir::{
    AssemblyConstraint, AssemblyOperand, AssemblyOption, AssemblySourceIdentity, BasicBlock,
    BlockId, DebugSourceMapDocumentV2, DebugSourceMapFileV1, DebugSourceMapKirSiteV1,
    DebugSourceMapSiteV1, DebugSourceMapSpanV1, Function, InlineAssembly, InlineAssemblyTarget,
    Kernel, LaunchDomain, LaunchExtent, Module, Operation, PreparedSimulationBundleV6,
    SemanticAggregateStorageMapV6, SemanticKernelStorageV1, SemanticKernelStorageV2,
    SemanticStorageMapV6, Signature, SimulationProductionKirIdentityV6, SimulationSourceLineageV1,
    Terminator, ValueDef, VerifiedCanonicalKernelIrV11, WorkgroupSize,
};
use std::collections::BTreeSet;

const ORIGINAL: &[u8] = b"fn baseline(a: u32, b: u32, mask: u32) -> u32 {\n    let selected = b ^ ((a ^ b) & mask);\n    selected\n}\n";
const EXPRESSION: &str = "b ^ ((a ^ b) & mask)";

fn source_range() -> SourceEditRangeV1 {
    let text = str::from_utf8(ORIGINAL).unwrap();
    let start = text.find(EXPRESSION).unwrap();
    SourceEditRangeV1 {
        start: start as u32,
        end: (start + EXPRESSION.len()) as u32,
    }
}

fn binary(op: BinaryOp, result: u32, lhs: u32, rhs: u32, ty: ScalarType) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(result), Type::Scalar(ty)),
        OperationKind::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}

fn module(ty: ScalarType) -> Module {
    let mut module = Module::new("synthetic-ordered-program-materialization");
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    let mut kernel = Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    let mut block = BasicBlock::new(BlockId(0));
    // Deliberately non-monotonic SSA IDs, unrelated to source identifiers.
    block.operations = vec![
        binary(BinaryOp::BitXor, 8, 41, 7, ty),
        binary(BinaryOp::BitAnd, 3, 8, 99, ty),
        binary(BinaryOp::BitXor, 70, 7, 3, ty),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(70)],
    });
    module.functions.push(Function::internal_helper(
        "logical_graph",
        Signature::new(vec![Type::Scalar(ty); 3], vec![Type::Scalar(ty)]),
        vec![ValueId(41), ValueId(7), ValueId(99)],
        vec![block],
    ));
    module
}

fn fixture(
    module: Module,
    target: &str,
    source_spans: [Vec<SourceEditRangeV1>; 3],
) -> AuthoringSnapshotV1 {
    let canonical = VerifiedCanonicalKernelIrV11::from_module(module).unwrap();
    let identity = *canonical.identity();
    let prepared = PreparedSimulationBundleV6::new(
        SimulationSourceLineageV1::new([2; 32], 123, [3; 32], 456).unwrap(),
        SimulationProductionKirIdentityV6::new(11, *identity.digest(), identity.canonical_length())
            .unwrap(),
        target,
        canonical,
    )
    .unwrap();
    let sites = source_spans
        .iter()
        .enumerate()
        .filter_map(|(index, spans)| {
            if spans.is_empty() {
                return None;
            }
            Some(
                DebugSourceMapSiteV1::new(
                    DebugSourceMapKirSiteV1::operation(1, 0, index as u64),
                    spans
                        .iter()
                        .map(|span| {
                            DebugSourceMapSpanV1::new(
                                [5; 32],
                                u64::from(span.start),
                                u64::from(span.end),
                                2,
                                5,
                            )
                            .unwrap()
                        })
                        .collect(),
                )
                .unwrap(),
            )
        })
        .collect();
    let source_map = DebugSourceMapDocumentV2::new(
        prepared.debug_source_map_binding(),
        vec![
            DebugSourceMapFileV1::new([5; 32], ORIGINAL.len() as u64, "baseline.rs".into())
                .unwrap(),
        ],
        sites,
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let semantic = b"synthetic-bitselect-not-authenticated-source".to_vec();
    let storage = SemanticStorageMapV6::new(
        *prepared.subject_identity(),
        1,
        Sha256::digest(&semantic).into(),
        semantic.len() as u64,
        [9; 32],
        *identity.digest(),
        identity.canonical_length(),
        vec![SemanticKernelStorageV1::new(0, 0, 0, vec![])],
        vec![],
    )
    .unwrap();
    let aggregate = SemanticAggregateStorageMapV6::new(
        *prepared.subject_identity(),
        *identity.digest(),
        identity.canonical_length(),
        vec![SemanticKernelStorageV2::new(0, 0, 0, 0, 1, vec![])],
    )
    .unwrap();
    AuthoringSnapshotV1::from_bundle_v6(
        prepared
            .finalize(source_map, semantic, storage, aggregate)
            .unwrap(),
    )
    .unwrap()
}

fn spans() -> [Vec<SourceEditRangeV1>; 3] {
    std::array::from_fn(|_| vec![source_range()])
}

fn snapshot() -> AuthoringSnapshotV1 {
    fixture(module(ScalarType::U32), "gfx942:xnack-", spans())
}

fn request(snapshot: &AuthoringSnapshotV1) -> OrderedProgramMaterializationRequestV1 {
    let summary = snapshot.summary();
    OrderedProgramMaterializationRequestV1 {
        selector: AuthoringRegionSelectorV1 {
            bundle_identity: summary.bundle_identity,
            canonical_kir_digest: summary.canonical_kir_digest,
            target: summary.target,
            operations: (0..3)
                .map(|operation| AuthoringOperationCoordinateV1 {
                    function: 1,
                    block: 0,
                    operation,
                })
                .collect(),
        },
        source: OrderedProgramSourceBindingV1 {
            relative_path: "src/baseline.rs".into(),
            expected_source_sha256: hex(&Sha256::digest(ORIGINAL).into()),
            expected_source_bytes: ORIGINAL.len() as u32,
            source_file_identity: "05".repeat(32),
            source_display_path: "baseline.rs".into(),
            source_range: source_range(),
            inputs: [(41, "a"), (7, "b"), (99, "mask")].map(|(value, identifier)| {
                OrderedProgramValueBindingV1 {
                    value,
                    identifier: identifier.into(),
                }
            }),
            output: OrderedProgramValueBindingV1 {
                value: 70,
                identifier: "selected".into(),
            },
        },
        registers: OrderedProgramRegisterRequestV1 {
            scratch: 32,
            output: 33,
            inputs: [34, 35, 36],
        },
    }
}

fn prepare(
    snapshot: &AuthoringSnapshotV1,
    request: &OrderedProgramMaterializationRequestV1,
) -> Result<OrderedProgramMaterializationPlanV1> {
    prepare_ordered_program_materialization_v1(snapshot, request, ORIGINAL)
}

#[test]
fn exact_graph_renders_explicit_names_and_preserves_all_baseline_identities() {
    let snapshot = snapshot();
    let request = request(&snapshot);
    let plan = prepare(&snapshot, &request).unwrap();
    assert_eq!(plan.baseline(), &snapshot.summary());
    assert_eq!(plan.request(), &request);
    assert_eq!(plan.program().count(), 3);
    assert_eq!(
        plan.program().active_descriptors(),
        &[0x0085, 0x0133, 0x019d]
    );
    assert!(
        plan.program().descriptors()[3..]
            .iter()
            .all(|word| *word == 0)
    );
    assert_eq!(plan.intermediate_values, [8, 3]);
    assert_eq!(plan.registers().vgpr_high_water(), 37);
    assert_eq!(
        plan.expression(),
        concat!(
            "fe2o3_device::amdgpu_ordered_program! {\n",
            "    gfx942_xnack_off_wave64;\n",
            "    scratch(32); out(33);\n",
            "    in(34) = a;\n",
            "    in(35) = b;\n",
            "    in(36) = mask;\n",
            "    xor(scratch, input0, input1);\n",
            "    and(scratch, scratch, input2);\n",
            "    xor(out, input1, scratch);\n",
            "}",
        )
    );
    assert_eq!(
        plan.statement(),
        format!("let selected: u32 = {};\n", plan.expression())
    );
    for inferred in ["v41", "v7", "v99", "v70", "v8", "v3"] {
        assert!(!plan.statement().contains(inferred));
    }
    let json = serde_json::to_value(&plan).unwrap();
    for field in [
        "grants_source_authentication",
        "grants_proof_authority",
        "grants_production_resume",
        "grants_load_or_launch",
        "reversible_rust_roundtrip",
    ] {
        assert_eq!(json[field], false);
    }
    assert_eq!(json["requires_fresh_frontend_admission"], true);
    assert_eq!(
        json["source_application"],
        "unavailable_requires_checked_hir_range_and_scope_binding"
    );
    assert_eq!(json["required_wave_width"], 64);
    assert_eq!(
        json["required_workgroup_size"],
        serde_json::json!([64, 1, 1])
    );
    // Original observations are still ordinary binary operations, never rewritten.
    assert!(
        snapshot
            .select_region(&request.selector)
            .unwrap()
            .operations
            .iter()
            .all(|operation| operation.kind == "binary" && operation.mnemonic.is_none())
    );
    plan.validate_current(&snapshot, "src/baseline.rs", ORIGINAL)
        .unwrap();
    assert_eq!(plan, prepare(&snapshot, &request).unwrap());
}

#[test]
fn checked_program_matches_independent_bitselect_truth_table_and_u32_cases() {
    let snapshot = snapshot();
    let plan = prepare(&snapshot, &request(&snapshot)).unwrap();
    // Every per-bit truth-table combination, at every u32 bit position.
    for bit in 0..32 {
        for a in 0..2 {
            for b in 0..2 {
                for mask in 0..2 {
                    let inputs = [a << bit, b << bit, mask << bit];
                    let expected = (inputs[0] & inputs[2]) | (inputs[1] & !inputs[2]);
                    assert_eq!(plan.program().evaluate(inputs), expected);
                }
            }
        }
    }
    let cases = [0, 1, u32::MAX, 0x80000000, 0xaaaaaaaa, 0x55555555];
    for a in cases {
        for b in cases {
            for mask in cases {
                assert_eq!(
                    plan.program().evaluate([a, b, mask]),
                    (a & mask) | (b & !mask)
                );
            }
        }
    }
}

#[test]
fn source_names_and_physical_registers_are_explicit_independent_choices() {
    let snapshot = snapshot();
    let first = request(&snapshot);
    let baseline = prepare(&snapshot, &first).unwrap();
    let mut changed = first.clone();
    changed.source.inputs[0].identifier = "explicit_other_name".into();
    changed.source.output.identifier = "renamed_result".into();
    changed.registers = OrderedProgramRegisterRequestV1 {
        scratch: 63,
        output: 0,
        inputs: [1, 2, 3],
    };
    let alternative = prepare(&snapshot, &changed).unwrap();
    assert_eq!(alternative.baseline(), baseline.baseline());
    assert_eq!(alternative.program(), baseline.program());
    assert_ne!(alternative, baseline);
    assert!(
        alternative
            .expression()
            .contains("in(1) = explicit_other_name;")
    );
    assert!(
        alternative
            .statement()
            .starts_with("let renamed_result: u32 = ")
    );
    assert_eq!(alternative.registers().vgpr_high_water(), 64);
    assert_eq!(
        alternative.source_binding_status,
        "explicit_caller_names_and_bytes_not_authenticated_ssa_to_source_bindings"
    );
}

#[test]
fn wrong_u32_graph_opcodes_edges_commutations_and_duplicate_live_ins_refuse() {
    for mutation in 0..5 {
        let mut module = module(ScalarType::U32);
        let operations = &mut module.functions[1].body.as_mut().unwrap().blocks[0].operations;
        match mutation {
            0 => operations[0] = binary(BinaryOp::BitOr, 8, 41, 7, ScalarType::U32),
            1 => operations[1] = binary(BinaryOp::BitAnd, 3, 41, 99, ScalarType::U32),
            2 => operations[1] = binary(BinaryOp::BitAnd, 3, 99, 8, ScalarType::U32),
            3 => operations[2] = binary(BinaryOp::BitXor, 70, 3, 7, ScalarType::U32),
            4 => operations[0] = binary(BinaryOp::BitXor, 8, 7, 7, ScalarType::U32),
            _ => unreachable!(),
        }
        let snapshot = fixture(module, "gfx942:xnack-", spans());
        assert_eq!(
            prepare(&snapshot, &request(&snapshot)),
            Err(OrderedProgramMaterializationErrorV1::UnsupportedGraph),
            "mutation {mutation}"
        );
    }
    for ty in [ScalarType::U64, ScalarType::I32] {
        let snapshot = fixture(module(ty), "gfx942:xnack-", spans());
        assert_eq!(
            prepare(&snapshot, &request(&snapshot)),
            Err(OrderedProgramMaterializationErrorV1::UnsupportedGraph)
        );
    }
}

#[test]
fn intermediate_external_operation_uses_and_terminator_escapes_refuse() {
    for escaped in [8, 3] {
        for through_return in [false, true] {
            let mut module = module(ScalarType::U32);
            let function = &mut module.functions[1];
            let block = &mut function.body.as_mut().unwrap().blocks[0];
            if through_return {
                block.terminator = Some(Terminator::Return {
                    values: vec![ValueId(escaped), ValueId(70)],
                });
                function
                    .signature
                    .results
                    .push(Type::Scalar(ScalarType::U32));
            } else {
                block
                    .operations
                    .push(binary(BinaryOp::BitXor, 88, escaped, 41, ScalarType::U32));
            }
            let snapshot = fixture(module, "gfx942:xnack-", spans());
            assert_eq!(
                prepare(&snapshot, &request(&snapshot)),
                Err(OrderedProgramMaterializationErrorV1::UnsupportedGraph)
            );
        }
    }
}

#[test]
fn dead_final_result_is_not_silently_promoted_to_a_live_out() {
    let mut module = module(ScalarType::U32);
    module.functions[1].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(7)],
    });
    let snapshot = fixture(module, "gfx942:xnack-", spans());
    assert_eq!(
        prepare(&snapshot, &request(&snapshot)),
        Err(OrderedProgramMaterializationErrorV1::UnsupportedGraph)
    );
}

#[test]
fn intermediate_escape_through_a_cfg_edge_is_an_external_use() {
    let mut module = module(ScalarType::U32);
    let body = module.functions[1].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(8)],
    });
    let mut successor = BasicBlock::new(BlockId(1));
    successor
        .parameters
        .push(ValueDef::new(ValueId(200), Type::Scalar(ScalarType::U32)));
    successor.terminator = Some(Terminator::Return {
        values: vec![ValueId(70)],
    });
    body.blocks.push(successor);
    let snapshot = fixture(module, "gfx942:xnack-", spans());
    assert_eq!(
        prepare(&snapshot, &request(&snapshot)),
        Err(OrderedProgramMaterializationErrorV1::UnsupportedGraph)
    );
}

#[test]
fn typed_inline_assembly_is_not_relabelled_as_a_pure_selected_graph() {
    let mut module = module(ScalarType::U32);
    module.functions[1].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: "v_xor_b32".into(),
            operands: vec![
                AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
                AssemblyOperand::input(ValueId(41), AssemblyConstraint::Vgpr32),
                AssemblyOperand::input(ValueId(7), AssemblyConstraint::Vgpr32),
            ],
            options: BTreeSet::from([AssemblyOption::NoMemory]),
            declared_effects: BTreeSet::new(),
        });
    let snapshot = fixture(module, "gfx942:xnack-", spans());
    assert_eq!(
        prepare(&snapshot, &request(&snapshot)),
        Err(OrderedProgramMaterializationErrorV1::UnsupportedGraph)
    );
}

#[test]
fn selection_identity_order_and_count_are_exact_not_numeric_ssa_guesses() {
    let snapshot = snapshot();
    let original = request(&snapshot);
    let mut changed = original.clone();
    changed.selector.bundle_identity = "aa".repeat(32);
    assert_eq!(
        prepare(&snapshot, &changed),
        Err(OrderedProgramMaterializationErrorV1::Authoring(
            AuthoringErrorV1::StaleBundleIdentity
        ))
    );
    changed = original.clone();
    changed.selector.canonical_kir_digest = "bb".repeat(32);
    assert_eq!(
        prepare(&snapshot, &changed),
        Err(OrderedProgramMaterializationErrorV1::Authoring(
            AuthoringErrorV1::StaleCanonicalIdentity
        ))
    );
    changed = original.clone();
    changed.selector.operations.swap(0, 1);
    assert_eq!(
        prepare(&snapshot, &changed),
        Err(OrderedProgramMaterializationErrorV1::Authoring(
            AuthoringErrorV1::NonContiguousSelection
        ))
    );
    changed = original.clone();
    changed.selector.operations.pop();
    assert_eq!(
        prepare(&snapshot, &changed),
        Err(OrderedProgramMaterializationErrorV1::UnsupportedGraph)
    );
    changed = original;
    changed.selector.target = "gfx950:xnack-".into();
    assert_eq!(
        prepare(&snapshot, &changed),
        Err(OrderedProgramMaterializationErrorV1::Authoring(
            AuthoringErrorV1::IncompatibleTarget
        ))
    );
}

#[test]
fn explicit_value_mapping_must_match_all_four_boundary_values_in_role_order() {
    let snapshot = snapshot();
    let original = request(&snapshot);
    for index in 0..4 {
        let mut changed = original.clone();
        if index == 3 {
            changed.source.output.value = 8;
        } else {
            changed.source.inputs[index].value = 70;
        }
        assert_eq!(
            prepare(&snapshot, &changed),
            Err(OrderedProgramMaterializationErrorV1::InvalidValueBindings)
        );
    }
    let mut changed = original;
    changed.source.inputs.swap(0, 1);
    assert_eq!(
        prepare(&snapshot, &changed),
        Err(OrderedProgramMaterializationErrorV1::InvalidValueBindings)
    );
}

#[test]
fn keywords_expressions_unicode_raw_names_injection_and_aliases_refuse() {
    let snapshot = snapshot();
    let original = request(&snapshot);
    for name in [
        "",
        "_",
        "type",
        "self",
        "gen",
        "a.b",
        "a()",
        "a + b",
        "r#name",
        "café",
        "a;\n evil()",
        "9name",
        "mask",
    ] {
        let mut changed = original.clone();
        changed.source.inputs[0].identifier = name.into();
        assert_eq!(
            prepare(&snapshot, &changed),
            Err(OrderedProgramMaterializationErrorV1::InvalidSourceIdentifier),
            "{name:?}"
        );
    }
    let mut changed = original.clone();
    changed.source.output.identifier = "a".into();
    assert_eq!(
        prepare(&snapshot, &changed),
        Err(OrderedProgramMaterializationErrorV1::InvalidSourceIdentifier)
    );
    changed.source.output.identifier = "z".repeat(65);
    assert_eq!(
        prepare(&snapshot, &changed),
        Err(OrderedProgramMaterializationErrorV1::InvalidSourceIdentifier)
    );
}

#[test]
fn every_register_role_is_range_checked_and_pairwise_distinct() {
    let snapshot = snapshot();
    let original = request(&snapshot);
    let make = |bindings: [u8; 5]| OrderedProgramRegisterRequestV1 {
        scratch: bindings[0],
        output: bindings[1],
        inputs: [bindings[2], bindings[3], bindings[4]],
    };
    for index in 0..5 {
        let mut bindings = [32, 33, 34, 35, 36];
        bindings[index] = 64;
        let mut changed = original.clone();
        changed.registers = make(bindings);
        assert_eq!(
            prepare(&snapshot, &changed),
            Err(OrderedProgramMaterializationErrorV1::InvalidRegisters)
        );
        for previous in 0..index {
            let mut bindings = [32, 33, 34, 35, 36];
            bindings[index] = bindings[previous];
            changed.registers = make(bindings);
            assert_eq!(
                prepare(&snapshot, &changed),
                Err(OrderedProgramMaterializationErrorV1::InvalidRegisters)
            );
        }
    }
}

#[test]
fn caller_source_byte_commitment_is_independent_and_rechecked() {
    let snapshot = snapshot();
    let request = request(&snapshot);
    let plan = prepare(&snapshot, &request).unwrap();
    let mut changed = ORIGINAL.to_vec();
    changed[3] = b'B';
    assert_eq!(
        plan.validate_current(&snapshot, "src/baseline.rs", &changed),
        Err(SourceEditErrorV1::StaleSource.into())
    );
    assert_eq!(
        prepare_ordered_program_materialization_v1(&snapshot, &request, &changed),
        Err(SourceEditErrorV1::StaleSource.into())
    );
    assert_eq!(
        plan.validate_current(&snapshot, "src/other.rs", ORIGINAL),
        Err(SourceEditErrorV1::PathMismatch.into())
    );
    assert_ne!(
        request.source.source_file_identity,
        request.source.expected_source_sha256
    );
    let mut invalid = request.clone();
    invalid.source.expected_source_sha256 = "AA".repeat(32);
    assert_eq!(
        prepare(&snapshot, &invalid),
        Err(SourceEditErrorV1::InvalidDigest.into())
    );
    invalid = request;
    invalid.source.expected_source_bytes -= 1;
    assert_eq!(
        prepare(&snapshot, &invalid),
        Err(SourceEditErrorV1::StaleSource.into())
    );
}

#[test]
fn source_paths_spans_and_requested_ranges_are_bound_without_claiming_hir() {
    let snapshot = snapshot();
    let original = request(&snapshot);
    for path in [
        "../baseline.rs",
        "/tmp/baseline.rs",
        "src//baseline.rs",
        "src/x.txt",
    ] {
        let mut changed = original.clone();
        changed.source.relative_path = path.into();
        assert_eq!(
            prepare(&snapshot, &changed),
            Err(SourceEditErrorV1::UnsafePath.into())
        );
    }
    let mut changed = original.clone();
    changed.source.source_file_identity = "06".repeat(32);
    assert_eq!(
        prepare(&snapshot, &changed),
        Err(SourceEditErrorV1::SourceSpanSubstitution.into())
    );
    changed = original.clone();
    changed.source.source_display_path = "different.rs".into();
    assert_eq!(
        prepare(&snapshot, &changed),
        Err(SourceEditErrorV1::SourceSpanSubstitution.into())
    );
    for range in [
        SourceEditRangeV1 { start: 0, end: 0 },
        SourceEditRangeV1 { start: 3, end: 2 },
        SourceEditRangeV1 {
            start: 0,
            end: ORIGINAL.len() as u32 + 1,
        },
        SourceEditRangeV1 {
            start: source_range().start + 1,
            end: source_range().end,
        },
    ] {
        changed = original.clone();
        changed.source.source_range = range;
        assert_eq!(
            prepare(&snapshot, &changed),
            Err(SourceEditErrorV1::InvalidByteRange.into())
        );
    }
}

#[test]
fn missing_or_ambiguous_source_origins_refuse_but_distinct_enclosed_spans_preserve() {
    let mut origins = spans();
    origins[1].clear();
    let snapshot = fixture(module(ScalarType::U32), "gfx942:xnack-", origins);
    assert_eq!(
        prepare(&snapshot, &request(&snapshot)),
        Err(SourceEditErrorV1::MissingSourceSpan.into())
    );
    let mut origins = spans();
    let extra = SourceEditRangeV1 {
        start: source_range().start + 1,
        end: source_range().end - 1,
    };
    origins[1].push(extra);
    let snapshot = fixture(module(ScalarType::U32), "gfx942:xnack-", origins);
    assert_eq!(
        prepare(&snapshot, &request(&snapshot)),
        Err(SourceEditErrorV1::AmbiguousSourceSpan.into())
    );
    let mut origins = spans();
    origins[1] = vec![extra];
    let snapshot = fixture(module(ScalarType::U32), "gfx942:xnack-", origins);
    let plan = prepare(&snapshot, &request(&snapshot)).unwrap();
    assert_eq!(
        plan.selected_source_spans()[1].byte_start,
        extra.start.to_string()
    );
    assert_ne!(
        plan.selected_source_spans()[0],
        plan.selected_source_spans()[1]
    );
}

#[test]
fn original_source_and_ranges_are_utf8_bounded_and_nonempty() {
    let snapshot = snapshot();
    let request = request(&snapshot);
    assert_eq!(
        prepare_ordered_program_materialization_v1(&snapshot, &request, b""),
        Err(SourceEditErrorV1::EmptySource.into())
    );
    assert_eq!(
        prepare_ordered_program_materialization_v1(&snapshot, &request, &[0xff]),
        Err(SourceEditErrorV1::InvalidUtf8.into())
    );
    let oversized = vec![b'x'; MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 + 1];
    assert_eq!(
        prepare_ordered_program_materialization_v1(&snapshot, &request, &oversized),
        Err(SourceEditErrorV1::ResourceLimit.into())
    );
    assert_eq!(
        validate_nonempty_range("é", SourceEditRangeV1 { start: 1, end: 2 }),
        Err(SourceEditErrorV1::InvalidByteRange.into())
    );
    assert_eq!(
        validate_nonempty_range("é", SourceEditRangeV1 { start: 0, end: 1 }),
        Err(SourceEditErrorV1::InvalidByteRange.into())
    );
}

#[test]
fn plan_revalidation_refuses_a_new_owner_even_when_graph_has_same_meaning() {
    let first = snapshot();
    let plan = prepare(&first, &request(&first)).unwrap();
    let mut module = module(ScalarType::U32);
    module.functions[1].body.as_mut().unwrap().blocks[0]
        .operations
        .push(binary(BinaryOp::BitXor, 88, 41, 7, ScalarType::U32));
    let changed = fixture(module, "gfx942:xnack-", spans());
    assert_eq!(
        plan.validate_current(&changed, "src/baseline.rs", ORIGINAL),
        Err(AuthoringErrorV1::StaleBundleIdentity.into())
    );
}

#[test]
fn request_serde_rejects_unknown_fields_and_wrong_fixed_binding_counts() {
    let snapshot = snapshot();
    let request = request(&snapshot);
    let mut value = serde_json::to_value(&request).unwrap();
    value["source"]["trust_names"] = true.into();
    assert!(serde_json::from_value::<OrderedProgramMaterializationRequestV1>(value).is_err());
    let mut value = serde_json::to_value(&request).unwrap();
    value["source"]["inputs"].as_array_mut().unwrap().pop();
    assert!(serde_json::from_value::<OrderedProgramMaterializationRequestV1>(value).is_err());
}

#[test]
fn source_map_changes_cannot_reuse_a_plan_for_identical_canonical_graph_bytes() {
    let first = snapshot();
    let plan = prepare(&first, &request(&first)).unwrap();
    let mut origins = spans();
    origins[1][0].start += 1;
    let changed = fixture(module(ScalarType::U32), "gfx942:xnack-", origins);
    assert_eq!(
        first.summary().canonical_kir_digest,
        changed.summary().canonical_kir_digest
    );
    assert_ne!(
        first.summary().source_map_identity,
        changed.summary().source_map_identity
    );
    assert_eq!(
        plan.validate_current(&changed, "src/baseline.rs", ORIGINAL),
        Err(AuthoringErrorV1::StaleBundleIdentity.into())
    );
}
