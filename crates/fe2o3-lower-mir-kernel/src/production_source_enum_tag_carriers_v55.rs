// Retain only the actual tag endpoint. No variant payload, loan or selected
// address can be reconstructed from this physical carrier after archive replay.
fn retain_source_enum_tag_carriers_v55(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    ty: SemanticTypeIdV1,
    binding: &SemanticValueBindingV1,
    carriers: &mut Vec<SourceSsaComponentV37>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceSsaPhysicalV36>, ProductionSemanticKirErrorV1> {
    let SemanticValueBindingV1::SourceEnumTag(binding) = binding else {
        return Ok(None);
    };
    references.check(budget)?;
    let plan = references.plan;
    let result = (|| {
        source_reference_owned_prepay_v29::<(
            SourceSsaComponentV37,
            SourceSsaPhysicalV36,
            Option<SourceSsaPhysicalV36>,
            [&(); 6],
            [usize; 4],
        )>(plan, budget)?;
        budget.source_reference_charge_v29(plan, 7)?;
        if !std::ptr::eq(plan.instances, instances)
            || plan.nodes.get(binding.node).map(|row| row.ty) != Some(ty)
        {
            return Err(source_enum_tag_error_v55());
        }
        validate_source_enum_tag_v55(plan, binding, budget)?;
        let Some(SemanticTypeShapeV1::Enum { discriminant, .. }) = instances
            .owner()
            .source_semantic()
            .types()
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Err(source_enum_tag_error_v55());
        };
        let Some(tag_type @ SourceSsaCarrierTypeV36::Scalar(_)) =
            SourceSsaCarrierTypeV36::from_type(&binding.tag.ty)
        else {
            return Err(source_enum_tag_error_v55());
        };
        let tag = carriers.len();
        emission_push_v1(
            carriers,
            SourceSsaComponentV37 {
                ty: *discriminant,
                physical: SourceSsaPhysicalV36::Value {
                    value: binding.tag.id,
                    ty: tag_type,
                    loan: None,
                },
            },
            budget,
        )?;
        Ok(Some(SourceSsaPhysicalV36::Enum {
            discriminant: tag,
            start: carriers.len(),
            length: 0,
            known_variant: None,
            presence: None,
        }))
    })();
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}
