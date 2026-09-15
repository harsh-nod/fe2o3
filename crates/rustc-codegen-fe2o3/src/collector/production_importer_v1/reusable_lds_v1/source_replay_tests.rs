//! Mutate the actual RustCall caller, preserving its nominal function/ABI labels.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn replace_blocks(
    f: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        f.identity(),
        f.role(),
        f.item_definition_identity(),
        f.monomorphization_identity(),
        f.generic_type_arguments_identity(),
        f.const_generic_arguments_identity(),
        f.source(),
        f.abi().clone(),
        f.locals().to_vec(),
        f.entry(),
        blocks,
    )
    .unwrap()
}

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    mir: &AdmittedInertSemanticMirV1,
) {
    let mut work = usize::try_from(
        SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork),
    )
    .unwrap();
    let roster = numerical_policy_v1::defined_source_roster_v1(
        tcx,
        plan,
        mir.types(),
        mir.functions(),
        mir.callables(),
    )
    .unwrap();
    let mut replay = source_body_v1::Replay::new(tcx, plan, &mut work).unwrap();
    for record in mir
        .functions()
        .iter()
        .filter_map(|f| match f.defined_capability_contract() {
            Some(SemanticDefinedCapabilityContractV1::ReusableLdsConversion(r)) => Some(r),
            _ => None,
        })
    {
        let caller = record.source().caller;
        let f = &mir.functions()[caller.index() as usize];
        let original = &plan.body_producers()[caller.index() as usize];
        let correspondence = roster
            .reconstructed_body(caller, &mut replay, &mut work)
            .unwrap();
        assert_eq!(
            f.locals().len(),
            original.locals.len() + 1,
            "actual closure retains the synthesized source-tuple argument"
        );
        for (raw, before) in original.raw_to_semantic_locals.iter().enumerate() {
            let after = correspondence.local(raw as u32).unwrap();
            assert_eq!(
                f.locals()[after.index() as usize].identity(),
                original.locals[before.index() as usize].identity
            );
        }
        for (raw, before) in original.raw_to_semantic_blocks.iter().enumerate() {
            assert_eq!(correspondence.block(raw as u32).unwrap(), *before);
        }
        assert!(matches!(
            correspondence.local(u32::MAX),
            Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "original source body replay correspondence"
            ))
        ));
        assert!(matches!(
            correspondence.block(u32::MAX),
            Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "original source body replay correspondence"
            ))
        ));
        let entry = f.entry().index() as usize;
        let block = &f.blocks()[entry];
        assert_eq!(
            block.statements().len(),
            original.blocks[entry].statements.len() + 1
        );
        let prefix = &block.statements()[0];
        let SemanticStatementKindV1::Assign(assignment) = prefix.kind() else {
            panic!("exact tuple-field initialization assignment")
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)) = assignment.value().kind()
        else {
            panic!("tuple-field initialization consumes its actual source operand")
        };
        assert!(
            matches!(place.projections(), [p] if matches!(p.kind(), SemanticProjectionKindV1::Field(0)))
        );
        for mutation in 0..4 {
            let mut statements = block.statements().to_vec();
            match mutation {
                0 => {
                    statements.remove(0);
                }
                1 => {
                    statements[0] =
                        SemanticStatementV1::new(prefix.source(), SemanticStatementKindV1::Nop)
                }
                2 => {
                    statements[0] = SemanticStatementV1::new(
                        prefix.source(),
                        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            assignment.destination().clone(),
                            SemanticRvalueV1::new(
                                assignment.value().result_type(),
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place.clone())),
                            ),
                        )),
                    )
                }
                3 => statements.push(SemanticStatementV1::new(
                    block.source(),
                    SemanticStatementKindV1::Nop,
                )),
                _ => unreachable!(),
            }
            let mut blocks = f.blocks().to_vec();
            blocks[entry] = SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                block.terminator().clone(),
            )
            .unwrap();
            let mut functions = mir.functions().to_vec();
            functions[caller.index() as usize] = replace_blocks(f, blocks);
            let mutated = numerical_policy_v1::defined_source_roster_v1(
                tcx,
                plan,
                mir.types(),
                &functions,
                mir.callables(),
            )
            .unwrap();
            assert!(
                matches!(
                    mutated.reconstructed_body(caller, &mut replay, &mut work),
                    Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                        "original source body replay correspondence"
                    ))
                ),
                "caller mutation {mutation}"
            );
        }
    }
}
