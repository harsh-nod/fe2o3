//! Shared charged completion machinery; invocation authority remains separately constructed.

use fe2o3_runtime::{
    GeneratedGfx942PersistentStorageV1, KfdRuntimeBackendErrorV1, RuntimeErrorV1,
    RuntimeGfx942GeneratedCarrierV1, RuntimeGfx942GeneratedCompletionCarrierV1,
    RuntimeGfx942GeneratedCompletionViewV1, RuntimeGfx942GeneratedSourceMutV1,
    RuntimeGfx942GeneratedSourceV1, RuntimeGfx942ReadbackErrorV1,
    WorkerV3Gfx942ExecutionAuthorityV1,
};

use crate::generated_runtime_arguments::{
    GeneratedRuntimeReadbackOwnerV1, GeneratedRuntimeStorageV1,
};
use crate::{
    GeneratedRuntimeArgumentErrorV1, GeneratedRuntimeArgumentFootprintV1,
    GeneratedRuntimeResultBudgetV1,
};

mod registry;
pub(crate) use registry::GeneratedRegistryCarrierV1;

pub(crate) trait GeneratedRuntimeAuthorityV1: WorkerV3Gfx942ExecutionAuthorityV1 {
    fn artifact_bytes(&self) -> &[u8];
}

// Storage is disposed before authority and credits. Neither the decoder nor authority escapes.
pub(crate) struct GeneratedRuntimeCarrierV1<A, P = GeneratedGfx942PersistentStorageV1> {
    pub(crate) storage: GeneratedRuntimeStorageV1<P>,
    pub(crate) authority: A,
    pub(crate) footprint: GeneratedRuntimeArgumentFootprintV1,
    pub(crate) result_budget: GeneratedRuntimeResultBudgetV1,
}

impl<A: GeneratedRuntimeAuthorityV1> RuntimeGfx942GeneratedCarrierV1
    for GeneratedRuntimeCarrierV1<A>
{
    type CurrentnessError = A::CurrentnessError;
    type Readback = GeneratedRuntimeReadbackOwnerV1;

    fn source(&self) -> RuntimeGfx942GeneratedSourceV1<'_, Self::CurrentnessError> {
        RuntimeGfx942GeneratedSourceV1::from_generated_storage(
            self.storage.prepared(),
            self.authority.artifact_bytes(),
            &self.authority,
        )
    }

    fn source_mut(
        &mut self,
    ) -> Option<RuntimeGfx942GeneratedSourceMutV1<'_, Self::CurrentnessError>> {
        Some(RuntimeGfx942GeneratedSourceMutV1::new(
            self.storage.prepared_mut(),
            self.authority.artifact_bytes(),
            &self.authority,
        ))
    }

    fn prepare_readback(&self) -> Result<Self::Readback, RuntimeGfx942ReadbackErrorV1> {
        self.storage.prepare_readback().map_err(readback_error)
    }

    fn install_readback(&mut self, readback: Self::Readback) {
        self.storage.install_readback(readback);
    }
}

// SAFETY: the private storage retains the original charged destinations, decoder and gate.
// The runtime settles native and Context custody before consuming it. Both invocation paths
// lend only that original source and its reserved destinations, with no replacement or retry.
unsafe impl<A: GeneratedRuntimeAuthorityV1> RuntimeGfx942GeneratedCompletionCarrierV1
    for GeneratedRuntimeCarrierV1<A>
{
    fn completion_domain_v1(
        &self,
    ) -> Result<fe2o3_runtime::RuntimeGeneratedResultDomainV1, RuntimeGfx942ReadbackErrorV1> {
        self.storage
            .completion_domain_v1()
            .map_err(|_| RuntimeGfx942ReadbackErrorV1::InvalidStorage)
    }

    fn with_completion_view_v1(
        &mut self,
        callback: impl for<'a> FnOnce(
            RuntimeGfx942GeneratedCompletionViewV1<'a, Self::CurrentnessError>,
        ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<(), RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
        let (payload, destinations) = self.storage.borrow_reserved_readback_v1().map_err(|_| {
            RuntimeErrorV1::Validation(
                fe2o3_runtime::RuntimeValidationErrorV1::InvalidBackendDescription,
            )
        })?;
        let source = RuntimeGfx942GeneratedSourceV1::from_generated_storage(
            payload,
            self.authority.artifact_bytes(),
            &self.authority,
        );
        callback(RuntimeGfx942GeneratedCompletionViewV1::new(
            source,
            destinations,
        ))
    }

    fn complete_readback_v1(self) -> Result<(), RuntimeGfx942ReadbackErrorV1> {
        let Self {
            storage,
            authority,
            footprint: _,
            result_budget,
        } = self;
        // Keep authority until the decoder has disposed storage and committed its final gate.
        let result = storage
            .decode_reserved_readback()
            .map_err(|error| match error {
                GeneratedRuntimeArgumentErrorV1::StaleOrAliasedOutput => {
                    RuntimeGfx942ReadbackErrorV1::InvalidStorage
                }
                error => readback_error(error),
            });
        drop(authority);
        drop(result_budget);
        result
    }
}

fn readback_error(error: GeneratedRuntimeArgumentErrorV1) -> RuntimeGfx942ReadbackErrorV1 {
    match error {
        GeneratedRuntimeArgumentErrorV1::ResultCredit(error) => {
            RuntimeGfx942ReadbackErrorV1::Credit(error)
        }
        GeneratedRuntimeArgumentErrorV1::Allocation => RuntimeGfx942ReadbackErrorV1::Allocation,
        GeneratedRuntimeArgumentErrorV1::StaleOrAliasedOutput => {
            RuntimeGfx942ReadbackErrorV1::AlreadyReserved
        }
        _ => RuntimeGfx942ReadbackErrorV1::InvalidStorage,
    }
}
