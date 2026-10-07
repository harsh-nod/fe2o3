//! One finite receipt scan, then original decoders, then real common destruction.

use super::*;

impl<P: RuntimeGfx942RegistryCompletionCarrierV1> RuntimeGfx942Arena1024ScopeV1<'_, '_, P> {
    /// One common obligation remains pending even after every copied result.
    pub fn pending_v1(&self) -> usize {
        usize::from(
            self.root
                .as_ref()
                .is_some_and(|root| root.state != State::Closed),
        )
    }

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

    pub fn progress_v1(&mut self) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        let result = self.progress_once();
        if result.is_err()
            && let Some(root) = &mut self.root
        {
            for cell in root.cells.iter_mut().flatten() {
                if cell.outcome.is_none() {
                    cell.reply
                        .complete(Err(crate::RuntimeAsyncEngineCallErrorV1::EngineStopped));
                }
            }
        }
        result
    }

    fn progress_once(&mut self) -> Result<usize, RuntimeGfx942ScopeErrorV1> {
        let _permit = self
            .epoch
            .enter()
            .map_err(|e| RuntimeGfx942ScopeErrorV1::Context(e.into()))?;
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
                .adopt_gfx942_arena_v1(
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
                .close_gfx942_arena_v1(prepared, &root.roster, &root.hold)
                .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
            // Actual native destroy, common journal debit and exact hold release
            // precede every residual carrier/source/host-loan destructor.
            drop(root.prepared.take());
            root.state = State::Closed;
            return Ok(1);
        }
        root.state = State::Unknown;
        let mut transitions = self
            .context
            .progress_gfx942_arena_round_v1(prepared, &root.roster, &root.hold, &mut root.copied)
            .map_err(RuntimeGfx942ScopeErrorV1::Context)?;
        transitions += results::decode_copied(&mut root.cells, &root.copied, |index| {
            let member = prepared.value_mut_v1().members[index]
                .as_mut()
                .unwrap_or_else(|| std::process::abort());
            member.decode_registry_cycle_retaining_source_v1(0)
        });
        root.state = if root
            .cells
            .iter()
            .all(|cell| cell.as_ref().is_some_and(|cell| cell.outcome.is_some()))
        {
            transitions += 1;
            State::Closing
        } else {
            State::Active
        };
        Ok(transitions)
    }
}

impl<P: RuntimeGfx942RegistryCompletionCarrierV1> futures::Driver
    for RuntimeGfx942Arena1024ScopeV1<'_, '_, P>
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
            for cell in root.cells.iter().flatten() {
                match &cell.outcome {
                    Some(Ok(())) => {}
                    Some(Err(error)) => {
                        return Err(RuntimeGfx942ScopeErrorV1::Readback(error.clone()));
                    }
                    None => return Err(RuntimeGfx942ScopeErrorV1::Unknown),
                }
            }
        }
        Ok(())
    }
}
