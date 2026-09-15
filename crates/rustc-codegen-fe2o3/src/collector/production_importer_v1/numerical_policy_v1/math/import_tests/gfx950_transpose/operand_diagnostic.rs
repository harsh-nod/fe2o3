use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedRootV1;

pub(super) fn transition_operand_failure(
    mir: &AdmittedInertSemanticMirV1,
    view: &SemanticExpandedRootV1,
    block: usize,
    operation: SemanticGfx950TransposeOperationV1,
    argument: usize,
) -> String {
    let phase = match operation {
        SemanticGfx950TransposeOperationV1::Issue { .. } => "issue",
        SemanticGfx950TransposeOperationV1::Stage { .. } => "stage",
        SemanticGfx950TransposeOperationV1::Publish { .. } => "publish",
        SemanticGfx950TransposeOperationV1::Read { .. } => "read",
    };
    let origin = view.block_origins().get(block);
    let original = origin
        .and_then(|origin| {
            mir.functions()
                .get(origin.function().index() as usize)
                .and_then(|function| function.blocks().get(origin.block().index() as usize))
        })
        .and_then(|block| match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => Some(call),
            _ => None,
        });
    let expanded =
        view.body()
            .blocks()
            .get(block)
            .and_then(|block| match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => Some(call),
                _ => None,
            });
    format!(
        "transpose {phase} arg={argument} requires exact Move; expanded.bb={block} callee={:?}; source(instance,function,block)={:?} callee={:?}; original={}; expanded={}",
        expanded.map(|call| call.callee().index()),
        origin.map(|origin| (
            origin.instance().index(),
            origin.function().index(),
            origin.block().index()
        )),
        original.map(|call| call.callee().index()),
        transition_operand_summary(original.and_then(|call| call.arguments().get(argument))),
        transition_operand_summary(expanded.and_then(|call| call.arguments().get(argument))),
    )
}

fn transition_operand_summary(operand: Option<&SemanticOperandV1>) -> String {
    let Some(operand) = operand else {
        return "missing".into();
    };
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            let kind = if matches!(operand, SemanticOperandV1::Copy(_)) {
                "Copy"
            } else {
                "Move"
            };
            let prefix: Vec<_> = place
                .projections()
                .iter()
                .take(4)
                .map(|projection| (projection.kind(), projection.result_type().index()))
                .collect();
            format!(
                "{kind}(local={},ty={},projection_count={},first4={prefix:?})",
                place.local().index(),
                place.ty().index(),
                place.projections().len()
            )
        }
        SemanticOperandV1::Constant(constant) => {
            let kind = match constant.value() {
                SemanticConstantValueV1::ZeroSized => "ZeroSized",
                SemanticConstantValueV1::Scalar(_) => "Scalar",
                SemanticConstantValueV1::Bytes(_) => "Bytes",
                SemanticConstantValueV1::Pointer(_) => "Pointer",
                SemanticConstantValueV1::Callable(_) => "Callable",
            };
            format!("Constant.{kind}(ty={})", constant.ty().index())
        }
    }
}

#[test]
fn transpose_operand_diagnostic_distinguishes_move_copy_and_zst_without_payload_dump() {
    let ty = SemanticTypeIdV1::from_index(7);
    let place = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), vec![], ty).unwrap();
    let copied = SemanticOperandV1::Copy(place.clone());
    let moved = SemanticOperandV1::Move(place);
    assert!(transition_operand_summary(Some(&copied)).starts_with("Copy(local=3,ty=7,"));
    assert!(transition_operand_summary(Some(&moved)).starts_with("Move(local=3,ty=7,"));
    let zero = SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::ZeroSized,
    ));
    assert_eq!(
        transition_operand_summary(Some(&zero)),
        "Constant.ZeroSized(ty=7)"
    );
    assert_eq!(transition_operand_summary(None), "missing");
}

#[test]
fn transpose_operand_diagnostic_projection_output_is_bounded() {
    let ty = SemanticTypeIdV1::from_index(7);
    let projections = (0..16)
        .map(|index| SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap())
        .collect();
    let operand = SemanticOperandV1::Move(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(3), projections, ty).unwrap(),
    );
    let summary = transition_operand_summary(Some(&operand));
    assert!(summary.contains("projection_count=16"));
    assert!(summary.contains("Field(3)"));
    assert!(!summary.contains("Field(4)"));
    assert!(summary.len() < 256);
}
