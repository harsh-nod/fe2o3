//! Indexed logical custody through ordinary binding and submission attempts.

use super::*;

pub(super) fn with_recycled_materialized_metadata_v1<T, E>(
    recycled: &mut Option<RecycledDispatchV1>,
    operation: impl FnOnce(&RecycledDispatchV1) -> Result<T, E>,
) -> Result<T, E> {
    let result = operation(
        recycled
            .as_ref()
            .expect("admitted recycled metadata remains indexed"),
    )?;
    // Include the outer lane close in operation: an inner success is insufficient.
    *recycled = None;
    Ok(result)
}

pub(super) enum MaterializedSubmissionAttemptV1 {
    Unattempted,
    NativeOwned,
    Retryable,
    Published(Gfx942DispatchBatchV1<1>),
    #[cfg(test)]
    ScriptedPublished,
}

pub(super) struct MaterializedBindingV1 {
    pub(super) profile: PersistentPublicationProfileV1,
    pub(super) origin: MaterializedPreparationOriginV1,
    pub(super) submission: MaterializedSubmissionAttemptV1,
    #[cfg(test)]
    pub(super) scripted: Option<(Vec<DataSpecV1>, usize)>,
    #[cfg(test)]
    scripted_initial_retry: bool,
}

impl MaterializedBindingV1 {
    pub(super) fn new(profile: PersistentPublicationProfileV1) -> Self {
        Self {
            profile,
            origin: MaterializedPreparationOriginV1::NewBinding,
            submission: MaterializedSubmissionAttemptV1::Unattempted,
            #[cfg(test)]
            scripted: None,
            #[cfg(test)]
            scripted_initial_retry: false,
        }
    }

    pub(super) fn indexed(active: &mut ActiveSubmissionV1) -> &mut Self {
        let Some(ActiveComputeExecutionV1::MaterializedBinding(root)) = &mut active.execution
        else {
            unreachable!("ordinary binding remains indexed until publication returns")
        };
        root
    }

    pub(super) fn submit<E>(
        &mut self,
        operation: impl FnOnce() -> Result<MaterializedSubmissionAttemptV1, E>,
    ) -> Result<(), E> {
        self.submission = MaterializedSubmissionAttemptV1::NativeOwned;
        // This runs inside the native lane callback, before its outer loan closes.
        self.submission = operation()?;
        Ok(())
    }
}

impl KfdRuntimeBackendV1 {
    pub(super) fn materialized_prepared_selected_v1(&self) -> bool {
        matches!(
            self.active
                .as_ref()
                .and_then(|active| active.execution.as_ref()),
            Some(ActiveComputeExecutionV1::MaterializedPrepared(_))
        )
    }

    pub(super) fn materialized_writebacks_intact_v1(active: &ActiveSubmissionV1) -> bool {
        let recipe = active.ordinary_recipe.as_ref().unwrap();
        let mut count = 0;
        for binding in recipe
            .bindings
            .iter()
            .filter(|binding| binding.region.access != RuntimeAccessV1::Read)
        {
            let Some(writeback) = active.writebacks.get(count) else {
                return false;
            };
            let Some((index, descriptor)) = active
                .resident_descriptors
                .iter()
                .enumerate()
                .find(|(_, descriptor)| descriptor.allocation == binding.region.allocation)
            else {
                return false;
            };
            if writeback.allocation != binding.region.allocation
                || Some(writeback.allocation_offset)
                    != usize::try_from(binding.region.byte_offset).ok()
                || writeback.data_index != index
                || Some(writeback.data_offset)
                    != binding
                        .region
                        .byte_offset
                        .checked_sub(descriptor.allocation_offset)
                || writeback.byte_len != binding.region.byte_len
            {
                return false;
            }
            count += 1;
        }
        count == active.writebacks.len()
    }

