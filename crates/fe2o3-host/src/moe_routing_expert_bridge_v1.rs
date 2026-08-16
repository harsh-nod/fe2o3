//! Host-observed routing-output consistency for exact MoE T8/E4/K2/C4 V1.
//!
//! This module checks caller-supplied CPU bytes and can synchronously upload
//! the checked offsets and inverse arrays into two retained device regions.
//! It does not establish that a router ran, that device output was read back,
//! or that any GPU operation may be submitted.

use crate::{
    MoeExpertCompactPackPlanErrorV1, MoeExpertCompactPackPlanV1,
    generated_moe_expert_v1::{EXPERT_OFFSETS, EXPERTS, ROUTES},
};
use fe2o3_core::{
    BorrowedDeviceOperation, ContextIdentity, DeviceBuffer, DeviceBufferIdentity,
    DeviceBufferRangeError, DeviceBufferView, PinnedHostBuffer, Stream, StreamIdentity,
};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt, ops::Range};

const TOKENS: usize = 8;
const TOP_K: usize = 2;
const EXPERT_CAPACITY: u32 = 4;
const DROP_ROUTE: u32 = u32::MAX;
const PAYLOAD_DOMAIN: &[u8] = b"FE2O3/MOE/ROUTING-EXPERT/HOST-OBSERVED-SNAPSHOT/V1\0";

/// Untrusted host values shaped like the exact router's complete output.
///
/// Construction performs no validation. Passing this value to
/// [`check_host_observed_moe_routing_output_v1`] is the only way to obtain the
/// opaque checked witness used by expert preparation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoeRoutingOutputCandidateV1 {
    pub(crate) top2_experts: [u32; ROUTES],
    pub(crate) requested_counts: [u32; EXPERTS],
    pub(crate) admitted_counts: [u32; EXPERTS],
    pub(crate) expert_offsets: [u32; EXPERT_OFFSETS],
    pub(crate) route_slots: [u32; ROUTES],
    pub(crate) permutation: [u32; ROUTES],
    pub(crate) inverse: [u32; ROUTES],
}

impl MoeRoutingOutputCandidateV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        top2_experts: [u32; ROUTES],
        requested_counts: [u32; EXPERTS],
        admitted_counts: [u32; EXPERTS],
        expert_offsets: [u32; EXPERT_OFFSETS],
        route_slots: [u32; ROUTES],
        permutation: [u32; ROUTES],
        inverse: [u32; ROUTES],
    ) -> Self {
        Self {
            top2_experts,
            requested_counts,
            admitted_counts,
            expert_offsets,
            route_slots,
            permutation,
            inverse,
        }
    }
}

/// Exact reason that untrusted host routing values were rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MoeRoutingOutputConsistencyErrorV1 {
    ExpertOutOfRange {
        route: usize,
        expert: u32,
    },
    DuplicateTokenExpert {
        token: usize,
        expert: u32,
    },
    RequestedCountMismatch {
        expert: usize,
        expected: u32,
        actual: u32,
    },
    Capacity {
        expert: usize,
        admitted: u32,
    },
    AdmittedCountMismatch {
        expert: usize,
        expected: u32,
        actual: u32,
    },
    FirstOffset {
        actual: u32,
    },
    NonMonotoneOffsets {
        expert: usize,
        start: u32,
        end: u32,
    },
    OffsetMismatch {
        index: usize,
        expected: u32,
        actual: u32,
    },
    CompactPlan(MoeExpertCompactPackPlanErrorV1),
    SlotOutOfRange {
        route: usize,
        slot: u32,
        accepted_routes: u32,
    },
    DuplicateSlot {
        route: usize,
        slot: usize,
    },
    StableSlotMismatch {
        route: usize,
        expected: u32,
        actual: u32,
    },
    InverseMismatch {
        route: usize,
        expected: u32,
        actual: u32,
    },
    PermutationRouteOutOfRange {
        slot: usize,
        route: u32,
    },
    PermutationMismatch {
        slot: usize,
        expected_route: u32,
        actual_route: u32,
    },
    AcceptedPrefixHole {
        slot: usize,
    },
    PermutationTail {
        slot: usize,
        actual: u32,
    },
}

