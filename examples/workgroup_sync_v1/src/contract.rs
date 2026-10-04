//! Checked host contracts for the fixed LDS-reduction and scoped-atomic profiles.

/// Number of physical lanes admitted by both V1 profiles.
pub const WORKGROUP_LANES_V1: usize = 64;
/// Sole lane allowed to publish the LDS reduction result.
pub const LDS_OWNER_LANE_V1: u32 = 0;

/// Address spaces admitted by the scoped atomic profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicAddressSpaceV1 {
    /// A coherent global-memory atomic object.
    Global,
    /// Workgroup-local LDS, deliberately outside the first atomic profile.
    Workgroup,
}

/// Memory orderings representable by the scoped atomic profile contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicOrderingV1 {
    /// Atomicity without inter-thread ordering beyond the modification order.
    Relaxed,
    /// Acquire-release ordering, deliberately outside the first profile.
    AcquireRelease,
}

/// Visibility scopes representable by the scoped atomic profile contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AtomicScopeV1 {
    /// The complete system scope assigned to ordinary Rust atomics by fe2o3-device.
    System,
    /// A workgroup-only extension, deliberately outside the first profile.
    Workgroup,
}

/// Exact atomic address space admitted by V1.
pub const ATOMIC_ADDRESS_SPACE_V1: AtomicAddressSpaceV1 = AtomicAddressSpaceV1::Global;
/// Exact atomic ordering admitted by V1.
pub const ATOMIC_ORDERING_V1: AtomicOrderingV1 = AtomicOrderingV1::Relaxed;
/// Exact atomic visibility scope admitted by V1.
pub const ATOMIC_SCOPE_V1: AtomicScopeV1 = AtomicScopeV1::System;

/// One lane's authenticated LDS publish/read/barrier trace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReductionLaneV1 {
    /// Physical lane identifier.
    pub lane: u32,
    /// LDS slot initialized by this lane.
    pub publish_slot: u32,
    /// Epoch attached to the published value.
    pub publish_epoch: u32,
    /// Epoch from which this lane reads during reduction.
    pub read_epoch: u32,
    /// Dynamic publish-barrier identity reached by the lane.
    pub publish_barrier: Option<u64>,
    /// Dynamic reuse-barrier identity reached by the lane.
    pub reuse_barrier: Option<u64>,
    /// Whether this lane writes the final global output.
    pub writes_output: bool,
}

/// Rejection from the fixed LDS reduction contract.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ReductionProfileErrorV1 {
    /// The value vector does not contain exactly one element per lane.
    InvalidLaneCount {
        /// Number of supplied values.
        provided: usize,
    },
    /// The trace does not contain exactly one event per lane.
    InvalidTraceCount {
        /// Number of supplied lane events.
        provided: usize,
    },
    /// A trace lane lies outside the fixed workgroup.
    LaneOutOfRange {
        /// Rejected lane identifier.
        lane: u32,
    },
    /// A lane appears more than once in the trace.
    DuplicateLane {
        /// Repeated lane identifier.
        lane: u32,
    },
    /// A lane does not initialize its unique same-index LDS slot.
    WrongPublishSlot {
        /// Lane performing the publish.
        lane: u32,
        /// Incorrect LDS slot.
        slot: u32,
    },
    /// A lane publishes in an unexpected epoch.
    WrongPublishEpoch {
        /// Lane performing the publish.
        lane: u32,
        /// Required epoch.
        expected: u32,
        /// Supplied epoch.
        actual: u32,
    },
    /// A lane reads stale or future LDS contents.
    StaleReadEpoch {
        /// Lane performing the read.
        lane: u32,
        /// Required epoch.
        expected: u32,
        /// Supplied epoch.
        actual: u32,
    },
    /// A lane omits the publish barrier.
    MissingPublishBarrier {
        /// Lane omitting the barrier.
        lane: u32,
    },
    /// Lanes do not reach the same dynamic publish barrier.
    DivergentPublishBarrier {
        /// Lane reaching the wrong barrier.
        lane: u32,
        /// Required barrier identity.
        expected: u64,
        /// Supplied barrier identity.
        actual: u64,
    },
    /// A lane omits the barrier that permits LDS reuse.
    MissingReuseBarrier {
        /// Lane omitting the barrier.
        lane: u32,
    },
    /// Lanes do not reach the same dynamic reuse barrier.
    DivergentReuseBarrier {
        /// Lane reaching the wrong barrier.
        lane: u32,
        /// Required barrier identity.
        expected: u64,
        /// Supplied barrier identity.
        actual: u64,
    },
    /// More than one lane attempts to write the result.
    DuplicateOutputWriter {
        /// First observed writer.
        first: u32,
        /// Second observed writer.
        second: u32,
    },
    /// The only output writer is not the fixed owner.
    WrongOwner {
        /// Observed sole writer, or `None` when no lane writes.
        actual: Option<u32>,
    },
    /// The mathematical sum is outside the exact `i32` profile.
    SumOutOfRange {
        /// Mathematical sum that cannot be represented by `i32`.
        sum: i64,
    },
    /// The caller did not provide exactly one output slot.
    InvalidOutputLength {
        /// Number of supplied output slots.
        provided: usize,
    },
}

