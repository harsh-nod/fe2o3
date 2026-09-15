//! Private native-provider/source-input custody for the pending memory consumer.
//! This does not discharge Result success, geometry, bounds, or observed reads.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionGlobalBf16SourceBatchV1, ProductionGlobalBf16SourceRowV1,
    ProductionScopedBf16LaneUseV1, ProductionScopedMatrixSourceSessionV1,
    ProductionSemanticKirErrorV1,
};
use numerical_policy_v1::defined_body_v1::matrix::ConstructorRowV1;

#[path = "kernel_context_bf16_inputs_v1/guarded_events_v1.rs"]
mod guarded_events_v1;
pub(crate) use guarded_events_v1::{GuardedBf16SourceEventsV1, RootBf16PhysicalInputV1};

pub(crate) struct CapturedGlobalBf16SourceInputsV1<'native, 'source> {
    session: ProductionScopedMatrixSourceSessionV1<'source>,
    batch: ProductionGlobalBf16SourceBatchV1<'source>,
    providers: Vec<&'native ConstructorRowV1>,
}

impl AuthenticatedProductionKernelContextsV1 {
    pub(crate) fn capture_ranked_bf16_inputs<'native, 'source>(
        &'native self,
        mut session: ProductionScopedMatrixSourceSessionV1<'source>,
    ) -> Result<CapturedGlobalBf16SourceInputsV1<'native, 'source>, ProductionSemanticKirErrorV1>
    {
        let roster = self
            .global_bf16_constructors
            .as_ref()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let (batch, providers) = session.with_global_bf16_source_inputs(|batch, charge| {
            charge(1)?;
            if !roster.matches_subject(batch.owner().source_semantic().semantic_sha256()) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            let count = batch.rows().len();
            let words = provider_storage_words(count)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            charge(words)?;
            let mut providers = Vec::new();
            providers
                .try_reserve_exact(count)
                .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if providers.capacity() != count {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            for row in batch.rows() {
                providers.push(roster.join(row, charge)?);
            }
            Ok((batch, providers))
        })?;
        Ok(CapturedGlobalBf16SourceInputsV1 {
            session,
            batch,
            providers,
        })
    }
}

impl<'source> CapturedGlobalBf16SourceInputsV1<'_, 'source> {
    pub(crate) fn len(&self) -> usize {
        self.providers.len()
    }

    pub(crate) fn row(&self, index: usize) -> Option<ProductionGlobalBf16SourceRowV1<'_, 'source>> {
        self.batch.row(index)
    }

    pub(crate) fn result_types(&self, index: usize) -> Option<[SemanticTypeIdV1; 3]> {
        self.providers.get(index).map(|p| p.result_types())
    }

    pub(crate) fn checked_constructor_lane(
        &mut self,
        index: usize,
    ) -> Result<ProductionScopedBf16LaneUseV1, ProductionSemanticKirErrorV1> {
        let source = self
            .batch
            .row(index)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let provider = self
            .providers
            .get(index)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        self.session.checked_bf16_constructor_lane(
            source,
            provider.identity(),
            provider.bound_types(),
        )
    }

    #[cfg(test)]
    pub(crate) fn test_inert_requirement(
        &mut self,
        index: usize,
        identity: fe2o3_mir_model::semantic_mir_v1::SemanticDefinedMatrixIdentityV1,
        types: fe2o3_mir_model::semantic_mir_v1::SemanticPolicyMatrixBindTypesV1,
    ) -> Result<ProductionScopedBf16LaneUseV1, ProductionSemanticKirErrorV1> {
        let row = self
            .batch
            .row(index)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        self.session
            .checked_bf16_constructor_lane(row, identity, types)
    }

    #[cfg(test)]
    pub(crate) fn test_finish_without_reads(self) -> Result<(), ProductionSemanticKirErrorV1> {
        self.session.finish(&[]).map(|_| ())
    }
}

fn provider_storage_words(count: usize) -> Option<usize> {
    let bytes = count
        .checked_mul(std::mem::size_of::<&ConstructorRowV1>())?
        .checked_add(std::mem::size_of::<Vec<&ConstructorRowV1>>())?;
    Some(bytes.div_ceil(std::mem::size_of::<usize>()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bf16_native_join_pointer_capacity_is_charged_before_reservation() {
        let word = std::mem::size_of::<usize>();
        for count in [0, 1, 4, 1024] {
            assert_eq!(
                provider_storage_words(count),
                Some(
                    (count * std::mem::size_of::<&ConstructorRowV1>()
                        + std::mem::size_of::<Vec<&ConstructorRowV1>>())
                    .div_ceil(word)
                )
            );
        }
        assert_eq!(provider_storage_words(usize::MAX), None);
    }
}
