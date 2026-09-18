use super::super::*;
use super::{CanonicalCommitmentSinkV1, DOMAIN};

fn constant(bits: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1(0),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap()),
    ))
}

fn place(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1(local), vec![], SemanticTypeIdV1(0)).unwrap()
}

fn function() -> SemanticFunctionDeclV1 {
    let ty = SemanticTypeIdV1(0);
    let source = SemanticSourceProvenanceV1::unavailable();
    let mode = SemanticAbiPassModeV1::Direct(
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap(),
    );
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1([1; 32]),
        SemanticLayoutIdentityV1([2; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![SemanticAbiValueV1::new(ty, mode.clone())],
        SemanticAbiValueV1::new(ty, mode),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1([3; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1([4; 32]),
        SemanticMonomorphizationIdentityV1([5; 32]),
        SemanticGenericTypeArgumentsIdentityV1([6; 32]),
        SemanticConstGenericArgumentsIdentityV1([7; 32]),
        source,
        abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1([8; 32]),
                ty,
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1([9; 32]),
                ty,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
        ],
        SemanticBlockIdV1(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1([10; 32]),
                source,
                vec![SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(0),
                        SemanticRvalueV1::new(ty, SemanticRvalueKindV1::Use(constant(7))),
                    )),
                )],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn commit(function: &SemanticFunctionDeclV1) -> SemanticFunctionCanonicalCommitmentV1 {
    canonical_function_commitment_v1(
        function,
        SemanticMirWireVersionV1::V29,
        SemanticMirLimitsV1::default(),
        &mut |_| Ok(()),
    )
    .unwrap()
}

fn mutation(
    original: &SemanticFunctionDeclV1,
    name: &str,
    change: impl FnOnce(&mut SemanticFunctionDeclV1),
) {
    let mut changed = original.clone();
    change(&mut changed);
    assert_ne!(
        commit(original).sha256(),
        commit(&changed).sha256(),
        "{name}"
    );
}

fn charge(used: &mut u64, amount: usize, max: u64) -> Result<(), SemanticMirErrorV1> {
    *used += u64::try_from(amount).unwrap();
    if *used > max {
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            actual: *used,
            max,
        })
    } else {
        Ok(())
    }
}