/// Output mismatch for the LDS reduction profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReductionComparisonErrorV1 {
    /// Oracle result.
    pub expected: i32,
    /// Candidate result.
    pub actual: i32,
}

/// Constructs the canonical 64-lane trace for one LDS epoch.
pub fn canonical_reduction_trace_v1(epoch: u32) -> Vec<ReductionLaneV1> {
    let publish_barrier = u64::from(epoch) * 2;
    let reuse_barrier = publish_barrier + 1;
    (0..WORKGROUP_LANES_V1 as u32)
        .map(|lane| ReductionLaneV1 {
            lane,
            publish_slot: lane,
            publish_epoch: epoch,
            read_epoch: epoch,
            publish_barrier: Some(publish_barrier),
            reuse_barrier: Some(reuse_barrier),
            writes_output: lane == LDS_OWNER_LANE_V1,
        })
        .collect()
}

/// Validates and computes the exact mathematical sum for one LDS epoch.
///
/// Every check completes before `output` is modified. Integer overflow is
/// rejected so the wrapping device addition is equal to this mathematical sum.
pub fn lds_reduction_oracle_v1(
    values: &[i32],
    epoch: u32,
    trace: &[ReductionLaneV1],
    output: &mut [i32],
) -> Result<i32, ReductionProfileErrorV1> {
    if values.len() != WORKGROUP_LANES_V1 {
        return Err(ReductionProfileErrorV1::InvalidLaneCount {
            provided: values.len(),
        });
    }
    if trace.len() != WORKGROUP_LANES_V1 {
        return Err(ReductionProfileErrorV1::InvalidTraceCount {
            provided: trace.len(),
        });
    }
    if output.len() != 1 {
        return Err(ReductionProfileErrorV1::InvalidOutputLength {
            provided: output.len(),
        });
    }

    let expected_publish_barrier = u64::from(epoch) * 2;
    let expected_reuse_barrier = expected_publish_barrier + 1;
    let mut seen = [false; WORKGROUP_LANES_V1];
    let mut writer = None;
    for event in trace {
        let lane = event.lane as usize;
        if lane >= WORKGROUP_LANES_V1 {
            return Err(ReductionProfileErrorV1::LaneOutOfRange { lane: event.lane });
        }
        if seen[lane] {
            return Err(ReductionProfileErrorV1::DuplicateLane { lane: event.lane });
        }
        seen[lane] = true;
        if event.publish_slot != event.lane {
            return Err(ReductionProfileErrorV1::WrongPublishSlot {
                lane: event.lane,
                slot: event.publish_slot,
            });
        }
        if event.publish_epoch != epoch {
            return Err(ReductionProfileErrorV1::WrongPublishEpoch {
                lane: event.lane,
                expected: epoch,
                actual: event.publish_epoch,
            });
        }
        if event.read_epoch != epoch {
            return Err(ReductionProfileErrorV1::StaleReadEpoch {
                lane: event.lane,
                expected: epoch,
                actual: event.read_epoch,
            });
        }
        let Some(publish_barrier) = event.publish_barrier else {
            return Err(ReductionProfileErrorV1::MissingPublishBarrier { lane: event.lane });
        };
        if publish_barrier != expected_publish_barrier {
            return Err(ReductionProfileErrorV1::DivergentPublishBarrier {
                lane: event.lane,
                expected: expected_publish_barrier,
                actual: publish_barrier,
            });
        }
        let Some(reuse_barrier) = event.reuse_barrier else {
            return Err(ReductionProfileErrorV1::MissingReuseBarrier { lane: event.lane });
        };
        if reuse_barrier != expected_reuse_barrier {
            return Err(ReductionProfileErrorV1::DivergentReuseBarrier {
                lane: event.lane,
                expected: expected_reuse_barrier,
                actual: reuse_barrier,
            });
        }
        if event.writes_output {
            if let Some(first) = writer {
                return Err(ReductionProfileErrorV1::DuplicateOutputWriter {
                    first,
                    second: event.lane,
                });
            }
            writer = Some(event.lane);
        }
    }
    if writer != Some(LDS_OWNER_LANE_V1) {
        return Err(ReductionProfileErrorV1::WrongOwner { actual: writer });
    }

    let sum = values.iter().map(|&value| i64::from(value)).sum::<i64>();
    let sum = i32::try_from(sum).map_err(|_| ReductionProfileErrorV1::SumOutOfRange { sum })?;
    output[0] = sum;
    Ok(sum)
}

