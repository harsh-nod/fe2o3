//! Complete copies into already-owned destinations, without result publication.

use super::*;
use crate::Gfx942RuntimeBufferAccessV1;

#[derive(Debug, Eq, PartialEq)]
enum ReadbackErrorV1<E> {
    Roster,
    Copy(E),
}

pub(super) fn roster_matches_plan_v1(
    plan: &GeneratedShellPlanV1,
    roster: &GeneratedHostRosterV1,
) -> bool {
    plan.count > 0
        && plan.count <= plan.members.len()
        && plan.count == roster.count
        && plan.members[..plan.count]
            .iter()
            .enumerate()
            .all(|(ordinal, member)| {
                member.is_some_and(|member| {
                    roster.buffers[ordinal].is_some_and(|slot| {
                        slot.ordinal == ordinal && slot.bytes == member.description.byte_len
                    })
                })
            })
}

fn read_roster_v1<E>(
    roster: &GeneratedHostRosterV1,
    destinations: &mut [(Gfx942RuntimeBufferAccessV1, Vec<u8>)],
    mut copy: impl FnMut(usize, &mut [u8]) -> Result<(), E>,
) -> Result<(), ReadbackErrorV1<E>> {
    if roster.count == 0
        || roster.count > roster.buffers.len()
        || destinations.len() != roster.count
        || roster.buffers[roster.count..].iter().any(Option::is_some)
    {
        return Err(ReadbackErrorV1::Roster);
    }
    let mut total = 0u64;
    // Validate the whole destination roster before modifying even its first byte.
    for (ordinal, (access, bytes)) in destinations.iter().enumerate() {
        let slot = roster.buffers[ordinal].ok_or(ReadbackErrorV1::Roster)?;
        if slot.ordinal != ordinal
            || slot.bytes == 0
            || slot.access != *access
            || u64::try_from(bytes.len()).ok() != Some(slot.bytes)
            || bytes.capacity() != bytes.len()
        {
            return Err(ReadbackErrorV1::Roster);
        }
        total = total
            .checked_add(slot.bytes)
            .ok_or(ReadbackErrorV1::Roster)?;
    }
    if total != roster.readback_bytes {
        return Err(ReadbackErrorV1::Roster);
    }
    for (ordinal, (_, bytes)) in destinations.iter_mut().enumerate() {
        copy(ordinal, bytes).map_err(ReadbackErrorV1::Copy)?;
    }
    Ok(())
}

impl KfdRuntimeBackendV1 {
    pub(crate) fn read_generated_submission_v1(
        &mut self,
        plan: &GeneratedShellPlanV1,
        submission: u64,
        roster: &GeneratedHostRosterV1,
        destinations: &mut [(Gfx942RuntimeBufferAccessV1, Vec<u8>)],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.generated_submission_plan_v1(submission)? != *plan {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated readback plan mismatch",
            ));
        }
        let native = self.generated_shells[&plan.key]
            .native
            .as_ref()
            .expect("native owner");
        let owner = native.submission.as_ref().expect("indexed submission");
        if !owner.roster.matches(roster)
            || !roster_matches_plan_v1(plan, roster)
            || !matches!(owner.receipt, ReceiptV1::Recycled)
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "generated readback requires exact recycled submission and source",
            ));
        }
        let handle = native.native_lane.expect("exact lane");
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.check_generated_device_v1(plan)?;
            let copied = self
                .queue
                .as_mut()
                .expect("retained queue")
                .with_compute_lane_v1(handle, |lane| {
                    let (generation, count) = lane
                        .recycled_fixed_dispatch_data_shape_v1()
                        .map_err(ReadbackErrorV1::Copy)?;
                    if count != plan.count {
                        return Err(ReadbackErrorV1::Roster);
                    }
                    read_roster_v1(roster, destinations, |ordinal, bytes| {
                        let request = fe2o3_kfd::Gfx942CompletedDispatchReadRequestV1::new(
                            generation,
                            ordinal,
                            0,
                            bytes.len() as u64,
                        );
                        // N5 initialized every complete DATA extent, including
                        // unused inputs and bytes outside a writable binding.
                        // This observes bytes, not proof that the GPU wrote them.
                        lane.read_recycled_fixed_dispatch_initialized_data_into(request, bytes)
                    })
                })
                .map_err(ReadbackErrorV1::Copy)
                .and_then(core::convert::identity);
            match copied {
                Ok(()) => {}
                Err(ReadbackErrorV1::Roster) => {
                    return Err(Self::rejected(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "generated readback destination roster mismatch",
                    ));
                }
                Err(ReadbackErrorV1::Copy(error)) => {
                    return Err(self.generated_native_error_v1("readback", error));
                }
            }
            self.check_generated_device_v1(plan)
        }));
        // The caller retains every destination, including a partially copied
        // prefix. Neither a copy nor a closing-check failure permits retry.
        self.finish_generated_native_call_v1(result)
    }
}

#[cfg(test)]
mod tests;