impl fmt::Display for MoeRoutingOutputConsistencyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "host-observed MoE routing snapshot rejected: {self:?}"
        )
    }
}

impl Error for MoeRoutingOutputConsistencyErrorV1 {}

/// Opaque witness for one internally consistent host payload.
///
/// This witness authenticates no producer. In particular, it is not evidence
/// of router dispatch, completion, device readback, compiler output, or GPU
/// execution. Its private fields prevent safe callers from splicing arrays
/// after validation. It carries no freshness: callers can reconstruct and
/// recheck equivalent public candidates.
#[must_use = "the checked routing snapshot must be consumed as one payload"]
pub struct CheckedMoeHostObservedRoutingOutputV1 {
    payload: MoeRoutingOutputCandidateV1,
    compact_pack: MoeExpertCompactPackPlanV1,
    payload_sha256: [u8; 32],
}

impl fmt::Debug for CheckedMoeHostObservedRoutingOutputV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedMoeHostObservedRoutingOutputV1")
            .field("admitted_counts", &self.payload.admitted_counts)
            .field("expert_offsets", &self.payload.expert_offsets)
            .field("payload_sha256", &self.payload_sha256)
            .field("producer", &"untrusted host observation")
            .finish_non_exhaustive()
    }
}

impl CheckedMoeHostObservedRoutingOutputV1 {
    pub(crate) const fn top2_experts(&self) -> [u32; ROUTES] {
        self.payload.top2_experts
    }

    pub const fn admitted_counts(&self) -> [u32; EXPERTS] {
        self.payload.admitted_counts
    }

    pub const fn expert_offsets(&self) -> [u32; EXPERT_OFFSETS] {
        self.payload.expert_offsets
    }

    pub const fn route_slots(&self) -> [u32; ROUTES] {
        self.payload.route_slots
    }

    pub const fn permutation(&self) -> [u32; ROUTES] {
        self.payload.permutation
    }

    pub const fn inverse(&self) -> [u32; ROUTES] {
        self.payload.inverse
    }

    pub const fn payload_sha256(&self) -> [u8; 32] {
        self.payload_sha256
    }

    pub const fn compact_pack_plan(&self) -> MoeExpertCompactPackPlanV1 {
        self.compact_pack
    }

    pub const fn producer_is_authenticated(&self) -> bool {
        false
    }

    pub const fn proves_gpu_execution(&self) -> bool {
        false
    }
}

