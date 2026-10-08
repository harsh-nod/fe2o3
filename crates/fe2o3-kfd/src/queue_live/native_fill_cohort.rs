//! Cohort admission enters the existing original-parent preparation custody.

use super::*;
use crate::queue::dispatch_binding::Gfx942NativeFillCohortV1;

impl SharedGttMemorySessionV1 {
    /// Creates one primary queue from this original VM and 2..=16 closed fills.
    ///
    /// Every program, packet and distinct DATA owner is retained before any
    /// fallible preparation. A refusal retains the original construction root;
    /// partial or unknown native effects keep the existing terminal policy.
    /// This is not a retry, member cancellation or singleton admission path.
    /// Publication, polling and recycle use the same whole-batch `N` afterward.
    /// No runtime aggregate bridge or batch-refinement theorem is supplied.
    pub fn create_compute_aql_queue_with_native_fill_cohort_v1<const N: usize>(
        self,
        ring_bytes: u32,
        cohort: Gfx942NativeFillCohortV1<'_, N>,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        self.create_compute_aql_queue_with_native_fill_cohort_and_capacity_v1(
            ring_bytes,
            cohort,
            Gfx942FixedDispatchCapacityV1::default(),
        )
    }

    /// Uses the original immutable dispatch-capacity account. Capacity profiles
    /// restricted to singleton recipes still reject a cohort before preparation.
    pub fn create_compute_aql_queue_with_native_fill_cohort_and_capacity_v1<const N: usize>(
        self,
        ring_bytes: u32,
        cohort: Gfx942NativeFillCohortV1<'_, N>,
        capacity: Gfx942FixedDispatchCapacityV1,
    ) -> Result<ComputeAqlQueueSessionV1, ComputeAqlQueueSessionErrorV1> {
        let Gfx942NativeFillCohortV1 {
            programs,
            packets,
            data,
        } = cohort;
        let mut root = PrimaryQueueConstructionV1::new(
            self,
            (
                programs,
                FixedDispatchPreparationCustodyV1::new_native_fill_cohort(packets, data),
            ),
        );
        root.dispatch_capacity = capacity;
        let mut root =
            root.run(|root, entry| root.construct_native_fill_cohort(entry, ring_bytes))?;
        let Some(completed) = root.completed.take() else {
            std::process::abort();
        };
        Ok(completed.into_session())
    }
}

type CohortPreparationV1<'a, const N: usize> = (
    Vec<fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'a>>,
    FixedDispatchPreparationCustodyV1<N>,
);

impl<const N: usize, E: construction_primary::PrimaryEnvironmentV1>
    PrimaryQueueConstructionV1<CohortPreparationV1<'_, N>, E>
where
    E::Memory: crate::queue::dispatch_binding::preparation::PreparationMemoryV1,
{
    pub(super) fn construct_native_fill_cohort(
        &mut self,
        entry: &mut construction_primary::UserptrConstructionEntryV1<'_>,
        ring_bytes: u32,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.dispatch_capacity.validate_batch::<N>()?;
        validate_fixed_batch_ring::<N>(ring_bytes)?;
        PreparedDispatchGenerationV1::validate_target(&self.prepared_generation, None)?;
        PreparedDispatchGenerationV1::ensure_preallocated::<N>(
            &mut self.prepared_generation,
            &self.dispatch_capacity,
            DispatchGenerationSeedV1::Fresh,
        )?;
        let memory = self
            .memory
            .as_mut()
            .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                "missing cohort construction memory",
            ))?;
        let geometry = memory.plan_aql_queue_resources(ring_bytes)?;
        crate::queue::dispatch_binding::prepare_public_fixed_dispatch_resources_with_capacity_in_place(
            memory,
            &self.preparation.0,
            &mut self.preparation.1,
            &self.dispatch_capacity,
            &mut self.prepared_generation,
        )?;
        self.dispatch = Some(self.preparation.1.take_completed()?);
        self.construct(
            entry,
            geometry,
            ring_bytes,
            QueueRingBackingV1::AqlSpecial,
            None,
        )
    }
}

impl ComputeAqlQueueSessionV1 {
    /// Binds 2..=16 distinct closed-full64 operations on an unused primary queue.
    ///
    /// Every packet remains `WaitForPrior`; this is not physical overlap or
    /// out-of-order completion. Use the existing fixed batch submission, poll,
    /// recycle and recycled readback APIs with the same `N`. No member result
    /// or resource release is available before the entire cohort is settled.
    /// Singleton admission and replay retain their existing separate contracts.
    ///
    /// This consumes original checked executable and DATA custody. All inputs
    /// are rooted before any native preparation. A refusal retains consumed
    /// inputs; partial/unknown preparation also retains the original parent
    /// under the existing terminal policy. It is not a cancellation result.
    /// This checked composition is not covered by a new batch-refinement proof.
    pub fn bind_initial_native_fill_cohort_v1<const N: usize>(
        &mut self,
        cohort: Gfx942NativeFillCohortV1<'_, N>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let Gfx942NativeFillCohortV1 {
            programs,
            packets,
            data,
        } = cohort;
        let mut outputs = data.into_iter();
        let mut root = initial_bind::InitialBindingCustodyV1::new(
            programs,
            packets,
            move |_: &mut SharedGttMemorySessionV1, _: usize| {
                outputs
                    .next()
                    .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
                        "cohort original DATA cardinality",
                    ))
            },
        );
        root.require_native_fill_cohort();
        initial_bind::bind_initial_with_v1(&mut &mut *self, root, N, core::mem::forget)
    }
}
