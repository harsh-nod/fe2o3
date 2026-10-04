use super::*;
use fe2o3_kernel_descriptor::{
    ConditionalArgumentBindingV1 as Row, ConditionalArgumentRoleV1 as Role,
    MAX_CONDITIONAL_ARGUMENTS_V1,
};
use fe2o3_mir_model::{
    SemanticLogicalArgumentErrorV1, SemanticLogicalArgumentMapV1, SemanticSourceArgumentBindingV1,
    semantic_mir_v1::*,
};

pub(super) fn check(
    request: &Request<'_>,
    (root_id, body_id): (SemanticFunctionIdV1, SemanticFunctionIdV1),
    table: &Table<'_>,
    descriptor: &Kernel<'_, '_>,
    contract: &Contract<'_>,
    budget: &mut Budget<'_>,
) -> R<()> {
    let owner = request.source();
    let semantic = owner.semantic_ssa().source_semantic();
    let root = semantic
        .functions()
        .get(root_id.index() as usize)
        .ok_or(E::Mismatch("root ABI"))?;
    let body = semantic
        .functions()
        .get(body_id.index() as usize)
        .ok_or(E::Mismatch("body ABI"))?;
    let count = descriptor.argument_count();
    budget.charge_work(8)?;
    require(
        (1..=MAX_CONDITIONAL_ARGUMENTS_V1).contains(&count)
            && root.abi().source_input_types().len() == count,
        "physical argument count",
    )?;
    // Logical context transport needs the original collector receipt, not merely
    // an inert checked context-root view. Do not reinterpret its Ignore argument.
    if body.abi().source_input_types().len() != count {
        return Err(E::MissingContextCollectorCustody);
    }
    flat(root.abi(), semantic.types(), budget)?;
    flat(body.abi(), semantic.types(), budget)?;
    budget.charge_work(product(count, 2)?)?;
    require(
        root.abi().source_input_types() == body.abi().source_input_types()
            && root.abi().source_argument_ownership() == body.abi().source_argument_ownership(),
        "exact physical/body ABI",
    )?;
    // The existing relation checks the admitted transparent body selector and
    // original N/SSA argument traces. No same-signature helper substitution.
    let relation = owner
        .checked_source_argument_relation_v1(root_id, body_id, budget)
        .map_err(E::Source)?;
    let function = relation.canonical_function();
    require(
        function.signature.parameters.len() == count && function.signature.results.is_empty(),
        "whole N signature",
    )?;
    let values = &function
        .body
        .as_ref()
        .ok_or(E::Mismatch("N function body"))?
        .parameters;
    require(values.len() == count, "N parameter values")?;
    let sources = sum(&[
        root.abi().source_input_types().len(),
        body.abi().source_input_types().len(),
    ])?;
    // Inherited mapper's declared source-local index extent, not allocator/RSS.
    let bytes = sum(&[
        2 * size_of::<SemanticLogicalArgumentMapV1<'_>>(),
        product(sources, size_of::<Option<SemanticLocalIdV1>>())?,
        size_of::<
            [Option<fe2o3_pliron::ProductionSourceArgumentBindingV1<'_, '_>>;
                MAX_CONDITIONAL_ARGUMENTS_V1],
        >(),
    ])?;
    budget.charge_work(product(
        sum(&[root.locals().len(), body.locals().len(), sources, 1])?,
        16,
    )?)?;
    account::scope(budget, bytes, |budget| {
        let root_map = semantic.logical_arguments_v1(root_id).map_err(map_error)?;
        let body_map = semantic.logical_arguments_v1(body_id).map_err(map_error)?;
        // Replay each actual parameter once, then borrow its checked binding.
        // This fixed prepaid scratch owns no graph or independent account.
        let mut bindings: [Option<fe2o3_pliron::ProductionSourceArgumentBindingV1<'_, '_>>;
            MAX_CONDITIONAL_ARGUMENTS_V1] = std::array::from_fn(|_| None);
        for (parameter, value) in values.iter().enumerate() {
            bindings[parameter] = Some(
                relation
                    .bind_whole_parameter_v1(
                        u32::try_from(parameter).map_err(|_| Resource::Arithmetic)?,
                        *value,
                        budget,
                    )
                    .map_err(E::Argument)?,
            );
        }
        let mut physical = root_map.source_arguments();
        let mut logical = body_map.source_arguments();
        let mut adjusted = body_map.adjusted_arguments();
        let mut descriptor_arguments = descriptor.arguments();
        let mut layout = nominal::Layout::default();
        let mut matched = 0usize;
        for field in 0..count {
            budget.charge_work(96)?;
            let physical = physical
                .next()
                .ok_or(E::Mismatch("physical argument map"))?;
            let source = logical.next().ok_or(E::Mismatch("logical argument map"))?;
            let adjusted = adjusted
                .next()
                .ok_or(E::Mismatch("adjusted argument map"))?;
            require(
                adjusted.source_argument() == source.ordinal()
                    && adjusted.tuple_field().is_none()
                    && adjusted.local_field().is_none()
                    && source.binding() == SemanticSourceArgumentBindingV1::Whole(adjusted.local())
                    && adjusted.abi().role() == SemanticAbiArgumentRoleV1::Source
                    && adjusted.abi().ty() == source.ty()
                    && adjusted.abi().value().adjusted().is_none()
                    && adjusted.abi().value().pointee_override().is_none()
                    && physical.ty() == source.ty()
                    && physical.source_ownership() == source.source_ownership(),
                "whole source/adjusted field",
            )?;
            let mut selected = None;
            for binding in bindings[..count].iter().flatten() {
                budget.charge_work(1)?;
                if binding.source_argument() == source.ordinal()
                    && selected.replace(binding).is_some()
                {
                    return Err(E::Mismatch("unique whole canonical parameter"));
                }
            }
            let binding = selected.ok_or(E::Mismatch("complete canonical argument map"))?;
            require(
                binding.adjusted_argument() == adjusted.ordinal()
                    && binding.semantic_local() == adjusted.local()
                    && binding.semantic_type() == source.ty(),
                "actual binding/argument map",
            )?;
            let descriptor_argument = descriptor_arguments
                .next(&mut |w| budget.charge_work(w))?
                .ok_or(E::Mismatch("descriptor field"))?;
            let ty = semantic
                .types()
                .get(source.ty().index() as usize)
                .ok_or(E::Mismatch("source argument type"))?;
            let parameter =
                usize::try_from(binding.canonical_parameter()).map_err(|_| Resource::Arithmetic)?;
            nominal::field(
                table,
                &descriptor_argument,
                field,
                ty,
                &root.abi().adjusted_arguments()[field],
                physical.source_ownership(),
                &function.signature.parameters[parameter],
                &mut layout,
                budget,
            )?;
            let mut rows = contract.arguments();
            while let Some(row) = rows.next(&mut |w| budget.charge_work(w))? {
                budget.charge_work(1)?;
                if row.canonical_parameter != binding.canonical_parameter() {
                    continue;
                }
                budget.charge_work(2 * size_of::<Row>())?;
                require(
                    row.source_argument == binding.source_argument()
                        && row.adjusted_argument == binding.adjusted_argument()
                        && row.semantic_local == binding.semantic_local().index()
                        && row.semantic_type == binding.semantic_type().index()
                        && usize::from(row.generated_field) == field
                        && &row.source_type_identity
                            == descriptor_argument.source_type().as_bytes()
                        && &row.device_layout_identity
                            == descriptor_argument.device_layout().as_bytes(),
                    "exact generated argument coordinates/records",
                )?;
                nominal::role(table, &descriptor_argument, row.role, budget)?;
                occurrences(request, row, binding.canonical_value(), budget)?;
                matched = matched.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        require(
            physical.next().is_none()
                && logical.next().is_none()
                && adjusted.next().is_none()
                && descriptor_arguments
                    .next(&mut |w| budget.charge_work(w))?
                    .is_none()
                && matched == contract.argument_count(),
            "complete generated argument table",
        )?;
        nominal::finish(descriptor, layout, budget)
    })
}

fn occurrences(
    request: &Request<'_>,
    row: Row,
    value: fe2o3_kernel_ir::ValueId,
    budget: &mut Budget<'_>,
) -> R<()> {
    let input = request.pliron_input();
    let mut found = false;
    for (i, actual) in request.arguments().iter().enumerate() {
        budget.charge_work(16)?;
        if actual.canonical_parameter() != row.canonical_parameter {
            continue;
        }
        let (coordinates, role) = if i == 0 {
            (input.outputs()[0].source(), Role::Output)
        } else {
            (input.reads()[i - 1].source(), Role::Input)
        };
        require(
            row.role == role
                && row.source_argument == actual.source_argument()
                && row.adjusted_argument == actual.adjusted_argument()
                && row.semantic_local == actual.semantic_local().index()
                && row.semantic_type == actual.semantic_type().index()
                && coordinates.canonical_parameter() == row.canonical_parameter
                && coordinates.canonical_value() == value
                && coordinates.source_argument() == row.source_argument
                && coordinates.adjusted_argument() == row.adjusted_argument
                && coordinates.semantic_local().index() == row.semantic_local
                && coordinates.semantic_type().index() == row.semantic_type,
            "exact occurrence source binding",
        )?;
        if i != 0 {
            require(
                input.reads()[i - 1].canonical().slice() == value,
                "read canonical slice binding",
            )?;
        }
        found = true;
    }
    require(found, "no extra conditional parameter")
}

fn map_error(error: SemanticLogicalArgumentErrorV1) -> E {
    match error {
        SemanticLogicalArgumentErrorV1::AllocationFailure => Resource::Allocation.into(),
        SemanticLogicalArgumentErrorV1::UnknownFunction => E::Mismatch("logical argument function"),
    }
}
fn flat(
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(8)?;
    require(
        !abi.c_variadic()
            && abi.extern_abi() != SemanticExternAbiV1::RustCall
            && abi.hidden_arguments().is_empty()
            && abi.arguments().len() == abi.source_input_types().len()
            && abi.source_argument_ownership().len() == abi.source_input_types().len()
            && abi.adjusted_arguments().len() == abi.source_input_types().len(),
        "flat source ABI",
    )?;
    for (argument, source) in abi.arguments().iter().zip(abi.source_input_types()) {
        budget.charge_work(12)?;
        let ty = types
            .get(source.index() as usize)
            .ok_or(E::Mismatch("ABI type"))?;
        if matches!(ty.rust_type_kind(), SemanticRustTypeKindV1::Execution(_)) {
            return Err(E::MissingContextCollectorCustody);
        }
        require(
            argument.role() == SemanticAbiArgumentRoleV1::Source
                && argument.ty() == *source
                && matches!(
                    argument.mode(),
                    SemanticAbiPassModeV1::Direct(_) | SemanticAbiPassModeV1::Pair { .. }
                )
                && !matches!(ty.shape(), SemanticTypeShapeV1::Tuple(_)),
            "whole flat ABI argument",
        )?;
    }
    Ok(())
}
