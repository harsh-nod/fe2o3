// Compose the independently validated source recipes with the admitted policy
// fixture. These tests exercise inert custody, not source-provider or SSA proof.
include!("defined_math_document_permutation_tests.rs");
include!("defined_math_document_codec_tests.rs");

fn defined_math_mapped_type(
    map: &[Option<SemanticTypeIdV1>],
    ty: SemanticTypeIdV1,
) -> SemanticTypeIdV1 {
    map.get(ty.index() as usize)
        .copied()
        .flatten()
        .expect("fixture type reference must belong to the selected imported closure")
}

fn defined_math_retype_abi(abi: &mut SemanticFunctionAbiV1, map: &[Option<SemanticTypeIdV1>]) {
    for ty in &mut abi.source_signature.inputs {
        *ty = defined_math_mapped_type(map, *ty);
    }
    abi.source_signature.output = defined_math_mapped_type(map, abi.source_signature.output);
    for argument in &mut abi.arguments {
        argument.value.source_ty = defined_math_mapped_type(map, argument.ty());
    }
    abi.return_value.source_ty = defined_math_mapped_type(map, abi.return_value.ty());
}

fn defined_math_retype_place(place: &mut SemanticPlaceV1, map: &[Option<SemanticTypeIdV1>]) {
    place.ty = defined_math_mapped_type(map, place.ty);
    for projection in &mut place.projections {
        projection.result_type = defined_math_mapped_type(map, projection.result_type);
    }
}

fn defined_math_retype_operand(operand: &mut SemanticOperandV1, map: &[Option<SemanticTypeIdV1>]) {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            defined_math_retype_place(place, map);
        }
        SemanticOperandV1::Constant(constant) => {
            assert!(matches!(
                constant.value(),
                SemanticConstantValueV1::ZeroSized
            ));
            constant.ty = defined_math_mapped_type(map, constant.ty);
        }
    }
}

fn defined_math_retype_body(body: &mut SemanticFunctionDeclV1, map: &[Option<SemanticTypeIdV1>]) {
    defined_math_retype_abi(&mut body.abi, map);
    for local in &mut body.locals {
        local.ty = defined_math_mapped_type(map, local.ty);
    }
    for block in &mut body.blocks {
        for statement in &mut block.statements {
            let SemanticStatementKindV1::Assign(assignment) = &mut statement.kind else {
                panic!()
            };
            defined_math_retype_place(&mut assignment.destination, map);
            assignment.value.result_type =
                defined_math_mapped_type(map, assignment.value.result_type);
            match &mut assignment.value.kind {
                SemanticRvalueKindV1::Aggregate(aggregate) => {
                    for operand in &mut aggregate.operands {
                        defined_math_retype_operand(operand, map);
                    }
                }
                SemanticRvalueKindV1::Borrow { place, .. } => defined_math_retype_place(place, map),
                _ => panic!("unexpected source recipe"),
            }
        }
        match &mut block.terminator.kind {
            SemanticTerminatorKindV1::Call(call) => {
                for argument in &mut call.arguments {
                    defined_math_retype_operand(argument, map);
                }
                defined_math_retype_place(&mut call.destination.as_mut().unwrap().place, map);
            }
            SemanticTerminatorKindV1::Return => {}
            _ => panic!("unexpected source recipe"),
        }
    }
}

fn defined_math_attach(request: &mut InertSemanticMirRequestV1) {
    let consumer = policy_math_contract(SemanticF32MathFunctionV1::Sqrt);
    let derive = SemanticKernelMathDeriveV1::for_defined_function(
        SemanticFunctionIdV1(1),
        &request.functions,
        &request.callables,
        &request.types,
        SemanticKernelMathDeriveTypesV1::new([2, 1, 7, 11].map(SemanticTypeIdV1)),
        consumer.provenance(),
        consumer.kernel_brand(),
    )
    .unwrap();
    let bind = SemanticPolicyMathBindV1::for_defined_function(
        SemanticFunctionIdV1(3),
        &request.functions,
        &request.callables,
        &request.types,
        SemanticPolicyMathBindTypesV1::new([6, 7, 8, 3, 5].map(SemanticTypeIdV1)),
        consumer.provenance(),
        consumer.policy(),
        consumer.kernel_brand(),
    )
    .unwrap();
    for (index, contract) in [
        (
            1,
            SemanticDefinedCapabilityContractV1::KernelMathDerive(derive),
        ),
        (3, SemanticDefinedCapabilityContractV1::PolicyMathBind(bind)),
    ] {
        request.functions[index] = request.functions[index]
            .clone()
            .with_defined_capability_contract(contract)
            .unwrap();
    }
}

