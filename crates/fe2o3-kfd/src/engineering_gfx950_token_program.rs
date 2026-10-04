//! Default-off retained CPU descriptions; native publication remains ordered64.

use super::*;

pub(super) struct RegisteredProgram {
    id: u64,
    epoch: u64,
    template: ProgramTemplate,
}

fn require_policy(enabled: bool, policy: Ordered64WaitPolicy) -> Result<()> {
    if !enabled || policy != Ordered64WaitPolicy::Sleep50usV1 {
        return Err("token program requires its separate diagnostic process entry".into());
    }
    Ok(())
}

fn require_identity(id: u64, epoch: u64, expected_id: u64, expected_epoch: u64) -> Result<()> {
    if id == 0 || id != expected_id || epoch != expected_epoch {
        return Err("token program identity or queue epoch".into());
    }
    Ok(())
}

fn require_frontier(
    write: u64,
    completed: u64,
    expected: u64,
    read: u64,
    count: usize,
) -> Result<()> {
    if completed != expected
        || write != completed
        || read > write
        || !(1..=MAX_TOKEN_PROGRAM_DISPATCHES_V1).contains(&count)
        || write
            .checked_add(count as u64)
            .and_then(|next| next.checked_sub(read))
            .is_none_or(|outstanding| outstanding > MAX_UNRETIRED_RING_PACKETS_V1)
    {
        return Err("token program frontier or retained ring budget; rollover required".into());
    }
    Ok(())
}

fn require_scalar_field(metadata: &KernelMetadataV1, offset: u32) -> Result<()> {
    if metadata
        .explicit_arguments
        .iter()
        .filter(|field| !field.global_buffer && field.offset == offset && field.bytes == 4)
        .count()
        != 1
    {
        return Err("token program scalar must cover one explicit four-byte by-value field".into());
    }
    Ok(())
}

fn prepare_program_commands<T>(
    commands: Vec<OrderedBatchDispatchV1>,
    payload: Vec<u8>,
    deadline: Option<Instant>,
    mut prepare: impl FnMut(OrderedBatchDispatchV1, Vec<u8>) -> Result<T>,
) -> Result<Vec<T>> {
    let mut prepared = Vec::with_capacity(commands.len());
    let mut offset = 0usize;
    for command in commands {
        if let Some(deadline) = deadline {
            ordered_batch::require_deadline(Instant::now(), deadline)?;
        }
        let end = offset
            .checked_add(command.payload_bytes as usize)
            .ok_or("token program payload overflow")?;
        let bytes = payload
            .get(offset..end)
            .ok_or("token program payload extent")?
            .to_vec();
        prepared.push(prepare(command, bytes)?);
        offset = end;
    }
    if offset != payload.len() {
        return Err("token program exact payload extent".into());
    }
    Ok(prepared)
}

pub(super) fn run_prepared_groups<T>(
    prepared: Vec<T>,
    deadline: Instant,
    mut run: impl FnMut(Vec<T>, Instant) -> Result<()>,
) -> Result<usize> {
    let count = prepared.len();
    if !(1..=MAX_TOKEN_PROGRAM_DISPATCHES_V1).contains(&count) {
        return Err("token program prepared count".into());
    }
    let mut remaining = prepared.into_iter();
    let mut retired = 0usize;
    while retired < count {
        ordered_batch::require_deadline(Instant::now(), deadline)?;
        let group: Vec<_> = remaining
            .by_ref()
            .take(MAX_ORDERED_BATCH64_DISPATCHES_V1)
            .collect();
        let group_count = group.len();
        run(group, deadline)?;
        ordered_batch::require_deadline(Instant::now(), deadline)?;
        retired += group_count;
    }
    Ok(retired)
}

impl Context {
    fn require_token_program_enabled(&self) -> Result<()> {
        require_policy(self.token_program_enabled, self.ordered64_wait_policy)
    }

    pub(super) fn require_program_resource_mutation(&self) -> Result<()> {
        if self.token_program.is_some() {
            return Err(
                "release registered token program before resource or queue mutation".into(),
            );
        }
        Ok(())
    }

    fn prepare_token_dispatches(
        &mut self,
        commands: Vec<OrderedBatchDispatchV1>,
        payload: Vec<u8>,
        deadline: Option<Instant>,
    ) -> Result<Vec<PreparedDispatch>> {
        let mut scope = self.preparation_scope();
        prepare_program_commands(commands, payload, deadline, |command, bytes| {
            let started = scope.profile_started();
            let prepared = scope.prepare(
                command.kernel,
                bytes,
                command.workgroup,
                command.grid,
                &command.pointers,
                None,
            )?;
            record_elapsed(&mut scope.counters.dispatch_prepare_ns, started)?;
            Ok(prepared)
        })
    }

