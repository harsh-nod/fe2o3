//! Keep the logical dispatch indexed across consuming persistent publication.

use super::*;

// Only the lower layer can return an armed receipt. After an indeterminate
// result or unwind, its native attachment remains responsible for teardown.
fn attempt_prepared_publication_v1<T, P, E>(
    state: &mut PreparedReceiptV1<T>,
    operation: impl FnOnce(T) -> Result<P, (E, Option<T>)>,
) -> Result<Option<P>, E> {
    let receipt = core::mem::replace(state, PreparedReceiptV1::NativeOwned)
        .into_armed()
        .expect("preflighted armed publication receipt");
    match operation(receipt) {
        Ok(published) => Ok(Some(published)),
        Err((error, Some(retryable))) => {
            *state = PreparedReceiptV1::Armed(retryable);
            drop(error);
            Ok(None)
        }
        Err((error, None)) => Err(error),
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ScriptedPreparedPublicationFaultV1 {
    Retryable,
    Terminal,
    Unwind,
    ProfileUnwind,
    InitialObserverUnwind,
    InitialRejected,
    InitialQuiescent,
}

#[cfg(test)]
fn scripted_publication_v1<T>(
    input: T,
    fault: &mut Option<ScriptedPreparedPublicationFaultV1>,
    park: impl FnOnce(T),
) -> Result<T, ((), Option<T>)> {
    use ScriptedPreparedPublicationFaultV1 as Fault;
    match *fault {
        Some(Fault::Retryable) => {
            *fault = None;
            Err(((), Some(input)))
        }
        Some(Fault::Terminal | Fault::Unwind) => {
            let fault = fault.take();
            park(input);
            if fault == Some(Fault::Unwind) {
                std::panic::panic_any("scripted consuming publication unwind");
            }
            Err(((), None))
        }
        _ => Ok(input),
    }
}

#[cfg(test)]
fn scripted_device_v1(input: KfdRuntimePersistentComputeInputV1) -> DirectionalSdmaDeviceOwnerV1 {
    match input {
        KfdRuntimePersistentComputeInputV1::ScriptedReady(ready) => ready.owner.normalize(),
        KfdRuntimePersistentComputeInputV1::ScriptedReplay(device)
        | KfdRuntimePersistentComputeInputV1::ScriptedStorage(device) => device,
        KfdRuntimePersistentComputeInputV1::Native(_) => unreachable!("preflighted scripted input"),
    }
}

impl KfdRuntimeBackendV1 {
    pub(super) fn publish_initial_persistent_prepared_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        #[cfg(test)]
        if self.scripted_sdma.is_some() {
            use ScriptedPreparedPublicationFaultV1 as Fault;
            match self.scripted_prepared_publication_fault {
                Some(Fault::InitialObserverUnwind) => {
                    self.scripted_prepared_publication_fault = None;
                    std::panic::panic_any("scripted initial queue observer unwind");
                }
                Some(Fault::InitialRejected | Fault::InitialQuiescent) => {
                    let fault = self.scripted_prepared_publication_fault.take();
                    return Err(if fault == Some(Fault::InitialRejected) {
                        Self::capacity("scripted initial publication rejection")
                    } else {
                        Self::quiescent_error(
                            KfdRuntimeBackendErrorKindV1::Native,
                            "scripted initial publication quiescent failure",
                        )
                    });
                }
                _ => {}
            }
            if self.scripted_persistent_publication_retries != 0
                && self.scripted_prepared_publication_fault.is_none()
            {
                self.scripted_persistent_publication_retries -= 1;
                self.scripted_prepared_publication_fault =
                    Some(ScriptedPreparedPublicationFaultV1::Retryable);
            }
            return self.poll_persistent_prepared_v1().map(|_| ());
        }
        self.retain_primary_compute_lane_v1();
        self.poll_persistent_prepared_v1().map(|_| ())
    }

    pub(super) fn persistent_prepared_selected_v1(&self) -> bool {
        match self
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref())
        {
            Some(
                ActiveComputeExecutionV1::PersistentPrepared { .. }
                | ActiveComputeExecutionV1::ThreeBindingPersistentPrepared { .. },
            ) => true,
            #[cfg(test)]
            Some(
                ActiveComputeExecutionV1::ScriptedPersistentPrepared { .. }
                | ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared { .. },
            ) => true,
            _ => false,
        }
    }

    pub(super) fn persistent_prepared_is_armed_v1(&self) -> bool {
        match self
            .active
            .as_ref()
            .and_then(|active| active.execution.as_ref())
        {
            Some(ActiveComputeExecutionV1::PersistentPrepared { prepared, .. }) => {
                prepared.armed().is_some()
            }
            Some(ActiveComputeExecutionV1::ThreeBindingPersistentPrepared { prepared, .. }) => {
                prepared.armed().is_some()
            }
            #[cfg(test)]
            Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared { input, .. }) => {
                input.armed().is_some()
            }
            #[cfg(test)]
            Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                inputs,
                ..
            }) => inputs.armed().is_some(),
            _ => false,
        }
    }

    pub(super) fn poll_persistent_prepared_v1(
        &mut self,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        let submission = self.active.as_ref().map_or(0, |active| active.id);
        if self.selected_compute_lane != 0
            || !self.persistent_prepared_is_armed_v1()
            || !self.prepared_persistent_custody_intact_v1(submission, false)
            || !self.prepared_persistent_storage_intact_v1()
            || self.terminal_sdma_custody.is_some()
        {
            return Err(
                self.terminal_error("prepared publication custody changed before native effects")
            );
        }
        let native = matches!(
            self.active.as_ref().unwrap().execution,
            Some(
                ActiveComputeExecutionV1::PersistentPrepared { .. }
                    | ActiveComputeExecutionV1::ThreeBindingPersistentPrepared { .. }
            )
        );
        if native && self.queue.is_none() {
            return Err(self.terminal_error("prepared publication lost its native queue"));
        }
        #[cfg(test)]
        if !native && self.scripted_sdma.is_none() {
            return Err(self.terminal_error("prepared publication lost its scripted driver"));
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let started = Instant::now();
            let profile = self.publish_indexed_prepared_v1()?;
            // Both retryable and successful owners are indexed before timing or
            // profiling, either of which may unwind independently of native work.
            let active = self.active.as_mut().unwrap();
            active.performance.publication += started.elapsed();
            if let Some(profile) = profile {
                active.published_at = Instant::now();
                let (id, stream, kernel, shape) = (
                    active.id,
                    active.stream,
                    active.kernel,
                    active.dispatch_shape_sha256,
                );
                self.observe_persistent_dispatch_published_v1(id, stream, kernel, shape, profile);
            }
            Ok(BackendPollV1::Pending)
        }));
        match result {
            Ok(result) => result,
            Err(payload) => super::sdma_host_write::resume_sdma_owner_panic_v1(payload, || {
                self.poison_terminal_v1()
            }),
        }
    }

    // Keep the error and linear receipt inline on the recovery path.
    #[allow(clippy::result_large_err)]
    fn publish_indexed_prepared_v1(
        &mut self,
    ) -> Result<
        Option<PersistentPublicationProfileV1>,
        RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
    > {
        let active = self.active.as_mut().unwrap();
        let profile = match active.execution.as_mut().unwrap() {
            ActiveComputeExecutionV1::PersistentPrepared { prepared, .. } => {
                let result = attempt_prepared_publication_v1(prepared, |prepared| {
                    self.queue
                        .as_mut()
                        .unwrap()
                        .submit_directional_persistent_fixed_dispatch_v1(prepared)
                        .map_err(|failure| failure.into_parts())
                });
                let dispatch = match result {
                    Ok(Some(dispatch)) => dispatch,
                    Ok(None) => return Ok(None),
                    Err(error) => {
                        return Err(self.terminal_error(format!(
                            "KFD persistent-compute publication became indeterminate: {error}"
                        )));
                    }
                };
                // The variant was authenticated above. Only infallible field
                // moves occur before the returned dispatch is indexed again.
                let Some(ActiveComputeExecutionV1::PersistentPrepared {
                    allocation,
                    access,
                    profile,
                    ..
                }) = active.execution.take()
                else {
                    unreachable!("preflighted single publication");
                };
                active.execution = Some(ActiveComputeExecutionV1::Persistent {
                    allocation,
                    access,
                    dispatch,
                });
                profile
            }
            ActiveComputeExecutionV1::ThreeBindingPersistentPrepared { prepared, .. } => {
                let result = attempt_prepared_publication_v1(prepared, |prepared| {
                    self.queue
                        .as_mut()
                        .unwrap()
                        .submit_three_binding_directional_persistent_fixed_dispatch_v1(prepared)
                        .map_err(|failure| failure.into_parts())
                });
                let dispatch = match result {
                    Ok(Some(dispatch)) => dispatch,
                    Ok(None) => return Ok(None),
                    Err(_) => {
                        return Err(self.terminal_error(
                            "KFD three-binding persistent publication became indeterminate",
                        ));
                    }
                };
                let Some(ActiveComputeExecutionV1::ThreeBindingPersistentPrepared {
                    admissions,
                    restore_shells,
                    profile,
                    ..
                }) = active.execution.take()
                else {
                    unreachable!("preflighted three-binding publication");
                };
                active.execution = Some(ActiveComputeExecutionV1::ThreeBindingPersistent {
                    admissions,
                    restore_shells,
                    dispatch,
                });
                profile
            }
            #[cfg(test)]
            ActiveComputeExecutionV1::ScriptedPersistentPrepared { input, .. } => {
                let shell = try_uninit_box_v1().map_err(|_| {
                    Self::capacity("scripted publication device shell allocation failed")
                })?;
                let result = attempt_prepared_publication_v1(input, |input| {
                    scripted_publication_v1(
                        input,
                        &mut self.scripted_prepared_publication_fault,
                        |input| {
                            self.terminal_sdma_custody = Some(
                                KfdRuntimeTerminalSdmaCustodyV1::PersistentRuntimeInput(*input),
                            )
                        },
                    )
                });
                let input = match result {
                    Ok(Some(input)) => input,
                    Ok(None) => return Ok(None),
                    Err(()) => {
                        return Err(self.terminal_error("scripted consuming publication failure"));
                    }
                };
                let device = fill_restore_shell_v1(shell, scripted_device_v1(*input));
                let Some(ActiveComputeExecutionV1::ScriptedPersistentPrepared {
                    allocation,
                    access,
                    profile,
                    ..
                }) = active.execution.take()
                else {
                    unreachable!("preflighted scripted publication");
                };
                active.execution = Some(ActiveComputeExecutionV1::ScriptedPersistent {
                    allocation,
                    access,
                    device,
                });
                profile
            }
            #[cfg(test)]
            ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared { inputs, .. } => {
                let result = attempt_prepared_publication_v1(inputs, |inputs| {
                    scripted_publication_v1(
                        inputs,
                        &mut self.scripted_prepared_publication_fault,
                        |inputs| {
                            self.terminal_sdma_custody = Some(
                                KfdRuntimeTerminalSdmaCustodyV1::ThreeBindingPersistentInputs(
                                    inputs,
                                ),
                            )
                        },
                    )
                });
                let inputs = match result {
                    Ok(Some(inputs)) => inputs,
                    Ok(None) => return Ok(None),
                    Err(()) => {
                        return Err(self.terminal_error("scripted consuming publication failure"));
                    }
                };
                let devices = inputs.map(scripted_device_v1);
                let Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistentPrepared {
                    admissions,
                    restore_shells,
                    profile,
                    ..
                }) = active.execution.take()
                else {
                    unreachable!("preflighted scripted three-binding publication");
                };
                active.execution = Some(ActiveComputeExecutionV1::ScriptedThreeBindingPersistent {
                    admissions,
                    restore_shells,
                    devices,
                });
                profile
            }
            _ => unreachable!("preflighted prepared publication"),
        };
        Ok(Some(profile))
    }
}
