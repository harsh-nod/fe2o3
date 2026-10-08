//! Every result cell and reply payload is prepaid on the original metadata account.

use super::*;
use fe2o3_resource_accounting::{ResourceKindV1, ResourceVectorV1, RetainedResourceCreditsV1};

pub(super) struct Cell {
    pub outcome: Option<Outcome>,
    pub domain: crate::RuntimeGeneratedResultDomainV1,
    pub reply: crate::async_engine::RuntimeAsyncReplyV1<Outcome>,
    pub future: Option<crate::RuntimeAsyncCommandFutureV1<Outcome>>,
}

pub(super) struct ReplyPayload(Option<RetainedResourceCreditsV1>);

impl Drop for ReplyPayload {
    fn drop(&mut self) {
        if !std::thread::panicking()
            && let Some(credits) = self.0.take()
            && credits.release_after_disposal().is_err()
        {
            std::process::abort();
        }
    }
}

// Keep this aggregate intact through fallible pre-root admission. Field order
// disposes every reply cell before the shared payload debit, including refusal.
pub(super) struct PreparedResults {
    pub(super) cells: HostMetadataTableV1<Option<Cell>>,
    pub(super) payload: Rc<ReplyPayload>,
}

#[cfg(test)]
pub(super) fn prepare(
    metadata: &ResourceCreditAccountV1,
    original_domain: impl FnMut(
        usize,
    ) -> Result<
        crate::RuntimeGeneratedResultDomainV1,
        RuntimeGfx942ReadbackErrorV1,
    >,
) -> Result<PreparedResults, RuntimeGfx942ScopeErrorV1> {
    prepare_count(metadata, SLOTS, original_domain)
}

pub(super) fn prepare_count(
    metadata: &ResourceCreditAccountV1,
    slots: usize,
    mut original_domain: impl FnMut(
        usize,
    ) -> Result<
        crate::RuntimeGeneratedResultDomainV1,
        RuntimeGfx942ReadbackErrorV1,
    >,
) -> Result<PreparedResults, RuntimeGfx942ScopeErrorV1> {
    if !matches!(slots, 1024 | 2048) {
        return Err(RuntimeGfx942ScopeErrorV1::Capacity);
    }
    let bytes = crate::async_engine::RuntimeAsyncReplyV1::<Outcome>::payload_bytes_v1()
        .checked_mul(slots)
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(RuntimeGfx942ScopeErrorV1::Capacity)?;
    let credit = metadata
        .reserve(ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes))
        .map_err(|_| RuntimeGfx942ScopeErrorV1::Capacity)?;
    let payload = Rc::new(ReplyPayload(Some(credit.retain())));
    // Reverse local destruction also preserves this order on refusal/unwind.
    let mut cells = HostMetadataTableV1::try_new(slots, Some(metadata), || None)
        .map_err(|_| RuntimeGfx942ScopeErrorV1::Capacity)?;
    let mut addresses = HostMetadataTableV1::try_new(slots, Some(metadata), || 0usize)
        .map_err(|_| RuntimeGfx942ScopeErrorV1::Capacity)?;
    for index in 0..slots {
        let domain = original_domain(index).map_err(RuntimeGfx942ScopeErrorV1::Readback)?;
        addresses[index] = domain.retained_address_v1();
        let (reply, future) = crate::async_engine::RuntimeAsyncReplyV1::pair();
        cells[index] = Some(Cell {
            outcome: None,
            domain,
            reply,
            future: Some(future),
        });
    }
    // Every domain's Arc remains retained. Sorting detects exact aliasing but
    // the addresses supply no semantic/currentness or execution authority.
    addresses.sort_unstable();
    if addresses.windows(2).any(|pair| pair[0] == pair[1]) {
        // Dispose payloads before returning their original shared debit.
        drop(cells);
        return Err(RuntimeGfx942ScopeErrorV1::Readback(
            RuntimeGfx942ReadbackErrorV1::InvalidStorage,
        ));
    }
    Ok(PreparedResults { cells, payload })
}

pub(super) fn decode_copied(
    cells: &mut [Option<Cell>],
    copied: &[bool],
    mut decode: impl FnMut(usize) -> Outcome,
) -> usize {
    if !matches!(cells.len(), 1024 | 2048) || copied.len() != cells.len() {
        std::process::abort();
    }
    let mut transitions = 0;
    for (index, (cell, copied)) in cells.iter_mut().zip(copied).enumerate() {
        let cell = cell.as_mut().unwrap_or_else(|| std::process::abort());
        if !copied || cell.outcome.is_some() {
            continue;
        }
        let outcome = decode(index);
        cell.reply.complete(Ok(outcome.clone()));
        cell.outcome = Some(outcome);
        transitions += 1;
    }
    transitions
}

#[cfg(test)]
mod tests;
