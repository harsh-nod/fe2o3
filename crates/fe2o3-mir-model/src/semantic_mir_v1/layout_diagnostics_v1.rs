//! Read-only localization of inert layout failures. This is not admission.

use super::*;

/// Locates a layout-identity conflict or the first failing canonical type.
///
/// Uses the admission validator for each type, but intentionally does not
/// validate functions, effects, roots, or source correspondence. Success grants
/// no authority and must not replace complete request admission.
pub fn diagnose_type_layouts_v1(
    request: &InertSemanticMirRequestV1,
    limits: SemanticMirLimitsV1,
) -> Result<(), (SemanticMirLocationV1, SemanticMirErrorV1)> {
    let module = SemanticMirLocationV1::Module;
    for (resource, count) in [
        (SemanticMirResourceV1::Types, request.types.len()),
        (SemanticMirResourceV1::Functions, request.functions.len()),
        (SemanticMirResourceV1::Callables, request.callables.len()),
    ] {
        enforce_count(resource, count, limits).map_err(|error| (module, error))?;
    }
    if request.target.object_size_bound_bytes == 0
        || !request.target.object_size_bound_bytes.is_power_of_two()
    {
        return Err((module, SemanticMirErrorV1::InvalidTypeLayout));
    }
    let mut context = ValidationContextV1 {
        request,
        limits,
        totals: ValidationTotalsV1::default(),
        work: 0,
    };
    let mut layouts = BTreeMap::new();
    for (index, ty) in request.types.iter().enumerate() {
        record_layout(
            &mut context,
            &mut layouts,
            SemanticMirLocationV1::Type(SemanticTypeIdV1(index as u32)),
            ty.layout_identity,
            &ty.layout,
        )?;
    }
    for (index, abi) in request
        .functions
        .iter()
        .map(|function| &function.abi)
        .enumerate()
    {
        let location = SemanticMirLocationV1::Function(SemanticFunctionIdV1(index as u32));
        context.one().map_err(|error| (location, error))?;
        for value in abi
            .arguments
            .iter()
            .map(|argument| &argument.value)
            .chain(std::iter::once(&abi.return_value))
        {
            context.one().map_err(|error| (location, error))?;
            if let Some(adjusted) = value.adjusted() {
                record_layout(
                    &mut context,
                    &mut layouts,
                    location,
                    adjusted.layout_identity,
                    &adjusted.layout,
                )?;
            }
        }
    }
    for (index, callable) in request.callables.iter().enumerate() {
        let location = SemanticMirLocationV1::Callable(SemanticCallableIdV1(index as u32));
        context.one().map_err(|error| (location, error))?;
        let Some(binding) = callable.binding() else {
            continue;
        };
        for value in binding
            .abi
            .arguments
            .iter()
            .map(|argument| &argument.value)
            .chain(std::iter::once(&binding.abi.return_value))
        {
            context.one().map_err(|error| (location, error))?;
            if let Some(adjusted) = value.adjusted() {
                record_layout(
                    &mut context,
                    &mut layouts,
                    location,
                    adjusted.layout_identity,
                    &adjusted.layout,
                )?;
            }
        }
    }
    for (index, ty) in request.types.iter().enumerate() {
        let id = SemanticTypeIdV1(index as u32);
        context
            .one()
            .and_then(|()| validate_type(&mut context, id, ty))
            .map_err(|error| (SemanticMirLocationV1::Type(id), error))?;
    }
    Ok(())
}

fn record_layout<'a>(
    context: &mut ValidationContextV1<'_>,
    layouts: &mut BTreeMap<SemanticLayoutIdentityV1, &'a SemanticTypeLayoutV1>,
    location: SemanticMirLocationV1,
    identity: SemanticLayoutIdentityV1,
    layout: &'a SemanticTypeLayoutV1,
) -> Result<(), (SemanticMirLocationV1, SemanticMirErrorV1)> {
    context.one().map_err(|error| (location, error))?;
    if let Some(previous) = layouts.insert(identity, layout)
        && !layout_identity_v1::same_structural_layout_v1(previous, layout)
    {
        return Err((location, SemanticMirErrorV1::InvalidTypeLayout));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