/// Requires exact equality between the LDS oracle and a candidate output.
pub fn compare_reduction_output_v1(
    expected: i32,
    actual: i32,
) -> Result<(), ReductionComparisonErrorV1> {
    if expected == actual {
        Ok(())
    } else {
        Err(ReductionComparisonErrorV1 { expected, actual })
    }
}

/// Exact address-space, ordering, scope, and target contract for atomic add.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicProfileV1 {
    /// Number of participating physical lanes.
    pub lane_count: u32,
    /// Address space containing the atomic object.
    pub address_space: AtomicAddressSpaceV1,
    /// Ordering used by every `fetch_add`.
    pub ordering: AtomicOrderingV1,
    /// Visibility scope of every atomic operation.
    pub scope: AtomicScopeV1,
    /// Sole atomic-object index modified by this profile.
    pub target_index: u32,
}

/// One lane's eligibility and contribution declaration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicLaneV1 {
    /// Physical lane identifier.
    pub lane: u32,
    /// Whether this lane contributes exactly once.
    pub eligible: bool,
    /// Value added when this lane is eligible.
    pub value: u32,
}

/// Rejection from the fixed scoped atomic profile.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AtomicProfileErrorV1 {
    /// The profile does not name exactly 64 lanes.
    InvalidLaneCount {
        /// Declared number of participating lanes.
        provided: u32,
    },
    /// The lane vector does not contain exactly 64 declarations.
    InvalidLaneVectorLength {
        /// Number of supplied lane declarations.
        provided: usize,
    },
    /// A lane identifier lies outside the fixed workgroup.
    LaneOutOfRange {
        /// Rejected lane identifier.
        lane: u32,
    },
    /// A lane appears more than once.
    DuplicateLane {
        /// Repeated lane identifier.
        lane: u32,
    },
    /// The atomic object is not in coherent global memory.
    WrongAddressSpace {
        /// Supplied address space.
        actual: AtomicAddressSpaceV1,
    },
    /// The operation is not relaxed atomic add.
    WrongOrdering {
        /// Supplied memory ordering.
        actual: AtomicOrderingV1,
    },
    /// The operation does not use fe2o3's system scope for Rust atomics.
    WrongScope {
        /// Supplied visibility scope.
        actual: AtomicScopeV1,
    },
    /// The profile targets an unsupported object index.
    WrongTargetIndex {
        /// Supplied atomic-object index.
        actual: u32,
    },
    /// The exact mathematical result exceeds `u32`.
    SumOutOfRange {
        /// Mathematical sum that cannot be represented by `u32`.
        sum: u64,
    },
    /// The caller did not provide exactly one output slot.
    InvalidOutputLength {
        /// Number of supplied output slots.
        provided: usize,
    },
}

