//! Private atomic serialization evidence for the existing byte-level frontier.
//!
//! One simulation is one device/launch. A generic scalar target does not choose
//! a subgroup width; workgroup size alone must not be used to invent one.
use fe2o3_kernel_ir::SynchronizationScope;

use crate::SimulationInvocationV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Relation {
    Serialized,
    OutsideScope,
    Unproved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct AtomicAccess {
    scope: SynchronizationScope,
    // Zero bytes is the canonical unknown-range sentinel, never a real access.
    // Keeping the optional range out of the stored representation avoids a
    // discriminant word in every byte-frontier representative.
    offset: usize,
    bytes: usize,
}

impl AtomicAccess {
    pub(super) fn new(scope: SynchronizationScope, offset: usize, bytes: usize) -> Self {
        let (offset, bytes) = (bytes != 0 && offset.checked_add(bytes).is_some())
            .then_some((offset, bytes))
            .unwrap_or((0, 0));
        Self {
            scope,
            offset,
            bytes,
        }
    }

    fn range(self) -> Option<(usize, usize)> {
        (self.bytes != 0).then_some((self.offset, self.bytes))
    }

    // Every access represented by one same-invocation frontier cell must admit
    // a later serialization. A wider later scope cannot erase a narrower one.
    pub(super) fn merge(self, other: Self) -> Self {
        let (offset, bytes) = self
            .range()
            .filter(|range| Some(*range) == other.range())
            .unwrap_or((0, 0));
        Self {
            scope: self.scope.min(other.scope),
            offset,
            bytes,
        }
    }

    pub(super) fn relation(
        self,
        earlier: SimulationInvocationV1,
        other: Self,
        later: SimulationInvocationV1,
    ) -> Relation {
        if self.range().is_none() || self.range() != other.range() {
            return Relation::Unproved;
        }
        let left = contains(self.scope, earlier, later);
        let right = contains(other.scope, later, earlier);
        match (left, right) {
            (Some(false), _) | (_, Some(false)) => Relation::OutsideScope,
            (Some(true), Some(true)) => Relation::Serialized,
            _ => Relation::Unproved,
        }
    }

    pub(super) fn is_narrower_than(self, other: Self) -> bool {
        self.scope.rank() < other.scope.rank()
    }
}

fn contains(
    scope: SynchronizationScope,
    owner: SimulationInvocationV1,
    other: SimulationInvocationV1,
) -> Option<bool> {
    if owner == other {
        return Some(true);
    }
    match scope {
        SynchronizationScope::Invocation => Some(false),
        SynchronizationScope::Subgroup => {
            if owner.workgroup != other.workgroup {
                Some(false)
            } else {
                None
            }
        }
        SynchronizationScope::Workgroup => Some(owner.workgroup == other.workgroup),
        SynchronizationScope::Device | SynchronizationScope::System => Some(true),
    }
}

// A bounded summary of all evicted atomic representatives. It records both the
// narrowest scope and the workgroup membership of *every* lost invocation;
// retaining only the last invocation would permit a later narrowed scope to
// erase an earlier cross-workgroup conflict.
#[derive(Clone, Copy, Debug)]
pub(super) struct AtomicHistory {
    range: (usize, usize),
    scope: SynchronizationScope,
    // The explicit known bit shares the scope's padding rather than storing an
    // Option discriminant word. Unknown membership stays unknown after merges.
    common_workgroup: [u64; 3],
    common_workgroup_known: bool,
}

impl AtomicHistory {
    pub(super) fn new(access: AtomicAccess, invocation: SimulationInvocationV1) -> Option<Self> {
        if access.scope.rank() < SynchronizationScope::Workgroup.rank() {
            return None;
        }
        Some(Self {
            range: access.range()?,
            scope: access.scope,
            common_workgroup: invocation.workgroup,
            common_workgroup_known: true,
        })
    }

    pub(super) fn include(self, other: Self) -> Option<Self> {
        if self.range != other.range {
            return None;
        }
        let common_workgroup_known = self.common_workgroup_known
            && other.common_workgroup_known
            && self.common_workgroup == other.common_workgroup;
        Some(Self {
            range: self.range,
            scope: self.scope.min(other.scope),
            common_workgroup: if common_workgroup_known {
                self.common_workgroup
            } else {
                [0; 3]
            },
            common_workgroup_known,
        })
    }

    pub(super) fn serializes_with(
        self,
        access: AtomicAccess,
        invocation: SimulationInvocationV1,
    ) -> bool {
        if access.range() != Some(self.range)
            || access.scope.rank() < SynchronizationScope::Workgroup.rank()
        {
            return false;
        }
        let same_group =
            self.common_workgroup_known && self.common_workgroup == invocation.workgroup;
        (self.scope.rank() >= SynchronizationScope::Device.rank() || same_group)
            && (access.scope.rank() >= SynchronizationScope::Device.rank() || same_group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation(group: u64, local: u32) -> SimulationInvocationV1 {
        SimulationInvocationV1 {
            global: [group * 128 + u64::from(local), 0, 0],
            workgroup: [group, 0, 0],
            local: [local, 0, 0],
            workgroup_size: [128, 1, 1],
            workgroup_count: [2, 1, 1],
            launch_extent: [256, 1, 1],
        }
    }

    fn access(scope: SynchronizationScope) -> AtomicAccess {
        AtomicAccess::new(scope, 0, 4)
    }

    #[test]
    fn atomic_scope_reciprocity_and_unknown_subgroups_are_distinct() {
        use SynchronizationScope::*;
        for (left, right, group, expected) in [
            (System, Device, 1, Relation::Serialized),
            (Device, System, 1, Relation::Serialized),
            (Workgroup, Device, 0, Relation::Serialized),
            (Device, Workgroup, 0, Relation::Serialized),
            (Workgroup, Device, 1, Relation::OutsideScope),
            (Device, Workgroup, 1, Relation::OutsideScope),
            (Subgroup, System, 1, Relation::OutsideScope),
            (System, Subgroup, 1, Relation::OutsideScope),
            (Subgroup, System, 0, Relation::Unproved),
            (System, Subgroup, 0, Relation::Unproved),
            (Invocation, System, 0, Relation::OutsideScope),
        ] {
            assert_eq!(
                access(left).relation(invocation(0, 0), access(right), invocation(group, 1)),
                expected,
            );
        }
    }

    #[test]
    fn atomic_scope_different_and_partially_overlapping_ranges_are_unproved() {
        let first = access(SynchronizationScope::System);
        for (offset, bytes) in [(0, 8), (2, 4), (4, 4), (0, 0), (usize::MAX, 4)] {
            assert_eq!(
                first.relation(
                    invocation(0, 0),
                    AtomicAccess::new(SynchronizationScope::System, offset, bytes),
                    invocation(0, 1)
                ),
                Relation::Unproved,
            );
        }
    }

    #[test]
    fn atomic_scope_merge_cannot_widen_or_restore_a_lost_range() {
        use SynchronizationScope::*;
        let narrow = access(Workgroup);
        let wide = access(System);
        for merged in [narrow.merge(wide), wide.merge(narrow)] {
            assert_eq!(
                merged.relation(invocation(0, 0), wide, invocation(1, 0)),
                Relation::OutsideScope
            );
        }
        let mixed = wide.merge(AtomicAccess::new(System, 0, 8));
        assert_eq!(
            mixed
                .merge(wide)
                .relation(invocation(0, 0), wide, invocation(0, 1)),
            Relation::Unproved
        );
    }

    #[test]
    fn atomic_scope_eviction_keeps_all_prior_workgroup_memberships() {
        use SynchronizationScope::*;
        let one = AtomicHistory::new(access(System), invocation(0, 0)).unwrap();
        let two = one
            .include(AtomicHistory::new(access(Device), invocation(1, 0)).unwrap())
            .unwrap();
        assert!(two.serializes_with(access(System), invocation(1, 1)));
        assert!(!two.serializes_with(access(Workgroup), invocation(1, 1)));
        let narrow = one
            .include(AtomicHistory::new(access(Workgroup), invocation(0, 1)).unwrap())
            .unwrap();
        assert!(narrow.serializes_with(access(Workgroup), invocation(0, 2)));
        assert!(!narrow.serializes_with(access(System), invocation(1, 2)));
        assert!(AtomicHistory::new(access(Subgroup), invocation(0, 0)).is_none());
        assert!(
            one.include(
                AtomicHistory::new(AtomicAccess::new(System, 0, 8), invocation(0, 1)).unwrap()
            )
            .is_none()
        );
    }

    #[test]
    fn atomic_scope_compact_unknown_ranges_are_canonical_and_irrecoverable() {
        use SynchronizationScope::System;
        let valid = AtomicAccess::new(System, 0, 4);
        assert_eq!(valid.range(), Some((0, 4)));
        assert_eq!(
            AtomicAccess::new(System, usize::MAX - 4, 4).range(),
            Some((usize::MAX - 4, 4)),
        );
        for (offset, bytes) in [(0, 0), (9, 0), (usize::MAX, 1), (usize::MAX - 1, 2)] {
            let unknown = AtomicAccess::new(System, offset, bytes);
            assert_eq!((unknown.offset, unknown.bytes), (0, 0));
            assert!(unknown.range().is_none());
            assert!(AtomicHistory::new(unknown, invocation(0, 0)).is_none());
            for merged in [
                unknown.merge(valid),
                valid.merge(unknown),
                unknown.merge(unknown),
            ] {
                assert_eq!((merged.offset, merged.bytes), (0, 0));
                assert_eq!(
                    merged.relation(invocation(0, 0), valid, invocation(0, 1)),
                    Relation::Unproved,
                );
            }
        }
        let mixed = valid.merge(AtomicAccess::new(System, 2, 4));
        assert_eq!((mixed.offset, mixed.bytes), (0, 0));
        assert_eq!(
            (mixed.merge(valid).offset, mixed.merge(valid).bytes),
            (0, 0)
        );
    }

    #[test]
    fn atomic_scope_compact_unknown_membership_cannot_be_restored() {
        use SynchronizationScope::*;
        let one = AtomicHistory::new(access(System), invocation(0, 0)).unwrap();
        let other = AtomicHistory::new(access(System), invocation(1, 0)).unwrap();
        let mixed = one.include(other).unwrap();
        for merged in [
            mixed,
            mixed.include(one).unwrap(),
            one.include(mixed).unwrap(),
        ] {
            assert!(!merged.common_workgroup_known);
            assert_eq!(merged.common_workgroup, [0; 3]);
            for group in [0, 1] {
                assert!(merged.serializes_with(access(System), invocation(group, 1)));
                assert!(!merged.serializes_with(access(Workgroup), invocation(group, 1)));
            }
        }
        let narrow = AtomicHistory::new(access(Workgroup), invocation(0, 0)).unwrap();
        let mixed_narrow = narrow.include(other).unwrap().include(one).unwrap();
        assert!(!mixed_narrow.common_workgroup_known);
        assert!(!mixed_narrow.serializes_with(access(System), invocation(0, 1)));
        assert!(!mixed_narrow.serializes_with(access(System), invocation(1, 1)));
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn atomic_scope_private_evidence_layout_remains_compact() {
        use std::mem::size_of;
        assert_eq!(size_of::<AtomicAccess>(), 24);
        assert_eq!(size_of::<Option<AtomicAccess>>(), 24);
        assert_eq!(size_of::<AtomicHistory>(), 48);
        assert_eq!(size_of::<Option<AtomicHistory>>(), 48);
    }
}
