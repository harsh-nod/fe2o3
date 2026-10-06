use super::*;
use attachments::{
    AttachmentTarget, ProductionOptimizedSourceOperationV18 as OperationPlacement,
    ProductionOptimizedSourceSpanV18 as Span,
};
use fe2o3_kernel_analysis::{
    CanonicalKirBlockControlV1, CanonicalKirCallEffectDecisionV1 as Decision,
    CanonicalKirCallEffectErrorV1, CanonicalKirCallEffectsV18,
};

/// Separate observations of historical and actual optimized instance effects.
/// In particular, dead-control elimination is not evidence of original purity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionOptimizedSourceEffectsV18 {
    /// Complete or incomplete effect decision for the original instance.
    pub input: Decision,
    /// Complete or incomplete decision for its actual transported output.
    pub output: Decision,
}

fn effect_error(error: CanonicalKirCallEffectErrorV1) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        CanonicalKirCallEffectErrorV1::Resource(error) => error.into(),
        _ => ProductionSourceOwnedViewErrorV18::Binding("optimized source effect occurrence"),
    }
}

fn combine(a: Decision, b: Decision) -> Decision {
    match (a, b) {
        (Decision::Incomplete, _) | (_, Decision::Incomplete) => Decision::Incomplete,
        (Decision::CompleteNonempty, _) | (_, Decision::CompleteNonempty) => {
            Decision::CompleteNonempty
        }
        (Decision::CompleteEmpty, Decision::CompleteEmpty) => Decision::CompleteEmpty,
    }
}

