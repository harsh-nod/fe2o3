use crate::{ThreadId1d, ThreadInDomain1d};

/// Runtime evidence that an element index is below a specific length.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundedIndex {
    value: usize,
    len: usize,
}

impl BoundedIndex {
    pub const fn new(value: usize, len: usize) -> Option<Self> {
        if value < len {
            Some(Self { value, len })
        } else {
            None
        }
    }

    pub const fn value(self) -> usize {
        self.value
    }

    pub const fn bound(self) -> usize {
        self.len
    }
}

/// A bounded output index whose location is the owning thread's linear ID.
///
/// Because callers cannot construct this type for another mapping, two values
/// owned by distinct logical threads necessarily identify distinct locations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentityWriteIndex {
    owner: ThreadId1d,
    index: BoundedIndex,
}

impl IdentityWriteIndex {
    pub const fn new(thread: ThreadInDomain1d, output_len: usize) -> Option<Self> {
        match BoundedIndex::new(thread.linear(), output_len) {
            Some(index) => Some(Self {
                owner: thread.id(),
                index,
            }),
            None => None,
        }
    }

    pub const fn owner(self) -> ThreadId1d {
        self.owner
    }

    pub const fn index(self) -> BoundedIndex {
        self.index
    }

    pub const fn is_disjoint_from(self, other: Self) -> bool {
        self.index.value != other.index.value
    }

    /// The executable form of the identity mapping's injectivity contract.
    pub const fn mapping_is_injective_with(self, other: Self) -> bool {
        self.owner.linear() == other.owner.linear() || self.is_disjoint_from(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LaunchDomain1d;

    #[test]
    fn identity_write_is_bounded_by_output() {
        let thread = LaunchDomain1d::new(8).thread(5).unwrap();

        let write = IdentityWriteIndex::new(thread, 6).unwrap();
        assert_eq!(write.index().value(), 5);
        assert_eq!(write.index().bound(), 6);
        assert_eq!(IdentityWriteIndex::new(thread, 5), None);
    }

    #[test]
    fn distinct_threads_have_disjoint_identity_writes() {
        let domain = LaunchDomain1d::new(4);

        for left in 0..domain.len() {
            for right in 0..domain.len() {
                let left = IdentityWriteIndex::new(domain.thread(left).unwrap(), 4).unwrap();
                let right = IdentityWriteIndex::new(domain.thread(right).unwrap(), 4).unwrap();

                assert!(left.mapping_is_injective_with(right));
                assert_eq!(left.is_disjoint_from(right), left.owner() != right.owner());
            }
        }
    }
}