/// Checks the fixed internal routing relations over one untrusted host payload.
///
/// The check is conditioned on caller-supplied top-2 expert IDs. It does not
/// validate those IDs against logits or bind route weights or packed activations.
pub fn check_host_observed_moe_routing_output_v1(
    candidate: MoeRoutingOutputCandidateV1,
) -> Result<CheckedMoeHostObservedRoutingOutputV1, MoeRoutingOutputConsistencyErrorV1> {
    let mut expected_requested = [0_u32; EXPERTS];
    for token in 0..TOKENS {
        let first_route = token * TOP_K;
        let first = candidate.top2_experts[first_route];
        let second = candidate.top2_experts[first_route + 1];
        for (route, expert) in [(first_route, first), (first_route + 1, second)] {
            if expert as usize >= EXPERTS {
                return Err(MoeRoutingOutputConsistencyErrorV1::ExpertOutOfRange { route, expert });
            }
            expected_requested[expert as usize] += 1;
        }
        if first == second {
            return Err(MoeRoutingOutputConsistencyErrorV1::DuplicateTokenExpert {
                token,
                expert: first,
            });
        }
    }

    for (expert, (&expected, &actual)) in expected_requested
        .iter()
        .zip(&candidate.requested_counts)
        .enumerate()
    {
        if actual != expected {
            return Err(MoeRoutingOutputConsistencyErrorV1::RequestedCountMismatch {
                expert,
                expected,
                actual,
            });
        }
    }

    for expert in 0..EXPERTS {
        let actual = candidate.admitted_counts[expert];
        if actual > EXPERT_CAPACITY {
            return Err(MoeRoutingOutputConsistencyErrorV1::Capacity {
                expert,
                admitted: actual,
            });
        }
        let expected = candidate.requested_counts[expert].min(EXPERT_CAPACITY);
        if actual != expected {
            return Err(MoeRoutingOutputConsistencyErrorV1::AdmittedCountMismatch {
                expert,
                expected,
                actual,
            });
        }
    }

    if candidate.expert_offsets[0] != 0 {
        return Err(MoeRoutingOutputConsistencyErrorV1::FirstOffset {
            actual: candidate.expert_offsets[0],
        });
    }
    let mut expected_offset = 0_u32;
    for expert in 0..EXPERTS {
        let start = candidate.expert_offsets[expert];
        let end = candidate.expert_offsets[expert + 1];
        if start > end {
            return Err(MoeRoutingOutputConsistencyErrorV1::NonMonotoneOffsets {
                expert,
                start,
                end,
            });
        }
        expected_offset += candidate.admitted_counts[expert];
        if end != expected_offset {
            return Err(MoeRoutingOutputConsistencyErrorV1::OffsetMismatch {
                index: expert + 1,
                expected: expected_offset,
                actual: end,
            });
        }
    }
    let compact_pack = MoeExpertCompactPackPlanV1::from_expert_offsets(candidate.expert_offsets)
        .map_err(MoeRoutingOutputConsistencyErrorV1::CompactPlan)?;
    let accepted_routes = candidate.expert_offsets[EXPERTS];

    let mut stable_ranks = [0_u32; EXPERTS];
    let mut seen_slots = [false; ROUTES];
    for route in 0..ROUTES {
        let expert = candidate.top2_experts[route] as usize;
        let stable_rank = stable_ranks[expert];
        stable_ranks[expert] += 1;
        let expected_slot = if stable_rank < EXPERT_CAPACITY {
            candidate.expert_offsets[expert] + stable_rank
        } else {
            DROP_ROUTE
        };
        let actual_slot = candidate.route_slots[route];
        if actual_slot != DROP_ROUTE && actual_slot >= accepted_routes {
            return Err(MoeRoutingOutputConsistencyErrorV1::SlotOutOfRange {
                route,
                slot: actual_slot,
                accepted_routes,
            });
        }
        if actual_slot != DROP_ROUTE {
            let slot = actual_slot as usize;
            if seen_slots[slot] {
                return Err(MoeRoutingOutputConsistencyErrorV1::DuplicateSlot { route, slot });
            }
            seen_slots[slot] = true;
        }
        if actual_slot != expected_slot {
            return Err(MoeRoutingOutputConsistencyErrorV1::StableSlotMismatch {
                route,
                expected: expected_slot,
                actual: actual_slot,
            });
        }
        if candidate.inverse[route] != expected_slot {
            return Err(MoeRoutingOutputConsistencyErrorV1::InverseMismatch {
                route,
                expected: expected_slot,
                actual: candidate.inverse[route],
            });
        }
        if expected_slot != DROP_ROUTE {
            let slot = expected_slot as usize;
            let actual_route = candidate.permutation[slot];
            if actual_route != route as u32 {
                return Err(MoeRoutingOutputConsistencyErrorV1::PermutationMismatch {
                    slot,
                    expected_route: route as u32,
                    actual_route,
                });
            }
        }
    }

    for (slot, seen) in seen_slots
        .iter()
        .copied()
        .enumerate()
        .take(accepted_routes as usize)
    {
        if !seen {
            return Err(MoeRoutingOutputConsistencyErrorV1::AcceptedPrefixHole { slot });
        }
        let route = candidate.permutation[slot];
        if route as usize >= ROUTES {
            return Err(
                MoeRoutingOutputConsistencyErrorV1::PermutationRouteOutOfRange { slot, route },
            );
        }
        if candidate.route_slots[route as usize] != slot as u32
            || candidate.inverse[route as usize] != slot as u32
        {
            return Err(MoeRoutingOutputConsistencyErrorV1::PermutationMismatch {
                slot,
                expected_route: route,
                actual_route: candidate.permutation[slot],
            });
        }
    }
    for slot in accepted_routes as usize..ROUTES {
        if candidate.permutation[slot] != DROP_ROUTE {
            return Err(MoeRoutingOutputConsistencyErrorV1::PermutationTail {
                slot,
                actual: candidate.permutation[slot],
            });
        }
    }

    let payload_sha256 = routing_payload_sha256(&candidate);
    Ok(CheckedMoeHostObservedRoutingOutputV1 {
        payload: candidate,
        compact_pack,
        payload_sha256,
    })
}

