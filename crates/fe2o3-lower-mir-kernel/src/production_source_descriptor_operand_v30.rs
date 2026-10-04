// This receipt comes from the original BaseUse and its archived whole binding,
// independently of the emitted metadata operation and archived rvalue result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceDescriptorOperandV30 {
    event: usize,
    original: SsaValueV1,
    receiver: ValueId,
}

fn source_descriptor_operand_headers_v30() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        [&'a (); 8],
        [usize; 4],
        SourceDescriptorOperandV30,
        Option<SourceDescriptorOperandV30>,
        Option<(&'a SemanticPlaceV1, usize, ExecutionOperandV29)>,
        Option<(usize, SsaValueV1)>,
        Option<&'a fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>,
        fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'a>,
        Result<Option<SourceDescriptorOperandV30>, ProductionSemanticKirErrorV1>,
        SourceOwnedResultV18<()>,
        Result<&'a SemanticValueBindingV1, ProductionSemanticKirErrorV1>,
        &'a SemanticValueBindingV1,
        std::iter::Enumerate<
            std::slice::Iter<'a, fe2o3_pliron::ProductionSemanticSsaEventOccurrenceV1>,
        >,
        std::slice::Iter<'a, SemanticProjectionV1>,
    );
    argument_sum_v1(&[size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>()])
}

fn source_descriptor_operand_place_v30(
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
) -> Option<(&SemanticPlaceV1, usize, ExecutionOperandV29)> {
    let (place, fields, role) = match assignment.value().kind() {
        SemanticRvalueKindV1::Length(place) => {
            let fields = place.projections().len()
                - usize::from(place.projections().last().is_some_and(|projection| {
                    projection.kind() == SemanticProjectionKindV1::Dereference
                }));
            (place, fields, ExecutionOperandV29::RvaluePlace)
        }
        SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::PointerMetadata,
            operand: SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
        } => (
            place,
            place.projections().len(),
            ExecutionOperandV29::RvalueOperand(0),
        ),
        _ => return None,
    };
    Some((place, fields, role))
}

fn retain_source_descriptor_operand_v30(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    archive: &ExecutionArchiveV29,
    site: ExecutionSiteV29,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<SourceDescriptorOperandV30>, ProductionSemanticKirErrorV1> {
    budget.charge_work(3)?;
    let Some((place, fields, role)) = source_descriptor_operand_place_v30(assignment) else {
        return Ok(None);
    };
    budget.charge_work(place.projections().len())?;
    if place.projections()[..fields]
        .iter()
        .any(|projection| !matches!(projection.kind(), SemanticProjectionKindV1::Field(_)))
    {
        return Ok(None);
    }
    let occurrences = instances
        .occurrences(instance)
        .ok_or_else(execution_archive_error_v29)?;
    let mut selected = None;
    for (index, event) in occurrences.events().iter().enumerate() {
        budget.charge_work(6)?;
        if event.site() != site
            || event.operand() != role
            || event.role() != ExecutionEventV29::BaseUse
        {
            continue;
        }
        if event.event().variable().get() != place.local().index() {
            return Err(execution_archive_error_v29());
        }
        if !event.is_reachable() || !event.is_promoted() {
            return Ok(None);
        }
        let Some(SsaResolvedEventV1::Use { variable, value }) = event.resolved() else {
            return Err(execution_archive_error_v29());
        };
        if variable.get() != place.local().index() || selected.replace((index, value)).is_some() {
            return Err(execution_archive_error_v29());
        }
    }
    let Some((event, original)) = selected else {
        return Ok(None);
    };
    let mut binding = archive.lookup_original_v29(instances, instance, original, budget)?;
    for projection in &place.projections()[..fields] {
        budget.charge_work(2)?;
        let (SemanticProjectionKindV1::Field(field), SemanticValueBindingV1::Aggregate(values)) =
            (projection.kind(), binding)
        else {
            return Ok(None);
        };
        binding = values
            .get(field as usize)
            .ok_or_else(execution_archive_error_v29)?;
    }
    match binding {
        SemanticValueBindingV1::Value {
            id,
            ty: Type::Slice(_),
        } => Ok(Some(SourceDescriptorOperandV30 {
            event,
            original,
            receiver: *id,
        })),
        _ => Ok(None),
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    fn check_descriptor_operand_v30(
        &self,
        root: usize,
        locator: &SourceRvalueRowV30,
        receiver: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        let (row, assignment) = self.assignment_result_row_v30(
            root,
            locator.instance,
            SemanticBlockIdV1::from_index(locator.block),
            locator.statement,
            budget,
        )?;
        if !std::ptr::eq(row, locator) {
            return self.source.missing("descriptor operand locator differs");
        }
        budget.charge_work(12)?;
        let Some((place, fields, role)) = source_descriptor_operand_place_v30(assignment) else {
            return self
                .source
                .missing("descriptor operand original place is unsupported");
        };
        budget.charge_work(place.projections().len())?;
        if place.projections()[..fields]
            .iter()
            .any(|projection| !matches!(projection.kind(), SemanticProjectionKindV1::Field(_)))
        {
            return self
                .source
                .missing("descriptor operand original place is unsupported");
        }
        let receipt = locator
            .descriptor
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "descriptor operand original binding is absent",
            ))?;
        let (function, _) = self.source.instance(root, locator.instance, budget)?;
        let source = self.source.source_ssa(budget)?;
        let occurrences = source
            .occurrences_v1()
            .and_then(|view| view.function(function))
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "descriptor operand occurrences are absent",
            ))?;
        let event = occurrences.events().get(receipt.event).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("descriptor operand source event is absent"),
        )?;
        if event.site()
            != execution_site_v29(
                SemanticBlockIdV1::from_index(locator.block),
                Some(locator.statement),
            )
            || event.operand() != role
            || event.role() != ExecutionEventV29::BaseUse
            || !event.is_reachable()
            || !event.is_promoted()
            || event.event().variable().get() != place.local().index()
            || event.resolved()
                != Some(SsaResolvedEventV1::Use {
                    variable: fe2o3_mir_model::SsaVariableIdV1::new(place.local().index()),
                    value: receipt.original,
                })
        {
            return self.source.missing("descriptor operand source use differs");
        }
        if receiver != receipt.receiver {
            return self
                .source
                .missing("descriptor operand archived receiver differs");
        }
        Ok(())
    }
}