fn defined_math_document_request(epoch: bool) -> InertSemanticMirRequestV1 {
    let mut request = policy_math_request(SemanticF32MathFunctionV1::Sqrt);
    let mut f = crate::semantic_mir_v1::defined_math_v1::tests::fixture();
    let map = [1, 2, 7, 11, 6, 3, 8, 5, 10].map(|id| Some(SemanticTypeIdV1(id)));
    let mut types = request.types.to_vec();
    let mut unbranded = f.declarations[3].clone();
    unbranded.identity = SemanticTypeIdentityV1(identity(190));
    unbranded.layout_identity = SemanticLayoutIdentityV1(identity(190));
    types.push(unbranded);
    // Bind's actual Rust ABI returns two references, not a memory-class value.
    types[5].layout = f.declarations[7].layout.clone();
    let pointee = SemanticAbiPointeeInfoV1::new(
        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
        0,
        1,
    )
    .unwrap();
    types[5].abi_properties = types[5]
        .abi_properties
        .with_scalar_pointee_info(Some(pointee), Some(pointee));
    for function in &mut f.functions {
        defined_math_retype_body(function, &map);
        for argument in &mut function.abi.arguments {
            argument.value.mode = numerical_abi().arguments[0].value.mode.clone();
        }
    }
    let prefix = if epoch { 5 } else { 4 };
    let SemanticTerminatorKindV1::Call(getter_call) = &mut f.functions[0].blocks[0].terminator.kind
    else {
        panic!()
    };
    getter_call.callee = SemanticCallableIdV1(2);
    let SemanticTerminatorKindV1::Call(bridge_call) = &mut f.functions[1].blocks[0].terminator.kind
    else {
        panic!()
    };
    bridge_call.callee = SemanticCallableIdV1(prefix);
    let mut current = f.callables.pop().unwrap();
    let SemanticCallableDeclV1::CompilerIntrinsic {
        binding, operation, ..
    } = &mut current
    else {
        panic!()
    };
    defined_math_retype_abi(&mut binding.abi, &map);
    *operation = SemanticCompilerIntrinsicOperationV1::MathContextCurrent {
        context: SemanticTypeIdV1(11),
    };
    let mut functions = request.functions.to_vec();
    functions.extend(f.functions);
    let mut callables = (0..prefix)
        .map(|id| SemanticCallableDeclV1::defined(SemanticFunctionIdV1(id)))
        .collect::<Vec<_>>();
    callables.push(current);
    callables.extend(request.callables[1..].iter().cloned());

    let source = SemanticSourceProvenanceV1::unavailable();
    let place = |local, ty| {
        SemanticPlaceV1::new(SemanticLocalIdV1(local), vec![], SemanticTypeIdV1(ty)).unwrap()
    };
    let borrow = |destination, reference, owner, owned| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(destination, reference),
                SemanticRvalueV1::new(
                    SemanticTypeIdV1(reference),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(owner, owned),
                    },
                ),
            )),
        )
    };
    let call = |callee, arguments, local, ty, next| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(local, ty),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1(next),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let block = |tag, statements, kind| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(tag)),
            source,
            statements,
            SemanticTerminatorV1::new(source, kind),
        )
        .unwrap()
    };
    let mut blocks = functions[0].blocks[..2].to_vec();
    for block in &mut blocks {
        let SemanticTerminatorKindV1::Call(call) = &mut block.terminator.kind else {
            panic!()
        };
        call.callee = SemanticCallableIdV1(call.callee.index() + prefix);
    }
    blocks.push(block(
        132,
        vec![],
        call(1, vec![SemanticOperandV1::Copy(place(2, 2))], 6, 7, 3),
    ));
    blocks.push(block(
        133,
        vec![borrow(7, 6, 6, 7), borrow(8, 8, 3, 3)],
        call(
            3,
            vec![
                SemanticOperandV1::Copy(place(7, 6)),
                SemanticOperandV1::Move(place(8, 8)),
            ],
            9,
            5,
            4,
        ),
    ));
    let mut consumer = functions[0].blocks[2].clone();
    consumer.identity = SemanticBlockIdentityV1(identity(134));
    consumer.statements = vec![borrow(4, 4, 9, 5)].into_boxed_slice();
    let SemanticTerminatorKindV1::Call(consumer_call) = &mut consumer.terminator.kind else {
        panic!()
    };
    consumer_call.callee = SemanticCallableIdV1(3 + prefix);
    consumer_call.destination.as_mut().unwrap().edge.target = SemanticBlockIdV1(5);
    blocks.push(consumer);
    if epoch {
        let borrowed = borrowed_workgroup_request(true);
        let mut epoch_map = vec![None; borrowed.types.len()];
        for (index, entry) in epoch_map.iter_mut().take(4).enumerate() {
            *entry = Some(SemanticTypeIdV1(index as u32));
        }
        // This fixture calls only WorkgroupDerive and epoch(). The independent
        // fixture's u32 and subgroup types (4, 11, 12) are not in that closure.
        for (index, original) in (5..=10).enumerate() {
            epoch_map[original] = Some(SemanticTypeIdV1((types.len() + index) as u32));
        }
        for (index, mut ty) in borrowed.types[5..=10].iter().cloned().enumerate() {
            // Compose after Math's identity 190; retain the selected type edges.
            ty.identity = SemanticTypeIdentityV1(identity(191 + index as u8));
            ty.layout_identity = SemanticLayoutIdentityV1(identity(191 + index as u8));
            match &mut ty.shape {
                SemanticTypeShapeV1::Aggregate(aggregate) => {
                    for field in &mut aggregate.fields {
                        *field = defined_math_mapped_type(&epoch_map, *field);
                    }
                }
                SemanticTypeShapeV1::Pointer(pointer) => {
                    pointer.pointee = defined_math_mapped_type(&epoch_map, pointer.pointee)
                }
                SemanticTypeShapeV1::Scalar(_) => {}
                _ => panic!(),
            }
            types.push(ty);
        }
        let mut getter = borrowed_workgroup_getter();
        getter.identity = SemanticFunctionIdentityV1(identity(230));
        defined_math_retype_body(&mut getter, &epoch_map);
        let epoch_types = SemanticWorkgroupEpochProjectionTypesV1::new(
            borrowed_workgroup_types()
                .all()
                .map(|ty| defined_math_mapped_type(&epoch_map, ty)),
        );
        let projection = SemanticWorkgroupEpochProjectionV1::for_defined_function(
            SemanticFunctionIdV1(4),
            &getter,
            epoch_types,
            numerical_contract().provenance(),
            SemanticTypeIdentityV1(identity(107)),
            SemanticTypeIdentityV1(identity(108)),
        )
        .unwrap();
        functions.push(getter.with_workgroup_epoch_projection(projection).unwrap());
        let mut derive = borrowed.callables[5].clone();
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        } = &mut derive
        else {
            panic!()
        };
        defined_math_retype_abi(&mut binding.abi, &epoch_map);
        let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } = operation
        else {
            panic!()
        };
        contract.operation = SemanticExecutionCapabilityOperationV1::WorkgroupDerive {
            context: SemanticTypeIdV1(1),
            workgroup: epoch_types.workgroup,
        };
        contract.signature.output = epoch_types.workgroup;
        callables.push(derive);
        let mut locals = functions[0].locals.to_vec();
        for (index, ty) in [
            epoch_types.workgroup,
            epoch_types.reference,
            epoch_types.epoch_reference,
        ]
        .into_iter()
        .enumerate()
        {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1(identity(180 + index as u8)),
                ty,
                SemanticLocalRoleV1::Temporary,
                source,
            ));
        }
        functions[0].locals = locals.into_boxed_slice();
        blocks.push(block(
            135,
            vec![],
            call(
                prefix + 4,
                vec![SemanticOperandV1::Copy(place(1, 1))],
                11,
                epoch_types.workgroup.index(),
                6,
            ),
        ));
        blocks.push(block(
            136,
            vec![borrow(
                12,
                epoch_types.reference.index(),
                11,
                epoch_types.workgroup.index(),
            )],
            call(
                4,
                vec![SemanticOperandV1::Copy(place(
                    12,
                    epoch_types.reference.index(),
                ))],
                13,
                epoch_types.epoch_reference.index(),
                7,
            ),
        ));
    }
    blocks.push(block(137, vec![], SemanticTerminatorKindV1::Return));
    functions[0].blocks = blocks.into_boxed_slice();
    // The composed fixture keeps its call/edge indices; only its synthetic
    // block identities are assigned in the required deterministic order.
    for (index, function) in functions.iter().enumerate() {
        assert!(
            function
                .blocks()
                .windows(2)
                .all(|pair| pair[0].identity() < pair[1].identity()),
            "epoch={epoch}: fixture function {index} block identity order before encoding"
        );
    }
    request.types = types.into_boxed_slice();
    assert!(
        request
            .types
            .windows(2)
            .all(|pair| pair[0].identity() < pair[1].identity()),
        "epoch={epoch}: fixture type identity order before encoding"
    );
    request.functions = functions.into_boxed_slice();
    request.callables = callables.into_boxed_slice();
    defined_math_attach(&mut request);
    request
}

