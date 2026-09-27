//! Bounded original-source observations, never an authorization input.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const CONFIG: &str = "FE2O3_TEST_SHARED_PRIMITIVE_POLICY5";
const BLOCKS: usize = 16;
const STATEMENTS: usize = 256;
const OPERANDS: usize = 16;
const PROJECTIONS: usize = 8;

fn place(place: &SemanticPlaceV1) -> (u32, u32, Vec<(SemanticProjectionKindV1, u32)>, bool) {
    (
        place.local().index(),
        place.ty().index(),
        place
            .projections()
            .iter()
            .take(PROJECTIONS)
            .map(|projection| (projection.kind(), projection.result_type().index()))
            .collect(),
        place.projections().len() > PROJECTIONS,
    )
}

pub(super) fn observe(
    owner: &ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    reads: &Reads<'_>,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if std::env::var_os(CONFIG).is_none() {
        return Ok(());
    }
    let declaration = &owner.source_semantic().functions()[function.index() as usize];
    let plan = owner
        .plan_for_function(function)
        .expect("exact original function")
        .plan();
    eprintln!(
        "shared-pre-ranked function={} blocks={} locals={}",
        function.index(),
        declaration.blocks().len(),
        declaration.locals().len()
    );
    for (index, local) in declaration.locals().iter().take(STATEMENTS).enumerate() {
        let promoted = plan
            .promoted_variables()
            .iter()
            .any(|variable| variable.get() as usize == index);
        eprintln!(
            "shared-pre-ranked local={index} ty={} promoted={promoted}",
            local.ty().index()
        );
        match owner.source_semantic().types()[local.ty().index() as usize].shape() {
            SemanticTypeShapeV1::Pointer(pointer) => eprintln!(
                "shared-pre-ranked local={index} pointer kind={:?} mutability={:?} pointee={} metadata={:?}",
                pointer.kind(),
                pointer.mutability(),
                pointer.pointee().index(),
                pointer.metadata()
            ),
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                eprintln!(
                    "shared-pre-ranked local={index} static fields={:?} truncated={}",
                    &fields.fields()[..fields.fields().len().min(OPERANDS)],
                    fields.fields().len() > OPERANDS
                )
            }
            _ => {}
        }
    }
    let mut visited = 0;
    for (block, body) in declaration.blocks().iter().take(BLOCKS).enumerate() {
        for (statement, row) in body.statements().iter().enumerate() {
            if visited == STATEMENTS {
                eprintln!("shared-pre-ranked statement cap reached");
                return Ok(());
            }
            visited += 1;
            let site = ProjectedSemanticAccessSiteV1 {
                block,
                statement: Some(statement),
            };
            match row.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    eprintln!(
                        "shared-pre-ranked site={block}:{statement} destination={:?}",
                        place(assignment.destination())
                    );
                    match assignment.value().kind() {
                        SemanticRvalueKindV1::Borrow {
                            kind,
                            place: source,
                        } => eprintln!(
                            "shared-pre-ranked Borrow kind={kind:?} source={:?}",
                            place(source)
                        ),
                        SemanticRvalueKindV1::Aggregate(value) => eprintln!(
                            "shared-pre-ranked Aggregate kind={:?} operands={}",
                            value.kind(),
                            value.operands().len()
                        ),
                        SemanticRvalueKindV1::Use(_) => eprintln!("shared-pre-ranked Use"),
                        SemanticRvalueKindV1::Load(load) => eprintln!(
                            "shared-pre-ranked explicit Load source={:?}",
                            place(load.source())
                        ),
                        SemanticRvalueKindV1::AddressOf { place: source, .. } => {
                            eprintln!("shared-pre-ranked AddressOf source={:?}", place(source))
                        }
                        SemanticRvalueKindV1::Cast { kind, .. } => {
                            eprintln!("shared-pre-ranked Cast kind={kind:?}")
                        }
                        _ => eprintln!("shared-pre-ranked other rvalue"),
                    }
                    if matches!(assignment.value().kind(), SemanticRvalueKindV1::Aggregate(value)
                        if value.operands().len() > OPERANDS)
                    {
                        eprintln!("shared-pre-ranked operand cap reached");
                        continue;
                    }
                    let mut operands = 0;
                    assignment.value().kind().try_visit_operands(|operand| {
                        if operands == OPERANDS {
                            return Ok(());
                        }
                        operands += 1;
                        let (kind, source) = match operand {
                            SemanticOperandV1::Copy(source) => ("Copy", source),
                            SemanticOperandV1::Move(source) => ("Move", source),
                            SemanticOperandV1::Constant(value) => {
                                eprintln!("shared-pre-ranked Constant ty={}", value.ty().index());
                                return Ok(());
                            }
                        };
                        // Query only the actual original ordinary operand.
                        // The production ledger pays the exact existing query;
                        // no result changes the compiler's decision below.
                        let accepted =
                            facts.shared_value_read_v1(reads, declaration, site, source)?;
                        eprintln!(
                            "shared-pre-ranked {kind} source={:?} read_view={accepted}",
                            place(source)
                        );
                        Ok::<_, ProductionRankedProjectionErrorV1>(())
                    })?;
                }
                SemanticStatementKindV1::StorageLive(local) => eprintln!(
                    "shared-pre-ranked site={block}:{statement} StorageLive {}",
                    local.index()
                ),
                SemanticStatementKindV1::StorageDead(local) => eprintln!(
                    "shared-pre-ranked site={block}:{statement} StorageDead {}",
                    local.index()
                ),
                SemanticStatementKindV1::Deinitialize(source) => eprintln!(
                    "shared-pre-ranked site={block}:{statement} Deinitialize {:?}",
                    place(source)
                ),
                _ => eprintln!("shared-pre-ranked site={block}:{statement} other statement"),
            }
        }
        let terminator = match body.terminator().kind() {
            SemanticTerminatorKindV1::Return => "Return",
            SemanticTerminatorKindV1::Goto(_) => "Goto",
            SemanticTerminatorKindV1::Call(_) => "Call",
            SemanticTerminatorKindV1::SwitchInt { .. } => "SwitchInt",
            _ => "Other",
        };
        eprintln!("shared-pre-ranked block={block} terminator={terminator}");
    }
    Ok(())
}