#[test]
fn function_commitment_matches_legacy_encoder_and_is_version_bound() {
    let function = rich_function(64);
    let versions = [
        SemanticMirWireVersionV1::V2,
        SemanticMirWireVersionV1::V3,
        SemanticMirWireVersionV1::V4,
        SemanticMirWireVersionV1::V5,
        SemanticMirWireVersionV1::V6,
        SemanticMirWireVersionV1::V7,
        SemanticMirWireVersionV1::V8,
        SemanticMirWireVersionV1::V9,
        SemanticMirWireVersionV1::V10,
        SemanticMirWireVersionV1::V11,
        SemanticMirWireVersionV1::V12,
        SemanticMirWireVersionV1::V13,
        SemanticMirWireVersionV1::V14,
        SemanticMirWireVersionV1::V15,
        SemanticMirWireVersionV1::V28,
        SemanticMirWireVersionV1::V29,
        SemanticMirWireVersionV1::V30,
    ];
    let mut seen = BTreeSet::new();
    for version in versions {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_function(&mut writer, &function, version).unwrap();
        let bytes = writer.finish();
        let mut expected = Sha256::new();
        expected.update(DOMAIN);
        expected.update(version.as_u16().to_le_bytes());
        expected.update(&bytes);
        let expected: [u8; 32] = expected.finalize().into();
        let actual = canonical_function_commitment_v1(
            &function,
            version,
            SemanticMirLimitsV1::default(),
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(actual.wire_version(), version);
        assert_eq!(actual.canonical_bytes(), bytes.len() as u64);
        assert_eq!(actual.sha256(), expected);
        assert!(seen.insert(expected));
    }
}

#[test]
fn function_commitment_detects_ordinary_constant_and_metadata_substitutions() {
    let original = function();
    let mut changed = original.clone();
    let SemanticStatementKindV1::Assign(assignment) = &mut changed.blocks[0].statements[0].kind
    else {
        panic!()
    };
    assignment.value.kind = SemanticRvalueKindV1::Use(constant(11));
    assert_eq!(
        commit(&original).canonical_bytes(),
        commit(&changed).canonical_bytes()
    );
    assert_ne!(commit(&original), commit(&changed));
    mutation(&original, "function identity", |f| f.identity.0[0] ^= 1);
    mutation(&original, "item identity", |f| {
        f.item_definition_identity.0[0] ^= 1
    });
    mutation(&original, "monomorphization", |f| {
        f.monomorphization_identity.0[0] ^= 1
    });
    mutation(&original, "generic arguments", |f| {
        f.generic_type_arguments_identity.0[0] ^= 1
    });
    mutation(&original, "const arguments", |f| {
        f.const_generic_arguments_identity.0[0] ^= 1
    });
    mutation(&original, "role", |f| {
        f.role = SemanticFunctionRoleV1::KernelRoot
    });
    mutation(&original, "entry block", |f| f.entry = SemanticBlockIdV1(1));
    mutation(&original, "local", |f| f.locals[1].identity.0[0] ^= 1);
    mutation(&original, "block", |f| f.blocks[0].identity.0[0] ^= 1);
    mutation(&original, "terminator", |f| {
        f.blocks[0].terminator.kind = SemanticTerminatorKindV1::Unreachable
    });
    mutation(&original, "ownership", |f| {
        f.abi.source_argument_ownership[0] = SemanticSourceArgumentOwnershipV1::ByValue
    });
}

fn call_function() -> SemanticFunctionDeclV1 {
    let mut f = function();
    f.blocks[0].terminator.kind = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
            SemanticCallableIdV1(1),
            vec![constant(7), constant(11)],
            vec![f.abi.return_value.clone()],
            Some(SemanticCallDestinationV1::new(
                place(0),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1(1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    f
}

fn mutate_call(f: &mut SemanticFunctionDeclV1, change: impl FnOnce(&mut SemanticDirectCallV1)) {
    let SemanticTerminatorKindV1::Call(call) = &mut f.blocks[0].terminator.kind else {
        panic!()
    };
    change(call);
}

#[test]
fn function_commitment_covers_complete_call_payload() {
    let f = call_function();
    mutation(&f, "callee", |f| {
        mutate_call(f, |c| c.callee = SemanticCallableIdV1(2))
    });
    mutation(&f, "argument value", |f| {
        mutate_call(f, |c| c.arguments[0] = constant(11))
    });
    mutation(&f, "argument order", |f| {
        mutate_call(f, |c| c.arguments.swap(0, 1))
    });
    mutation(&f, "destination", |f| {
        mutate_call(f, |c| c.destination.as_mut().unwrap().place = place(1))
    });
    mutation(&f, "continuation", |f| {
        mutate_call(f, |c| {
            c.destination.as_mut().unwrap().edge.target = SemanticBlockIdV1(2)
        })
    });
    mutation(&f, "no continuation", |f| {
        mutate_call(f, |c| c.destination = None)
    });
    mutation(&f, "unwind", |f| {
        mutate_call(f, |c| c.unwind = SemanticUnwindActionV1::Terminate)
    });
    mutation(&f, "variadic ABI", |f| {
        mutate_call(f, |c| {
            c.variadic_argument_abis[0].mode = SemanticAbiPassModeV1::Ignore
        })
    });
    mutation(&f, "tail call", |f| {
        f.blocks[0].terminator.kind = SemanticTerminatorKindV1::TailCall(
            SemanticDirectTailCallV1::new_callable(
                SemanticCallableIdV1(1),
                vec![constant(7), constant(11)],
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        );
    });
}

#[test]
fn function_commitment_covers_assert_operands_and_edges() {
    let mut original = function();
    original.blocks[0].terminator.kind = SemanticTerminatorKindV1::Assert {
        condition: constant(1),
        expected: true,
        message: SemanticAssertMessageV1::BoundsCheck {
            length: constant(11),
            index: constant(7),
        },
        target: SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::AssertSuccess,
            SemanticBlockIdV1(1),
        ),
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    for index in 0..6 {
        mutation(&original, "assert payload", |f| {
            let SemanticTerminatorKindV1::Assert {
                condition,
                expected,
                message,
                target,
                unwind,
            } = &mut f.blocks[0].terminator.kind
            else {
                panic!()
            };
            match index {
                0 => *condition = constant(0),
                1 => *expected = false,
                2 => {
                    *message = SemanticAssertMessageV1::BoundsCheck {
                        length: constant(12),
                        index: constant(7),
                    }
                }
                3 => {
                    *message = SemanticAssertMessageV1::BoundsCheck {
                        length: constant(11),
                        index: constant(8),
                    }
                }
                4 => target.target = SemanticBlockIdV1(2),
                5 => *unwind = SemanticUnwindActionV1::Terminate,
                _ => unreachable!(),
            }
        });
    }
}

fn adjusted_function() -> SemanticFunctionDeclV1 {
    let mut f = function();
    let layout = SemanticTypeLayoutV1::aggregate(
        Some(16),
        4,
        SemanticAggregateLayoutV1::new(vec![0, 8], vec![SemanticPaddingV1::new(4, 4).unwrap()])
            .unwrap(),
    )
    .unwrap();
    f.abi.return_value.adjusted = Some(Box::new(SemanticAbiAdjustedTypeV1::new(
        SemanticTypeIdV1(1),
        SemanticLayoutIdentityV1([12; 32]),
        layout,
    )));
    f
}

#[test]
fn function_commitment_covers_embedded_layout_not_only_abi_identity() {
    let original = adjusted_function();
    mutation(&original, "adjusted layout identity", |f| {
        f.abi
            .return_value
            .adjusted
            .as_mut()
            .unwrap()
            .layout_identity
            .0[0] ^= 1
    });
    mutation(&original, "adjusted layout size", |f| {
        f.abi
            .return_value
            .adjusted
            .as_mut()
            .unwrap()
            .layout
            .rustc_size_bytes = 20
    });
    mutation(&original, "field offset", |f| {
        let SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            ..
        } = &mut f.abi.return_value.adjusted.as_mut().unwrap().layout.fields
        else {
            panic!()
        };
        source_order_offsets_bytes[1] = 12;
    });
    mutation(&original, "padding", |f| {
        let SemanticTypeLayoutDetailsV1::Aggregate(aggregate) =
            &mut f.abi.return_value.adjusted.as_mut().unwrap().layout.details
        else {
            panic!()
        };
        aggregate.padding[0].size_bytes = 3;
    });
}

#[test]
fn function_commitment_byte_limit_is_cumulative_and_exact() {
    let f = adjusted_function();
    let expected = commit(&f);
    let exact = SemanticMirLimitsV1::default()
        .with_limit(
            SemanticMirResourceV1::CanonicalBytes,
            expected.canonical_bytes(),
        )
        .unwrap();
    assert_eq!(canonical_function_commitment_v1(
        &f, SemanticMirWireVersionV1::V29, exact, &mut |_| Ok(()),
    ).unwrap(), expected);
    let short = exact
        .with_limit(
            SemanticMirResourceV1::CanonicalBytes,
            expected.canonical_bytes() - 1,
        )
        .unwrap();
    assert_eq!(
        canonical_function_commitment_v1(&f, SemanticMirWireVersionV1::V29, short, &mut |_| Ok(()),),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            actual: expected.canonical_bytes(),
            max: expected.canonical_bytes() - 1,
        })
    );
}

#[test]
fn function_commitment_caller_ledger_is_cumulative_and_not_refunded() {
    let f = adjusted_function();
    let mut total = 0;
    let expected = canonical_function_commitment_v1(
        &f,
        SemanticMirWireVersionV1::V29,
        SemanticMirLimitsV1::default(),
        &mut |n| charge(&mut total, n, u64::MAX),
    )
    .unwrap();
    let max = total;
    let mut exact = 0;
    assert_eq!(
        canonical_function_commitment_v1(
            &f,
            SemanticMirWireVersionV1::V29,
            SemanticMirLimitsV1::default(),
            &mut |n| charge(&mut exact, n, max),
        )
        .unwrap(),
        expected
    );
    assert_eq!(exact, max);
    let mut used = 0;
    let mut ledger = |n| charge(&mut used, n, 2 * max - 1);
    assert_eq!(
        canonical_function_commitment_v1(
            &f,
            SemanticMirWireVersionV1::V29,
            SemanticMirLimitsV1::default(),
            &mut ledger,
        )
        .unwrap(),
        expected
    );
    assert_eq!(
        canonical_function_commitment_v1(
            &f,
            SemanticMirWireVersionV1::V29,
            SemanticMirLimitsV1::default(),
            &mut ledger,
        ),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            actual: 2 * max,
            max: 2 * max - 1,
        })
    );
    assert_eq!(used, 2 * max);
}

