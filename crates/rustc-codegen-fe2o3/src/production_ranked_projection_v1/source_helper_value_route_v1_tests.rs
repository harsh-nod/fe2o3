use super::*;

fn rebuilt(
    old: &SemanticFunctionDeclV1,
    abi: SemanticFunctionAbiV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let function = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        abi,
        old.locals().to_vec(),
        old.entry(),
        blocks,
    )
    .unwrap();
    match old.kernel_entry() {
        Some(entry) => function.with_kernel_entry(entry.clone()),
        None => function,
    }
}

fn admitted(
    root: SemanticFunctionDeclV1,
    helper: SemanticFunctionDeclV1,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    let base = fixture::source(vec![fixture::subtract()]);
    InertSemanticMirRequestV1::new(
        base.target(),
        base.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![SemanticFunctionIdV1::from_index(0)],
    )?
    .admit_current_production(SemanticMirLimitsV1::default())
}

fn helper_abi(
    canon: SemanticCanonAbiV1,
    external: SemanticExternAbiV1,
    unwind: bool,
    indirect: bool,
) -> SemanticFunctionAbiV1 {
    let old = fixture::abi(false);
    let mut arguments = old.arguments().to_vec();
    if indirect {
        arguments[0] = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            fixture::WORD,
            SemanticAbiPassModeV1::Indirect {
                attributes: SemanticAbiValueAttributesV1::plain(),
                metadata_attributes: None,
                on_stack: false,
            },
        ));
    }
    SemanticFunctionAbiV1::from_rustc(
        old.identity(),
        old.layout_identity(),
        canon,
        external,
        unwind,
        false,
        2,
        arguments,
        old.return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 2])
    .unwrap()
}

#[test]
fn admitted_same_return_helpers_do_not_hide_memory_or_unvisited_blocks() {
    let arithmetic = fixture::subtract().blocks()[0].statements().to_vec();
    let store = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            fixture::place(3),
            fixture::copy(1),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    );
    for extra_block in [false, true] {
        let mut statements = arithmetic.clone();
        if !extra_block {
            statements.insert(0, store.clone());
        }
        let mut blocks = vec![fixture::block(
            61,
            statements,
            SemanticTerminatorKindV1::Return,
        )];
        if extra_block {
            blocks.push(fixture::block(62, vec![], SemanticTerminatorKindV1::Return));
        }
        let source = fixture::source(vec![fixture::function(60, false, 1, blocks)]);
        let expected = if extra_block {
            "helper has unvisited or extra return blocks"
        } else {
            "helper contains memory, assumption or unsupported statement effects"
        };
        let (result, floor, _, _, _) = run(1_000_000, 16 * 1024 * 1024, |meter, _| {
            with_source_helper_values(&source, 0, meter, |context, meter| {
                assert_eq!(source::derive(&source, 1, &[], meter).err(), Some(expected),);
                assert_eq!(
                    context
                        .call(&source, &source.functions()[0], 0, call(&source), meter)
                        .err(),
                    Some("unresolved helper value recipe")
                );
                Ok(())
            })
        });
        result.unwrap();
        assert_eq!(floor, 4096);
    }
}

#[test]
fn admitted_alternate_scalar_abis_are_not_exact_rust_helper_recipes() {
    for (canon, external) in [
        (SemanticCanonAbiV1::RustCold, SemanticExternAbiV1::RustCold),
        (
            SemanticCanonAbiV1::C,
            SemanticExternAbiV1::C { unwind: false },
        ),
    ] {
        let base = fixture::source(vec![fixture::subtract()]);
        let helper = rebuilt(
            &base.functions()[1],
            helper_abi(canon, external, false, false),
            base.functions()[1].blocks().to_vec(),
        );
        let source = admitted(base.functions()[0].clone(), helper).unwrap();
        let (result, floor, _, _, _) = run(1_000_000, 16 * 1024 * 1024, |meter, _| {
            with_source_helper_values(&source, 0, meter, |context, meter| {
                assert_eq!(
                    source::abi(source.types(), source.functions()[1].abi(), meter),
                    Err("helper value requires exact nounwind Rust scalar ABI")
                );
                assert_eq!(
                    context
                        .call(&source, &source.functions()[0], 0, call(&source), meter)
                        .err(),
                    Some("unresolved helper value recipe")
                );
                Ok(())
            })
        });
        result.unwrap();
        assert_eq!(floor, 4096);
    }
}

#[test]
fn invalid_unwind_and_scalar_indirect_abis_stop_at_real_semantic_admission() {
    for (unwind, indirect) in [(true, false), (false, true)] {
        let base = fixture::source(vec![fixture::subtract()]);
        let helper = rebuilt(
            &base.functions()[1],
            helper_abi(
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::Rust,
                unwind,
                indirect,
            ),
            base.functions()[1].blocks().to_vec(),
        );
        assert!(matches!(
            admitted(base.functions()[0].clone(), helper),
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}

#[test]
fn actual_defined_result_resolution_requires_one_dominating_call_definition() {
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    for repeated in [false, true] {
        let base = fixture::source(vec![fixture::subtract()]);
        let first = if repeated {
            fixture::call(1, vec![fixture::copy(1), fixture::copy(2)], 3, 1)
        } else {
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: fixture::copy(1),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            }
        };
        let root = rebuilt(
            &base.functions()[0],
            base.functions()[0].abi().clone(),
            vec![
                fixture::block(31, vec![], first),
                fixture::block(
                    32,
                    vec![],
                    fixture::call(1, vec![fixture::copy(1), fixture::copy(2)], 3, 2),
                ),
                fixture::block(
                    33,
                    vec![fixture::assign(
                        4,
                        SemanticRvalueKindV1::Use(fixture::copy(3)),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        );
        let source = admitted(root, base.functions()[1].clone()).unwrap();
        let function = &source.functions()[0];
        let expected = if repeated {
            "GPU semantic local has no exact reaching assignment"
        } else {
            "GPU scalar intrinsic result is not defined on every path"
        };
        let (result, floor, _, _, _) = run(1_000_000, 16 * 1024 * 1024, |meter, _| {
            with_source_helper_values(&source, 0, meter, |context, meter| {
                let mut resolver =
                    GpuSemanticExpressionResolverV2::new(source.types(), function).unwrap();
                resolver.helper_semantic = Some(&source);
                resolver.helper_values = Some(context);
                resolver.helper_meter = Some(&mut *meter);
                let mut resolver = resolver
                    .with_scalar_callables_v1(source.callables())
                    .unwrap();
                let result = resolver.resolve_store_v2(
                    function.blocks()[2].statements()[0].kind(),
                    ScalarAssignmentSiteV1 {
                        block: 2,
                        statement: 0,
                    },
                );
                assert_eq!(result.err(), Some(expected));
                assert!(resolver.visiting.is_empty());
                assert!(resolver.use_site.is_none());
                assert_eq!(resolver.helper_reserved, 0);
                Ok(())
            })
        });
        result.unwrap();
        assert_eq!(floor, 4096);
    }
}
