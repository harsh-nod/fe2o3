//! Original epoch and common hold surround every per-recipe progress transition.

use super::*;

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    /// Async-primary four-recipe registry over the existing caller-wake driver.
    /// `metadata` is an existing caller account, never a newly minted budget.
    /// It prepays the lower six tables before VM/DATA admission. The fixed inline
    /// scope, reply cells and allocator bookkeeping are not a total RSS claim.
    /// This first scope destroys its original queue and is not a rolling queue.
    ///
    /// ```compile_fail
    /// use fe2o3_runtime::*;
    /// async fn escape<P: RuntimeGfx942RegistryCompletionCarrierV1>(
    ///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>, d: RuntimeDeviceIdV1,
    ///     s: RuntimeStreamIdV1, p: RuntimeGfx942GeneratedRegistry4V1<P>,
    ///     a: fe2o3_resource_accounting::ResourceCreditAccountV1) {
    ///     let _ = c.with_generated_gfx942_registry4_scope_async_v1(a,
    ///         std::time::Instant::now(), |_|std::future::ready(()), async |scope| {
    ///             scope.try_submit_v1(d,s, |_|Ok::<_,()>(p)).unwrap()
    ///         }).await;
    /// }
    /// ```
    pub async fn with_generated_gfx942_registry4_scope_async_v1<'env, P, R, W, F>(
        &'env mut self,
        metadata: ResourceCreditAccountV1,
        deadline: Instant,
        wait: W,
        use_scope: impl for<'scope> AsyncFnOnce(
            &mut RuntimeGfx942Registry4ScopeV1<'scope, 'env, P>,
        ) -> R,
    ) -> Result<R, RuntimeGfx942ScopeErrorV1>
    where
        P: RuntimeGfx942RegistryCompletionCarrierV1 + 'env,
        W: FnMut(Instant) -> F,
        F: Future<Output = ()>,
    {
        let mut scope = self.new_registry4_scope_v1(metadata, deadline, false)?;
        let result = use_scope(&mut scope).await;
        scope.drive_with_wake_v1(wait).await?;
        Ok(result)
    }

    /// Same original one-shot registry with synchronous caller progress.
    pub fn with_generated_gfx942_registry4_scope_v1<'env, P, R>(
        &'env mut self,
        metadata: ResourceCreditAccountV1,
        deadline: Instant,
        use_scope: impl for<'scope> FnOnce(&mut RuntimeGfx942Registry4ScopeV1<'scope, 'env, P>) -> R,
    ) -> Result<R, RuntimeGfx942ScopeErrorV1>
    where
        P: RuntimeGfx942RegistryCompletionCarrierV1 + 'env,
    {
        let mut scope = self.new_registry4_scope_v1(metadata, deadline, false)?;
        let result = use_scope(&mut scope);
        while scope.pending_v1() != 0 {
            if scope.progress_v1()? == 0 {
                std::thread::yield_now();
            }
        }
        futures::Driver::settled(&scope)?;
        Ok(result)
    }

    fn new_registry4_scope_v1<'scope, 'env, P, const N: usize>(
        &'env mut self,
        metadata: ResourceCreditAccountV1,
        deadline: Instant,
        repeat2: bool,
    ) -> Result<RuntimeGfx942Registry4ScopeV1<'scope, 'env, P, N>, RuntimeGfx942ScopeErrorV1> {
        if Instant::now() >= deadline {
            return Err(RuntimeGfx942ScopeErrorV1::Deadline);
        }
        self.require_live()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        self.backend
            .preflight_generated_cohort3_lane_v1()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(map_backend_error(error)))?;
        let storage = if N == 4 && repeat2 {
            fe2o3_kfd::Gfx942NativeFillRegistryRepeat2StorageV1::preallocate(metadata)
                .map(RegistryStorageV1::Repeat2)
        } else if N == 4 {
            fe2o3_kfd::Gfx942NativeFillRegistryStorageV1::preallocate(metadata)
                .map(RegistryStorageV1::Once)
        } else if N == 16 && !repeat2 {
            fe2o3_kfd::Gfx942NativeFillResidentRegistryStorageV1::<16>::preallocate(metadata)
                .map(RegistryStorageV1::Sixteen)
        } else {
            return Err(RuntimeGfx942ScopeErrorV1::Capacity);
        }
        .map_err(|_| RuntimeGfx942ScopeErrorV1::Capacity)?;
        let identity = Rc::new(());
        let epoch = self
            .scope_epoch
            .begin()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        Ok(RuntimeGfx942Registry4ScopeV1 {
            epoch,
            context: self,
            root: None,
            storage: Some(storage),
            repeat2,
            identity,
            deadline,
            invariant: PhantomData,
        })
    }
}

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    /// Two bounded cycles of the same four original recipes and common backing.
    /// Fresh copied-result frames are prepaid before entry. The caller's wake
    /// driver is unchanged; this admits no rolling source replacement or graph.
    ///
    /// ```compile_fail
    /// use fe2o3_runtime::*;
    /// async fn escape<P: RuntimeGfx942RegistryRepeat2CarrierV1>(
    ///     c: &mut RuntimeContextV1<KfdRuntimeBackendV1>, d: RuntimeDeviceIdV1,
    ///     s: RuntimeStreamIdV1, p: RuntimeGfx942GeneratedRegistry4Repeat2V1<P>,
    ///     a: fe2o3_resource_accounting::ResourceCreditAccountV1) {
    ///     let _ = c.with_generated_gfx942_registry4_repeat2_scope_async_v1(a,
    ///         std::time::Instant::now(), |_|std::future::ready(()), async |scope| {
    ///             scope.try_submit_repeat2_v1(d,s, |_|Ok::<_,()>(p)).unwrap()
    ///         }).await;
    /// }
    /// ```
    pub async fn with_generated_gfx942_registry4_repeat2_scope_async_v1<'env, P, R, W, F>(
        &'env mut self,
        metadata: ResourceCreditAccountV1,
        deadline: Instant,
        wait: W,
        use_scope: impl for<'scope> AsyncFnOnce(
            &mut RuntimeGfx942Registry4ScopeV1<'scope, 'env, P>,
        ) -> R,
    ) -> Result<R, RuntimeGfx942ScopeErrorV1>
    where
        P: crate::RuntimeGfx942RegistryRepeat2CarrierV1 + 'env,
        W: FnMut(Instant) -> F,
        F: Future<Output = ()>,
    {
        let mut scope = self.new_registry4_scope_v1(metadata, deadline, true)?;
        let result = use_scope(&mut scope).await;
        scope.drive_with_wake_v1(wait).await?;
        Ok(result)
    }
}

