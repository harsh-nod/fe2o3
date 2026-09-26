//! Bounded reconciliation of materialized HostVisible compute writebacks.
//! DeviceLocal writes use persistent compute, not the coherent recycled reader.

use super::*;
type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RecycledSourceV1 {
    Native {
        lane: ComputeAqlQueueLaneV1,
        generation: u64,
    },
    #[cfg(test)]
    Scripted { generation: u64 },
}

#[derive(Debug)]
pub(super) struct NativeReconciliationV1 {
    id: u64,
    pub(super) allocation: u64,
    scratch: u64,
    extent: NativeDirtyExtentV1,
    descriptor: ResidentDataDescriptorV1,
    source: RecycledSourceV1,
    pinned_lanes: [bool; KFD_RUNTIME_MAX_COMPUTE_QUEUES_V1],
    cursor: u64,
}

#[cfg(test)]
pub(super) struct ScriptedNativeReconcileV1 {
    pub(super) generation: u64,
    pub(super) data: Vec<Vec<u8>>,
    pub(super) reads: Vec<(u64, usize, u64, u64)>,
}

impl KfdRuntimeBackendV1 {
    pub(super) fn write_cooperative_host_range_v1(
        &mut self,
        allocation: u64,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), Failure> {
        self.require_live()?;
        self.allocations.reject_generated(allocation)?;
        if self.allocation_is_active(allocation) {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "cooperative host write conflicts with retained allocation custody",
            ));
        }
        let record = self.allocations.get(&allocation).ok_or_else(|| {
            Self::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown cooperative host allocation",
            )
        })?;
        if record.kind != RuntimeMemoryKindV1::HostVisible
            || !record.native_dirty.is_empty()
            || offset
                .checked_add(bytes.len() as u64)
                .is_none_or(|end| end > record.bytes.len() as u64)
        {
            return Err(self.terminal_error("cooperative host write changed reconciled authority"));
        }
        let backed = match record.sdma_storage {
            KfdRuntimeSdmaStorageV1::Host(_) if record.sdma_backed => true,
            KfdRuntimeSdmaStorageV1::Synthetic if !record.sdma_backed => false,
            _ => return Err(self.terminal_error("cooperative host write lost backing custody")),
        };
        if backed {
            self.upload_sdma_range_v1(allocation, offset, bytes)?;
        }
        let record = self.allocations.get_mut(&allocation).unwrap();
        record.content_sha256 = None;
        record.last_full_host_write = None;
        if !backed {
            Arc::make_mut(&mut record.bytes)[offset as usize..offset as usize + bytes.len()]
                .copy_from_slice(bytes);
        }
        record.sdma_shadow_dirty = backed;
        // Cached descriptors still describe unchanged native bytes. Ordinary
        // launch preparation refreshes this shadow and requires host overwrite;
        // active/ordered-successor reuse is excluded by allocation custody above.
        let profile_allocation =
            self.profile_resource_v1(KfdProfileResourceKindV1::Allocation, allocation);
        let content = self.profile_host_content_v1(bytes, None);
        self.observe_profile_v1(
            profile_allocation
                .zip(content)
                .map(
                    |(allocation, content)| KfdRuntimeProfileEventKindV1::HostWrite {
                        allocation,
                        byte_offset: offset,
                        content,
                    },
                ),
        );
        Ok(())
    }

    fn native_reconciliation_v1(&self, id: u64) -> &NativeReconciliationV1 {
        self.native_reconciliations
            .iter()
            .flatten()
            .find(|root| root.id == id)
            .expect("private reconciliation remains rooted")
    }

    pub(super) fn native_reconciliation_holds_v1(&self, allocation: u64) -> bool {
        self.native_reconciliations
            .iter()
            .flatten()
            .any(|root| root.allocation == allocation)
    }

    pub(super) fn native_reconciliation_blocker_v1(
        &self,
        allocation: u64,
    ) -> Option<(u64, u64, u64)> {
        let extent = self.allocations.get(&allocation)?.native_dirty.first()?;
        self.native_reconciliations
            .iter()
            .flatten()
            .find_map(|root| {
                let conflict = root.allocation == allocation
                    || root.pinned_lanes.iter().enumerate().any(|(lane, pinned)| {
                        *pinned
                            && (lane == extent.compute_lane
                                || self.recycled_on_lane_v1(lane).is_some_and(|dispatch| {
                                    dispatch
                                        .descriptors
                                        .iter()
                                        .any(|descriptor| descriptor.allocation == allocation)
                                }))
                    });
                conflict.then_some((root.id, root.allocation, root.scratch))
            })
    }

    pub(super) fn native_reconciliation_pins_lane_v1(&self, lane: usize) -> bool {
        self.native_reconciliations
            .iter()
            .flatten()
            .any(|root| root.pinned_lanes[lane])
    }

    pub(super) fn require_unpinned_native_lane_v1(&self, lane: usize) -> Result<(), Failure> {
        if self.native_reconciliation_pins_lane_v1(lane) {
            Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native reconciliation retains this compute lane",
            ))
        } else {
            Ok(())
        }
    }

    fn recycled_on_lane_v1(&self, lane: usize) -> Option<&RecycledDispatchV1> {
        if lane == self.selected_compute_lane {
            self.recycled_dispatch.as_ref()
        } else {
            let index = if lane == 0 {
                self.selected_compute_lane - 1
            } else {
                lane - 1
            };
            self.auxiliary_compute_lanes[index]
                .recycled_dispatch
                .as_ref()
        }
    }

    fn capture_recycled_source_v1(&mut self, lane: usize) -> Result<RecycledSourceV1, Failure> {
        #[cfg(test)]
        if let Some(script) = &self.scripted_native_reconcile {
            return Ok(RecycledSourceV1::Scripted {
                generation: script.generation,
            });
        }
        let Some(native_lane) = self.native_compute_lanes.get(lane).copied().flatten() else {
            return Err(self.terminal_error("native reconciliation lost its compute lane"));
        };
        match self
            .queue
            .as_mut()
            .expect("native lane retains queue")
            .with_compute_lane_v1(native_lane, |queue| {
                queue.recycled_fixed_dispatch_generation()
            }) {
            Ok(Ok(generation)) => Ok(RecycledSourceV1::Native {
                lane: native_lane,
                generation,
            }),
            _ => Err(self.terminal_error(
                "native reconciliation could not authenticate recycled generation",
            )),
        }
    }

    pub(super) fn begin_native_reconciliation_v1(
        &mut self,
        allocation: u64,
        scratch: u64,
    ) -> Result<Option<u64>, Failure> {
        self.require_live()?;
        if self.native_reconciliation_holds_v1(allocation)
            || self.allocation_custody.contains_key(&allocation)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native reconciliation endpoint already retained",
            ));
        }
        let record = &self.allocations[&allocation];
        let Some(extent) = record.native_dirty.first().copied() else {
            return Ok(None);
        };
        let lane = extent.compute_lane;
        if lane >= KFD_RUNTIME_MAX_COMPUTE_QUEUES_V1 {
            return Err(self.terminal_error("native reconciliation extent has invalid lane"));
        }
        let mut pinned_lanes = [false; KFD_RUNTIME_MAX_COMPUTE_QUEUES_V1];
        for (index, pin) in pinned_lanes.iter_mut().enumerate() {
            *pin = index == lane
                || self.recycled_on_lane_v1(index).is_some_and(|dispatch| {
                    dispatch
                        .descriptors
                        .iter()
                        .any(|descriptor| descriptor.allocation == allocation)
                });
            if *pin
                && (self.native_reconciliation_pins_lane_v1(index)
                    || self.active_compute_progress_roster_v1()[index]
                    || self
                        .stream_compute_lanes
                        .values()
                        .any(|assigned| *assigned == index))
            {
                return Err(Self::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "native reconciliation requires returned compute lanes",
                ));
            }
        }
        let descriptor = self
            .recycled_on_lane_v1(lane)
            .and_then(|dispatch| dispatch.descriptors.get(extent.data_index))
            .copied();
        let Some(descriptor) = descriptor else {
            return Err(self.terminal_error("native reconciliation lost its data descriptor"));
        };
        let valid = record.kind == RuntimeMemoryKindV1::HostVisible
            && descriptor.allocation == allocation
            && descriptor.kind == record.kind
            && descriptor.device_may_have_modified
            && extent.byte_len != 0
            && descriptor.allocation_offset.checked_add(extent.data_offset)
                == Some(extent.allocation_offset as u64)
            && extent
                .data_offset
                .checked_add(extent.byte_len)
                .is_some_and(|end| end <= descriptor.byte_len)
            && (extent.allocation_offset as u64)
                .checked_add(extent.byte_len)
                .is_some_and(|end| end <= record.bytes.len() as u64)
            && scratch != allocation
            && self.allocations.get(&scratch).is_some_and(|record| {
                record.kind == RuntimeMemoryKindV1::HostVisible
                    && !record.bytes.is_empty()
                    && record.native_dirty.is_empty()
                    && !record.sdma_shadow_dirty
            });
        if !valid {
            return Err(self.terminal_error(
                "native reconciliation extent or private scratch changed authority",
            ));
        }
        let source = self.capture_recycled_source_v1(lane)?;
        let id = self.next_id()?;
        self.native_reconciliations[lane] = Some(NativeReconciliationV1 {
            id,
            allocation,
            scratch,
            extent,
            descriptor,
            source,
            pinned_lanes,
            cursor: 0,
        });
        Ok(Some(id))
    }

    fn authenticate_native_reconciliation_v1(&mut self, id: u64) -> Result<(), Failure> {
        let root = self.native_reconciliation_v1(id);
        let valid = self
            .allocations
            .get(&root.allocation)
            .is_some_and(|record| record.native_dirty.contains(&root.extent))
            && self
                .recycled_on_lane_v1(root.extent.compute_lane)
                .and_then(|dispatch| dispatch.descriptors.get(root.extent.data_index))
                == Some(&root.descriptor);
        let (lane, source) = (root.extent.compute_lane, root.source);
        // Query only to authenticate; never replace the captured generation.
        if !valid || self.capture_recycled_source_v1(lane)? != source {
            return Err(self.terminal_error(
                "retained native reconciliation generation or descriptor changed",
            ));
        }
        Ok(())
    }

    pub(super) fn progress_native_reconciliation_v1(&mut self, id: u64) -> Result<bool, Failure> {
        self.authenticate_native_reconciliation_v1(id)?;
        let root = self.native_reconciliation_v1(id);
        let (source, extent, cursor, scratch, allocation) = (
            root.source,
            root.extent,
            root.cursor,
            root.scratch,
            root.allocation,
        );
        let len = (extent.byte_len - cursor).min(self.allocations[&scratch].bytes.len() as u64);
        let record = self.allocations.get_mut(&scratch).unwrap();
        let Some(bytes) = Arc::get_mut(&mut record.bytes) else {
            return Err(
                self.terminal_error("private reconciliation scratch has shared CPU custody")
            );
        };
        let destination = &mut bytes[..len as usize];
        let offset = extent.data_offset + cursor;
        let read = match source {
            RecycledSourceV1::Native { lane, generation } => self
                .queue
                .as_mut()
                .unwrap()
                .with_compute_lane_v1(lane, |queue| {
                    queue.read_recycled_fixed_dispatch_data_into(
                        Gfx942CompletedDispatchReadRequestV1::new(
                            generation,
                            extent.data_index,
                            offset,
                            len,
                        ),
                        destination,
                    )
                })
                .is_ok_and(|result| result.is_ok()),
            #[cfg(test)]
            RecycledSourceV1::Scripted { generation } => {
                let script = self.scripted_native_reconcile.as_mut().unwrap();
                script
                    .reads
                    .push((generation, extent.data_index, offset, len));
                match script
                    .data
                    .get(extent.data_index)
                    .and_then(|bytes| bytes.get(offset as usize..(offset + len) as usize))
                {
                    Some(bytes) => {
                        destination.copy_from_slice(bytes);
                        true
                    }
                    None => false,
                }
            }
        };
        if !read {
            return Err(self.terminal_error("generation-pinned native readback failed"));
        }
        record.content_sha256 = None;
        record.last_full_host_write = None;
        let bytes = Arc::clone(&record.bytes);
        // HostVisible backing uses a mapped host write, never GPU submit/wait.
        self.upload_sdma_range_v1(
            allocation,
            extent.allocation_offset as u64 + cursor,
            &bytes[..len as usize],
        )?;
        let record = self.allocations.get_mut(&allocation).unwrap();
        record.last_full_host_write = None;
        if !record.sdma_backed {
            let start = extent.allocation_offset + cursor as usize;
            Arc::make_mut(&mut record.bytes)[start..start + len as usize]
                .copy_from_slice(&bytes[..len as usize]);
        }
        record.sdma_shadow_dirty = true;
        record.content_sha256 = None;
        record.last_full_host_write = None;
        let root = self.native_reconciliations[extent.compute_lane]
            .as_mut()
            .unwrap();
        root.cursor += len;
        if root.cursor != extent.byte_len {
            return Ok(false);
        }
        self.authenticate_native_reconciliation_v1(id)?;
        let record = self.allocations.get_mut(&allocation).unwrap();
        let index = record
            .native_dirty
            .iter()
            .position(|candidate| *candidate == extent)
            .unwrap();
        record.native_dirty.remove(index);
        self.native_dirty_extents = self
            .native_dirty_extents
            .checked_sub(1)
            .expect("dirty authority is counted");
        self.native_reconciliations[extent.compute_lane] = None;
        Ok(true)
    }

    pub(super) fn release_native_reconciliation_v1(&mut self, id: u64) {
        let lane = self.native_reconciliation_v1(id).extent.compute_lane;
        // Aborted chunks leave the complete recycled extent authoritative.
        self.native_reconciliations[lane] = None;
    }
}