    pub(super) fn poll_materialized_prepared_v1(
        &mut self,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let submission = self.active.as_ref().map_or(0, |active| active.id);
        if !self.materialized_prepared_selected_v1()
            || self.selected_compute_lane >= self.native_compute_lanes.len()
            || !self.materialized_prepared_custody_intact_v1(submission)
            || !Self::materialized_writebacks_intact_v1(self.active.as_ref().unwrap())
            || self.native_reconciliation_pins_lane_v1(self.selected_compute_lane)
        {
            return Err(self.terminal_error("materialized retry lost exact prepared custody"));
        }
        let scripted = false;
        #[cfg(test)]
        let scripted = scripted
            || matches!(self.active.as_ref().unwrap().execution.as_ref(),
            Some(ActiveComputeExecutionV1::MaterializedPrepared(prepared)) if prepared.scripted.is_some());
        if !scripted {
            if self.queue.is_none() {
                return Err(self.terminal_error("materialized retry lost its native queue"));
            }
            self.selected_native_compute_lane_v1().map_err(|_| {
                self.terminal_error("materialized retry lost its exact native lane")
            })?;
        }
        #[cfg(test)]
        if scripted && self.scripted_sdma.is_none() {
            return Err(self.terminal_error("materialized retry lost its scripted driver"));
        }
        let active = self.active.as_mut().unwrap();
        let Some(ActiveComputeExecutionV1::MaterializedPrepared(prepared)) =
            active.execution.take()
        else {
            unreachable!("preflighted exact prepared execution")
        };
        let mut root = MaterializedBindingV1::new(prepared.profile);
        root.origin = prepared.origin;
        #[cfg(test)]
        {
            root.scripted = prepared.scripted;
        }
        active.execution = Some(ActiveComputeExecutionV1::MaterializedBinding(root));
        self.submit_materialized_binding_v1()
            .map(|()| BackendPollV1::Pending)
    }

