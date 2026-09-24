// The owning Pliron source layer implements the complete argument checker.
use fe2o3_pliron::source_argument_v1 as source_arguments_v1;
use source_arguments_v1::{
    ArgumentBudgetV1, ArgumentLedgerV1, ArgumentResourceV1, ArgumentTraceV1, argument_product_v1,
    argument_sum_v1, argument_vec_v1, sort_correspondence_keys_v1,
};
#[cfg(test)]
use source_arguments_v1::{
    IndexedArgumentTraceV1, PhysicalArgumentTraceV1, sort_argument_trace_v1,
};

impl From<ArgumentResourceV1> for ProductionSemanticKirErrorV1 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::ArgumentCorrespondenceResource(error)
    }
}

impl From<source_arguments_v1::ProductionSourceArgumentErrorV1> for ProductionSemanticKirErrorV1 {
    fn from(error: source_arguments_v1::ProductionSourceArgumentErrorV1) -> Self {
        use source_arguments_v1::ProductionSourceArgumentErrorV1 as E;
        match error {
            E::ArgumentCorrespondenceResource(error) => Self::ArgumentCorrespondenceResource(error),
            E::CorrespondenceMismatch | E::Visitor => Self::CorrespondenceMismatch,
            E::Unsupported {
                function,
                block,
                statement,
                detail,
            } => Self::Unsupported {
                function,
                block,
                statement,
                detail,
            },
            E::ScalarTypeUnavailable {
                semantic_type,
                shape,
            } => Self::ScalarTypeUnavailable {
                semantic_type,
                shape,
            },
            E::AllocationFailure {
                resource: source_arguments_v1::ProductionSourceArgumentResourceV1::DebugBindings,
            } => Self::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::DebugBindings,
            },
        }
    }
}

// A visitor's lower-layer error stays in the lower layer. Shared cleanup errors
// take precedence, exactly as they did in the original scoped checker.
fn argument_visitor_result_v1<R>(
    result: Result<R, source_arguments_v1::ProductionSourceArgumentErrorV1>,
    visitor_error: Option<ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    match result {
        Err(source_arguments_v1::ProductionSourceArgumentErrorV1::Visitor) => {
            Err(visitor_error.unwrap_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch))
        }
        other => other.map_err(Into::into),
    }
}

fn validate_parameter_correspondence_v1(
    semantic: &AdmittedInertSemanticMirV1,
    instance: &SemanticKirFunctionCorrespondenceV1,
    target: &Function,
    trace: ArgumentTraceV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_arguments_v1::validate_parameter_correspondence_v1(
        semantic, instance, target, trace, budget,
    )
    .map_err(Into::into)
}

fn with_parameter_correspondence_v1<'w, R>(
    semantic: &AdmittedInertSemanticMirV1,
    instance: &SemanticKirFunctionCorrespondenceV1,
    target: &Function,
    trace: ArgumentTraceV1<'_>,
    budget: &mut ArgumentBudgetV1<'w>,
    use_view: impl for<'s> FnOnce(
        &mut ProductionArgumentViewV1<'s, 'w>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    let mut visitor_error = None;
    let result = source_arguments_v1::with_parameter_correspondence_v1(
        semantic,
        instance,
        target,
        trace,
        budget,
        |data, budget| {
            use_view(&mut ProductionArgumentViewV1 { data, budget }).map_err(|error| {
                visitor_error = Some(error);
                source_arguments_v1::ProductionSourceArgumentErrorV1::Visitor
            })
        },
    );
    argument_visitor_result_v1(result, visitor_error)
}

fn prepay_argument_shape_v1(
    semantic: &AdmittedInertSemanticMirV1,
    ty: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_arguments_v1::prepay_argument_shape_v1(semantic, ty, budget).map_err(Into::into)
}

fn prepay_typed_shape_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    callable_count: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    source_arguments_v1::prepay_typed_shape_v1(types, ty, callable_count, budget)
        .map_err(Into::into)
}
