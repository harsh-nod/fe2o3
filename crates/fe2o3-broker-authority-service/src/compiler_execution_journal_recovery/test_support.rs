use fe2o3_artifact_transaction::{
    RetainedDurableDirectoryHooksV1, RetainedDurableFaultTimingV1 as Timing,
    RetainedDurableRecordBoundaryV1 as Record, RetainedDurableRecoveryBoundaryV1 as Recovery,
    RetainedDurableRecoveryMutationBoundaryV1 as Mutation,
};
use std::{io, os::fd::OwnedFd};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Point {
    Rename,
    Record(Record),
    Directory,
    Sync,
}

#[derive(Debug)]
pub(crate) struct RecoveryFault {
    point: Point,
    timing: Timing,
    occurrence: usize,
    seen: usize,
    pub fired: bool,
}

impl RecoveryFault {
    /// Each full canonical recovery performs three directory syncs. Occurrence
    /// two reaches the anchor journal after the Worker record's recovery cycle.
    pub fn cases(occurrence: usize) -> Vec<Self> {
        assert!(occurrence > 0);
        let mut cases = Vec::new();
        for point in [
            Point::Rename,
            Point::Record(Record::SyncRedoName),
            Point::Record(Record::RenameRedoToCanonical),
            Point::Record(Record::SyncCanonicalName),
            Point::Directory,
        ] {
            for timing in [Timing::Before, Timing::After] {
                cases.push(Self::new(point, timing, occurrence));
            }
        }
        for sync in 1..=3 {
            cases.push(Self::new(
                Point::Sync,
                Timing::Before,
                (occurrence - 1) * 3 + sync,
            ));
        }
        cases
    }

    pub fn after_rename() -> Self {
        Self::new(Point::Rename, Timing::After, 1)
    }

    pub fn resume_cases() -> Vec<Self> {
        let mut cases = Vec::new();
        for boundary in [Record::RenameRedoToCanonical, Record::SyncCanonicalName] {
            for timing in [Timing::Before, Timing::After] {
                cases.push(Self::new(Point::Record(boundary), timing, 1));
            }
        }
        cases.push(Self::new(Point::Sync, Timing::Before, 1));
        cases
    }

    fn new(point: Point, timing: Timing, occurrence: usize) -> Self {
        Self {
            point,
            timing,
            occurrence,
            seen: 0,
            fired: false,
        }
    }

    fn hit(&mut self, point: Point, timing: Timing) -> io::Result<()> {
        if self.point == point && self.timing == timing {
            self.seen += 1;
            if self.seen == self.occurrence {
                self.fired = true;
                return Err(io::Error::from_raw_os_error(libc::EIO));
            }
        }
        Ok(())
    }
}

impl RetainedDurableDirectoryHooksV1 for RecoveryFault {
    fn sync_directory(&mut self, directory: &OwnedFd) -> io::Result<()> {
        self.hit(Point::Sync, Timing::Before)?;
        rustix::fs::fsync(directory).map_err(io::Error::from)
    }

    fn record(&mut self, boundary: Record, timing: Timing) -> io::Result<()> {
        self.hit(Point::Record(boundary), timing)
    }

    fn recovery(&mut self, _: Recovery, timing: Timing) -> io::Result<()> {
        self.hit(Point::Directory, timing)
    }

    fn recovery_mutation(&mut self, _: Mutation, timing: Timing) -> io::Result<()> {
        self.hit(Point::Rename, timing)
    }
}
