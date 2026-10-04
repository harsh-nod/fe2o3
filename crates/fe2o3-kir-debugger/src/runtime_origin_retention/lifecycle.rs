//! Private same-execution lifecycle observation, not a durable trace schema.
//! Fresh raw allocation IDs are not reusable storage-slot generations.
//! Only capture.rs can assemble a session from transcript and lifecycle rows.
use super::session::{ObservedSession, ReplayWork, SessionError};
use super::*;
use fe2o3_kernel_ir::AddressSpace;
use fe2o3_kir_sim::{SimulationEventSiteV1, SimulationInvocationV1};

#[path = "lifecycle_capture.rs"]
mod capture;
#[path = "lifecycle_controls_tests.rs"]
mod controls_tests;
#[path = "lifecycle_fixtures_tests.rs"]
mod fixtures;
#[path = "lifecycle_runtime_tests.rs"]
mod runtime_tests;
#[path = "lifecycle_source_tests.rs"]
mod source_tests;

use capture::{QueryError, State, capture};

const MAX_LIFECYCLE_ROWS: usize = 65_536;
const MAX_LIFECYCLE_BYTES: usize = 16 * 1024 * 1024;
const MAX_CAPTURE_WORK: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LifecycleLimits {
    rows: usize,
    bytes: usize,
    work: usize,
}
impl LifecycleLimits {
    fn new(rows: usize, bytes: usize, work: usize) -> Result<Self, &'static str> {
        if !(1..=MAX_LIFECYCLE_ROWS).contains(&rows)
            || !(size_of::<Ledger>() + size_of::<Transition>()..=MAX_LIFECYCLE_BYTES)
                .contains(&bytes)
            || !(1..=MAX_CAPTURE_WORK).contains(&work)
        {
            return Err("lifecycle observation limits");
        }
        Ok(Self { rows, bytes, work })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Gap {
    Disabled,
    RowLimit,
    ByteLimit,
    WorkLimit,
    AllocationFailure,
    InvalidCapacity,
    InvalidProducer,
    DebugPrefix,
    ExecutionFailure,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Preexisting,
    Create,
    Release,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Scope {
    Dispatch,
    Workgroup {
        coordinate: [u64; 3],
        size: [u32; 3],
        count: [u64; 3],
        launch: [u64; 3],
    },
    Invocation(SimulationInvocationV1),
}
impl Scope {
    fn created(address_space: AddressSpace, invocation: SimulationInvocationV1) -> Self {
        if address_space == AddressSpace::Workgroup {
            Self::Workgroup {
                coordinate: invocation.workgroup,
                size: invocation.workgroup_size,
                count: invocation.workgroup_count,
                launch: invocation.launch_extent,
            }
        } else {
            Self::Invocation(invocation)
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Transition {
    /// Apply before this accepted debug-record index; N is the terminal boundary.
    boundary: usize,
    allocation: u64,
    action: Action,
    scope: Scope,
    address_space: AddressSpace,
    bytes: usize,
    site: SimulationEventSiteV1,
}
const _: () = assert!(size_of::<Transition>() <= 192);

struct Ledger {
    rows: Vec<Transition>,
    limit: usize,
    work_left: usize,
    capacity_gap: Gap,
    gap: Option<(usize, Gap)>,
    last_allocation: u64,
}
impl Ledger {
    fn new(limits: LifecycleLimits, enabled: bool) -> Self {
        let count = limits
            .rows
            .min((limits.bytes - size_of::<Self>()) / size_of::<Transition>());
        let mut value = Self {
            rows: Vec::new(),
            limit: count,
            work_left: limits.work,
            capacity_gap: if count == limits.rows {
                Gap::RowLimit
            } else {
                Gap::ByteLimit
            },
            gap: (!enabled).then_some((0, Gap::Disabled)),
            last_allocation: 0,
        };
        if !enabled {
            return value;
        }
        if value.rows.try_reserve_exact(count).is_err() {
            value.fail(0, Gap::AllocationFailure);
        } else {
            let bytes = value
                .rows
                .capacity()
                .checked_mul(size_of::<Transition>())
                .and_then(|bytes| bytes.checked_add(size_of::<Self>()));
            if value.rows.capacity() < count || bytes.is_none_or(|bytes| bytes > limits.bytes) {
                value.rows = Vec::new();
                value.fail(0, Gap::InvalidCapacity);
            }
        }
        value
    }
    fn fail(&mut self, boundary: usize, reason: Gap) {
        if self.gap.is_none() {
            self.gap = Some((boundary, reason));
        }
    }
    fn charge(&mut self, boundary: usize, work: usize) -> bool {
        match self.work_left.checked_sub(work) {
            Some(left) => {
                self.work_left = left;
                true
            }
            None => {
                self.fail(boundary, Gap::WorkLimit);
                false
            }
        }
    }
    fn metadata_bytes(&self) -> usize {
        size_of::<Self>() + self.rows.capacity() * size_of::<Transition>()
    }
}