impl ProductionOptimizedSourceCorrespondenceV18<'_> {
    /// Authenticates the original semantic block before querying checked control.
    pub fn source_block(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<CanonicalKirBlockControlV1>> {
        self.retain((|| {
            self.query(budget)?;
            self.original
                .source_block_entry(root, instance, block, budget)?
                .map(|input| self.control.block(input, budget).map_err(transition_error))
                .transpose()
        })())
    }

    /// Preserves original assertion polarity and distinguishes every control outcome.
    pub fn assertion(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<SemanticKirOptimizedAssertOutcomeV1> {
        self.retain((|| {
            self.query(budget)?;
            let binding = self.original.assertion(root, instance, block, budget)?;
            optimized_assert_outcome_v1(binding, self.control, budget).map_err(
                |error| match error {
                    SemanticKirOptimizedAssertOriginErrorV1::Resource(error) => error.into(),
                    SemanticKirOptimizedAssertOriginErrorV1::Transition(error) => {
                        transition_error(error)
                    }
                    _ => ProductionSourceOwnedViewErrorV18::Binding(
                        "optimized source assertion outcome",
                    ),
                },
            )
        })())
    }

    /// Visits every attachment of one original site in source order. Gaps,
    /// rewritten values and unreachable occurrences have distinct dispositions.
    pub fn visit_source_operations(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        budget: &mut ArgumentBudgetV1<'_>,
        mut visit: impl FnMut(Span, &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()>,
    ) -> SourceOwnedResultV18<()> {
        self.retain((|| {
            self.query(budget)?;
            self.original.source.instance(root, instance, budget)?;
            let rows = self.index.site(root, instance, block, statement, budget)?;
            for row in rows {
                self.query(budget)?;
                budget.charge_work(1)?;
                let span = match row {
                    AttachmentTarget::Operation(row) => Span::Operation(*row),
                    AttachmentTarget::Gap(row) => Span::Gap(*row),
                    AttachmentTarget::OriginalRemovedCall => Span::OriginalRemovedCall,
                    AttachmentTarget::OriginalNoOutput => Span::OriginalNoOperations,
                    AttachmentTarget::Definition { .. }
                    | AttachmentTarget::Block { .. }
                    | AttachmentTarget::Terminator { .. }
                    | AttachmentTarget::Edge { .. }
                    | AttachmentTarget::Use { .. }
                    | AttachmentTarget::EdgeArgument { .. } => {
                        return resources::binding(
                            "optimized source span has a non-operation attachment",
                        );
                    }
                };
                visit(span, budget)?;
                self.check(budget)?;
            }
            Ok(())
        })())
    }

    /// Joins both exact analysis inventories and preserves incomplete decisions.
    pub fn instance_effects(
        &self,
        root: usize,
        instance: usize,
        input_effects: &CanonicalKirCallEffectsV18<'_, '_>,
        output_effects: &CanonicalKirCallEffectsV18<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedSourceEffectsV18> {
        use TileAttachmentFamilyV29 as Family;
        use TileAttachmentFieldV29 as Field;
        self.retain((|| {
            self.query(budget)?;
            let input =
                self.original
                    .instance_effect_decision(root, instance, input_effects, budget)?;
            budget.charge_work(1)?;
            if !output_effects.belongs_to(self.checked.output()) {
                return resources::binding("foreign optimized instance effect inventory");
            }
            let mut output = Decision::CompleteEmpty;
            let mut classify = |input,
                                budget: &mut ArgumentBudgetV1<'_>|
             -> SourceOwnedResultV18<()> {
                match attachments::operation(self.checked, self.control, self.index, input, budget)?
                {
                    OperationPlacement::Retained { output: actual, .. } => {
                        output = combine(
                            output,
                            output_effects
                                .operation_decision(actual, budget)
                                .map_err(effect_error)?,
                        );
                    }
                    OperationPlacement::Rewritten { .. } => {
                        if input_effects
                            .operation_decision(input, budget)
                            .map_err(effect_error)?
                            != Decision::CompleteEmpty
                        {
                            return resources::binding(
                                "optimized source erased an unaccounted effect",
                            );
                        }
                    }
                    OperationPlacement::RemovedUnreachable { .. } => {}
                }
                Ok(())
            };
            for row in self.original.attachments {
                budget.charge_work(1)?;
                if row.key.root != root
                    || !matches!(
                        (row.key.family, row.key.field),
                        (Family::InstanceSpans, Field::Span)
                            | (Family::Lifecycle, Field::LifecycleOperation)
                            | (Family::Assertion, Field::AssertFailureBlock)
                            | (
                                Family::TerminalFailure,
                                Field::FailureCleanup | Field::FailureDiagnostic
                            )
                    )
                {
                    continue;
                }
                if !self.original.instance_descends_from_v18(
                    root,
                    instance,
                    row.key.instance,
                    budget,
                )? {
                    continue;
                }
                if matches!(
                    (row.key.family, row.key.field),
                    (Family::Assertion, Field::AssertFailureBlock)
                ) {
                    let TileAttachmentLocationV29::Origin(TileScalarSourceV29::Block {
                        function,
                        block,
                    }) = row.location
                    else {
                        return resources::binding(
                            "optimized instance assertion failure attachment",
                        );
                    };
                    let coordinate = attachments::block_coordinate(function, block)?;
                    let ordinal = resources::block_index(self.checked.input(), coordinate, budget)?;
                    for operation in &self.checked.input().operations()
                        [self.checked.input().blocks()[ordinal].operations.clone()]
                    {
                        budget.charge_work(1)?;
                        classify(operation.coordinate, budget)?;
                    }
                } else {
                    match row.location {
                        TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(
                            point,
                        )) => classify(attachments::operation_coordinate(point)?, budget)?,
                        TileAttachmentLocationV29::Gap(_)
                        | TileAttachmentLocationV29::Tombstone
                        | TileAttachmentLocationV29::NoOutput => {}
                        TileAttachmentLocationV29::Origin(_)
                        | TileAttachmentLocationV29::Use(_)
                        | TileAttachmentLocationV29::EdgeArgument(_) => {
                            return resources::binding("optimized instance effect attachment kind");
                        }
                    }
                }
            }
            Ok(ProductionOptimizedSourceEffectsV18 { input, output })
        })())
    }
}