fn defined_math_metadata_bytes(contract: SemanticDefinedCapabilityContractV1) -> Vec<u8> {
    let mut writer = CanonicalWriterV1::new(1024);
    contract.encode(&mut writer).unwrap();
    writer.finish()
}

#[test]
fn defined_math_v21_document_roundtrips_preserve_bodies_and_canonical_indices() {
    for epoch in [false, true] {
        let request = defined_math_document_request(epoch);
        let bodies = request.functions.clone();
        let admitted = request
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap_or_else(|error| panic!("epoch={epoch}: original admission: {error:?}"));
        assert_eq!(admitted.wire_version(), SemanticMirWireVersionV1::V21);
        let decoded = AdmittedInertSemanticMirV1::decode_exact_v21_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap_or_else(|error| panic!("epoch={epoch}: exact V21 decode: {error:?}"));
        assert_eq!(decoded.functions(), bodies.as_ref());
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
        assert_eq!(decoded.semantic_sha256(), admitted.semantic_sha256());
        let production = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap_or_else(|error| panic!("epoch={epoch}: current-production decode: {error:?}"));
        assert_eq!(
            production.canonical_encoding(),
            decoded.canonical_encoding()
        );
        let Some(SemanticDefinedCapabilityContractV1::KernelMathDerive(record)) =
            decoded.functions()[1].defined_capability_contract()
        else {
            panic!()
        };
        assert_eq!(record.function(), SemanticFunctionIdV1(1));
        assert_eq!(record.bridge().function(), SemanticFunctionIdV1(2));
        assert_eq!(record.current_callable().index(), if epoch { 5 } else { 4 });
        assert!(
            decoded.functions()[2]
                .defined_capability_contract()
                .is_none()
        );
    }
}

