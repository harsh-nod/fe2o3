//! Error-only scalar observations; no resource or semantic decision is made here.

use super::{ProductionSemanticSsaErrorV1, SsaPlannerResourceV1};

#[derive(Clone, Copy)]
pub(super) enum Stage {
    Auxiliary,
    Combined,
    StateBase,
    Workspace,
    Retention,
    DynamicState,
}

impl Stage {
    fn name(self) -> &'static str {
        match self {
            Self::Auxiliary => "auxiliary",
            Self::Combined => "combined",
            Self::StateBase => "state-base",
            Self::Workspace => "workspace",
            Self::Retention => "retention",
            Self::DynamicState => "dynamic-state",
        }
    }
}

/// State scalars are (successful live words, prior peak, rejected increment).
/// Base-only gates have zero state scalars; None means planning has not run.
pub(super) fn wrap(
    error: ProductionSemanticSsaErrorV1,
    stage: Stage,
    auxiliary_storage_words: usize,
    plan_storage_words: Option<usize>,
    state: (usize, usize, usize),
) -> ProductionSemanticSsaErrorV1 {
    if !matches!(
        error,
        ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            resource: SsaPlannerResourceV1::StorageWords,
            ..
        }
    ) {
        return error;
    }
    ProductionSemanticSsaErrorV1::ResourceStage {
        stage: stage.name(),
        auxiliary_storage_words,
        plan_storage_words,
        live_storage_words: state.0,
        peak_storage_words: state.1,
        requested_storage_words: state.2,
        error: Box::new(error),
    }
}

#[cfg(test)]
#[path = "resource_diagnostic_v1/tests.rs"]
mod tests;
