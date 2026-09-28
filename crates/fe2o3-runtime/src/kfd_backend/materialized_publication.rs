//! Indexed logical custody through ordinary binding and the first submission.

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

pub(super) enum MaterializedFirstSubmissionV1 {
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
    pub(super) submission: MaterializedFirstSubmissionV1,
    #[cfg(test)]
    pub(super) scripted: Option<(Vec<DataSpecV1>, usize)>,
}

impl MaterializedBindingV1 {
    pub(super) fn new(profile: PersistentPublicationProfileV1) -> Self {
        Self {
            profile,
            origin: MaterializedPreparationOriginV1::NewBinding,
            submission: MaterializedFirstSubmissionV1::Unattempted,
            #[cfg(test)]
            scripted: None,
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
        operation: impl FnOnce() -> Result<MaterializedFirstSubmissionV1, E>,
    ) -> Result<(), E> {
        self.submission = MaterializedFirstSubmissionV1::NativeOwned;
        // This runs inside the native lane callback, before its outer loan closes.
        self.submission = operation()?;
        Ok(())
    }
}

impl KfdRuntimeBackendV1 {
    pub(super) fn finish_materialized_binding_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let active = self.active.as_mut().expect("indexed ordinary binding");
        if matches!(
            MaterializedBindingV1::indexed(active).submission,
            MaterializedFirstSubmissionV1::Unattempted | MaterializedFirstSubmissionV1::NativeOwned
        ) {
            return Err(self.terminal_error("ordinary binding has no confirmed submission outcome"));
        }
        let Some(ActiveComputeExecutionV1::MaterializedBinding(root)) = active.execution.take()
        else {
            unreachable!("checked indexed ordinary binding")
        };
        match root.submission {
            MaterializedFirstSubmissionV1::Retryable => {
                let prepared = MaterializedPreparedV1::new(root.profile, root.origin);
                #[cfg(test)]
                let prepared = MaterializedPreparedV1 {
                    scripted: root.scripted,
                    ..prepared
                };
                active.execution = Some(ActiveComputeExecutionV1::MaterializedPrepared(prepared));
                return Ok(());
            }
            MaterializedFirstSubmissionV1::Published(batch) => {
                active.execution = Some(ActiveComputeExecutionV1::Materialized(batch));
            }
            #[cfg(test)]
            MaterializedFirstSubmissionV1::ScriptedPublished => {
                active.execution = Some(ActiveComputeExecutionV1::ScriptedMaterialized);
            }
            MaterializedFirstSubmissionV1::Unattempted
            | MaterializedFirstSubmissionV1::NativeOwned => {
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
        let fault = self.scripted_materialized_publication_fault;
        if fault != Some(Fault::ProfileUnwind) {
            self.scripted_materialized_publication_fault = None;
        }
        let active = self.active.as_mut().unwrap();
        active.performance.data_path = KfdRuntimeLaunchDataPathV1::Materialized;
        active.performance.user_data_materializations = data.len() as u64;
        let root = MaterializedBindingV1::indexed(active);
        let (origin, retries) =
            configuration.unwrap_or((MaterializedPreparationOriginV1::NewBinding, 0));
        root.origin = origin;
        root.scripted = Some((data, retries));
        match fault {
            Some(Fault::BindingRejected) => {
                return Err(Self::capacity("scripted ordinary binding rejection"));
            }
            Some(Fault::BindingUnwind) => panic!("scripted ordinary binding unwind"),
            Some(Fault::InitialObserverUnwind) => panic!("scripted ordinary queue observer unwind"),
            _ => {}
        }
        root.submit(|| match fault {
            Some(Fault::SubmitUnwind) => panic!("scripted ordinary submit unwind"),
            Some(Fault::SubmitRejected | Fault::SubmitTerminal) => Err(()),
            Some(Fault::OuterErrorAfterRetry | Fault::OuterUnwindAfterRetry) => {
                Ok(MaterializedFirstSubmissionV1::Retryable)
            }
            _ if configuration.is_some()
                && fault != Some(Fault::ProfileUnwind)
                && !matches!(
                    fault,
                    Some(Fault::OuterErrorAfterPublish | Fault::OuterUnwindAfterPublish)
                ) =>
            {
                Ok(MaterializedFirstSubmissionV1::Retryable)
            }
            _ => Ok(MaterializedFirstSubmissionV1::ScriptedPublished),
        })
        .map_err(|()| self.terminal_error("scripted ordinary first submission failure"))?;
        match fault {
            Some(Fault::OuterErrorAfterRetry | Fault::OuterErrorAfterPublish) => {
                return Err(self.terminal_error("scripted ordinary outer lane failure"));
            }
            Some(Fault::OuterUnwindAfterRetry | Fault::OuterUnwindAfterPublish) => {
                panic!("scripted ordinary outer lane unwind");
            }
            _ => {}
        }
        self.finish_materialized_binding_v1()
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
