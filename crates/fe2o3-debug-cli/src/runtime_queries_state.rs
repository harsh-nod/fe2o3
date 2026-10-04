//! Backend-lifetime custody and bounded, consumed opaque continuation tokens.
use super::mapping::number;
use fe2o3_debug_protocol::*;
use fe2o3_kir_debugger::RuntimeReplayWorkV1;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_CURSORS: usize = 256;
const MAX_CURSOR_BYTES: usize = 256 * 1024;
const TOKEN_CAPACITY: usize = 64;
const MAX_QUERY_WORK: usize = 1_000_000;
const MAX_SESSION_QUERY_WORK: usize = 64_000_000;
static NEXT_BACKEND_SESSION: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QueryShape {
    binding: RuntimeObservationBindingV1,
    operation: ResourceOperationV2,
    max_items: u16,
    max_scanned: u16,
    address_space: Option<AddressSpaceV1>,
    allocation: Option<ResourceStorageIdentityV2>,
}
impl QueryShape {
    fn new(request: &ResourceRequestV2) -> Option<Self> {
        let page = request.page()?;
        Some(Self {
            binding: request.expected_binding(),
            operation: request.operation(),
            max_items: page.max_items,
            max_scanned: page.max_scanned,
            address_space: match request {
                ResourceRequestV2::QueryAllocations { address_space, .. } => *address_space,
                _ => None,
            },
            allocation: match request {
                ResourceRequestV2::QueryMemoryAccesses { allocation, .. } => Some(*allocation),
                _ => None,
            },
        })
    }
}
struct CursorEntry {
    token: ResourcePageTokenV1,
    token_capacity: usize,
    shape: QueryShape,
    next: usize,
}
pub(crate) struct RuntimeQueryStateV1 {
    backend_session: u64,
    binding: Option<RuntimeObservationBindingV1>,
    next_token: u64,
    cursors: Vec<CursorEntry>,
    token_bytes: usize,
    remaining_work: usize,
}
impl RuntimeQueryStateV1 {
    pub(crate) fn new() -> Result<Self, String> {
        let previous = NEXT_BACKEND_SESSION
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| "runtime backend lifetime counter exhausted")?;
        Ok(Self {
            backend_session: previous + 1,
            binding: None,
            next_token: 0,
            cursors: Vec::new(),
            token_bytes: 0,
            remaining_work: MAX_SESSION_QUERY_WORK,
        })
    }
    pub(super) fn owner(&self, capture_instance: u64) -> RuntimeObservationOwnerV1 {
        RuntimeObservationOwnerV1 {
            backend_session: number(self.backend_session),
            capture_instance: number(capture_instance),
        }
    }
    pub(super) fn refresh(&mut self, binding: Option<RuntimeObservationBindingV1>) {
        if self.binding != binding {
            self.cursors.clear();
            self.token_bytes = 0;
            self.binding = binding;
        }
    }
    pub(super) fn work(&self) -> Result<RuntimeReplayWorkV1, &'static str> {
        if self.remaining_work == 0 {
            return Err("runtime query work budget exhausted");
        }
        RuntimeReplayWorkV1::new(self.remaining_work.min(MAX_QUERY_WORK))
            .map_err(|_| "runtime query work configuration invalid")
    }
    pub(super) fn settle_work(&mut self, before: usize, after: usize) {
        // The work object can only decrease; impossible arithmetic exhausts the
        // budget rather than refunding or wrapping it.
        self.remaining_work = before
            .checked_sub(after)
            .and_then(|spent| self.remaining_work.checked_sub(spent))
            .unwrap_or(0);
    }
    pub(super) fn take_page(
        &mut self,
        request: &ResourceRequestV2,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<usize, &'static str> {
        let Some(page) = request.page() else {
            return Ok(0);
        };
        let Some(token) = page.token.as_ref() else {
            return Ok(0);
        };
        work.charge(self.cursors.len())
            .map_err(|_| "runtime cursor scan budget exhausted")?;
        let index = self
            .cursors
            .iter()
            .position(|entry| entry.token == *token)
            .ok_or("runtime cursor is unknown, consumed, or stale")?;
        let entry = self.cursors.swap_remove(index);
        self.token_bytes = self
            .token_bytes
            .checked_sub(entry.token_capacity)
            .ok_or("runtime cursor accounting invalid")?;
        if Some(entry.shape) != QueryShape::new(request)
            || self.binding != Some(entry.shape.binding)
        {
            return Err("runtime cursor query binding changed");
        }
        Ok(entry.next)
    }
    pub(super) fn retain_page(
        &mut self,
        request: &ResourceRequestV2,
        next: Option<usize>,
    ) -> Result<Option<ResourcePageTokenV1>, &'static str> {
        let Some(next) = next else {
            return Ok(None);
        };
        let shape = QueryShape::new(request).ok_or("runtime memory reads cannot continue")?;
        if self.binding != Some(shape.binding) {
            return Err("runtime cursor binding is no longer current");
        }
        if self.cursors.len() >= MAX_CURSORS {
            return Err("runtime cursor count budget exhausted");
        }
        if self.cursors.capacity() == 0 {
            self.cursors
                .try_reserve_exact(MAX_CURSORS)
                .map_err(|_| "runtime cursor allocation failed")?;
            if self.cursors.capacity() != MAX_CURSORS
                || self
                    .cursors
                    .capacity()
                    .checked_mul(std::mem::size_of::<CursorEntry>())
                    .and_then(|v| v.checked_add(MAX_CURSORS * TOKEN_CAPACITY))
                    .is_none_or(|v| v > MAX_CURSOR_BYTES)
            {
                self.cursors = Vec::new();
                return Err("runtime cursor capacity budget exceeded");
            }
        }
        let serial = self
            .next_token
            .checked_add(1)
            .ok_or("runtime cursor token counter exhausted")?;
        let mut text = String::new();
        text.try_reserve_exact(TOKEN_CAPACITY)
            .map_err(|_| "runtime cursor token allocation failed")?;
        if text.capacity() > TOKEN_CAPACITY {
            return Err("runtime cursor token capacity exceeded");
        }
        use std::fmt::Write;
        write!(&mut text, "runtime.{:x}.{:x}", self.backend_session, serial)
            .map_err(|_| "runtime cursor token encoding failed")?;
        let capacity = text.capacity();
        let token = ResourcePageTokenV1::new(text).map_err(|_| "runtime cursor token rejected")?;
        let bytes = self
            .token_bytes
            .checked_add(capacity)
            .ok_or("runtime cursor bytes overflow")?;
        let total = self
            .cursors
            .capacity()
            .checked_mul(std::mem::size_of::<CursorEntry>())
            .and_then(|v| v.checked_add(bytes))
            .ok_or("runtime cursor storage overflow")?;
        if total > MAX_CURSOR_BYTES {
            return Err("runtime cursor byte budget exhausted");
        }
        self.next_token = serial;
        self.token_bytes = bytes;
        self.cursors.push(CursorEntry {
            token: token.clone(),
            token_capacity: capacity,
            shape,
            next,
        });
        Ok(Some(token))
    }
}
