//! Original-ledger Option-producer collection and exact Some-region facts.
//! These use the same resource adapter as enum-payload facts, not a fresh ledger.
use super::*;

#[cfg(test)]
#[path = "semantic_option_resources_v1_tests.rs"]
mod tests;

/// Collects actual centrally classified Option producers on the caller meter.
///
/// All accepted storage remains caller-owned until the returned vector and any
/// partial values have dropped, including on error/unwind. No storage is released
/// here. Producer order/classification and the legacy collector remain unchanged.
/// This inventory is inert and does not confer compiler or artifact authority.
pub fn semantic_option_producers_with_meter_v1<M: SemanticEnumPayloadMeterV1>(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    meter: &mut M,
) -> Result<Vec<SemanticOptionProducerV1>, SemanticEnumPayloadMeteredErrorV1<M::Error>> {
    let Some(frame_storage) = metered_fact_frame_storage_v1::<M, Vec<SemanticOptionProducerV1>>()
    else {
        return Err(SemanticEnumPayloadMeteredErrorV1::Arithmetic);
    };
    let mut adapter = Adapter {
        original: meter,
        failure: None,
    };
    let result = {
        let mut budget = WorkBudgetV1 {
            used: 0,
            meter: Some(&mut adapter),
        };
        budget.reserve_bytes(frame_storage).and_then(|()| {
            semantic_option_producers_with_budget_v1(function, callables, &mut budget)
        })
    };
    match adapter.failure {
        Some(error) => Err(error),
        None => result.map_err(SemanticEnumPayloadMeteredErrorV1::Analysis),
    }
}

impl SemanticOptionDominanceV1 {
    /// Derives exact Option Some-region facts on the original caller meter.
    ///
    /// Shares the legacy algorithm, local counter and independent local limit.
    /// Additional allocation/initialization/copy work is admitted externally
    /// without changing `work_units()`. Every accepted reservation remains owned
    /// by the caller until facts and partial values drop on every exit path.
    /// Neither producer descriptions nor these facts grant compiler authority.
    pub fn analyze_with_meter_v1<M: SemanticEnumPayloadMeterV1>(
        function: &SemanticFunctionDeclV1,
        producers: &[SemanticOptionProducerV1],
        meter: &mut M,
    ) -> Result<Self, SemanticEnumPayloadMeteredErrorV1<M::Error>> {
        let Some(frame_storage) = metered_fact_frame_storage_v1::<M, Self>() else {
            return Err(SemanticEnumPayloadMeteredErrorV1::Arithmetic);
        };
        let mut adapter = Adapter {
            original: meter,
            failure: None,
        };
        let result = {
            let mut budget = WorkBudgetV1 {
                used: 0,
                meter: Some(&mut adapter),
            };
            budget
                .reserve_bytes(frame_storage)
                .and_then(|()| Self::analyze_with_budget(function, producers, &mut budget))
        };
        match adapter.failure {
            Some(error) => Err(error),
            None => result.map_err(SemanticEnumPayloadMeteredErrorV1::Analysis),
        }
    }
}
