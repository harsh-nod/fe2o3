//! Synthetic codec fixtures only: these tests do not authenticate a source build.

use super::*;
use fe2o3_kernel_ir::{
    AssemblyConstraint, AssemblyOperand, AssemblySourceIdentity, BasicBlock, BinaryOp, BlockId,
    DebugSourceMapFileV1, DebugSourceMapKirSiteV1, DebugSourceMapSiteV1, InlineAssembly,
    InlineAssemblyTarget, Kernel, LaunchDomain, LaunchExtent, PreparedSimulationBundleV6,
    SemanticAggregateStorageMapV6, SemanticKernelStorageV1, SemanticKernelStorageV2,
    SemanticStorageMapV6, Signature, SimulationProductionKirIdentityV6, SimulationSourceLineageV1,
    ValueDef, VerifiedCanonicalKernelIrV11, WorkgroupSize,
};
use sha2::{Digest, Sha256};

fn instruction(
    mnemonic: &str,
    result: u32,
    lhs: u32,
    rhs: u32,
    ty: ScalarType,
    options: &BTreeSet<AssemblyOption>,
) -> Operation {
    let descriptor = fe2o3_kernel_ir::gfx942_inline_assembly_instruction_v1(mnemonic).unwrap();
    let mut operands = vec![
        AssemblyOperand::output(0, descriptor.constraint()),
        AssemblyOperand::input(ValueId(lhs), descriptor.constraint()),
    ];
    if descriptor.input_count() == 2 {
        operands.push(AssemblyOperand::input(
            ValueId(rhs),
            descriptor.constraint(),
        ));
    }
    Operation::effect_free(
        ValueDef::new(ValueId(result), Type::Scalar(ty)),
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: mnemonic.to_owned(),
            operands,
            options: options.clone(),
            declared_effects: BTreeSet::new(),
        }),
    )
}

fn fixture_module(
    assembly: bool,
    ty: ScalarType,
    mnemonic: &str,
    extra_option: Option<AssemblyOption>,
    branch: bool,
) -> Module {
    let mut module = Module::new("synthetic-authoring-codec-fixture");
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
    let mut options = BTreeSet::from([AssemblyOption::NoMemory]);
    if let Some(option) = extra_option {
        options.insert(option);
    }
    let mut block = BasicBlock::new(BlockId(0));
    let first = if assembly {
        instruction(mnemonic, 90, 40, 7, ty, &options)
    } else {
        Operation::effect_free(
            ValueDef::new(ValueId(90), Type::Scalar(ty)),
            OperationKind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(40),
                rhs: ValueId(7),
            },
        )
    };
    let lhs = if branch { 10 } else { 90 };
    let second = if assembly {
        instruction("v_xor_b32", 3, lhs, 40, ty, &options)
    } else {
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::Scalar(ty)),
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(lhs),
                rhs: ValueId(40),
            },
        )
    };
    block.operations.push(first);
    let blocks = if branch {
        block.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(90)],
        });
        let mut successor = BasicBlock::new(BlockId(1));
        successor
            .parameters
            .push(ValueDef::new(ValueId(10), Type::Scalar(ty)));
        successor.operations.push(second);
        successor.terminator = Some(Terminator::Return {
            values: vec![ValueId(3)],
        });
        vec![block, successor]
    } else {
        block.operations.push(second);
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(3)],
        });
        vec![block]
    };
    module.functions.push(Function::internal_helper(
        "synthetic_helper",
        Signature::new(
            vec![Type::Scalar(ty), Type::Scalar(ty)],
            vec![Type::Scalar(ty)],
        ),
        vec![ValueId(40), ValueId(7)],
        blocks,
    ));
    module
}