fn put_array<const N: usize>(digest: &mut Sha256, name: &[u8], values: &[u32; N]) {
    digest.update((name.len() as u64).to_le_bytes());
    digest.update(name);
    digest.update((N as u64).to_le_bytes());
    for value in values {
        digest.update(value.to_le_bytes());
    }
}

fn routing_payload_sha256(candidate: &MoeRoutingOutputCandidateV1) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(PAYLOAD_DOMAIN);
    put_array(&mut digest, b"top2-experts", &candidate.top2_experts);
    put_array(
        &mut digest,
        b"requested-counts",
        &candidate.requested_counts,
    );
    put_array(&mut digest, b"admitted-counts", &candidate.admitted_counts);
    put_array(&mut digest, b"expert-offsets", &candidate.expert_offsets);
    put_array(&mut digest, b"route-slots", &candidate.route_slots);
    put_array(&mut digest, b"permutation", &candidate.permutation);
    put_array(&mut digest, b"inverse", &candidate.inverse);
    digest.finalize().into()
}

/// One retained offsets/inverse upload derived from the same checked payload.
///
/// Both copies completed synchronously before this value was issued. The
/// witness owns immutable views of both destinations, so safe code cannot
/// mutate or free them while expert preparation retains it. This remains
/// caller-supplied host evidence and grants no copy, load, or dispatch right.
#[must_use = "the routing bridge retains both uploaded device regions"]
pub struct MoeHostObservedRoutingExpertBridgeV1<'offsets, 'inverse> {
    checked: CheckedMoeHostObservedRoutingOutputV1,
    offsets_view: DeviceBufferView<'offsets, u32>,
    inverse_view: DeviceBufferView<'inverse, u32>,
    context_identity: ContextIdentity,
    stream_identity: StreamIdentity,
    offsets_allocation_identity: DeviceBufferIdentity,
    inverse_allocation_identity: DeviceBufferIdentity,
    offsets_region_byte_range: Range<usize>,
    inverse_region_byte_range: Range<usize>,
}

impl fmt::Debug for MoeHostObservedRoutingExpertBridgeV1<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MoeHostObservedRoutingExpertBridgeV1")
            .field("checked", &self.checked)
            .field("context_identity", &self.context_identity)
            .field("stream_identity", &self.stream_identity)
            .field(
                "offsets_allocation_identity",
                &self.offsets_allocation_identity,
            )
            .field(
                "inverse_allocation_identity",
                &self.inverse_allocation_identity,
            )
            .field("offsets_region_byte_range", &self.offsets_region_byte_range)
            .field("inverse_region_byte_range", &self.inverse_region_byte_range)
            .field("producer", &"untrusted host observation")
            .finish_non_exhaustive()
    }
}

impl MoeHostObservedRoutingExpertBridgeV1<'_, '_> {
    pub const fn admitted_counts(&self) -> [u32; EXPERTS] {
        self.checked.admitted_counts()
    }

    pub const fn expert_offsets(&self) -> [u32; EXPERT_OFFSETS] {
        self.checked.expert_offsets()
    }

