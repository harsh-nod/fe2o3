//! Synthetic canonical fixtures only; no source compilation or native evidence.
use super::*;
use fe2o3_kernel_ir::{CastKind, Constant};

fn const_module(assembly: bool, bits: u32, constant_left: bool) -> Module {
    let mut module = fixture_module(assembly, ScalarType::U32, "v_or_b32", None, false);
    let helper = &mut module.functions[1];
    helper.signature.parameters = vec![Type::Scalar(ScalarType::U32)];
    let body = helper.body.as_mut().unwrap();
    body.parameters = vec![ValueId(40)];
    let (lhs, rhs) = if constant_left { (7, 40) } else { (40, 7) };
    body.blocks[0].operations[0] = if assembly {
        instruction(
            "v_or_b32",
            90,
            lhs,
            rhs,
            ScalarType::U32,
            &BTreeSet::from([AssemblyOption::NoMemory]),
        )
    } else {
        Operation::effect_free(
            ValueDef::new(ValueId(90), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        )
    };
    body.blocks[0].operations.insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(bits)),
        ),
    );
    module
}

fn const_snapshot(module: Module) -> AuthoringSnapshotV1 {
    AuthoringSnapshotV1::from_bundle_v6(fixture_bundle(module, "gfx942:xnack-", false)).unwrap()
}

fn one(snapshot: &AuthoringSnapshotV1, block: u32, operation: u32) -> AuthoringRegionSelectorV1 {
    let mut selected = selector(snapshot, 1);
    selected.operations[0].block = block;
    selected.operations[0].operation = operation;
    selected
}

#[test]
fn const_u32_helper_derives_exact_bits_and_keeps_order_boundary_and_owner() {
    for assembly in [false, true] {
        for constant_left in [false, true] {
            for bits in [0, 256, u32::MAX] {
                let snapshot = const_snapshot(const_module(assembly, bits, constant_left));
                let selected = one(&snapshot, 0, 1);
                let before = snapshot.bundle.canonical_kir_v11().to_vec();
                let summary = snapshot.summary();
                let old_draft = snapshot
                    .materialize_typed_rust(&selected, "baseline")
                    .unwrap();
                let candidate = snapshot
                    .materialize_const_u32_helper_v1(&selected, "specialized")
                    .unwrap();
                assert_eq!(candidate.const_parameter.original_value, bits);
                assert_eq!(candidate.const_parameter.name, "C0");
                assert_eq!(candidate.const_parameter.value.value, 7);
                assert_eq!(candidate.const_parameter.value.ty, "Scalar(U32)");
                assert_eq!(
                    candidate.const_parameter.definition,
                    AuthoringOperationCoordinateV1 {
                        function: 1,
                        block: 0,
                        operation: 0,
                    }
                );
                assert_eq!(candidate.runtime_parameters[0].value, 40);
                assert_eq!(candidate.region.live_in.len(), 2);
                assert_eq!(candidate.region.live_out[0].value, 90);
                assert_eq!(
                    candidate.original_call_template,
                    format!("specialized::<{bits}u32>(v40)")
                );
                assert!(candidate.source.contains("#[inline(never)]"));
                assert!(
                    candidate
                        .source
                        .contains("pub fn specialized<const C0: u32>(v40: u32) -> (u32,)")
                );
                let operands = if constant_left { "C0, v40" } else { "v40, C0" };
                assert!(
                    candidate
                        .source
                        .contains(&format!("amdgpu_asm!(v_or_b32({operands}))"))
                );
                assert!(candidate.source.ends_with("    (v90,)\n}\n"));
                assert_eq!(candidate.authority, AUTHORITY);
                assert_eq!(candidate.semantic_equivalence, "unproved");
                assert_eq!(candidate.exact_machine_contract, "unproved");
                assert_eq!(
                    candidate.frontend_readmission,
                    "not_performed_requires_fresh_source_compilation"
                );
                assert_eq!(
                    candidate.source_application,
                    "unavailable_requires_explicit_new_source_and_normal_frontend"
                );
                assert_eq!(
                    candidate.physical_register_bindings,
                    "unavailable_compiler_owned_scalar_values"
                );
                assert!(candidate.source.len() <= MAX_CONST_U32_HELPER_SOURCE_BYTES_V1);
                assert!(
                    serde_json::to_vec(&candidate).unwrap().len() <= MAX_AUTHORING_REPORT_BYTES_V1
                );
                assert_eq!(
                    candidate,
                    snapshot
                        .materialize_const_u32_helper_v1(&selected, "specialized")
                        .unwrap()
                );
                assert_eq!(before, snapshot.bundle.canonical_kir_v11());
                assert_eq!(summary, snapshot.summary());
                assert_eq!(
                    old_draft,
                    snapshot
                        .materialize_typed_rust(&selected, "baseline")
                        .unwrap()
                );
            }
        }
    }
}

