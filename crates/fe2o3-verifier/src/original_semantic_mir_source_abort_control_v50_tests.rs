use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 256 * 1024 * 1024;

pub(super) fn transform(
    _: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
) {
    let helper = functions.last_mut().unwrap();
    assert_eq!(helper.locals().len(), 4);
    assert_eq!(helper.blocks().len(), 1);
    let source = helper.source();
    let original = &helper.blocks()[0];
    let word = helper.locals()[1].ty();
    let block = |identity, statements, kind| {
        SemanticBasicBlockV1::new(
            identity,
            source,
            statements,
            SemanticTerminatorV1::new(source, kind),
        )
        .unwrap()
    };
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let blocks = vec![
        block(
            original.identity(),
            original.statements().to_vec(),
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], word).unwrap(),
                ),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 1),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        ),
        block(
            SemanticBlockIdentityV1::from_sha256([240; 32]),
            vec![],
            SemanticTerminatorKindV1::Abort,
        ),
        block(
            SemanticBlockIdentityV1::from_sha256([241; 32]),
            vec![],
            original.terminator().kind().clone(),
        ),
    ];
    *helper = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        source,
        helper.abi().clone(),
        helper.locals().to_vec(),
        helper.entry(),
        blocks,
    )
    .unwrap();
}

fn run(work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        transform,
        |plan, out| {
            super::tests::with_slots(plan, out, |slots, out| {
                let mut program = SourceByteProgram::derive(plan, slots, out)?;
                let paired = super::super::paired::PairedInvocations::derive(
                    plan,
                    &program,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    out,
                )?;
                super::super::byte_bindings::SourceByteBindings::derive(slots, out)?.emit(out)?;
                let mut aborts = [None; 4];
                let mut count = 0;
                for function in program.functions.iter().flatten() {
                    for (block, control) in function.control.iter().enumerate() {
                        if matches!(control.end, End::Abort) {
                            assert!(function.instance == 1 || function.instance == 2);
                            assert_eq!(block, 1);
                            aborts[count] = Some(function.blocks.start + block);
                            count += 1;
                        }
                    }
                }
                assert_eq!(count, 4);
                let start = out.text.len();
                program.emit(out)?;
                let emitted = &out.text[start..];
                let closed = emitted.split('\n').next().unwrap();
                assert!(closed.starts_with("open spec fn invocation_source_abort_site_v50"));
                assert_eq!(closed.matches(" || pc == ").count(), 4);
                for pc in aborts.into_iter().flatten() {
                    assert!(closed.contains(&format!(" || pc == {pc}int")));
                }
                assert_eq!(
                    emitted
                        .matches("let source = invocation_source_byte_trap_v40(cursor.source);")
                        .count(),
                    4
                );
                assert!(!emitted.contains("assume("));
                paired.emit(out)
            })
        },
    )
}

#[test]
fn original_abort_joins_exact_source_sites_with_the_mandatory_paired_trap_route() {
    run(LIMIT, LIMIT).0.unwrap();
    let effects = include_str!("original_semantic_mir_observed_effects_v39.vrs");
    assert!(effects.contains("invocation_source_abort_site_v50(result.before_control.machine.pc)"));
    assert!(effects.contains("result.operands.len() == 0"));
    assert!(effects.contains("result.source == invocation_source_byte_trap_v40(before)"));
    assert!(effects.contains("} else { result.before_control }"));
}

#[test]
fn original_abort_complete_program_has_exact_and_one_short_resource_boundaries() {
    let measured = run(LIMIT, LIMIT);
    measured.0.unwrap();
    assert_eq!(measured.2, super::super::super::invocations::tests::FLOOR);
    let exact = run(measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    assert!(matches!(run(measured.1 - 1, measured.3).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1));
    assert!(matches!(run(measured.1, measured.3 - 1).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1));
}

#[test]
fn original_abort_site_emission_has_an_independent_fixed_frame_envelope() {
    assert_eq!(
        abort_headers(),
        size_of::<std::slice::Iter<'_, Option<SourceByteFunction<'_, '_, '_>>>>()
            + size_of::<std::iter::Enumerate<std::slice::Iter<'_, BodyBlock>>>()
            + size_of::<Option<usize>>()
            + size_of::<usize>()
            + 2 * size_of::<&()>()
    );
}
