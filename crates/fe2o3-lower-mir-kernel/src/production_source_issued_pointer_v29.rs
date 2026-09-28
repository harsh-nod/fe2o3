// Original issuer recipes classify a source access, not an arbitrary physical
// pointer. The actual descriptor tail and guarded access are checked separately.
include!("production_source_issued_roles_v29.rs");
include!("production_source_issued_semantic_v29.rs");
#[cfg(test)]
#[path = "production_source_issued_pointer_source_v29_tests.rs"]
mod source_issued_pointer_source_tests_v29;
#[cfg(test)]
#[path = "production_source_issued_pointer_v29_tests.rs"]
mod source_issued_pointer_tests_v29;

#[derive(Clone, Copy)]
struct SourceIssuedRecipeV29 {
    issuer: SsaValueV1,
    block: SemanticBlockIdV1,
    option_type: SemanticTypeIdV1,
    pointer_type: SemanticTypeIdV1,
    availability: SemanticOptionAvailabilityV1,
    element: ScalarType,
    access: AccessMode,
    present: ValueId,
    pointer: ValueId,
    form: SourceIssuedFormV29,
}

struct SourceIssuedOriginalV29<'scope, 'owner, 'source> {
    instances: &'scope ExecutionInstancesV29<'owner>,
    instance: ProductionCallInstanceIdV1,
    source_index: &'scope SourceAddressSourceIndexV29<'source>,
    semantic: SourceIssuedSemanticV29<'scope, 'owner, 'source>,
}

fn source_issued_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29(
        "source issued pointer differs from its original issuer or actual guard",
    )
}

impl<'scope, 'owner, 'source> SourceIssuedOriginalV29<'scope, 'owner, 'source> {
    fn new(
        instances: &'scope ExecutionInstancesV29<'owner>,
        instance: ProductionCallInstanceIdV1,
        source_index: &'scope SourceAddressSourceIndexV29<'source>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let semantic = SourceIssuedSemanticV29::new(
            instances,
            instance,
            SourceIssuedReplayModeV29::Emitted(source_index),
            budget,
        )?;
        Ok(Self {
            instances,
            instance,
            source_index,
            semantic,
        })
    }

    fn archived<'a>(
        &'a self,
        value: SsaValueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'a SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.semantic.retain((|| {
            self.semantic.check(budget)?;
            self.source_index
                .sidecar(self.instance, budget)?
                .execution_observation
                .as_ref()
                .ok_or_else(source_issued_error_v29)?
                .lookup_original_v29(self.instances, self.instance, value, budget)
        })())
    }

    fn use_value(
        &self,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
        self.semantic.use_value(site, role, place, budget)
    }

    fn check_archive(
        &self,
        value: SsaValueV1,
        recipe: SourceIssuedRecipeV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.semantic.retain((|| {
            self.semantic.check(budget)?;
            check_source_issued_archive_v29(
                self.instances,
                self.instance,
                self.source_index,
                value,
                recipe,
                budget,
            )
        })())
    }

    fn resolve(
        &mut self,
        value: SsaValueV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<SourceIssuedRecipeV29>, ProductionSemanticKirErrorV1> {
        let result = (|| {
            self.semantic.check(budget)?;
            budget.charge_work(1)?;
            if !matches!(&self.semantic.mode, SourceIssuedReplayModeV29::Emitted(index)
            if std::ptr::eq(*index, self.source_index))
            {
                return Err(source_issued_error_v29());
            }
            let Some(recipe) = self.semantic.resolve(value, budget)? else {
                return Ok(None);
            };
            // The semantic result has no physical identities. This adapter cannot
            // produce them without the exact retained issuer archive.
            Ok(Some(source_issued_archive_recipe_v29(
                self.instances,
                self.instance,
                self.source_index,
                recipe,
                budget,
            )?))
        })();
        self.semantic.retain(result)
    }
}

fn source_issued_archive_recipe_v29(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    source_index: &SourceAddressSourceIndexV29<'_>,
    recipe: SourceIssuedSemanticRecipeV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SourceIssuedRecipeV29, ProductionSemanticKirErrorV1> {
    let archive = source_index
        .sidecar(instance, budget)?
        .execution_observation
        .as_ref()
        .ok_or_else(source_issued_error_v29)?;
    let binding = archive.lookup_original_v29(instances, instance, recipe.issuer, budget)?;
    budget.charge_work(5)?;
    let SemanticValueBindingV1::OptionPointer {
        present,
        pointer,
        pointer_ty,
        availability,
    } = binding
    else {
        return Err(source_issued_error_v29());
    };
    if *availability != recipe.availability
        || !matches!(pointer_ty, Type::Pointer(ty)
        if *ty.pointee == Type::Scalar(recipe.element) && ty.address_space == AddressSpace::Global
            && ty.access == recipe.access)
    {
        return Err(source_issued_error_v29());
    }
    Ok(SourceIssuedRecipeV29 {
        issuer: recipe.issuer,
        block: recipe.block,
        option_type: recipe.option_type,
        pointer_type: recipe.pointer_type,
        availability: recipe.availability,
        element: recipe.element,
        access: recipe.access,
        present: *present,
        pointer: *pointer,
        form: recipe.form,
    })
}

fn check_source_issued_archive_v29(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    source_index: &SourceAddressSourceIndexV29<'_>,
    value: SsaValueV1,
    recipe: SourceIssuedRecipeV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(8)?;
    let expected = |ty: &Type| {
        matches!(ty, Type::Pointer(pointer)
        if *pointer.pointee == Type::Scalar(recipe.element)
            && pointer.address_space == AddressSpace::Global && pointer.access == recipe.access)
    };
    let binding = source_index
        .sidecar(instance, budget)?
        .execution_observation
        .as_ref()
        .ok_or_else(source_issued_error_v29)?
        .lookup_original_v29(instances, instance, value, budget)?;
    match (recipe.form, binding) {
        (
            SourceIssuedFormV29::Option,
            SemanticValueBindingV1::OptionPointer {
                present,
                pointer,
                pointer_ty,
                availability,
            },
        ) if *present == recipe.present
            && *pointer == recipe.pointer
            && *availability == recipe.availability
            && expected(pointer_ty) =>
        {
            Ok(())
        }
        (SourceIssuedFormV29::Pointer, SemanticValueBindingV1::Value { id, ty })
            if *id == recipe.pointer && expected(ty) =>
        {
            Ok(())
        }
        _ => Err(source_issued_error_v29()),
    }
}