#[test]
fn function_commitment_propagates_every_hook_failure_including_finalization() {
    let f = rich_function(64);
    let mut calls = 0;
    canonical_function_commitment_v1(
        &f,
        SemanticMirWireVersionV1::V29,
        SemanticMirLimitsV1::default(),
        &mut |_| {
            calls += 1;
            Ok(())
        },
    )
    .unwrap();
    let error = SemanticMirErrorV1::AllocationFailed {
        resource: SemanticMirResourceV1::Functions,
    };
    for fail_at in 1..=calls {
        let mut observed = 0;
        assert_eq!(
            canonical_function_commitment_v1(
                &f,
                SemanticMirWireVersionV1::V29,
                SemanticMirLimitsV1::default(),
                &mut |_| {
                    observed += 1;
                    if observed == fail_at {
                        Err(error.clone())
                    } else {
                        Ok(())
                    }
                },
            ),
            Err(error.clone())
        );
        assert_eq!(observed, fail_at);
    }
}

#[test]
fn function_commitment_writer_never_allocates_payload_and_prepays_walks() {
    let mut sha = Sha256::new();
    let mut work = 0;
    let mut ledger = |n| charge(&mut work, n, u64::MAX);
    let mut writer = CanonicalWriterV1::new(512);
    writer.commitment = Some(CanonicalCommitmentSinkV1 {
        sha256: &mut sha,
        charge_work: &mut ledger,
    });
    for _ in 0..512 {
        writer.u8(7).unwrap();
    }
    assert_eq!(writer.written, 512);
    assert_eq!(writer.bytes.len(), 0);
    assert_eq!(writer.bytes.capacity(), 0);
    assert!(matches!(
        writer.u8(7),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            actual: 513,
            max: 512,
        })
    ));
    assert_eq!(writer.written, 512);
    drop(writer);
    assert_eq!(work, 1026);
    let expected: [u8; 32] = Sha256::digest([7; 512]).into();
    assert_eq!(<[u8; 32]>::from(sha.finalize()), expected);

    let mut sha = Sha256::new();
    let mut observed = vec![];
    let error = SemanticMirErrorV1::InvalidFunctionAbi;
    let mut reject = |n| {
        observed.push(n);
        Err(error.clone())
    };
    let mut writer = CanonicalWriterV1::new(0);
    writer.commitment = Some(CanonicalCommitmentSinkV1 {
        sha256: &mut sha,
        charge_work: &mut reject,
    });
    assert_eq!(writer.count(23), Err(error.clone()));
    assert_eq!(writer.raw(&[0; 5]), Err(error.clone()));
    assert_eq!(writer.written, 0);
    drop(writer);
    assert_eq!(observed, vec![23, 6]);
    assert_eq!(sha.finalize(), Sha256::digest([]));
}

