use super::*;
use fe2o3_artifact_transaction::{
    RetainedDurableFaultTimingV1 as Timing, RetainedDurableRecordBoundaryV1 as Boundary,
};
use std::{fs, io, os::unix::fs::PermissionsExt, path::PathBuf};

const NAMES: JournalNames = JournalNames {
    canonical: "record",
    redo: "redo",
    recovery: "recovery",
    maximum_bytes: 1,
};

struct PostSyncMutation {
    root: PathBuf,
    kind: u8,
    fired: bool,
}

impl RetainedDurableDirectoryHooksV1 for PostSyncMutation {
    fn record(&mut self, boundary: Boundary, timing: Timing) -> io::Result<()> {
        if boundary == Boundary::SyncCanonicalName && timing == Timing::After {
            self.fired = true;
            match self.kind {
                0 => fs::remove_file(self.root.join(NAMES.canonical))?,
                1 => fs::write(self.root.join(NAMES.canonical), [3])?,
                2 | 3 => {
                    let name = if self.kind == 2 {
                        NAMES.redo
                    } else {
                        NAMES.recovery
                    };
                    std::os::unix::fs::symlink("missing-target", self.root.join(name))?;
                }
                _ => unreachable!(),
            }
        }
        Ok(())
    }
}

#[test]
fn every_recovery_branch_rechecks_exact_bytes_and_both_sidecar_names() {
    for entry in [NAMES.canonical, NAMES.redo, NAMES.recovery] {
        for kind in 0..4 {
            let root = tempfile::TempDir::new().unwrap();
            fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).unwrap();
            let path = root.path().join(entry);
            fs::write(&path, [2]).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            let store = RetainedDurableDirectoryV1::admit_service_owned(
                fs::File::open(root.path()).unwrap().into(),
            )
            .unwrap();
            let mut hook = PostSyncMutation {
                root: root.path().to_owned(),
                kind,
                fired: false,
            };
            let result = NAMES.recover(
                &store,
                &mut hook,
                |bytes| {
                    if bytes == [2] {
                        Ok(2_u8)
                    } else {
                        Err(IoError::ContentMismatch {
                            entry: entry.to_owned(),
                        })
                    }
                },
                |_, _| Ok(()),
            );
            assert!(hook.fired);
            assert!(result.is_err(), "{entry}/{kind} returned an owner");
        }
    }
}
