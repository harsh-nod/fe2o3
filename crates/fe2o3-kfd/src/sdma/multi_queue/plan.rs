//! Bounded striped SDMA request-to-queue planning.

use super::*;

impl Gfx942SdmaMultiQueuePlanV1 {
    pub fn new(
        queue_ids: &[u32],
        request_count: usize,
        first_queue: usize,
    ) -> Result<Self, Gfx942SdmaMultiQueuePlanErrorV1> {
        if queue_ids.len() < KFD_GFX942_SDMA_ENGINE_COUNT_V1 as usize
            || !queue_ids
                .len()
                .is_multiple_of(KFD_GFX942_SDMA_ENGINE_COUNT_V1 as usize)
            || queue_ids.len() > GFX942_SDMA_MAX_STRIPED_QUEUES_V1
        {
            return Err(Gfx942SdmaMultiQueuePlanErrorV1::QueueCount {
                actual: queue_ids.len(),
            });
        }
        for (index, queue_id) in queue_ids.iter().copied().enumerate() {
            if queue_ids[..index].contains(&queue_id) {
                return Err(Gfx942SdmaMultiQueuePlanErrorV1::DuplicateQueueId { queue_id });
            }
        }
        let maximum = queue_ids
            .len()
            .checked_mul(GFX942_SDMA_MAX_IN_FLIGHT_V1)
            .ok_or(Gfx942SdmaMultiQueuePlanErrorV1::RequestCount {
                actual: request_count,
                maximum: GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1,
            })?;
        if request_count == 0
            || request_count > maximum
            || request_count > GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1
        {
            return Err(Gfx942SdmaMultiQueuePlanErrorV1::RequestCount {
                actual: request_count,
                maximum,
            });
        }
        if first_queue >= queue_ids.len() {
            return Err(Gfx942SdmaMultiQueuePlanErrorV1::InvalidCursor {
                actual: first_queue,
                queue_count: queue_ids.len(),
            });
        }
        let mut retained_queue_ids = Vec::new();
        retained_queue_ids
            .try_reserve_exact(queue_ids.len())
            .map_err(|_| Gfx942SdmaMultiQueuePlanErrorV1::Allocation)?;
        retained_queue_ids.extend_from_slice(queue_ids);
        let mut assignments = Vec::new();
        assignments
            .try_reserve_exact(request_count)
            .map_err(|_| Gfx942SdmaMultiQueuePlanErrorV1::Allocation)?;
        let mut shard_counts = Vec::new();
        shard_counts
            .try_reserve_exact(queue_ids.len())
            .map_err(|_| Gfx942SdmaMultiQueuePlanErrorV1::Allocation)?;
        shard_counts.resize(queue_ids.len(), 0_u16);
        for request_index in 0..request_count {
            let queue = (first_queue + request_index) % queue_ids.len();
            assignments.push(queue as u16);
            shard_counts[queue] += 1;
        }
        debug_assert!(
            shard_counts
                .iter()
                .all(|count| usize::from(*count) <= GFX942_SDMA_MAX_IN_FLIGHT_V1)
        );
        Ok(Self {
            queue_ids: retained_queue_ids,
            first_queue: first_queue as u16,
            request_count: request_count as u16,
            assignments,
            shard_counts,
        })
    }

    pub fn queue_ids(&self) -> &[u32] {
        &self.queue_ids
    }

    pub const fn first_queue(&self) -> usize {
        self.first_queue as usize
    }

    pub const fn request_count(&self) -> usize {
        self.request_count as usize
    }

    pub fn queue_for_request(&self, request_index: usize) -> Option<usize> {
        self.assignments
            .get(request_index)
            .map(|queue| *queue as usize)
    }

    pub fn shard_count(&self, queue: usize) -> Option<usize> {
        self.shard_counts.get(queue).map(|count| *count as usize)
    }

    pub fn active_shard_count(&self) -> usize {
        self.shard_counts
            .iter()
            .filter(|count| **count != 0)
            .count()
    }

    pub fn next_queue_after_success(&self) -> usize {
        (self.first_queue() + self.request_count()) % self.queue_ids.len()
    }

    pub fn is_current_for(&self, queue_ids: &[u32], first_queue: usize) -> bool {
        self.queue_ids.as_slice() == queue_ids && self.first_queue() == first_queue
    }

    pub fn is_balanced(&self) -> bool {
        let minimum = self.shard_counts.iter().copied().min().unwrap_or(0);
        let maximum = self.shard_counts.iter().copied().max().unwrap_or(0);
        maximum - minimum <= 1
    }
}