    pub const fn inverse(&self) -> [u32; ROUTES] {
        self.checked.inverse()
    }

    pub const fn payload_sha256(&self) -> [u8; 32] {
        self.checked.payload_sha256()
    }

    pub const fn compact_pack_plan(&self) -> MoeExpertCompactPackPlanV1 {
        self.checked.compact_pack_plan()
    }

    pub const fn context_identity(&self) -> ContextIdentity {
        self.context_identity
    }

    pub const fn stream_identity(&self) -> StreamIdentity {
        self.stream_identity
    }

    pub const fn offsets_allocation_identity(&self) -> DeviceBufferIdentity {
        self.offsets_allocation_identity
    }

    pub const fn inverse_allocation_identity(&self) -> DeviceBufferIdentity {
        self.inverse_allocation_identity
    }

    pub fn offsets_region_byte_range(&self) -> Range<usize> {
        self.offsets_region_byte_range.clone()
    }

    pub fn inverse_region_byte_range(&self) -> Range<usize> {
        self.inverse_region_byte_range.clone()
    }

    pub const fn upload_completed(&self) -> bool {
        true
    }

    pub const fn producer_is_authenticated(&self) -> bool {
        false
    }

    pub const fn grants_copy_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_dispatch_authority(&self) -> bool {
        false
    }

    pub(crate) const fn offsets_view(&self) -> &DeviceBufferView<'_, u32> {
        &self.offsets_view
    }

    pub(crate) const fn inverse_view(&self) -> &DeviceBufferView<'_, u32> {
        &self.inverse_view
    }
}

/// Uploads exact offsets and inverse arrays from one consumed checked witness.
pub fn upload_checked_moe_routing_expert_bridge_v1<'offsets, 'inverse>(
    stream: &Stream,
    offsets_destination: &'offsets mut DeviceBuffer<u32>,
    inverse_destination: &'inverse mut DeviceBuffer<u32>,
    checked: CheckedMoeHostObservedRoutingOutputV1,
) -> Result<
    MoeHostObservedRoutingExpertBridgeV1<'offsets, 'inverse>,
    MoeRoutingExpertBridgeUploadErrorV1,
> {
    if offsets_destination.len() != EXPERT_OFFSETS {
        return Err(MoeRoutingExpertBridgeUploadErrorV1::OffsetsLength {
            expected: EXPERT_OFFSETS,
            actual: offsets_destination.len(),
        });
    }
    if inverse_destination.len() != ROUTES {
        return Err(MoeRoutingExpertBridgeUploadErrorV1::InverseLength {
            expected: ROUTES,
            actual: inverse_destination.len(),
        });
    }
    let context_identity = stream.context().identity();
    if offsets_destination.context().identity() != context_identity {
        return Err(MoeRoutingExpertBridgeUploadErrorV1::OffsetsContext);
    }
    if inverse_destination.context().identity() != context_identity {
        return Err(MoeRoutingExpertBridgeUploadErrorV1::InverseContext);
    }
    if offsets_destination.allocation_identity() == inverse_destination.allocation_identity() {
        return Err(MoeRoutingExpertBridgeUploadErrorV1::AliasedDestinations);
    }

    let offsets_source =
        PinnedHostBuffer::from_slice(stream.context(), &checked.payload.expert_offsets)
            .map_err(MoeRoutingExpertBridgeUploadErrorV1::OffsetsPinnedSource)?;
    let inverse_source = PinnedHostBuffer::from_slice(stream.context(), &checked.payload.inverse)
        .map_err(MoeRoutingExpertBridgeUploadErrorV1::InversePinnedSource)?;
    BorrowedDeviceOperation::copy_to_device(stream, &offsets_source, offsets_destination, |_| ())
        .map_err(MoeRoutingExpertBridgeUploadErrorV1::OffsetsUpload)?;
    BorrowedDeviceOperation::copy_to_device(stream, &inverse_source, inverse_destination, |_| ())
        .map_err(MoeRoutingExpertBridgeUploadErrorV1::InverseUpload)?;

    let offsets_view = offsets_destination
        .view(..)
        .map_err(MoeRoutingExpertBridgeUploadErrorV1::OffsetsRegion)?;
    let inverse_view = inverse_destination
        .view(..)
        .map_err(MoeRoutingExpertBridgeUploadErrorV1::InverseRegion)?;
    let offsets_allocation_identity = offsets_view.allocation_identity();
    let inverse_allocation_identity = inverse_view.allocation_identity();
    let offsets_region_byte_range = offsets_view.region_byte_range();
    let inverse_region_byte_range = inverse_view.region_byte_range();
    Ok(MoeHostObservedRoutingExpertBridgeV1 {
        checked,
        offsets_view,
        inverse_view,
        context_identity,
        stream_identity: stream.identity(),
        offsets_allocation_identity,
        inverse_allocation_identity,
        offsets_region_byte_range,
        inverse_region_byte_range,
    })
}