#[test]
fn const_u32_helper_closes_the_instruction_profile_for_ordinary_and_typed_inputs() {
    for (op, mnemonic) in [
        (BinaryOp::BitAnd, "v_and_b32"),
        (BinaryOp::BitOr, "v_or_b32"),
        (BinaryOp::BitXor, "v_xor_b32"),
    ] {
        for assembly in [false, true] {
            let mut module = const_module(assembly, 1, false);
            module.functions[1].body.as_mut().unwrap().blocks[0].operations[1] = if assembly {
                instruction(
                    mnemonic,
                    90,
                    40,
                    7,
                    ScalarType::U32,
                    &BTreeSet::from([AssemblyOption::NoMemory]),
                )
            } else {
                Operation::effect_free(
                    ValueDef::new(ValueId(90), Type::Scalar(ScalarType::U32)),
                    OperationKind::Binary {
                        op,
                        lhs: ValueId(40),
                        rhs: ValueId(7),
                    },
                )
            };
            let snapshot = const_snapshot(module);
            let draft = snapshot
                .materialize_const_u32_helper_v1(&one(&snapshot, 0, 1), "compute")
                .unwrap();
            assert!(draft.source.contains(&format!("{mnemonic}(v40, C0)")));
        }
    }
    for assembly in [false, true] {
        let mut module = const_module(assembly, 1, false);
        module.functions[1].body.as_mut().unwrap().blocks[0].operations[1] = if assembly {
            instruction(
                "v_add_u32",
                90,
                40,
                7,
                ScalarType::U32,
                &BTreeSet::from([AssemblyOption::NoMemory]),
            )
        } else {
            Operation::effect_free(
                ValueDef::new(ValueId(90), Type::Scalar(ScalarType::U32)),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(40),
                    rhs: ValueId(7),
                },
            )
        };
        let snapshot = const_snapshot(module);
        assert_eq!(
            snapshot.materialize_const_u32_helper_v1(&one(&snapshot, 0, 1), "compute"),
            Err(AuthoringErrorV1::UnsupportedMaterialization)
        );
    }
}

#[test]
fn const_u32_helper_refuses_no_constant_both_constants_and_constant_aliases() {
    let snapshot = snapshot(false);
    assert_eq!(
        snapshot.materialize_const_u32_helper_v1(&selector(&snapshot, 1), "compute"),
        Err(AuthoringErrorV1::UnsupportedMaterialization)
    );
    let mut both = const_module(false, 256, false);
    let helper = &mut both.functions[1];
    helper.signature.parameters.clear();
    let body = helper.body.as_mut().unwrap();
    body.parameters.clear();
    body.blocks[0].operations.insert(
        0,
        Operation::effect_free(
            ValueDef::new(ValueId(40), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(19)),
        ),
    );
    let snapshot = const_snapshot(both);
    assert_eq!(
        snapshot.materialize_const_u32_helper_v1(&one(&snapshot, 0, 2), "compute"),
        Err(AuthoringErrorV1::UnsupportedMaterialization)
    );
    for marker_alias in [false, true] {
        let mut module = const_module(false, 256, false);
        let operations = &mut module.functions[1].body.as_mut().unwrap().blocks[0].operations;
        if !marker_alias {
            // A same-type integer bitcast is invalid KIR. Use a legal
            // same-width F32-to-U32 cast so refusal tests alias handling.
            operations[0] = Operation::effect_free(
                ValueDef::new(ValueId(7), Type::Scalar(ScalarType::F32)),
                OperationKind::Constant(Constant::F32Bits(256)),
            );
        }
        operations.insert(
            1,
            if marker_alias {
                instruction(
                    "v_mov_b32",
                    8,
                    7,
                    0,
                    ScalarType::U32,
                    &BTreeSet::from([AssemblyOption::NoMemory]),
                )
            } else {
                Operation::effect_free(
                    ValueDef::new(ValueId(8), Type::Scalar(ScalarType::U32)),
                    OperationKind::Cast {
                        kind: CastKind::Bitcast,
                        value: ValueId(7),
                        to: Type::Scalar(ScalarType::U32),
                    },
                )
            },
        );
        operations[2].kind = OperationKind::Binary {
            op: BinaryOp::BitOr,
            lhs: ValueId(40),
            rhs: ValueId(8),
        };
        let snapshot = const_snapshot(module);
        assert_eq!(
            snapshot.materialize_const_u32_helper_v1(&one(&snapshot, 0, 2), "compute"),
            Err(AuthoringErrorV1::UnsupportedMaterialization)
        );
    }
}