#[test]
fn defined_math_v21_epoch_composition_keeps_exact_root_type_closure() {
    let mut request = defined_math_document_request(true);
    assert_eq!(
        request.types.len(),
        defined_math_document_request(false).types.len() + 6
    );
    request
        .clone()
        .admit_exact_v21(SemanticMirLimitsV1::default())
        .expect("the combined fixture contains only the selected epoch type closure");

    let mut unused = borrowed_workgroup_request(true).types[4].clone();
    unused.identity = SemanticTypeIdentityV1(identity(200));
    unused.layout_identity = SemanticLayoutIdentityV1(identity(200));
    let extra = SemanticTypeIdV1(request.types.len() as u32);
    let mut types = request.types.to_vec();
    types.push(unused);
    request.types = types.into_boxed_slice();
    assert_eq!(
        request
            .admit_exact_v21(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::TypeOutsideRootClosure { ty: extra },
        "an ordered but unreferenced subgroup-only type must still reject",
    );
}

#[test]
fn defined_math_v21_gates_tags_and_preserves_historical_encodings() {
    let request = defined_math_document_request(false);
    let limits = SemanticMirLimitsV1::default();
    assert!(matches!(
        request.clone().admit_exact_v20(limits),
        Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: SemanticMirWireVersionV1::V20,
            required: SemanticMirWireVersionV1::V21,
        })
    ));
    for index in [1, 3] {
        assert!(
            canonical_semantic_function_fragment_sha256_v1(
                &request.functions[index],
                SemanticMirWireVersionV1::V20,
                16384
            )
            .is_err()
        );
        let contract = *request.functions[index]
            .defined_capability_contract()
            .unwrap();
        let bytes = defined_math_metadata_bytes(contract);
        assert_eq!(bytes[0], if index == 1 { 1 } else { 2 });
        assert_eq!(bytes.len(), if index == 1 { 513 } else { 381 });
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
        decoder.wire_version = SemanticMirWireVersionV1::V20;
        assert!(matches!(
            decoder.defined_capability_contract(),
            Err(SemanticMirDecodeErrorV1::InvalidTag { .. })
        ));
    }
    let admitted = request.admit_exact_v21(limits).unwrap();
    let mut bytes = admitted.canonical_encoding().to_vec();
    bytes[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&20u16.to_le_bytes());
    assert!(AdmittedInertSemanticMirV1::decode_exact_v20_canonical(&bytes, limits).is_err());

    let old_math = policy_math_request(SemanticF32MathFunctionV1::Sqrt);
    let v19 = old_math.clone().admit_exact_v19(limits).unwrap();
    assert_eq!(
        old_math
            .admit_current_production(limits)
            .unwrap()
            .canonical_encoding(),
        v19.canonical_encoding()
    );
    let old_epoch = borrowed_workgroup_request(true);
    let v20 = old_epoch.clone().admit_exact_v20(limits).unwrap();
    assert_eq!(
        old_epoch
            .admit_current_production(limits)
            .unwrap()
            .canonical_encoding(),
        v20.canonical_encoding()
    );
    let epoch = *v20.functions()[1].defined_capability_contract().unwrap();
    let bytes = defined_math_metadata_bytes(epoch);
    assert_eq!((bytes.len(), bytes[0]), (350, 0));
    for version in [SemanticMirWireVersionV1::V20, SemanticMirWireVersionV1::V21] {
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);
        decoder.wire_version = version;
        assert_eq!(decoder.defined_capability_contract().unwrap(), epoch);
        decoder.finish().unwrap();
    }
}