    pub(super) fn submit_materialized_binding_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if !matches!(
            MaterializedBindingV1::indexed(self.active.as_mut().unwrap()).submission,
            MaterializedSubmissionAttemptV1::Unattempted
        ) {
            return Err(
                self.terminal_error("ordinary publication cannot repeat a consumed attempt")
            );
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let started = Instant::now();
            let scripted = false;
            #[cfg(test)]
            let scripted = scripted
                || MaterializedBindingV1::indexed(self.active.as_mut().unwrap())
                    .scripted
                    .is_some();
            if !scripted {
                self.submit_native_materialized_binding_v1()?;
            }
            #[cfg(test)]
            if scripted {
                self.submit_scripted_materialized_binding_v1()?;
            }
            self.active.as_mut().unwrap().performance.publication += started.elapsed();
            self.finish_materialized_binding_v1()
        }));
        match result {
            Ok(result) => result,
            Err(payload) => super::sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                self.poison_terminal_v1()
            }),
        }
    }

    fn submit_native_materialized_binding_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let native_lane = self
            .selected_native_compute_lane_v1()
            .map_err(|_| self.terminal_error("ordinary publication lost its exact native lane"))?;
        let root = MaterializedBindingV1::indexed(self.active.as_mut().unwrap());
        let Some(queue) = self.queue.as_mut() else {
            return Err(self.terminal_error("ordinary publication lost its native queue"));
        };
        let publication = queue
            .with_compute_lane_v1(native_lane, |queue| {
                root.submit(|| match queue.submit_fixed_dispatch_classified_v1::<1>() {
                    Ok(batch) => Ok(MaterializedSubmissionAttemptV1::Published(batch)),
                    Err(Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(_)) => {
                        Ok(MaterializedSubmissionAttemptV1::Retryable)
                    }
                    Err(error) => Err(error),
                })
            })
            .map_err(|error| self.terminal_error(format!("KFD compute-lane selection: {error}")))?;
        match publication {
            Ok(()) => Ok(()),
            Err(Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(_)) => {
                unreachable!("retry classification stored inside the native callback")
            }
            Err(Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)) => {
                Err(self.terminal_error(format!(
                    "KFD retained dispatch binding was rejected before publication: {error}"
                )))
            }
            Err(Gfx942FixedDispatchSubmissionFailureV1::Terminal(error)) => Err(self
                .terminal_error(format!(
                    "KFD dispatch publication became indeterminate: {error}"
                ))),
        }
    }

    pub(super) fn finish_materialized_binding_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let active = self.active.as_mut().expect("indexed ordinary binding");
        if matches!(
            MaterializedBindingV1::indexed(active).submission,
            MaterializedSubmissionAttemptV1::Unattempted
                | MaterializedSubmissionAttemptV1::NativeOwned
        ) {
            return Err(self.terminal_error("ordinary binding has no confirmed submission outcome"));
        }
        let Some(ActiveComputeExecutionV1::MaterializedBinding(root)) = active.execution.take()
        else {
            unreachable!("checked indexed ordinary binding")
        };
        match root.submission {
            MaterializedSubmissionAttemptV1::Retryable => {
                let prepared = MaterializedPreparedV1::new(root.profile, root.origin);
                #[cfg(test)]
                let prepared = MaterializedPreparedV1 {
                    scripted: root.scripted,
                    ..prepared
                };
                active.execution = Some(ActiveComputeExecutionV1::MaterializedPrepared(prepared));
                return Ok(());
            }
            MaterializedSubmissionAttemptV1::Published(batch) => {
                active.execution = Some(ActiveComputeExecutionV1::Materialized(batch));
            }
            #[cfg(test)]
            MaterializedSubmissionAttemptV1::ScriptedPublished => {
                active.execution = Some(ActiveComputeExecutionV1::ScriptedMaterialized);
            }
            MaterializedSubmissionAttemptV1::Unattempted
            | MaterializedSubmissionAttemptV1::NativeOwned => {
                unreachable!("preflighted returned submission outcome")
            }
        }
        active.published_at = Instant::now();
        let (id, stream, kernel, shape) = (
            active.id,
            active.stream,
            active.kernel,
            active.dispatch_shape_sha256,
        );
        self.observe_materialized_dispatch_published_v1(id, stream, kernel, shape, root.profile);
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn publish_scripted_materialized_binding_v1(
        &mut self,
        data: Vec<DataSpecV1>,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        use ScriptedMaterializedPublicationFaultV1 as Fault;
        let configuration = self.scripted_materialized_preparation.take();
        let active = self.active.as_mut().unwrap();
        active.performance.data_path = KfdRuntimeLaunchDataPathV1::Materialized;
        active.performance.user_data_materializations = data.len() as u64;
        let root = MaterializedBindingV1::indexed(active);
        let (origin, retries) =
            configuration.unwrap_or((MaterializedPreparationOriginV1::NewBinding, 0));
        root.origin = origin;
        root.scripted = Some((data, retries));
        root.scripted_initial_retry = configuration.is_some();
        let fault = self.scripted_materialized_publication_fault;
        if matches!(
            fault,
            Some(Fault::BindingRejected | Fault::BindingUnwind | Fault::InitialObserverUnwind)
        ) {
            self.scripted_materialized_publication_fault = None;
        }
        match fault {
            Some(Fault::BindingRejected) => {
                return Err(Self::capacity("scripted ordinary binding rejection"));
            }
            Some(Fault::BindingUnwind) => panic!("scripted ordinary binding unwind"),
            Some(Fault::InitialObserverUnwind) => panic!("scripted ordinary queue observer unwind"),
            _ => {}
        }
        self.submit_materialized_binding_v1()
    }

    #[cfg(test)]
    fn submit_scripted_materialized_binding_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        use ScriptedMaterializedPublicationFaultV1 as Fault;
        let fault = self.scripted_materialized_publication_fault;
        if fault != Some(Fault::ProfileUnwind) {
            self.scripted_materialized_publication_fault = None;
        }
        let root = MaterializedBindingV1::indexed(self.active.as_mut().unwrap());
        let retry = root.scripted_initial_retry || root.scripted.as_ref().unwrap().1 != 0;
        root.submit(|| match fault {
            Some(Fault::SubmitUnwind) => panic!("scripted ordinary submit unwind"),
            Some(Fault::SubmitRejected | Fault::SubmitTerminal) => Err(()),
            Some(Fault::OuterErrorAfterRetry | Fault::OuterUnwindAfterRetry) => {
                Ok(MaterializedSubmissionAttemptV1::Retryable)
            }
            _ if retry
                && fault != Some(Fault::ProfileUnwind)
                && !matches!(
                    fault,
                    Some(Fault::OuterErrorAfterPublish | Fault::OuterUnwindAfterPublish)
                ) =>
            {
                Ok(MaterializedSubmissionAttemptV1::Retryable)
            }
            _ => Ok(MaterializedSubmissionAttemptV1::ScriptedPublished),
        })
        .map_err(|()| self.terminal_error("scripted ordinary submission failure"))?;
        let root = MaterializedBindingV1::indexed(self.active.as_mut().unwrap());
        if fault.is_none() && retry {
            if root.scripted_initial_retry {
                root.scripted_initial_retry = false;
            } else {
                root.scripted.as_mut().unwrap().1 -= 1;
            }
        }
        match fault {
            Some(Fault::OuterErrorAfterRetry | Fault::OuterErrorAfterPublish) => {
                return Err(self.terminal_error("scripted ordinary outer lane failure"));
            }
            Some(Fault::OuterUnwindAfterRetry | Fault::OuterUnwindAfterPublish) => {
                panic!("scripted ordinary outer lane unwind");
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScriptedMaterializedPublicationFaultV1 {
    BindingRejected,
    BindingUnwind,
    InitialObserverUnwind,
    SubmitRejected,
    SubmitTerminal,
    SubmitUnwind,
    OuterErrorAfterRetry,
    OuterUnwindAfterRetry,
    OuterErrorAfterPublish,
    OuterUnwindAfterPublish,
    ProfileUnwind,
}