/// Rejection before a two-region retained upload witness is issued.
#[derive(Debug)]
#[non_exhaustive]
pub enum MoeRoutingExpertBridgeUploadErrorV1 {
    OffsetsLength { expected: usize, actual: usize },
    InverseLength { expected: usize, actual: usize },
    OffsetsContext,
    InverseContext,
    AliasedDestinations,
    OffsetsPinnedSource(fe2o3_core::Error),
    InversePinnedSource(fe2o3_core::Error),
    OffsetsUpload(fe2o3_core::Error),
    InverseUpload(fe2o3_core::Error),
    OffsetsRegion(DeviceBufferRangeError),
    InverseRegion(DeviceBufferRangeError),
}

impl fmt::Display for MoeRoutingExpertBridgeUploadErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "MoE routing-to-expert upload rejected: {self:?}")
    }
}

impl Error for MoeRoutingExpertBridgeUploadErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn reference_candidate(top2_experts: [u32; ROUTES]) -> MoeRoutingOutputCandidateV1 {
        let mut requested_counts = [0_u32; EXPERTS];
        for expert in top2_experts {
            requested_counts[expert as usize] += 1;
        }
        let admitted_counts = requested_counts.map(|count| count.min(EXPERT_CAPACITY));
        let mut expert_offsets = [0_u32; EXPERT_OFFSETS];
        for expert in 0..EXPERTS {
            expert_offsets[expert + 1] = expert_offsets[expert] + admitted_counts[expert];
        }
        let mut route_slots = [DROP_ROUTE; ROUTES];
        let mut permutation = [DROP_ROUTE; ROUTES];
        let mut inverse = [DROP_ROUTE; ROUTES];
        for (expert, &expert_offset) in expert_offsets.iter().take(EXPERTS).enumerate() {
            for (rank, route) in top2_experts
                .iter()
                .enumerate()
                .filter_map(|(route, selected)| (*selected as usize == expert).then_some(route))
                .enumerate()
                .take(EXPERT_CAPACITY as usize)
            {
                let slot = expert_offset as usize + rank;
                route_slots[route] = slot as u32;
                permutation[slot] = route as u32;
                inverse[route] = slot as u32;
            }
        }
        MoeRoutingOutputCandidateV1::new(
            top2_experts,
            requested_counts,
            admitted_counts,
            expert_offsets,
            route_slots,
            permutation,
            inverse,
        )
    }

    fn repeated(first: u32, second: u32) -> MoeRoutingOutputCandidateV1 {
        let mut top2 = [0_u32; ROUTES];
        for token in 0..TOKENS {
            top2[token * TOP_K] = first;
            top2[token * TOP_K + 1] = second;
        }
        reference_candidate(top2)
    }

    #[test]
    fn all_reachable_requested_count_shapes_have_one_accepted_differential_witness() {
        let pairs: Vec<[u32; 2]> = (0..EXPERTS as u32)
            .flat_map(|first| {
                (0..EXPERTS as u32)
                    .filter(move |second| *second != first)
                    .map(move |second| [first, second])
            })
            .collect();
        let mut states = BTreeMap::from([([0_u32; EXPERTS], Vec::<[u32; 2]>::new())]);
        for _ in 0..TOKENS {
            let mut next = BTreeMap::new();
            for (counts, prefix) in states {
                for pair in &pairs {
                    let mut next_counts = counts;
                    next_counts[pair[0] as usize] += 1;
                    next_counts[pair[1] as usize] += 1;
                    next.entry(next_counts).or_insert_with(|| {
                        let mut routes = prefix.clone();
                        routes.push(*pair);
                        routes
                    });
                }
            }
            states = next;
        }
        assert!(states.len() > 100);
        for (counts, pairs) in states {
            let mut top2 = [0_u32; ROUTES];
            for (token, pair) in pairs.into_iter().enumerate() {
                top2[token * TOP_K..token * TOP_K + TOP_K].copy_from_slice(&pair);
            }
            let candidate = reference_candidate(top2);
            assert_eq!(candidate.requested_counts, counts);
            let _checked = check_host_observed_moe_routing_output_v1(candidate).unwrap();
        }
    }

    #[test]
    fn every_capacity_count_vector_has_the_exact_compact_plan_relation() {
        let mut checked = 0;
        for a in 0..=EXPERT_CAPACITY {
            for b in 0..=EXPERT_CAPACITY {
                for c in 0..=EXPERT_CAPACITY {
                    for d in 0..=EXPERT_CAPACITY {
                        let offsets = [0, a, a + b, a + b + c, a + b + c + d];
                        let plan =
                            MoeExpertCompactPackPlanV1::from_expert_offsets(offsets).unwrap();
                        assert_eq!(plan.accepted_routes(), (a + b + c + d) as usize);
                        assert_eq!(
                            plan.defined_tail_elements(),
                            (ROUTES as u32 - offsets[EXPERTS]) as usize * 16
                        );
                        for (expert, copy) in plan.copies().into_iter().enumerate() {
                            let count = offsets[expert + 1] - offsets[expert];
                            assert_eq!(copy.expert(), expert as u8);
                            assert_eq!(copy.admitted_rows(), count as usize);
                            assert_eq!(
                                copy.compact_element_offset(),
                                offsets[expert] as usize * 16
                            );
                        }
                        checked += 1;
                    }
                }
            }
        }
        assert_eq!(checked, 625);
    }

    #[test]
    fn exact_hostile_mutations_fail_at_named_relations() {
        let base = repeated(0, 1);

        let mut candidate = base;
        candidate.expert_offsets[2] -= 1;
        assert!(matches!(
            check_host_observed_moe_routing_output_v1(candidate),
            Err(MoeRoutingOutputConsistencyErrorV1::OffsetMismatch { .. })
        ));

        let mut candidate = base;
        candidate.admitted_counts[0] -= 1;
        assert!(matches!(
            check_host_observed_moe_routing_output_v1(candidate),
            Err(MoeRoutingOutputConsistencyErrorV1::AdmittedCountMismatch { .. })
        ));

        let mut candidate = base;
        candidate.requested_counts[0] -= 1;
        assert!(matches!(
            check_host_observed_moe_routing_output_v1(candidate),
            Err(MoeRoutingOutputConsistencyErrorV1::RequestedCountMismatch { .. })
        ));

        let mut candidate = base;
        candidate.admitted_counts[0] = EXPERT_CAPACITY + 1;
        assert_eq!(
            check_host_observed_moe_routing_output_v1(candidate).unwrap_err(),
            MoeRoutingOutputConsistencyErrorV1::Capacity {
                expert: 0,
                admitted: EXPERT_CAPACITY + 1,
            }
        );

        let mut candidate = base;
        candidate.expert_offsets[2] = 3;
        assert!(matches!(
            check_host_observed_moe_routing_output_v1(candidate),
            Err(MoeRoutingOutputConsistencyErrorV1::NonMonotoneOffsets { expert: 1, .. })
        ));

        let mut candidate = base;
        candidate.route_slots[1] = candidate.route_slots[0];
        assert!(matches!(
            check_host_observed_moe_routing_output_v1(candidate),
            Err(MoeRoutingOutputConsistencyErrorV1::DuplicateSlot { .. })
        ));

        let mut candidate = base;
        candidate.route_slots[0] = ROUTES as u32;
        assert!(matches!(
            check_host_observed_moe_routing_output_v1(candidate),
            Err(MoeRoutingOutputConsistencyErrorV1::SlotOutOfRange { .. })
        ));

        let mut candidate = base;
        candidate.permutation[0] = 2;
        assert!(matches!(
            check_host_observed_moe_routing_output_v1(candidate),
            Err(MoeRoutingOutputConsistencyErrorV1::PermutationMismatch { .. })
        ));

        let mut candidate = base;
        candidate.inverse[0] = 1;
        assert!(matches!(
            check_host_observed_moe_routing_output_v1(candidate),
            Err(MoeRoutingOutputConsistencyErrorV1::InverseMismatch { .. })
        ));

        let mut candidate = base;
        candidate.route_slots[8] = 0;
        assert_eq!(
            check_host_observed_moe_routing_output_v1(candidate).unwrap_err(),
            MoeRoutingOutputConsistencyErrorV1::DuplicateSlot { route: 8, slot: 0 }
        );

        let mut candidate = base;
        candidate.inverse[8] = 0;
        assert_eq!(
            check_host_observed_moe_routing_output_v1(candidate).unwrap_err(),
            MoeRoutingOutputConsistencyErrorV1::InverseMismatch {
                route: 8,
                expected: DROP_ROUTE,
                actual: 0,
            }
        );

        let mut candidate = base;
        candidate.permutation[8] = 0;
        assert_eq!(
            check_host_observed_moe_routing_output_v1(candidate).unwrap_err(),
            MoeRoutingOutputConsistencyErrorV1::PermutationTail { slot: 8, actual: 0 }
        );
    }

    #[test]
    fn cross_snapshot_substitution_is_rejected() {
        let first = repeated(0, 1);
        let second = repeated(2, 3);

        let mut offsets_substitution = first;
        offsets_substitution.admitted_counts = second.admitted_counts;
        offsets_substitution.expert_offsets = second.expert_offsets;
        assert!(matches!(
            check_host_observed_moe_routing_output_v1(offsets_substitution),
            Err(MoeRoutingOutputConsistencyErrorV1::AdmittedCountMismatch { .. })
        ));

        let reversed = repeated(1, 0);
        let mut inverse_substitution = first;
        inverse_substitution.inverse = reversed.inverse;
        assert!(matches!(
            check_host_observed_moe_routing_output_v1(inverse_substitution),
            Err(MoeRoutingOutputConsistencyErrorV1::InverseMismatch { .. })
        ));
    }

    #[test]
    fn digest_is_domain_separated_and_commits_every_payload_array() {
        let base = repeated(0, 1);
        let digest = routing_payload_sha256(&base);
        assert_ne!(digest, Sha256::digest(b"").as_slice());

        let mutations: [fn(&mut MoeRoutingOutputCandidateV1); 7] = [
            |value| value.top2_experts[0] ^= 1,
            |value| value.requested_counts[0] ^= 1,
            |value| value.admitted_counts[0] ^= 1,
            |value| value.expert_offsets[1] ^= 1,
            |value| value.route_slots[0] ^= 1,
            |value| value.permutation[0] ^= 1,
            |value| value.inverse[0] ^= 1,
        ];
        for mutate in mutations {
            let mut changed = base;
            mutate(&mut changed);
            assert_ne!(routing_payload_sha256(&changed), digest);
        }

        let mut wrong_domain = Sha256::new();
        wrong_domain.update(b"FE2O3/MOE/ROUTING-EXPERT/HOST-OBSERVED-SNAPSHOT/V0\0");
        assert_ne!(wrong_domain.finalize().as_slice(), digest);
    }
}
