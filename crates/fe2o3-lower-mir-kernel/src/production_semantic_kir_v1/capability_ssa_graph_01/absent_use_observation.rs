use std::io::{self, Write};

pub(super) fn emit(
    body: &[u8; 32],
    block: u32,
    local: u32,
    events: usize,
    remaining: usize,
    caller: &'static std::panic::Location<'static>,
) {
    // Rejection-only observation. A failed diagnostic write cannot change the
    // original error, and no graph query, allocation or provenance is added.
    let _ = write_observation(
        &mut io::stderr().lock(),
        body,
        block,
        local,
        events,
        remaining,
        caller.file(),
        caller.line(),
        caller.column(),
    );
}

pub(super) fn write_observation(
    out: &mut impl Write,
    body: &[u8; 32],
    block: u32,
    local: u32,
    events: usize,
    remaining: usize,
    caller_file: &str,
    caller_line: u32,
    caller_column: u32,
) -> io::Result<()> {
    const MAX_CALLER_BYTES: usize = 160;
    let end = caller_file.len().min(MAX_CALLER_BYTES);
    let caller_prefix = caller_file.get(..end).unwrap_or("<utf8-boundary>");
    write!(out, "capability-ssa-absent-use body=")?;
    for byte in body {
        write!(out, "{byte:02x}")?;
    }
    // The block-wide API has no retained statement argument. Do not invent
    // one from a definition or label the Rust query caller as a MIR statement.
    writeln!(
        out,
        " block={block} local={local} statement=unavailable(block-wide-query) events={events} remaining={remaining} query_caller={caller_prefix}:{caller_line}:{caller_column} caller_truncated={}",
        end != caller_file.len(),
    )
}
