//! Count admission for async reply cells, including completed caller-retained cells.

use super::*;
use std::sync::atomic::AtomicUsize;

pub(super) struct ReplyBudgetV1 {
    capacity: usize,
    used: AtomicUsize,
}

impl ReplyBudgetV1 {
    pub(super) fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            capacity,
            used: AtomicUsize::new(0),
        })
    }
    pub(super) fn used(&self) -> usize {
        self.used.load(Ordering::Acquire)
    }
    pub(super) fn reserve(
        self: &Arc<Self>,
    ) -> Result<ReplyPermitV1, RuntimeAsyncEngineCallErrorV1> {
        self.used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                fe2o3_runtime_model::r64_payload_reserve_v1(used, 1, self.capacity)
            })
            .map_err(|_| RuntimeAsyncEngineCallErrorV1::ReplyCapacity)?;
        Ok(ReplyPermitV1(Arc::clone(self)))
    }
}

pub(super) struct ReplyPermitV1(Arc<ReplyBudgetV1>);
impl Drop for ReplyPermitV1 {
    fn drop(&mut self) {
        self.0
            .used
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |used| {
                fe2o3_runtime_model::r64_payload_release_v1(used, 1)
            })
            .expect("unique reply permit");
    }
}