#[test]
fn const_u32_helper_refuses_cross_block_constants_and_block_argument_aliases() {
    for via_argument in [false, true] {
        let mut module = fixture_module(false, ScalarType::U32, "", None, true);
        let helper = &mut module.functions[1];
        helper.signature.parameters = vec![Type::Scalar(ScalarType::U32)];
        let body = helper.body.as_mut().unwrap();
        body.parameters = vec![ValueId(40)];
        body.blocks[0].operations.insert(
            0,
            Operation::effect_free(
                ValueDef::new(ValueId(7), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(256)),
            ),
        );
        if via_argument {
            body.blocks[0].terminator = Some(Terminator::Branch {
                target: BlockId(1),
                arguments: vec![ValueId(7)],
            });
        } else {
            body.blocks[1].operations[0].kind = OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(10),
                rhs: ValueId(7),
            };
        }
        let snapshot = const_snapshot(module);
        assert_eq!(
            snapshot.materialize_const_u32_helper_v1(&one(&snapshot, 1, 0), "compute"),
            Err(AuthoringErrorV1::UnsupportedMaterialization)
        );
    }
}

#[test]
fn const_u32_helper_preserves_refusals_for_type_target_effects_and_live_out() {
    for target in ["gfx950:xnack-", "gfx942:xnack+"] {
        let snapshot = AuthoringSnapshotV1::from_bundle_v6(fixture_bundle(
            const_module(false, 256, false),
            target,
            false,
        ))
        .unwrap();
        assert_eq!(
            snapshot.materialize_const_u32_helper_v1(&one(&snapshot, 0, 1), "compute"),
            Err(AuthoringErrorV1::UnsupportedMaterialization)
        );
    }
    let snapshot = AuthoringSnapshotV1::from_bundle_v6(fixture_bundle(
        fixture_module(false, ScalarType::U64, "", None, false),
        "gfx942:xnack-",
        false,
    ))
    .unwrap();
    assert_eq!(
        snapshot.materialize_const_u32_helper_v1(&selector(&snapshot, 1), "compute"),
        Err(AuthoringErrorV1::UnsupportedMaterialization)
    );
    let mut module = const_module(true, 256, false);
    let OperationKind::InlineAssembly(assembly) =
        &mut module.functions[1].body.as_mut().unwrap().blocks[0].operations[1].kind
    else {
        unreachable!();
    };
    assembly.options.insert(AssemblyOption::Pure);
    let snapshot = const_snapshot(module);
    assert_eq!(
        snapshot.materialize_const_u32_helper_v1(&one(&snapshot, 0, 1), "compute"),
        Err(AuthoringErrorV1::UnsupportedMaterialization)
    );
    let mut module = const_module(false, 256, false);
    module.functions[1].body.as_mut().unwrap().blocks[0].operations[2].kind =
        OperationKind::Binary {
            op: BinaryOp::BitXor,
            lhs: ValueId(40),
            rhs: ValueId(40),
        };
    let snapshot = const_snapshot(module);
    assert_eq!(
        snapshot.materialize_const_u32_helper_v1(&one(&snapshot, 0, 1), "compute"),
        Err(AuthoringErrorV1::InvalidBoundary)
    );
}

#[test]
fn const_u32_helper_keeps_exact_selector_name_and_size_limits() {
    let snapshot = const_snapshot(const_module(false, u32::MAX, false));
    let original = one(&snapshot, 0, 1);
    for (field, error) in [
        ("bundle", AuthoringErrorV1::StaleBundleIdentity),
        ("canonical", AuthoringErrorV1::StaleCanonicalIdentity),
        ("target", AuthoringErrorV1::IncompatibleTarget),
    ] {
        let mut changed = original.clone();
        match field {
            "bundle" => changed.bundle_identity = "0".repeat(64),
            "canonical" => changed.canonical_kir_digest = "0".repeat(64),
            _ => changed.target = "gfx950:xnack-".into(),
        }
        assert_eq!(
            snapshot.materialize_const_u32_helper_v1(&changed, "compute"),
            Err(error)
        );
    }
    for name in ["", "_", "fn", "gen", "bad();", "r#fn", "é"] {
        assert_eq!(
            snapshot.materialize_const_u32_helper_v1(&original, name),
            Err(AuthoringErrorV1::InvalidHelperName)
        );
    }
    assert_eq!(
        snapshot.materialize_const_u32_helper_v1(&original, &"a".repeat(65)),
        Err(AuthoringErrorV1::InvalidHelperName)
    );
    let draft = snapshot
        .materialize_const_u32_helper_v1(&original, &"a".repeat(64))
        .unwrap();
    assert!(draft.source.len() <= MAX_CONST_U32_HELPER_SOURCE_BYTES_V1);
    let mut changed = original.clone();
    changed.operations.push(AuthoringOperationCoordinateV1 {
        function: 1,
        block: 0,
        operation: 2,
    });
    assert_eq!(
        snapshot.materialize_const_u32_helper_v1(&changed, "compute"),
        Err(AuthoringErrorV1::UnsupportedMaterialization)
    );
    changed.operations.resize(65, original.operations[0]);
    assert_eq!(
        snapshot.materialize_const_u32_helper_v1(&changed, "compute"),
        Err(AuthoringErrorV1::ResourceLimit)
    );
}
