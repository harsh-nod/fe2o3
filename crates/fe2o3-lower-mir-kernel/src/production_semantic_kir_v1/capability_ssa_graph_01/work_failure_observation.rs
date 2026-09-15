use std::ffi::OsStr;
use std::io::{self, Write};

#[derive(Clone, Copy)]
pub(super) struct Failure<'a> {
    pub(super) body: &'a [u8; 32],
    pub(super) remaining: usize,
    pub(super) requested: usize,
    pub(super) limit: usize,
    pub(super) blocks: usize,
    pub(super) locals: usize,
    pub(super) cache_rows: [usize; 4],
}

fn enabled_value(value: Option<&OsStr>) -> bool {
    value == Some(OsStr::new("1"))
}

fn enabled() -> bool {
    #[cfg(test)]
    if let Some(enabled) = ENABLED.with(|value| value.get()) {
        return enabled;
    }
    enabled_value(std::env::var_os("FE2O3_TRACE_CAPABILITY_CUSTODY").as_deref())
}

pub(super) fn emit(failure: Failure<'_>, caller: &'static std::panic::Location<'static>) {
    // Called only after checked_sub fails. No successful-query instrumentation,
    // graph scan, proof-state mutation or new work allowance is introduced.
    if !enabled() {
        return;
    }
    let _ = write_observation(
        &mut io::stderr().lock(),
        failure,
        caller.file(),
        caller.line(),
        caller.column(),
    );
}

fn write_observation(
    out: &mut impl Write,
    failure: Failure<'_>,
    caller_file: &str,
    caller_line: u32,
    caller_column: u32,
) -> io::Result<()> {
    const MAX_CALLER_BYTES: usize = 160;
    let end = caller_file.len().min(MAX_CALLER_BYTES);
    let caller_prefix = caller_file.get(..end).unwrap_or("<utf8-boundary>");
    write!(out, "capability-ssa-analysis-work body=")?;
    for byte in failure.body {
        write!(out, "{byte:02x}")?;
    }
    let [uses, definitions, reachability, loans] = failure.cache_rows;
    // charge_caller is compiler code, not an invented MIR statement. Cache
    // sizes describe successfully retained rows, not cumulative work counters.
    writeln!(
        out,
        " remaining={} requested={} limit={} blocks={} locals={} cache_uses={uses} cache_definitions={definitions} cache_reachability={reachability} cache_loans={loans} charge_caller={caller_prefix}:{caller_line}:{caller_column} caller_truncated={}",
        failure.remaining,
        failure.requested,
        failure.limit,
        failure.blocks,
        failure.locals,
        end != caller_file.len(),
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
    include!("work_failure_format_tests.rs");
}
