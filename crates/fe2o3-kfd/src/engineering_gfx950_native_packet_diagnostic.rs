//! Raw packet-processing intervals, not shader-only time or completion authority.

use super::*;
use std::io::Write;

#[path = "engineering_native_packet_observation.rs"]
mod observation;

pub(super) struct Diagnostic {
    identity: [u64; 3],
    kernels: Vec<u64>,
    symbols: BTreeMap<u64, String>,
    saved_properties: Option<u32>,
}

impl Diagnostic {
    pub(super) fn new(context: &Context, prepared: &[PreparedDispatch]) -> Result<Self> {
        let mut kernels = Vec::with_capacity(prepared.len());
        let mut symbols = BTreeMap::new();
        for item in prepared {
            let mut matches = context.kernels.iter().filter(|(_, kernel)| {
                kernel.code.va.checked_add(kernel.descriptor_offset) == Some(item.descriptor)
            });
            let (&handle, kernel) = matches.next().ok_or("missing diagnostic kernel")?;
            if matches.next().is_some() {
                return Err("ambiguous native timestamp kernel identity".into());
            }
            kernels.push(handle);
            symbols.insert(handle, kernel.metadata.symbol.clone());
        }
        let identity = [context.unique_id, context.queue_epoch, context.ring.write()];
        observation::validate_identity(identity, kernels.len())?;
        Ok(Self {
            identity,
            kernels,
            symbols,
            saved_properties: None,
        })
    }

    pub(super) fn begin(&mut self, context: &mut Context, layout: &ProgramLayout) -> Result<()> {
        if self.saved_properties.is_some() || layout.slots.len() != self.kernels.len() {
            return Err("native timestamp repeated begin or changed layout".into());
        }
        self.check(context, self.identity[2])?;
        let signal = &mut context.token_program_storage.allocations[PROGRAM_SIGNAL];
        // SAFETY: current idle queue, exclusive retained arena, no publication yet.
        unsafe {
            Backend::clear_engineering_program_timestamps(
                &mut signal.mapping,
                signal.requested,
                self.kernels.len(),
            )
        }
        .map_err(explain)?;
        // SAFETY: no outstanding packets; only this owned queue's profiling bit changes.
        self.saved_properties = Some(
            unsafe {
                Backend::enable_engineering_dispatch_timestamps(
                    &mut context.internal[CONTROL].mapping,
                )
            }
            .map_err(explain)?,
        );
        Ok(())
    }

    fn check(&self, context: &mut Context, frontier: u64) -> Result<()> {
        require_pending_dispatch_identity(
            [context.unique_id, context.queue_epoch, context.ring.write()],
            [self.identity[0], self.identity[1], frontier],
            context.ordered_batch_poisoned,
        )?;
        require_completed_frontier(context.completed_write, frontier)?;
        context.check_currentness(true)?;
        context.check_idle()
    }

    pub(super) fn finish(&mut self, context: &mut Context) -> Result<()> {
        let next = observation::validate_identity(self.identity, self.kernels.len())?;
        self.check(context, next)?;
        let signal = &mut context.token_program_storage.allocations[PROGRAM_SIGNAL];
        let mut ticks = Vec::with_capacity(self.kernels.len());
        for slot in 0..self.kernels.len() {
            // SAFETY: original native retirement acquired every signal; queue
            // remains idle and this borrow excludes publication or arena reuse.
            ticks.push(
                unsafe {
                    Backend::observe_engineering_program_timestamps(
                        &mut signal.mapping,
                        signal.requested,
                        self.kernels.len(),
                        slot as u32,
                    )
                }
                .map_err(explain)?,
            );
        }
        let record = observation::Record::new(self.identity, &self.kernels, &self.symbols, ticks)?;
        self.check(context, next)?;
        let saved = self
            .saved_properties
            .ok_or("missing native timestamp properties")?;
        // SAFETY: success-only stable identity and fresh idle/currentness check.
        // Failure paths never restore or reuse the queue or its signal arena.
        unsafe {
            Backend::restore_engineering_dispatch_timestamps(
                &mut context.internal[CONTROL].mapping,
                saved,
            )
        }
        .map_err(explain)?;
        let mut bytes = serde_json::to_vec(&record).map_err(explain)?;
        bytes.push(b'\n');
        std::io::stderr().lock().write_all(&bytes).map_err(explain)
    }
}