impl<P: RuntimeGfx942RegistryCompletionCarrierV1, const N: usize>
    RuntimeGfx942Registry4ScopeV1<'_, '_, P, N>
{
    /// Returns one while common native custody is outstanding, even after all
    /// result observers resolve. This is not the count of published packets.
    pub fn pending_v1(&self) -> usize {
        usize::from(
            self.root
                .as_ref()
                .is_some_and(|root| root.state != State::Closed),
        )
    }

    /// Same finite cooperative driver used by ordinary and cohort scopes.
    pub async fn drive_with_wake_v1<W, F>(
        &mut self,
        wait: W,
    ) -> Result<(), RuntimeGfx942ScopeErrorV1>
    where
        W: FnMut(Instant) -> F,
        F: Future<Output = ()>,
    {
        futures::drive(self, wait).await
    }

    /// At most one original native receipt step per unfinished recipe per scan.
    pub fn progress_v1(&mut self) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        let result = self.progress_once();
        if result.is_err()
            && let Some(root) = &mut self.root
        {
            for (outcome, reply) in root.outcomes.iter().zip(&mut root.replies) {
                if outcome.is_none() {
                    reply.complete(Err(crate::RuntimeAsyncEngineCallErrorV1::EngineStopped));
                }
            }
            if let Some(second) = &mut root.second {
                for (outcome, reply) in second.outcomes.iter().zip(&mut second.replies) {
                    if outcome.is_none() {
                        reply.complete(Err(crate::RuntimeAsyncEngineCallErrorV1::EngineStopped));
                    }
                }
            }
        }
        result
    }

    fn progress_once(&mut self) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        let _permit = self
            .epoch
            .enter()
            .map_err(|error| RuntimeGfx942ScopeErrorV1::Context(error.into()))?;
        if self.context.is_terminal() {
            return Err(RuntimeGfx942ScopeErrorV1::Unknown);
        }
        let Some(root) = &mut self.root else {
            return Ok(0);
        };
        if root.state == State::Closed {
            return Ok(0);
        }
        if root.state == State::Unknown {
            return Err(RuntimeGfx942ScopeErrorV1::Unknown);
        }
        if Instant::now() >= self.deadline {
            return Err(RuntimeGfx942ScopeErrorV1::Deadline);
        }
        let prepared = root
            .prepared
            .as_mut()
            .unwrap_or_else(|| std::process::abort());
        if root.state == State::Adopting {
            root.state = State::Unknown;
            self.context
                .adopt_gfx942_registry4_v1(
                    prepared,
                    &root.roster,
                    &root.hold,
                    root.storage.take().unwrap_or_else(|| std::process::abort()),
                )
                .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
            root.state = State::Active;
            return Ok(1);
        }
        if root.state == State::Closing {
            root.state = State::Unknown;
            self.context
                .close_gfx942_registry4_v1(prepared, &root.roster, &root.hold)
                .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
            // Native common destruction and exact Context debit/hold release
            // precede all original residual source/authority/account owner Drops.
            drop(root.prepared.take());
            root.state = State::Closed;
            return Ok(1);
        }
        if root.state == State::Rearming {
            root.state = State::Unknown;
            self.context
                .rearm_gfx942_registry4_v1(prepared, &root.roster, &root.hold)
                .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
            root.cycle = 1;
            root.state = State::Active;
            return Ok(1);
        }
        let (outcomes, replies) = if root.cycle == 0 {
            (&mut root.outcomes, &mut root.replies)
        } else {
            let second = root
                .second
                .as_mut()
                .unwrap_or_else(|| std::process::abort());
            (&mut second.outcomes, &mut second.replies)
        };
        let mut transitions = 0;
        for index in 0..N {
            if outcomes[index].is_some() {
                continue;
            }
            root.state = State::Unknown;
            let ready = self
                .context
                .progress_gfx942_registry4_v1(prepared, &root.roster, &root.hold, index, root.cycle)
                .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
            if ready {
                self.context
                    .copy_gfx942_registry4_result_v1(
                        prepared,
                        &root.roster,
                        &root.hold,
                        index,
                        root.cycle,
                    )
                    .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
                let outcome = prepared.value_mut_v1().members[index]
                    .decode_registry_cycle_retaining_source_v1(root.cycle);
                replies[index].complete(Ok(outcome.clone()));
                outcomes[index] = Some(outcome);
                transitions += 1;
            }
            root.state = State::Active;
        }
        if let Some(next) = decoded_cycle_state(root.cycle, self.repeat2, outcomes) {
            root.state = next;
            if root.state == State::Closing
                && root.cycle == 0
                && let Some(second) = &mut root.second
            {
                for reply in &mut second.replies {
                    reply.complete(Err(crate::RuntimeAsyncEngineCallErrorV1::EngineStopped));
                }
            }
            transitions += 1;
        }
        Ok(transitions)
    }
}

