//! Best-effort, non-authoritative observer diagnostics.

use std::fmt;
use std::io::{self, Write};

pub(crate) fn write_line(arguments: fmt::Arguments<'_>) {
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    let _ = write_line_to(&mut stderr, arguments);
}

pub(crate) fn write_line_to(
    writer: &mut (impl Write + ?Sized),
    arguments: fmt::Arguments<'_>,
) -> io::Result<()> {
    writer.write_fmt(arguments)?;
    writer.write_all(b"\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingWriter {
        bytes_before_failure: usize,
    }

    impl Write for FailingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if self.bytes_before_failure == 0 {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected"));
            }
            let written = self.bytes_before_failure.min(buffer.len());
            self.bytes_before_failure -= written;
            Ok(written)
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected"))
        }
    }

    #[test]
    fn production_writer_reports_zero_and_partial_io_failures_without_panicking() {
        for bytes_before_failure in [0, 5] {
            assert!(
                write_line_to(
                    &mut FailingWriter {
                        bytes_before_failure
                    },
                    format_args!("observer {}", 7)
                )
                .is_err()
            );
        }
    }

    #[test]
    fn production_writer_emits_one_exact_line() {
        let mut bytes = Vec::new();
        write_line_to(&mut bytes, format_args!("observer {}", 7)).unwrap();
        assert_eq!(bytes, b"observer 7\n");
    }
}