fn fixture_bundle(
    module: Module,
    target: &str,
    invalid_source_site: bool,
) -> VerifiedSimulationBundleV6 {
    let branch = module.functions[1].body.as_ref().unwrap().blocks.len() == 2;
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
    let first_span = DebugSourceMapSpanV1::new([5; 32], 1, 4, 1, 2).unwrap();
    let second_span = DebugSourceMapSpanV1::new([5; 32], 5, 8, 2, 1).unwrap();
    let source_map = DebugSourceMapDocumentV2::new(
        prepared.debug_source_map_binding(),
        vec![DebugSourceMapFileV1::new([5; 32], 32, "synthetic.rs".into()).unwrap()],
        vec![
            DebugSourceMapSiteV1::new(
                DebugSourceMapKirSiteV1::operation(1, 0, if invalid_source_site { 99 } else { 0 }),
                vec![first_span, second_span],
            )
            .unwrap(),
            DebugSourceMapSiteV1::new(
                DebugSourceMapKirSiteV1::operation(1, u64::from(branch), u64::from(!branch)),
                vec![first_span],
            )
            .unwrap(),
        ],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let semantic = b"synthetic-inert-semantic-bytes-not-a-source-build".to_vec();
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
    prepared
        .finalize(source_map, semantic, storage, aggregate)
        .unwrap()
}

fn snapshot(assembly: bool) -> AuthoringSnapshotV1 {
    AuthoringSnapshotV1::from_bundle_v6(fixture_bundle(
        fixture_module(assembly, ScalarType::U32, "v_add_u32", None, false),
        "gfx942:xnack-",
        false,
    ))
    .unwrap()
}

fn selector(snapshot: &AuthoringSnapshotV1, count: u32) -> AuthoringRegionSelectorV1 {
    let summary = snapshot.summary();
    AuthoringRegionSelectorV1 {
        bundle_identity: summary.bundle_identity,
        canonical_kir_digest: summary.canonical_kir_digest,
        target: summary.target,
        operations: (0..count)
            .map(|operation| AuthoringOperationCoordinateV1 {
                function: 1,
                block: 0,
                operation,
            })
            .collect(),
    }
}

#[test]
fn scalar_bundle_inspection_preserves_exact_identity_and_many_source_origins() {
    let snapshot = snapshot(false);
    let summary = snapshot.summary();
    assert_eq!(summary.operation_count, 2);
    assert_eq!(summary.canonical_kir_version, 11);
    assert_eq!(summary.authority, AUTHORITY);
    assert_eq!(
        summary.final_artifact_identity,
        "unavailable_extraction_precedes_final_artifact"
    );
    assert!(!summary.authority.source_authenticated);
    let page = snapshot
        .operation_page(&summary.bundle_identity, 0, 1)
        .unwrap();
    assert_eq!(page.next_start, Some(1));
    assert_eq!(page.operations[0].kind, "binary");
    assert_eq!(page.operations[0].semantic_detail.as_deref(), Some("BitOr"));
    assert_eq!(page.operations[0].source_spans.len(), 2);
    assert_eq!(page.operations[0].source_spans[0].byte_start, "1");
    assert_eq!(
        page.operations[0]
            .inputs
            .iter()
            .map(|value| value.value)
            .collect::<Vec<_>>(),
        [40, 7]
    );
    let tail = snapshot
        .operation_page(&summary.bundle_identity, 1, 64)
        .unwrap();
    assert_eq!(tail.next_start, None);
    assert_eq!(
        tail.operations[0].semantic_detail.as_deref(),
        Some("BitXor")
    );
    assert_eq!(
        tail.operations[0].source_spans[0],
        page.operations[0].source_spans[0]
    );
    let region = snapshot.select_region(&selector(&snapshot, 2)).unwrap();
    assert_eq!(region.live_out[0].value, 3);
    assert_eq!(
        snapshot.materialize_typed_rust(&selector(&snapshot, 2), "candidate"),
        Err(AuthoringErrorV1::UnsupportedMaterialization)
    );
    let encoded = serde_json::to_value(summary).unwrap();
    assert!(encoded["canonical_kir_bytes"].is_string());
}

#[test]
fn diagnostic_draft_keeps_typed_operands_and_definition_order_without_authority() {
    let snapshot = snapshot(true);
    let selection = selector(&snapshot, 2);
    let candidate = snapshot
        .materialize_typed_rust(&selection, "authored_region")
        .unwrap();
    assert_eq!(
        candidate,
        snapshot
            .materialize_typed_rust(&selection, "authored_region")
            .unwrap()
    );
    assert_eq!(
        candidate
            .live_in
            .iter()
            .map(|value| value.value)
            .collect::<Vec<_>>(),
        [40, 7]
    );
    assert_eq!(
        candidate
            .live_out
            .iter()
            .map(|value| value.value)
            .collect::<Vec<_>>(),
        [3]
    );
    assert!(
        candidate
            .source
            .contains("pub fn authored_region(v40: u32, v7: u32) -> (u32,)")
    );
    assert!(
        candidate
            .source
            .contains("let v90: u32 = fe2o3_device::amdgpu_asm!(v_add_u32(v40, v7));")
    );
    assert!(
        candidate
            .source
            .contains("let v3: u32 = fe2o3_device::amdgpu_asm!(v_xor_b32(v90, v40));")
    );
    assert!(candidate.source.ends_with("    (v3,)\n}\n"));
    assert_eq!(candidate.authority, AUTHORITY);
    assert_eq!(candidate.semantic_equivalence, "unproved");
    assert_eq!(candidate.exact_machine_contract, "unproved");
    assert!(candidate.frontend_readmission.starts_with("unavailable"));
    let first_only = snapshot.select_region(&selector(&snapshot, 1)).unwrap();
    assert_eq!(first_only.live_out[0].value, 90);
}

#[test]
fn region_boundary_includes_branch_argument_uses_and_rejects_cross_block_selection() {
    let snapshot = AuthoringSnapshotV1::from_bundle_v6(fixture_bundle(
        fixture_module(true, ScalarType::U32, "v_add_u32", None, true),
        "gfx942:xnack-",
        false,
    ))
    .unwrap();
    let mut selection = selector(&snapshot, 1);
    let region = snapshot.select_region(&selection).unwrap();
    assert_eq!(
        region
            .live_out
            .iter()
            .map(|value| value.value)
            .collect::<Vec<_>>(),
        [90]
    );
    selection.operations.push(AuthoringOperationCoordinateV1 {
        function: 1,
        block: 1,
        operation: 0,
    });
    assert_eq!(
        snapshot.select_region(&selection),
        Err(AuthoringErrorV1::NonContiguousSelection)
    );
}

#[test]
fn exact_identity_target_and_selection_checks_fail_closed() {
    let snapshot = snapshot(true);
    let original = selector(&snapshot, 2);
    let mut changed = original.clone();
    changed.bundle_identity = "0".repeat(64);
    assert_eq!(
        snapshot.select_region(&changed),
        Err(AuthoringErrorV1::StaleBundleIdentity)
    );
    changed = original.clone();
    changed.canonical_kir_digest = "0".repeat(64);
    assert_eq!(
        snapshot.select_region(&changed),
        Err(AuthoringErrorV1::StaleCanonicalIdentity)
    );
    changed = original.clone();
    changed.target = "gfx950:xnack-".into();
    assert_eq!(
        snapshot.select_region(&changed),
        Err(AuthoringErrorV1::IncompatibleTarget)
    );
    changed = original.clone();
    changed.operations.reverse();
    assert_eq!(
        snapshot.select_region(&changed),
        Err(AuthoringErrorV1::NonContiguousSelection)
    );
    changed = original.clone();
    changed.operations[1] = changed.operations[0];
    assert_eq!(
        snapshot.select_region(&changed),
        Err(AuthoringErrorV1::NonContiguousSelection)
    );
    changed = original.clone();
    changed.operations[0].function = u32::MAX;
    assert_eq!(
        snapshot.select_region(&changed),
        Err(AuthoringErrorV1::InvalidCoordinate)
    );
    changed = original.clone();
    changed.operations.clear();
    assert_eq!(
        snapshot.select_region(&changed),
        Err(AuthoringErrorV1::EmptySelection)
    );
    changed = original.clone();
    changed.operations.resize(65, original.operations[0]);
    assert_eq!(
        snapshot.select_region(&changed),
        Err(AuthoringErrorV1::ResourceLimit)
    );
    assert_eq!(
        snapshot.operation_page(&original.bundle_identity, 0, 65),
        Err(AuthoringErrorV1::InvalidPage)
    );
    assert_eq!(
        snapshot.operation_page(&original.bundle_identity, 0, 0),
        Err(AuthoringErrorV1::InvalidPage)
    );
    assert_eq!(
        snapshot.operation_page(&original.bundle_identity, 3, 1),
        Err(AuthoringErrorV1::InvalidPage)
    );
    assert_eq!(
        snapshot.operation_page("wrong", 0, 1),
        Err(AuthoringErrorV1::StaleBundleIdentity)
    );
    assert!(
        snapshot
            .operation_page(&original.bundle_identity, 2, 1)
            .unwrap()
            .operations
            .is_empty()
    );
    let different = AuthoringSnapshotV1::from_bundle_v6(fixture_bundle(
        fixture_module(true, ScalarType::U32, "v_sub_u32", None, false),
        "gfx942:xnack-",
        false,
    ))
    .unwrap();
    assert_eq!(
        different.select_region(&original),
        Err(AuthoringErrorV1::StaleBundleIdentity)
    );
}

#[test]
fn unsupported_target_signedness_scalar_registers_and_options_remain_inspectable() {
    for (ty, mnemonic, option, target) in [
        (ScalarType::U32, "v_add_u32", None, "gfx950:xnack-"),
        (ScalarType::I32, "v_add_u32", None, "gfx942:xnack-"),
        (ScalarType::U32, "s_mov_b32", None, "gfx942:xnack-"),
        (
            ScalarType::U32,
            "v_add_u32",
            Some(AssemblyOption::Pure),
            "gfx942:xnack-",
        ),
        (
            ScalarType::U32,
            "v_add_u32",
            Some(AssemblyOption::PreservesFlags),
            "gfx942:xnack-",
        ),
        (
            ScalarType::U32,
            "v_add_u32",
            Some(AssemblyOption::NoStack),
            "gfx942:xnack-",
        ),
    ] {
        let snapshot = AuthoringSnapshotV1::from_bundle_v6(fixture_bundle(
            fixture_module(true, ty, mnemonic, option, false),
            target,
            false,
        ))
        .unwrap();
        let selection = selector(&snapshot, 1);
        assert!(snapshot.select_region(&selection).is_ok());
        assert_eq!(
            snapshot.materialize_typed_rust(&selection, "candidate"),
            Err(AuthoringErrorV1::UnsupportedMaterialization)
        );
    }
}

#[test]
fn source_map_coordinates_are_checked_against_the_exact_program() {
    let bundle = fixture_bundle(
        fixture_module(false, ScalarType::U32, "v_add_u32", None, false),
        "gfx942:xnack-",
        true,
    );
    assert!(matches!(
        AuthoringSnapshotV1::from_bundle_v6(bundle),
        Err(AuthoringErrorV1::InvalidSourceMap)
    ));
}

#[test]
fn hostile_names_unknown_fields_and_growth_limits_reject() {
    let snapshot = snapshot(true);
    let selection = selector(&snapshot, 1);
    for name in [
        "",
        "_",
        "fn",
        "Self",
        "try",
        "gen",
        "candidate();bad",
        "a\nb",
        "1candidate",
        "r#fn",
        "é",
    ] {
        assert_eq!(
            snapshot.materialize_typed_rust(&selection, name),
            Err(AuthoringErrorV1::InvalidHelperName)
        );
    }
    assert_eq!(
        snapshot.materialize_typed_rust(&selection, &"a".repeat(65)),
        Err(AuthoringErrorV1::InvalidHelperName)
    );
    let mut json = serde_json::to_value(&selection).unwrap();
    json.as_object_mut()
        .unwrap()
        .insert("resume_production".into(), true.into());
    assert!(serde_json::from_value::<AuthoringRegionSelectorV1>(json).is_err());
    let mut text = BoundedText::new(4);
    assert!(write!(text, "1234").is_ok());
    assert!(write!(text, "5").is_err());
    assert_eq!(text.text, "1234");
    assert_eq!(
        bounded_report("x".repeat(MAX_AUTHORING_REPORT_BYTES_V1)),
        Err(AuthoringErrorV1::ResourceLimit)
    );
    let mut scan = MAX_AUTHORING_SCAN_ITEMS_V1;
    assert_eq!(charge(&mut scan, 1), Err(AuthoringErrorV1::ResourceLimit));
    scan = usize::MAX;
    assert_eq!(charge(&mut scan, 1), Err(AuthoringErrorV1::ResourceLimit));
    assert_eq!(
        AssemblyConstraint::Vgpr32,
        fe2o3_kernel_ir::gfx942_inline_assembly_instruction_v1("v_add_u32")
            .unwrap()
            .constraint()
    );
}

#[test]
fn scalar_details_retain_opcodes_and_full_width_literal_bits() {
    use fe2o3_kernel_ir::{CastKind, ComparePredicate, Constant, UnaryOp};
    for (kind, expected) in [
        (
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(1),
            },
            "Not",
        ),
        (
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
            "Equal",
        ),
        (
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(1),
                to: Type::Scalar(ScalarType::U32),
            },
            "Bitcast",
        ),
        (
            OperationKind::Constant(Constant::U64(u64::MAX)),
            "U64(18446744073709551615)",
        ),
    ] {
        assert_eq!(semantic_detail(&kind).unwrap().as_deref(), Some(expected));
    }
}
