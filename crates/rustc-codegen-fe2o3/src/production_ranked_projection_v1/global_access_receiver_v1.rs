// Called only after authenticate_global_access_v1 matched the live receiver origin.
fn global_access_retains_receiver_v1(
    types: &[SemanticTypeDeclV1],
    callable: &SemanticCallableDeclV1,
    call: &SemanticDirectCallV1,
) -> bool {
    use SemanticSourceArgumentOwnershipV1::{ByValue, SharedBorrow, UniqueBorrow};
    let SemanticCallableDeclV1::CompilerIntrinsic {
        binding, operation, ..
    } = callable
    else {
        return false;
    };
    // These operations return only a scalar result and cannot retain a loan.
    let (view, result, ownership): (_, _, &[SemanticSourceArgumentOwnershipV1]) = match operation {
        SemanticCompilerIntrinsicOperationV1::CapabilityGlobalStore { view, result, .. }
        | SemanticCompilerIntrinsicOperationV1::CapabilityGlobalExclusiveStore {
            view,
            result,
            ..
        } => (*view, *result, &[UniqueBorrow, ByValue, ByValue]),
        SemanticCompilerIntrinsicOperationV1::CapabilityGlobalStoreBlock {
            view, result, ..
        } => (
            *view,
            *result,
            &[UniqueBorrow, SharedBorrow, ByValue, ByValue],
        ),
        _ => return false,
    };
    if call.arguments().len() != ownership.len()
        || binding.abi().source_argument_ownership() != ownership
        || binding.abi().source_input_types().len() != ownership.len()
        || binding.abi().return_type() != result
        || !matches!(
            types
                .get(result.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
        )
    {
        return false;
    }
    let Some(SemanticOperandV1::Copy(receiver)) = call.arguments().first() else {
        return false;
    };
    if !receiver.projections().is_empty()
        || !is_exact_reference_to_v1(types, receiver.ty(), view, SemanticMutabilityV1::Mutable)
        || !binding
            .abi()
            .source_input_types()
            .iter()
            .zip(call.arguments())
            .all(|(ty, operand)| *ty == operand.ty())
        || call.arguments()[1..].iter().any(|operand| match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                place.local() == receiver.local()
            }
            SemanticOperandV1::Constant(_) => false,
        })
    {
        return false;
    }
    call.destination().is_some_and(|destination| {
        destination.place().ty() == result
            && destination.place().projections().is_empty()
            && destination.place().local() != receiver.local()
    }) && !matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
}
