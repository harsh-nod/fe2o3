//! Bounded, diagnostic-only JSON output; no authority or retry behavior.

use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;

pub(super) const MAX_REPORT_BYTES: usize = 4 * 1024 * 1024;

pub(super) struct CensusSinkV1 {
    file: File,
}

impl CensusSinkV1 {
    pub(super) fn create(path: &Path) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options.open(path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "census output must be a new regular file",
            ));
        }
        Ok(Self { file })
    }

    pub(super) fn finish(mut self, report: &impl serde::Serialize) -> io::Result<()> {
        let mut writer = Limited {
            inner: Vec::new(),
            remaining: MAX_REPORT_BYTES,
        };
        serde_json::to_writer(&mut writer, report).map_err(io::Error::other)?;
        writer.write_all(b"\n")?;
        write_buffered_report(&mut self.file, writer.inner)
    }
}

fn write_buffered_report(writer: &mut (impl Write + Seek), mut bytes: Vec<u8>) -> io::Result<()> {
    // A failed write or seek leaves empty or NUL-prefixed, unparseable output.
    let first = std::mem::replace(&mut bytes[0], 0);
    writer.write_all(&bytes)?;
    writer.seek(SeekFrom::Start(0))?;
    writer.write_all(&[first])
}

struct Limited<W> {
    inner: W,
    remaining: usize,
}

impl<W: Write> Write for Limited<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(io::Error::other(
                "census report exceeds diagnostic byte cap",
            ));
        }
        let written = self.inner.write(bytes)?;
        self.remaining -= written;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_temp_dir::TestTempDir;
    use std::fs;
    use std::io::Cursor;

    #[test]
    fn output_is_new_regular_private_and_exact_json_with_one_newline() {
        let scratch = TestTempDir::create("fe2o3-census-exact");
        let path = scratch.path().join("report.json");
        let sink = CensusSinkV1::create(&path).unwrap();
        let metadata = fs::metadata(&path).unwrap();
        assert!(metadata.is_file());
        assert_eq!(metadata.len(), 0);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        }
        let report = serde_json::json!({"entries": 2});
        sink.finish(&report).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes, b"{\"entries\":2}\n");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            report
        );
    }

    #[test]
    fn stale_existing_path_is_untouched() {
        let scratch = TestTempDir::create("fe2o3-census-stale");
        let path = scratch.path().join("report.json");
        let stale = b"{\"stale\":true}\n";
        fs::write(&path, stale).unwrap();
        let error = CensusSinkV1::create(&path).err().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(&path).unwrap(), stale);
    }

    #[test]
    fn exact_byte_cap_includes_trailing_newline() {
        let scratch = TestTempDir::create("fe2o3-census-cap");
        let path = scratch.path().join("report.json");
        let report = "x".repeat(MAX_REPORT_BYTES - 3);
        CensusSinkV1::create(&path)
            .unwrap()
            .finish(&report)
            .unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes.len(), MAX_REPORT_BYTES);
        assert!(bytes.ends_with(b"\"\n"));
        assert_eq!(serde_json::from_slice::<String>(&bytes).unwrap(), report);
    }

    #[test]
    fn exceeding_cap_during_json_or_newline_leaves_empty_output() {
        let scratch = TestTempDir::create("fe2o3-census-oversized");
        for length in [MAX_REPORT_BYTES - 2, MAX_REPORT_BYTES - 1] {
            let path = scratch.path().join(format!("{length}.json"));
            let report = "x".repeat(length);
            assert!(
                CensusSinkV1::create(&path)
                    .unwrap()
                    .finish(&report)
                    .is_err()
            );
            assert!(fs::read(&path).unwrap().is_empty());
        }
    }

    #[test]
    fn serializer_failure_after_complete_value_leaves_empty_output() {
        struct FailingReport;

        impl serde::Serialize for FailingReport {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_u8(7)?;
                Err(serde::ser::Error::custom("intentional serializer failure"))
            }
        }

        let scratch = TestTempDir::create("fe2o3-census-serializer");
        let path = scratch.path().join("report.json");
        let error = CensusSinkV1::create(&path)
            .unwrap()
            .finish(&FailingReport)
            .unwrap_err();
        assert!(error.to_string().contains("intentional serializer failure"));
        assert!(fs::read(&path).unwrap().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn existing_and_dangling_symlinks_are_refused() {
        use std::os::unix::fs::symlink;

        let scratch = TestTempDir::create("fe2o3-census-symlink");
        let target = scratch.path().join("target.json");
        fs::write(&target, b"unchanged").unwrap();
        let missing = scratch.path().join("missing.json");
        for (name, destination) in [("existing", &target), ("dangling", &missing)] {
            let link = scratch.path().join(name);
            symlink(destination, &link).unwrap();
            assert!(CensusSinkV1::create(&link).is_err());
            assert!(
                fs::symlink_metadata(&link)
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(fs::read_link(&link).unwrap(), *destination);
        }
        assert_eq!(fs::read(&target).unwrap(), b"unchanged");
        assert!(!missing.exists());
    }

    #[test]
    fn write_and_seek_failures_never_leave_parseable_json() {
        struct FailingWriter {
            inner: Cursor<Vec<u8>>,
            remaining: usize,
            fail_seek: bool,
        }

        impl Write for FailingWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.remaining == 0 {
                    return Err(io::Error::other("intentional write failure"));
                }
                let length = bytes.len().min(self.remaining);
                let written = self.inner.write(&bytes[..length])?;
                self.remaining -= written;
                Ok(written)
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        impl Seek for FailingWriter {
            fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
                if self.fail_seek {
                    return Err(io::Error::other("intentional seek failure"));
                }
                self.inner.seek(position)
            }
        }

        for bytes in [b"{\"entries\":2}\n".as_slice(), b"123\n".as_slice()] {
            for fail_seek in [false, true] {
                for remaining in 0..=bytes.len() {
                    let mut writer = FailingWriter {
                        inner: Cursor::new(Vec::new()),
                        remaining,
                        fail_seek,
                    };
                    assert!(write_buffered_report(&mut writer, bytes.to_vec()).is_err());
                    assert!(
                        serde_json::from_slice::<serde_json::Value>(writer.inner.get_ref())
                            .is_err()
                    );
                }
            }
        }
    }
}
