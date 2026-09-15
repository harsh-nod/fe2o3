use super::work_failure_observation::Failure;
use std::ffi::OsStr;
use std::io::{self, Write};
use std::panic::Location;

const SITE_LIMIT: usize = 64;
const FILE_LIMIT: usize = 32;
const CALLER_BYTES: usize = 160;

#[derive(Clone, Copy)]
struct Row {
    caller: &'static Location<'static>,
    accepted: usize,
}

// Diagnostic-only storage, deliberately outside the charged query cache.
// Overflow sites retain their work in `unclassified`; no debit is sampled.
pub(super) struct Profile {
    rows: [Option<Row>; SITE_LIMIT],
    len: usize,
    accepted: usize,
    unclassified: usize,
    files: Files,
    valid: bool,
}

fn enabled_value(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new("1"))
}

fn enabled() -> bool {
    #[cfg(test)]
    if let Some(enabled) = ENABLED.with(|value| value.get()) {
        return enabled;
    }
    enabled_value(std::env::var_os("FE2O3_TRACE_CAPABILITY_WORK").as_deref())
}

impl Profile {
    pub(super) fn configured() -> Option<Box<Self>> {
        enabled().then(|| Box::new(Self::new()))
    }

    fn new() -> Self {
        Self {
            rows: [None; SITE_LIMIT],
            len: 0,
            accepted: 0,
            unclassified: 0,
            files: Files::new(),
            valid: true,
        }
    }

    pub(super) fn record(&mut self, amount: usize, caller: &'static Location<'static>) {
        if amount == 0 || !self.valid {
            return;
        }
        let Some(accepted) = self.accepted.checked_add(amount) else {
            self.valid = false;
            return;
        };
        self.accepted = accepted;
        self.files.record(amount, caller.file());
        for row in self.rows[..self.len].iter_mut().flatten() {
            if row.caller == caller {
                // Every row is bounded by the checked total above.
                row.accepted += amount;
                return;
            }
        }
        if self.len < SITE_LIMIT {
            self.rows[self.len] = Some(Row {
                caller,
                accepted: amount,
            });
            self.len += 1;
        } else {
            self.unclassified += amount;
        }
    }

    pub(super) fn emit(&self, failure: Failure<'_>, caller: &'static Location<'static>) {
        let _ = self.write_to(&mut io::stderr().lock(), failure, caller);
    }

    pub(super) fn write_to(
        &self,
        out: &mut impl Write,
        failure: Failure<'_>,
        caller: &'static Location<'static>,
    ) -> io::Result<()> {
        let accounted = self.rows[..self.len]
            .iter()
            .flatten()
            .try_fold(self.unclassified, |sum, row| sum.checked_add(row.accepted));
        let valid = self.valid
            && accounted == Some(self.accepted)
            && self.files.accounted() == Some(self.accepted)
            && failure.limit.checked_sub(failure.remaining) == Some(self.accepted);
        write!(out, "capability-ssa-work-profile-v1 body=")?;
        for byte in failure.body {
            write!(out, "{byte:02x}")?;
        }
        write!(
            out,
            " boundary=budget-failure limit={} remaining={} requested={} accepted={} unclassified={} sites={} site_limit={SITE_LIMIT} site_accounting_complete={} accounting_valid={valid}",
            failure.limit,
            failure.remaining,
            failure.requested,
            self.accepted,
            self.unclassified,
            self.len,
            self.unclassified == 0,
        )?;
        write_caller(out, caller.file(), caller.line(), caller.column())?;
        for (index, row) in self.rows[..self.len].iter().flatten().enumerate() {
            write!(
                out,
                "capability-ssa-work-profile-site-v1 index={index} accepted={}",
                row.accepted
            )?;
            write_caller(
                out,
                row.caller.file(),
                row.caller.line(),
                row.caller.column(),
            )?;
        }
        self.files.write_to(out, valid)?;
        writeln!(out, "capability-ssa-work-profile-end-v1 record_complete=1")
    }
}

#[derive(Clone, Copy)]
struct FileRow {
    file: &'static str,
    accepted: usize,
}

// Independent file aggregation still counts debits after the site table fills.
// Names come from compiler Locations, never from kernel names or source input.
struct Files {
    rows: [Option<FileRow>; FILE_LIMIT],
    len: usize,
    unclassified: usize,
}

impl Files {
    fn new() -> Self {
        Self {
            rows: [None; FILE_LIMIT],
            len: 0,
            unclassified: 0,
        }
    }

    fn record(&mut self, amount: usize, file: &'static str) {
        if amount == 0 {
            return;
        }
        for row in self.rows[..self.len].iter_mut().flatten() {
            if row.file == file {
                // The profile's checked total bounds every file subtotal.
                row.accepted += amount;
                return;
            }
        }
        if self.len < FILE_LIMIT {
            self.rows[self.len] = Some(FileRow {
                file,
                accepted: amount,
            });
            self.len += 1;
        } else {
            self.unclassified += amount;
        }
    }

    fn accounted(&self) -> Option<usize> {
        self.rows[..self.len]
            .iter()
            .flatten()
            .try_fold(self.unclassified, |sum, row| sum.checked_add(row.accepted))
    }

    fn write_to(&self, out: &mut impl Write, valid: bool) -> io::Result<()> {
        writeln!(
            out,
            "capability-ssa-work-profile-files-v1 files={} file_limit={FILE_LIMIT} unclassified={} file_accounting_complete={} accounting_valid={valid}",
            self.len,
            self.unclassified,
            self.unclassified == 0,
        )?;
        for (index, row) in self.rows[..self.len].iter().flatten().enumerate() {
            let end = row.file.len().min(CALLER_BYTES);
            let prefix = row.file.get(..end).unwrap_or("<utf8-boundary>");
            writeln!(
                out,
                "capability-ssa-work-profile-file-v1 index={index} accepted={} charge_file={prefix} file_truncated={}",
                row.accepted,
                end != row.file.len(),
            )?;
        }
        Ok(())
    }
}

fn write_caller(out: &mut impl Write, file: &str, line: u32, column: u32) -> io::Result<()> {
    let end = file.len().min(CALLER_BYTES);
    let prefix = file.get(..end).unwrap_or("<utf8-boundary>");
    writeln!(
        out,
        " charge_caller={prefix}:{line}:{column} caller_truncated={}",
        end != file.len()
    )
}

#[cfg(test)]
thread_local! {
    static ENABLED: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(super) fn with_enabled<T>(enabled: bool, action: impl FnOnce() -> T) -> T {
    struct Restore(Option<bool>);
    impl Drop for Restore {
        fn drop(&mut self) {
            ENABLED.with(|value| value.set(self.0));
        }
    }
    let _restore = Restore(ENABLED.with(|value| value.replace(Some(enabled))));
    action()
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("work_profile_tests.rs");
    include!("work_profile_file_tests125.rs");
}