/// Output mismatch for the scoped atomic profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtomicComparisonErrorV1 {
    /// Oracle result.
    pub expected: u32,
    /// Candidate result.
    pub actual: u32,
}

/// Returns the only atomic metadata profile admitted by V1.
pub const fn canonical_atomic_profile_v1() -> AtomicProfileV1 {
    AtomicProfileV1 {
        lane_count: WORKGROUP_LANES_V1 as u32,
        address_space: ATOMIC_ADDRESS_SPACE_V1,
        ordering: ATOMIC_ORDERING_V1,
        scope: ATOMIC_SCOPE_V1,
        target_index: 0,
    }
}

/// Constructs lane declarations from parallel values and eligibility bits.
pub fn canonical_atomic_lanes_v1(values: &[u32], eligible: &[bool]) -> Vec<AtomicLaneV1> {
    values
        .iter()
        .copied()
        .zip(eligible.iter().copied())
        .enumerate()
        .map(|(lane, (value, eligible))| AtomicLaneV1 {
            lane: lane as u32,
            eligible,
            value,
        })
        .collect()
}

/// Validates and computes the deterministic final value of 64 atomic adds.
///
/// Eligible lanes contribute exactly once; ineligible lanes contribute zero.
/// Interleaving affects returned old values but not the final exact sum. Every
/// validation completes before `output` is modified.
pub fn atomic_add_oracle_v1(
    initial: u32,
    profile: AtomicProfileV1,
    lanes: &[AtomicLaneV1],
    output: &mut [u32],
) -> Result<u32, AtomicProfileErrorV1> {
    if profile.lane_count != WORKGROUP_LANES_V1 as u32 {
        return Err(AtomicProfileErrorV1::InvalidLaneCount {
            provided: profile.lane_count,
        });
    }
    if profile.address_space != ATOMIC_ADDRESS_SPACE_V1 {
        return Err(AtomicProfileErrorV1::WrongAddressSpace {
            actual: profile.address_space,
        });
    }
    if profile.ordering != ATOMIC_ORDERING_V1 {
        return Err(AtomicProfileErrorV1::WrongOrdering {
            actual: profile.ordering,
        });
    }
    if profile.scope != ATOMIC_SCOPE_V1 {
        return Err(AtomicProfileErrorV1::WrongScope {
            actual: profile.scope,
        });
    }
    if profile.target_index != 0 {
        return Err(AtomicProfileErrorV1::WrongTargetIndex {
            actual: profile.target_index,
        });
    }
    if lanes.len() != WORKGROUP_LANES_V1 {
        return Err(AtomicProfileErrorV1::InvalidLaneVectorLength {
            provided: lanes.len(),
        });
    }
    if output.len() != 1 {
        return Err(AtomicProfileErrorV1::InvalidOutputLength {
            provided: output.len(),
        });
    }

    let mut seen = [false; WORKGROUP_LANES_V1];
    let mut sum = u64::from(initial);
    for lane in lanes {
        let index = lane.lane as usize;
        if index >= WORKGROUP_LANES_V1 {
            return Err(AtomicProfileErrorV1::LaneOutOfRange { lane: lane.lane });
        }
        if seen[index] {
            return Err(AtomicProfileErrorV1::DuplicateLane { lane: lane.lane });
        }
        seen[index] = true;
        if lane.eligible {
            sum += u64::from(lane.value);
        }
    }
    let result = u32::try_from(sum).map_err(|_| AtomicProfileErrorV1::SumOutOfRange { sum })?;
    output[0] = result;
    Ok(result)
}

/// Requires exact equality between the atomic oracle and a candidate output.
pub fn compare_atomic_output_v1(expected: u32, actual: u32) -> Result<(), AtomicComparisonErrorV1> {
    if expected == actual {
        Ok(())
    } else {
        Err(AtomicComparisonErrorV1 { expected, actual })
    }
}
