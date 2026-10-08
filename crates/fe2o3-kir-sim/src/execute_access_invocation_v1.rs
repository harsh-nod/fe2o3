//! Lossless private access-history identity within one Engine.
//!
//! Engine construction fixes the workgroup size/count for the entire request.
//! Every selected invocation originates in that Engine's plan or an immutable
//! InvocationMachine copy, including calls, collectives and later workgroups.
//! Access history never leaves that Engine. Store the varying identity here and
//! restore only with record_access's current invocation from the same Engine.
//! This is not a public identity, codec, or cross-request comparison key.
use super::SimulationInvocationV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AccessInvocationV1 {
    global: [u64; 3],
    workgroup: [u64; 3],
    local: [u32; 3],
    launch_extent: [u64; 3],
}

impl AccessInvocationV1 {
    pub(super) fn store(invocation: SimulationInvocationV1) -> Self {
        Self {
            global: invocation.global,
            workgroup: invocation.workgroup,
            local: invocation.local,
            launch_extent: invocation.launch_extent,
        }
    }

    /// The context must be the current invocation of the same Engine that
    /// stored this record. Only its two immutable launch-shape fields are used.
    pub(super) fn restore(self, context: SimulationInvocationV1) -> SimulationInvocationV1 {
        SimulationInvocationV1 {
            global: self.global,
            workgroup: self.workgroup,
            local: self.local,
            workgroup_size: context.workgroup_size,
            workgroup_count: context.workgroup_count,
            launch_extent: self.launch_extent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AccessFrontier, AtomicAccess, AtomicHistory, CompactSite, LastAccess};
    use super::*;
    use crate::resident::reserved_hash_map_bytes;
    use std::mem::size_of;

    fn invocation(global: [u64; 3], size: [u32; 3], extent: [u64; 3]) -> SimulationInvocationV1 {
        SimulationInvocationV1 {
            global,
            workgroup: std::array::from_fn(|axis| global[axis] / u64::from(size[axis])),
            local: std::array::from_fn(|axis| {
                u32::try_from(global[axis] % u64::from(size[axis])).unwrap()
            }),
            workgroup_size: size,
            workgroup_count: std::array::from_fn(|axis| {
                extent[axis].div_ceil(u64::from(size[axis]))
            }),
            launch_extent: extent,
        }
    }

    #[test]
    fn round_trip_retains_all_six_public_arrays_for_three_dimensional_tails() {
        for (size, extent) in [
            ([1, 1, 1], [3, 2, 2]),
            ([2, 3, 4], [5, 7, 9]),
            ([4, 2, 1], [7, 3, 2]),
        ] {
            for z in 0..extent[2] {
                for y in 0..extent[1] {
                    for x in 0..extent[0] {
                        let full = invocation([x, y, z], size, extent);
                        assert_eq!(AccessInvocationV1::store(full).restore(full), full);
                    }
                }
            }
        }
    }

    #[test]
    fn later_workgroup_context_restores_earlier_identity_not_later_coordinates() {
        for (size, extent) in [
            ([1, 1, 1], [3, 2, 2]),
            ([2, 3, 4], [5, 7, 9]),
            ([4, 2, 1], [7, 3, 2]),
        ] {
            let earlier = invocation([0, 1, 1], size, extent);
            let later = invocation(extent.map(|value| value - 1), size, extent);
            assert_ne!(earlier.global, later.global);
            assert_ne!(earlier.workgroup, later.workgroup);
            let stored = AccessInvocationV1::store(earlier);
            assert_eq!(stored.restore(later), earlier);
            assert_eq!(stored.restore(later).workgroup_size, size);
            assert_eq!(
                stored.restore(later).workgroup_count,
                earlier.workgroup_count
            );
        }
    }

    #[test]
    fn each_varying_identity_component_remains_distinct_without_narrowing() {
        let original = SimulationInvocationV1 {
            global: [u64::MAX, 17, 29],
            workgroup: [u64::MAX - 1, 11, 13],
            local: [u32::MAX, 3, 5],
            workgroup_size: [7, 11, 13],
            workgroup_count: [u64::MAX, 19, 23],
            launch_extent: [u64::MAX, 31, 37],
        };
        let stored = AccessInvocationV1::store(original);
        assert_eq!(stored.restore(original), original);
        for axis in 0..3 {
            for field in 0..4 {
                let mut changed = original;
                match field {
                    0 => changed.global[axis] ^= 1,
                    1 => changed.workgroup[axis] ^= 1,
                    2 => changed.local[axis] ^= 1,
                    3 => changed.launch_extent[axis] ^= 1,
                    _ => unreachable!(),
                }
                let other = AccessInvocationV1::store(changed);
                assert_ne!(stored, other);
                assert_eq!(other.restore(original), changed);
            }
        }
    }

    // Exact old field shapes are retained only to measure the representation
    // reduction. The live accountant continues to use the actual AccessFrontier.
    #[allow(dead_code)]
    #[derive(Clone, Copy)]
    struct FullLastAccess {
        invocation: SimulationInvocationV1,
        site: CompactSite,
        atomic: Option<AtomicAccess>,
        happens_before_epoch: u64,
    }

    #[allow(dead_code)]
    struct FullAccessFrontier {
        write: Option<FullLastAccess>,
        displaced_write: Option<FullLastAccess>,
        read: Option<FullLastAccess>,
        displaced_read: Option<FullLastAccess>,
        conflicted: bool,
        raced: bool,
        incomplete: bool,
        lost_write: bool,
        lost_writes_atomic: Option<AtomicHistory>,
        lost_read: bool,
        lost_reads_atomic: Option<AtomicHistory>,
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn actual_layout_removes_only_repeated_launch_shape_storage() {
        assert_eq!(size_of::<SimulationInvocationV1>(), 120);
        assert_eq!(size_of::<AccessInvocationV1>(), 88);
        assert_eq!(
            size_of::<FullLastAccess>().checked_sub(size_of::<LastAccess>()),
            Some(32)
        );
        assert_eq!(
            size_of::<Option<FullLastAccess>>().checked_sub(size_of::<Option<LastAccess>>()),
            Some(32)
        );
        assert_eq!(
            size_of::<FullAccessFrontier>().checked_sub(size_of::<AccessFrontier>()),
            Some(128)
        );
    }

    #[test]
    fn resident_quotes_use_real_smaller_cells_at_unchanged_record_counts() {
        for records in [1, 17, 257, 65_536] {
            let full = reserved_hash_map_bytes::<(u64, usize), FullAccessFrontier>(records)
                .expect("bounded full-identity table");
            let compact = reserved_hash_map_bytes::<(u64, usize), AccessFrontier>(records)
                .expect("bounded compact-identity table");
            assert!(full.checked_sub(compact).is_some_and(|saved| saved > 0));
        }
    }
}