#[test]
fn defined_math_v21_document_decoder_rechecks_metadata_not_just_payload_shape() {
    let request = defined_math_document_request(false);
    let admitted = request
        .clone()
        .admit_exact_v21(SemanticMirLimitsV1::default())
        .unwrap();
    for (index, offsets) in [
        (
            1,
            vec![
                1, 5, 37, 69, 101, 105, 137, 169, 201, 205, 237, 269, 285, 481,
            ],
        ),
        (3, vec![1, 5, 37, 69, 101, 121, 317, 349]),
    ] {
        let metadata = defined_math_metadata_bytes(
            *request.functions[index]
                .defined_capability_contract()
                .unwrap(),
        );
        let positions = admitted
            .canonical_encoding()
            .windows(metadata.len())
            .enumerate()
            .filter_map(|(offset, bytes)| (bytes == metadata).then_some(offset))
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 1);
        for offset in offsets {
            let mut bytes = admitted.canonical_encoding().to_vec();
            bytes[positions[0] + offset] ^= 1;
            assert!(
                AdmittedInertSemanticMirV1::decode_exact_v21_canonical(
                    &bytes,
                    SemanticMirLimitsV1::default()
                )
                .is_err(),
                "function {index}, metadata byte {offset}"
            );
        }
        let mut bytes = admitted.canonical_encoding().to_vec();
        bytes[positions[0]] = 3;
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v21_canonical(
                &bytes,
                SemanticMirLimitsV1::default()
            )
            .is_err()
        );
    }
    for length in 0..admitted.canonical_encoding().len() {
        assert!(
            AdmittedInertSemanticMirV1::decode_exact_v21_canonical(
                &admitted.canonical_encoding()[..length],
                SemanticMirLimitsV1::default()
            )
            .is_err()
        );
    }
    let mut suffix = admitted.canonical_encoding().to_vec();
    suffix.push(0);
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v21_canonical(
            &suffix,
            SemanticMirLimitsV1::default()
        )
        .is_err()
    );
}

