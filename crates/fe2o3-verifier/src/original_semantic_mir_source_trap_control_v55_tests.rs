use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 256 * 1024 * 1024;

fn never_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([245; 32]),
        SemanticLayoutIdentityV1::from_sha256([246; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::Primitive,
            SemanticRustcVariantsV1::Empty,
            SemanticBackendReprV1::memory(true),
            None,
            true,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Never,
    )
}

fn intrinsic(operation: Intrinsic, abi: SemanticFunctionAbiV1) -> Callable {
    Callable::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([231; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([232; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([233; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([234; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([235; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([236; 32]),
    }
}

fn abi(ty: SemanticTypeIdV1, arguments: Vec<SemanticAbiValueV1>) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([237; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        arguments,
        SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
}

fn run(root_traps: bool, work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_callable_transform(
        work,
        storage,
        |types, functions, callables| {
            abort_tests::transform(types, functions);
            let never = SemanticTypeIdV1::from_index(types.len() as u32);
            types.push(never_type());
            let callee = SemanticCallableIdV1::from_index(callables.len() as u32);
            callables.push(intrinsic(Intrinsic::Trap, abi(never, vec![])));
            for (index, function) in functions.iter_mut().enumerate() {
                let block = if index == 2 {
                    1
                } else if root_traps {
                    2
                } else {
                    continue;
                };
                let source = function.source();
                let mut blocks = function.blocks().to_vec();
                blocks[block] = SemanticBasicBlockV1::new(
                    blocks[block].identity(),
                    source,
                    blocks[block].statements().to_vec(),
                    SemanticTerminatorV1::new(
                        source,
                        Terminator::Call(
                            Call::new_callable(callee, vec![], None, Unwind::Unreachable).unwrap(),
                        ),
                    ),
                )
                .unwrap();
                let mut changed = SemanticFunctionDeclV1::new(
                    function.identity(),
                    function.role(),
                    function.item_definition_identity(),
                    function.monomorphization_identity(),
                    function.generic_type_arguments_identity(),
                    function.const_generic_arguments_identity(),
                    source,
                    function.abi().clone(),
                    function.locals().to_vec(),
                    function.entry(),
                    blocks,
                )
                .unwrap();
                if let Some(entry) = function.kernel_entry() {
                    changed = changed.with_kernel_entry(entry.clone());
                }
                *function = changed;
            }
        },
        |plan, out| {
            super::tests::with_slots(plan, out, |slots, out| {
                let mut program = SourceByteProgram::derive(plan, slots, out)?;
                let mut count = 0;
                for root in 0..2 {
                    assert_eq!(program.in_place_call(root, 0, 2, out)?, root_traps);
                    for instance in [1, 2] {
                        assert!(program.in_place_call(root, instance, 1, out)?);
                        assert!(!program.in_place_call(root, instance, 2, out)?);
                        let calls = plan.calls(root, instance, out)?;
                        assert_eq!(calls.len(), 1);
                        assert_eq!(calls[0].kind, CallKind::Direct);
                        assert!(calls[0].ssa_reachable);
                        assert!(calls[0].child.is_none());
                    }
                }
                let paired = super::super::paired::PairedInvocations::derive(
                    plan,
                    &program,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    out,
                )?;
                super::super::byte_bindings::SourceByteBindings::derive(slots, out)?.emit(out)?;
                let start = out.text.len();
                program.emit(out)?;
                let emitted = &out.text[start..];
                let closed = emitted.split('\n').next().unwrap();
                assert!(closed.starts_with("open spec fn invocation_source_abort_site_v50"));
                for function in program.functions.iter().flatten() {
                    for (block, control) in function.control.iter().enumerate() {
                        if matches!(control.end, End::Abort) {
                            let pc = function.blocks.start + block;
                            assert!(closed.contains(&format!(" || pc == {pc}int")));
                            count += 1;
                        }
                    }
                }
                assert_eq!(count, if root_traps { 6 } else { 4 });
                assert_eq!(closed.matches(" || pc == ").count(), count);
                assert_eq!(
                    emitted.matches("let source = invocation_source_byte_trap_v40(cursor.source);\n InvocationSourceBlockResultV36 { source, before_control: cursor.source, observations: cursor.observations, operands: seq![], returned: None }").count(),
                    count
                );
                assert!(!emitted.contains("assume("));
                paired.emit(out)
            })
        },
    )
}

#[test]
fn original_trap_calls_preserve_terminal_root_and_repeated_helper_control() {
    run(false, LIMIT, LIMIT).0.unwrap();
    run(true, LIMIT, LIMIT).0.unwrap();
    let observed = include_str!("original_semantic_mir_observed_effects_v39.vrs");
    assert!(
        observed.contains("invocation_source_abort_site_v50(result.before_control.machine.pc)")
    );
    assert!(observed.contains("result.operands.len() == 0"));
    assert!(observed.contains("result.source == invocation_source_byte_trap_v40(before)"));
    assert!(observed.contains("if authentic { MemoryOperationEffectV30::Trap }"));
}

#[test]
fn original_trap_calls_have_exact_and_one_short_complete_resources() {
    for roots in [false, true] {
        let measured = run(roots, LIMIT, LIMIT);
        measured.0.unwrap();
        let exact = run(roots, measured.1, measured.3);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (measured.1, measured.2, measured.3)
        );
        assert!(matches!(run(roots, measured.1 - 1, measured.3).0,
            Err(Error::Source(SourceError::Resource(Resource::Work(error))))
                if error.actual() == measured.1 && error.limit() == measured.1 - 1));
        assert!(matches!(run(roots, measured.1, measured.3 - 1).0,
            Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
                if error.actual() == measured.3 && error.limit() == measured.3 - 1));
    }
}

#[test]
fn original_trap_classifier_rejects_fabricated_continuation_arguments_and_unwind() {
    super::super::super::invocations::tests::run(LIMIT, LIMIT, |_, out| {
        let never = SemanticTypeIdV1::from_index(0);
        let callee = SemanticCallableIdV1::from_index(0);
        let types = [never_type()];
        let callables = [intrinsic(Intrinsic::Trap, abi(never, vec![]))];
        let valid = Call::new_callable(callee, vec![], None, Unwind::Unreachable).unwrap();
        assert!(source_trap_call_v55(&valid, &callables, &types, out)?);
        let place = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], never).unwrap();
        let malformed = [
            Call::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![],
                None,
                Unwind::Unreachable,
            )
            .unwrap(),
            Call::new_callable(
                callee,
                vec![SemanticOperandV1::Move(place.clone())],
                None,
                Unwind::Unreachable,
            )
            .unwrap(),
            Call::new_callable_with_variadic_argument_abis(
                callee,
                vec![],
                vec![SemanticAbiValueV1::new(
                    never,
                    SemanticAbiPassModeV1::Ignore,
                )],
                None,
                Unwind::Unreachable,
            )
            .unwrap(),
            Call::new_callable(
                callee,
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place,
                    SemanticControlFlowEdgeV1::new(
                        EdgeRole::CallReturn,
                        SemanticBlockIdV1::from_index(0),
                    ),
                )),
                Unwind::Unreachable,
            )
            .unwrap(),
            Call::new_callable(callee, vec![], None, Unwind::Continue).unwrap(),
            Call::new_callable(
                callee,
                vec![],
                None,
                Unwind::Cleanup(SemanticControlFlowEdgeV1::new(
                    EdgeRole::CallUnwind,
                    SemanticBlockIdV1::from_index(0),
                )),
            )
            .unwrap(),
        ];
        for call in &malformed {
            assert!(matches!(
                source_trap_call_v55(call, &callables, &types, out),
                Err(Error::Statement(
                    "original MIR byte control differs from its exact invocation"
                ))
            ));
        }
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_trap_classifier_requires_exact_intrinsic_and_never_abi() {
    super::super::super::invocations::tests::run(LIMIT, LIMIT, |_, out| {
        let never = SemanticTypeIdV1::from_index(0);
        let call = Call::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            None,
            Unwind::Unreachable,
        )
        .unwrap();
        let types = [never_type()];
        for operation in [
            Intrinsic::WorkgroupBarrier,
            Intrinsic::ThreadIndex(SemanticAxisV1::X),
        ] {
            assert!(!source_trap_call_v55(
                &call,
                &[intrinsic(operation, abi(never, vec![]))],
                &types,
                out
            )?);
        }
        assert!(!source_trap_call_v55(
            &call,
            &[Callable::defined(SemanticFunctionIdV1::from_index(0))],
            &types,
            out
        )?);
        let with_argument = intrinsic(
            Intrinsic::Trap,
            abi(
                never,
                vec![SemanticAbiValueV1::new(
                    never,
                    SemanticAbiPassModeV1::Ignore,
                )],
            ),
        );
        let missing_return = intrinsic(
            Intrinsic::Trap,
            abi(SemanticTypeIdV1::from_index(1), vec![]),
        );
        for callable in [with_argument, missing_return] {
            assert!(source_trap_call_v55(&call, &[callable], &types, out).is_err());
        }
        let mut not_never = types[0].clone();
        not_never = SemanticTypeDeclV1::new(
            not_never.identity(),
            not_never.layout_identity(),
            not_never.layout().clone(),
            TypeShape::Unit,
        );
        assert!(
            source_trap_call_v55(
                &call,
                &[intrinsic(Intrinsic::Trap, abi(never, vec![]))],
                &[not_never],
                out
            )
            .is_err()
        );
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_trap_classifier_fixed_frame_is_part_of_function_accounting() {
    assert_eq!(
        trap_headers_v55(),
        size_of::<(
            &Call,
            &[Callable],
            &[TypeDecl],
            &SemanticNonBodyCallableBindingV1
        )>() + size_of::<Option<&Callable>>()
            + size_of::<Option<&TypeDecl>>()
            + size_of::<Option<&TypeShape>>()
            + size_of::<Result<bool>>()
            + size_of::<bool>()
    );
    assert!(headers() >= trap_headers_v55());
}
