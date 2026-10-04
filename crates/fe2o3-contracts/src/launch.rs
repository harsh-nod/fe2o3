/// A logical, target-neutral one-dimensional kernel domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchDomain1d {
    len: usize,
}

impl LaunchDomain1d {
    pub const fn new(len: usize) -> Self {
        Self { len }
    }

    pub const fn len(self) -> usize {
        self.len
    }

    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Creates evidence that `linear` names a thread in this domain.
    pub const fn thread(self, linear: usize) -> Option<ThreadInDomain1d> {
        if linear < self.len {
            Some(ThreadInDomain1d {
                id: ThreadId1d { linear },
                domain: self,
            })
        } else {
            None
        }
    }
}

/// Target-neutral physical geometry for a one-dimensional GPU launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LaunchGeometry1d {
    blocks: usize,
    threads_per_block: usize,
    threads: usize,
}

impl LaunchGeometry1d {
    /// Rejects empty launches and multiplication overflow.
    pub const fn new(blocks: usize, threads_per_block: usize) -> Option<Self> {
        if blocks == 0 || threads_per_block == 0 {
            return None;
        }

        match blocks.checked_mul(threads_per_block) {
            Some(threads) => Some(Self {
                blocks,
                threads_per_block,
                threads,
            }),
            None => None,
        }
    }

    pub const fn blocks(self) -> usize {
        self.blocks
    }

    pub const fn threads_per_block(self) -> usize {
        self.threads_per_block
    }

    pub const fn thread_count(self) -> usize {
        self.threads
    }

    pub const fn domain(self) -> LaunchDomain1d {
        LaunchDomain1d::new(self.threads)
    }

    /// Maps a physical block/lane pair to its checked logical thread witness.
    pub const fn thread(self, block: usize, lane: usize) -> Option<ThreadInDomain1d> {
        if block >= self.blocks || lane >= self.threads_per_block {
            return None;
        }

        let linear = block * self.threads_per_block + lane;
        self.domain().thread(linear)
    }
}

/// A logical thread identifier. It becomes an in-domain witness only after a
/// successful check by [`LaunchDomain1d::thread`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct ThreadId1d {
    linear: usize,
}

impl ThreadId1d {
    pub const fn linear(self) -> usize {
        self.linear
    }
}

/// Runtime evidence that a thread identifier is inside a particular domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreadInDomain1d {
    id: ThreadId1d,
    domain: LaunchDomain1d,
}

impl ThreadInDomain1d {
    pub const fn id(self) -> ThreadId1d {
        self.id
    }

    pub const fn linear(self) -> usize {
        self.id.linear
    }

    pub const fn domain(self) -> LaunchDomain1d {
        self.domain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_linearizes_threads() {
        let geometry = LaunchGeometry1d::new(3, 4).unwrap();

        assert_eq!(geometry.thread_count(), 12);
        assert_eq!(geometry.thread(2, 3).unwrap().linear(), 11);
        assert_eq!(geometry.thread(3, 0), None);
        assert_eq!(geometry.thread(0, 4), None);
    }

    #[test]
    fn geometry_rejects_empty_or_overflowing_launches() {
        assert_eq!(LaunchGeometry1d::new(0, 1), None);
        assert_eq!(LaunchGeometry1d::new(1, 0), None);
        assert_eq!(LaunchGeometry1d::new(usize::MAX, 2), None);
    }

    #[test]
    fn domain_issues_only_in_bounds_witnesses() {
        let domain = LaunchDomain1d::new(2);

        assert_eq!(domain.thread(0).unwrap().domain(), domain);
        assert_eq!(domain.thread(1).unwrap().linear(), 1);
        assert_eq!(domain.thread(2), None);
    }
}