#[test]
fn defined_math_v21_full_validation_rejects_stale_body_abi_type_and_root() {
    let valid = defined_math_document_request(false);
    for mutation in 0..7 {
        let mut request = valid.clone();
        match mutation {
            // Preserve identity order so stale body commitments reach ABI validation.
            0 => request.functions[1].locals[1].identity = SemanticLocalIdentityV1(identity(3)),
            1 => request.functions[2].locals[1].identity = SemanticLocalIdentityV1(identity(3)),
            2 => request.functions[3].abi.identity = SemanticAbiIdentityV1(identity(222)),
            3 => request.functions[0].export = None,
            4 => {
                let SemanticTypeShapeV1::Pointer(pointer) = &mut request.types[6].shape else {
                    panic!()
                };
                pointer.pointee = SemanticTypeIdV1(3);
            }
            5 => {
                let SemanticTerminatorKindV1::Call(call) =
                    &mut request.functions[1].blocks[0].terminator.kind
                else {
                    panic!()
                };
                call.callee = SemanticCallableIdV1(4);
            }
            _ => {
                request.functions[3].abi.source_argument_ownership[0] =
                    SemanticSourceArgumentOwnershipV1::ByValue
            }
        }
        let result = request.admit_current_production(SemanticMirLimitsV1::default());
        if mutation <= 1 {
            assert_eq!(
                result.unwrap_err(),
                SemanticMirErrorV1::InvalidFunctionAbi,
                "mutation {mutation}"
            );
        } else {
            assert!(result.is_err(), "mutation {mutation}");
        }
    }
}

#[test]
fn defined_math_v21_constructor_claims_must_match_issuer_and_consumer() {
    let valid = defined_math_document_request(false);
    let Some(SemanticDefinedCapabilityContractV1::PolicyMathBind(original)) =
        valid.functions[3].defined_capability_contract().copied()
    else {
        panic!()
    };
    for (policy, brand) in [
        (
            SemanticTypeIdentityV1(identity(220)),
            original.kernel_brand(),
        ),
        (original.policy(), SemanticTypeIdentityV1(identity(221))),
    ] {
        let mut request = valid.clone();
        request.functions[3].defined_capability_contract = None;
        let changed = SemanticPolicyMathBindV1::for_defined_function(
            SemanticFunctionIdV1(3),
            &request.functions,
            &request.callables,
            &request.types,
            original.types(),
            original.provenance(),
            policy,
            brand,
        )
        .unwrap();
        request.functions[3] = request.functions[3]
            .clone()
            .with_defined_capability_contract(SemanticDefinedCapabilityContractV1::PolicyMathBind(
                changed,
            ))
            .unwrap();
        assert!(matches!(
            request.admit_exact_v21(SemanticMirLimitsV1::default()),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}

#[test]
fn defined_math_v21_document_validation_and_encoding_remain_bounded() {
    let request = defined_math_document_request(false);
    let admitted = request
        .clone()
        .admit_exact_v21(SemanticMirLimitsV1::default())
        .unwrap();
    let max = admitted.canonical_encoding().len() as u64;
    let exact = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::CanonicalBytes, max)
        .unwrap();
    assert_eq!(
        request
            .clone()
            .admit_exact_v21(exact)
            .unwrap()
            .canonical_encoding(),
        admitted.canonical_encoding()
    );
    let small = exact
        .with_limit(SemanticMirResourceV1::CanonicalBytes, max - 1)
        .unwrap();
    assert!(matches!(
        request.clone().admit_exact_v21(small),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        })
    ));
    assert!(matches!(
        AdmittedInertSemanticMirV1::decode_exact_v21_canonical(
            admitted.canonical_encoding(),
            small
        ),
        Err(SemanticMirDecodeErrorV1::InputLimitExceeded { .. })
    ));
    let small = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::ValidationWork, 32)
        .unwrap();
    assert!(matches!(
        request.admit_exact_v21(small),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            ..
        })
    ));
}