    pub(super) fn register_token_program(
        &mut self,
        definition_bytes: u32,
        kernarg_bytes: u32,
        payload: Vec<u8>,
    ) -> Result<ResponseV1> {
        self.require_token_program_enabled()?;
        self.check_currentness(true)?;
        self.check_idle()?;
        self.require_program_resource_mutation()?;
        let template =
            ProgramTemplate::decode(definition_bytes, kernarg_bytes, &payload).map_err(explain)?;
        for slot in &template.definition.slots {
            match slot {
                TokenProgramSlotV1::ScalarU32 {
                    dispatch, offset, ..
                } => {
                    let kernel = template.definition.dispatches[usize::from(*dispatch)].kernel;
                    require_scalar_field(
                        &self
                            .kernels
                            .get(&kernel)
                            .ok_or("token program unknown kernel")?
                            .metadata,
                        *offset,
                    )?;
                }
                TokenProgramSlotV1::Pointer { buffers, .. } => {
                    if buffers
                        .iter()
                        .any(|buffer| !self.buffers.contains_key(buffer))
                    {
                        return Err("token program allowed buffer is not owned".into());
                    }
                }
            }
        }
        // Validate the original unpatched template, then discard all prepared
        // addresses. Registration itself cannot stage or publish anything.
        let (commands, bytes) = template.initial();
        drop(self.prepare_token_dispatches(commands, bytes, None)?);
        self.check_currentness(true)?;
        self.check_idle()?;
        let id = self.next_token_program;
        self.next_token_program = id.checked_add(1).ok_or("token program handle exhausted")?;
        let response = ResponseV1::TokenProgramRegistered {
            program: id,
            device_unique_id: self.unique_id,
            queue_epoch: self.queue_epoch,
            dispatches: u32::try_from(template.definition.dispatches.len()).map_err(explain)?,
            slots: u32::try_from(template.definition.slots.len()).map_err(explain)?,
        };
        self.token_program = Some(RegisteredProgram {
            id,
            epoch: self.queue_epoch,
            template,
        });
        Ok(response)
    }

    pub(super) fn release_token_program(&mut self, program: u64, epoch: u64) -> Result<ResponseV1> {
        self.require_token_program_enabled()?;
        self.check_currentness(true)?;
        self.check_idle()?;
        let registered = self
            .token_program
            .as_ref()
            .ok_or("token program not registered")?;
        require_identity(registered.id, registered.epoch, program, epoch)?;
        require_identity(registered.id, self.queue_epoch, program, epoch)?;
        self.token_program = None;
        Ok(ResponseV1::TokenProgramReleased {
            program,
            queue_epoch: epoch,
        })
    }

    pub(super) fn execute_token_program(
        &mut self,
        program: u64,
        epoch: u64,
        completed: u64,
        timeout_ms: u32,
        updates: Vec<TokenProgramUpdateV1>,
    ) -> Result<ResponseV1> {
        let result = (|| {
            self.require_token_program_enabled()?;
            if !(1..=600_000).contains(&timeout_ms) {
                return Err("token program timeout".into());
            }
            let started = Instant::now();
            let deadline = started
                .checked_add(Duration::from_millis(u64::from(timeout_ms)))
                .ok_or("token program deadline overflow")?;
            self.check_currentness(false)?;
            self.check_idle()?;
            let registered = self
                .token_program
                .as_ref()
                .ok_or("token program not registered")?;
            require_identity(registered.id, registered.epoch, program, epoch)?;
            require_identity(registered.id, self.queue_epoch, program, epoch)?;
            require_frontier(
                self.ring.write(),
                self.completed_write,
                completed,
                self.last_observed_read,
                registered.template.definition.dispatches.len(),
            )?;
            let (commands, payload) = registered.template.materialize(&updates).map_err(explain)?;
            // All dynamic argument, geometry, buffer ownership/range and alias
            // validation completes before the first group can publish.
            let prepared = self.prepare_token_dispatches(commands, payload, Some(deadline))?;
            self.check_idle()?;
            let retired = run_prepared_groups(prepared, deadline, |group, deadline| {
                self.run_prepared_token_group(group, deadline)
            })?;
            self.check_currentness(false)?;
            self.check_idle()?;
            ordered_batch::require_deadline(Instant::now(), deadline)?;
            Ok(ResponseV1::TokenProgramCompleted {
                program,
                device_unique_id: self.unique_id,
                queue_epoch: self.queue_epoch,
                completed_dispatches: u32::try_from(retired).map_err(explain)?,
                completed_packets: self.completed_write,
                elapsed_ns: u64::try_from(started.elapsed().as_nanos()).map_err(explain)?,
            })
        })();
        // A later-group failure never returns a partial-success acknowledgement.
        // Published effects may exist; the ordinary fatal worker route retains
        // the poisoned owner until disposable-process teardown.
        if result.is_err() {
            self.ordered_batch_poisoned = true;
        }
        result
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_token_program_tests.rs"]
mod tests;