pub(super) fn decoded_cycle_state<const N: usize>(
    cycle: u8,
    repeat2: bool,
    outcomes: &[Option<Outcome>; N],
) -> Option<State> {
    if outcomes.iter().any(Option::is_none) {
        return None;
    }
    Some(
        if cycle == 0
            && repeat2
            && outcomes
                .iter()
                .all(|outcome| matches!(outcome, Some(Ok(()))))
        {
            State::Rearming
        } else {
            State::Closing
        },
    )
}

impl<P: RuntimeGfx942RegistryCompletionCarrierV1, const N: usize> futures::Driver
    for RuntimeGfx942Registry4ScopeV1<'_, '_, P, N>
{
    fn pending(&self) -> usize {
        self.pending_v1()
    }
    fn progress(&mut self) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        self.progress_v1()
    }
    fn deadline(&self) -> Instant {
        self.deadline
    }
    fn settled(&self) -> Result<(), RuntimeGfx942ScopeErrorV1> {
        if let Some(root) = &self.root {
            if root.state != State::Closed {
                return Err(RuntimeGfx942ScopeErrorV1::Unknown);
            }
            for outcome in &root.outcomes {
                if let Some(Err(error)) = outcome {
                    return Err(RuntimeGfx942ScopeErrorV1::Readback(error.clone()));
                }
            }
            if let Some(second) = &root.second {
                for outcome in &second.outcomes {
                    match outcome {
                        Some(Ok(())) => {}
                        Some(Err(error)) => {
                            return Err(RuntimeGfx942ScopeErrorV1::Readback(error.clone()));
                        }
                        None => return Err(RuntimeGfx942ScopeErrorV1::Unknown),
                    }
                }
            }
        }
        Ok(())
    }
}

impl RuntimeContextV1<KfdRuntimeBackendV1> {
    /// One bounded N16 registry using the same finite caller-wake progress driver.
    /// Original metadata prepays eighteen lower tables before native admission;
    /// sixteen copied results do not release any common DATA custody. This is
    /// neither a rolling registry nor an independent native completion guarantee.
    ///
    /// ```compile_fail
    /// use fe2o3_runtime::*;
    /// fn substitute<'s, P>(scope: &RuntimeGfx942Registry16ScopeV1<'s, '_, P>,
    ///     ticket: &RuntimeGfx942Registry4TicketV1<'s>) where P: RuntimeGfx942RegistryCompletionCarrierV1 {
    ///     let _ = scope.result_matches_owner_v1(ticket, &std::sync::Arc::new(()));
    /// }
    /// ```
    pub async fn with_generated_gfx942_registry16_scope_async_v1<'env, P, R, W, F>(
        &'env mut self,
        metadata: ResourceCreditAccountV1,
        deadline: Instant,
        wait: W,
        use_scope: impl for<'scope> AsyncFnOnce(
            &mut RuntimeGfx942Registry16ScopeV1<'scope, 'env, P>,
        ) -> R,
    ) -> Result<R, RuntimeGfx942ScopeErrorV1>
    where
        P: RuntimeGfx942RegistryCompletionCarrierV1 + 'env,
        W: FnMut(Instant) -> F,
        F: Future<Output = ()>,
    {
        let mut scope = self.new_registry4_scope_v1::<P, 16>(metadata, deadline, false)?;
        let result = use_scope(&mut scope).await;
        scope.drive_with_wake_v1(wait).await?;
        Ok(result)
    }
}