#[test]
fn defined_math_v21_common_occurrences_preserve_arguments_and_filter_epoch_adapter() {
    use crate::semantic_direct_call_expansion_v1::{
        SemanticCallExpansionLimitsV1, SemanticCallExpansionV1,
    };
    for epoch in [false, true] {
        let request = defined_math_document_request(epoch);
        let admitted = request
            .clone()
            .admit_exact_v21(SemanticMirLimitsV1::default())
            .unwrap_or_else(|error| {
                panic!("epoch={epoch}: occurrence source admission: {error:?}")
            });
        let expansion =
            SemanticCallExpansionV1::try_new(&admitted, SemanticCallExpansionLimitsV1::default())
                .unwrap();
        let common = expansion.defined_capability_bindings(&admitted).unwrap();
        assert_eq!(common.len(), if epoch { 3 } else { 2 });
        let epochs = expansion
            .workgroup_epoch_projection_bindings(&admitted)
            .unwrap();
        assert_eq!(epochs.len(), usize::from(epoch));
        for binding in &common {
            assert_eq!(binding.root(), SemanticFunctionIdV1(0));
            assert_eq!(binding.expansion_identity(), expansion.identity());
            assert_eq!(binding.root_identity(), expansion.roots()[0].identity());
            match binding.contract() {
                SemanticDefinedCapabilityContractV1::KernelMathDerive(record) => {
                    assert_eq!(binding.call_block(), SemanticBlockIdV1(2));
                    assert_eq!(binding.arguments().len(), 1);
                    assert_eq!(
                        binding.arguments()[0].ty(),
                        record.types().context_reference
                    );
                    assert_eq!(binding.destination().ty(), record.types().math);
                    assert_eq!(binding.callee_arguments().len(), 1);
                }
                SemanticDefinedCapabilityContractV1::PolicyMathBind(record) => {
                    assert_eq!(binding.call_block(), SemanticBlockIdV1(3));
                    assert!(matches!(
                        binding.arguments(),
                        [SemanticOperandV1::Copy(_), SemanticOperandV1::Move(_)]
                    ));
                    assert_eq!(binding.arguments()[0].ty(), record.types().math_reference);
                    assert_eq!(binding.arguments()[1].ty(), record.types().policy_reference);
                    assert_eq!(binding.destination().ty(), record.types().bound);
                    assert_eq!(binding.callee_arguments().len(), 2);
                }
                SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(_) => {
                    assert_eq!(epochs[0].binding(), binding)
                }
                SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
                | SemanticDefinedCapabilityContractV1::KernelMatrixDerive(_)
                | SemanticDefinedCapabilityContractV1::ReusableLdsConversion(_)
                | SemanticDefinedCapabilityContractV1::GuardedGridLeader(_)
                | SemanticDefinedCapabilityContractV1::ReusablePhase(_)
                | SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_) => {
                    panic!("Math-only source must not acquire a matrix binding")
                }
            }
        }
        let mut changed = request;
        changed.functions[0].blocks[0].identity = SemanticBlockIdentityV1(identity(129));
        let changed = changed
            .admit_exact_v21(SemanticMirLimitsV1::default())
            .unwrap();
        assert!(expansion.defined_capability_bindings(&changed).is_err());
        assert!(
            expansion
                .workgroup_epoch_projection_bindings(&changed)
                .is_err()
        );
    }
}