fn rich_function(blob_len: usize) -> SemanticFunctionDeclV1 {
    let mut f = adjusted_function();
    let origin =
        SemanticSourceOriginV1::new(SemanticSourceFileIdentityV1([13; 32]), 2, 8, 1, 2, 1, 8)
            .unwrap();
    let source = SemanticSourceProvenanceV1::new(Some(origin), Some(origin));
    f.blocks[0].source = source;
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), SemanticTypeIdV1(0))
                .unwrap(),
        ],
        SemanticTypeIdV1(0),
    )
    .unwrap();
    let bytes = SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1(0),
        SemanticConstantValueV1::Bytes(SemanticConstantBytesV1::new(vec![7; blob_len]).unwrap()),
    ));
    f.blocks[0].statements = vec![
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(0),
                SemanticRvalueV1::new(SemanticTypeIdV1(0), SemanticRvalueKindV1::Use(bytes)),
            )),
        ),
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(0),
                SemanticRvalueV1::new(
                    SemanticTypeIdV1(0),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected)),
                ),
            )),
        ),
        SemanticStatementV1::new(source, SemanticStatementKindV1::Nop),
    ]
    .into_boxed_slice();
    f
}

#[test]
fn function_commitment_binds_blobs_projections_source_and_statement_order() {
    let f = rich_function(64);
    mutation(&f, "same-length bytes", |f| {
        let SemanticStatementKindV1::Assign(assignment) = &mut f.blocks[0].statements[0].kind
        else {
            panic!()
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant)) =
            &mut assignment.value.kind
        else {
            panic!()
        };
        let SemanticConstantValueV1::Bytes(bytes) = &mut constant.value else {
            panic!()
        };
        bytes.0[17] ^= 1;
    });
    for change in 0..3 {
        mutation(&f, "projected operand", |f| {
            let SemanticStatementKindV1::Assign(assignment) = &mut f.blocks[0].statements[1].kind
            else {
                panic!()
            };
            let SemanticRvalueKindV1::Use(operand) = &mut assignment.value.kind else {
                panic!()
            };
            let SemanticOperandV1::Copy(place) = operand else {
                panic!()
            };
            match change {
                0 => *operand = SemanticOperandV1::Move(place.clone()),
                1 => place.projections[0].kind = SemanticProjectionKindV1::Field(1),
                2 => place.projections[0].result_type = SemanticTypeIdV1(1),
                _ => unreachable!(),
            }
        });
    }
    mutation(&f, "source byte coordinate", |f| {
        f.blocks[0].source.expansion.as_mut().unwrap().byte_start += 1
    });
    mutation(&f, "source call site", |f| {
        f.blocks[0].statements[0]
            .source
            .call_site
            .as_mut()
            .unwrap()
            .column_start += 1
    });
    mutation(&f, "statement order", |f| f.blocks[0].statements.swap(0, 1));
    mutation(&f, "Nop removal", |f| {
        f.blocks[0].statements = f.blocks[0].statements[..2].into()
    });

    let cost = |len| {
        let mut work = 0;
        canonical_function_commitment_v1(
            &rich_function(len),
            SemanticMirWireVersionV1::V29,
            SemanticMirLimitsV1::default(),
            &mut |n| charge(&mut work, n, u64::MAX),
        )
        .unwrap();
        work
    };
    assert_eq!(cost(128) - cost(64), 64);
}
