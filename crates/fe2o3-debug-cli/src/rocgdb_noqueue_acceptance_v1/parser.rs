//! Strict bounded parsing for the proposed no-queue acceptance controller.
//! Parsed bytes are inert observations, never process/device or read authority.
//! Used by pure controls and the closed launch-owned qualification example.
use crate::rocgdb_mi_parser_v3::{
    MiAsyncKindV3, MiListV3, MiParserLimitsV3, MiRecordV3, MiResultsV3, MiValueV3,
    parse_mi_record_v3,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(super) const MAX_ARTIFACT_BYTES: usize = 64 * 1024;
pub(super) const ENTRY_HOST_SYMBOL: &str = "fe2o3_gfx950_noqueue_process_entry_v1";
pub(super) const PRE_HOST_SYMBOL: &str = "fe2o3_gfx950_noqueue_pre_activation_v1";
pub(super) const POST_HOST_SYMBOL: &str = "fe2o3_gfx950_noqueue_post_publication_v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Refusal {
    Bound,
    Syntax,
    Shape,
    Token,
    Process,
    Artifact,
    Duplicate,
    State,
    Stop,
    Changed,
    Exit,
    Incomplete,
}

/// An observed URI extent only. It cannot issue a read or manufacture a pidfd.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MemoryObject {
    pub(super) pid: u32,
    pub(super) address: u64,
    pub(super) bytes: usize,
}

/// Shared syntactic budget for the semantic parser and launch-owned protocol.
/// The required `shared-libraries=[{ranges=[{from="...",to="..."}]}]` shape
/// reaches depth 12: the MI parser counts results, values, and containers.
/// Nested values inside the range's scalar fields exceed this exact budget;
/// semantic checks below still reject unknown fields and non-scalar ranges.
pub(super) fn record(line: &[u8]) -> Result<MiRecordV3, Refusal> {
    parse_mi_record_v3(
        line,
        MiParserLimitsV3 {
            max_line_bytes: 192 * 1024,
            max_string_bytes: 128 * 1024,
            max_fields: 512,
            max_depth: 12,
        },
    )
    .map_err(|_| Refusal::Syntax)
}
fn fields(value: &MiResultsV3, allowed: &[&str], required: &[&str]) -> Result<(), Refusal> {
    if value.keys().any(|key| !allowed.contains(&key.as_str()))
        || required.iter().any(|key| !value.contains_key(*key))
    {
        return Err(Refusal::Shape);
    }
    Ok(())
}
fn text<'a>(value: &'a MiResultsV3, key: &str) -> Result<&'a [u8], Refusal> {
    value
        .get(key)
        .and_then(MiValueV3::as_const)
        .ok_or(Refusal::Shape)
}
fn tuples(value: &MiValueV3, max: usize) -> Result<Vec<&MiResultsV3>, Refusal> {
    let MiValueV3::List(MiListV3::Values(rows)) = value else {
        return Err(Refusal::Shape);
    };
    if rows.len() > max {
        return Err(Refusal::Bound);
    }
    rows.iter()
        .map(|row| row.as_tuple().ok_or(Refusal::Shape))
        .collect()
}
fn decimal(raw: &[u8]) -> Result<u64, Refusal> {
    if raw.is_empty()
        || raw.len() > 20
        || (raw.len() != 1 && raw[0] == b'0')
        || !raw.iter().all(u8::is_ascii_digit)
    {
        return Err(Refusal::Shape);
    }
    std::str::from_utf8(raw)
        .map_err(|_| Refusal::Shape)?
        .parse()
        .map_err(|_| Refusal::Bound)
}
fn hex(raw: &[u8]) -> Result<u64, Refusal> {
    let digits = raw.strip_prefix(b"0x").ok_or(Refusal::Shape)?;
    if digits.is_empty() || digits.len() > 16 || !digits.iter().all(u8::is_ascii_hexdigit) {
        return Err(Refusal::Shape);
    }
    u64::from_str_radix(std::str::from_utf8(digits).map_err(|_| Refusal::Shape)?, 16)
        .map_err(|_| Refusal::Bound)
}
fn clean(raw: &[u8], max: usize) -> Result<(), Refusal> {
    if raw.is_empty() || raw.len() > max || !raw.iter().all(|b| (b' '..=b'~').contains(b)) {
        return Err(Refusal::Bound);
    }
    Ok(())
}
fn result(line: &[u8], token: u64) -> Result<MiResultsV3, Refusal> {
    if token == 0 {
        return Err(Refusal::Token);
    }
    let MiRecordV3::Result {
        token: actual,
        class,
        results,
    } = record(line)?
    else {
        return Err(Refusal::Shape);
    };
    if actual != Some(token) {
        return Err(Refusal::Token);
    }
    if class != "done" {
        return Err(Refusal::Shape);
    }
    Ok(results)
}

pub(super) fn parse_memory_uri(
    raw: &[u8],
    expected_pid: u32,
    expected_bytes: usize,
) -> Result<MemoryObject, Refusal> {
    if expected_pid == 0 || expected_bytes == 0 || expected_bytes > MAX_ARTIFACT_BYTES {
        return Err(Refusal::Bound);
    }
    clean(raw, 127)?;
    let value = raw.strip_prefix(b"memory://").ok_or(Refusal::Shape)?;
    let (pid, tail) = value.split_marker(b"#offset=").ok_or(Refusal::Shape)?;
    let (address, size) = tail.split_marker(b"&size=").ok_or(Refusal::Shape)?;
    let pid = u32::try_from(decimal(pid)?).map_err(|_| Refusal::Bound)?;
    if pid != expected_pid {
        return Err(Refusal::Process);
    }
    let address = hex(address)?;
    let bytes = usize::try_from(decimal(size)?).map_err(|_| Refusal::Bound)?;
    if address == 0 || bytes != expected_bytes || address.checked_add(bytes as u64).is_none() {
        return Err(Refusal::Artifact);
    }
    Ok(MemoryObject {
        pid,
        address,
        bytes,
    })
}

pub(super) fn parse_libraries(
    line: &[u8],
    token: u64,
    expected_pid: u32,
    expected_bytes: usize,
) -> Result<Option<MemoryObject>, Refusal> {
    let result = result(line, token)?;
    fields(&result, &["shared-libraries"], &["shared-libraries"])?;
    let rows = tuples(result.get("shared-libraries").ok_or(Refusal::Shape)?, 64)?;
    let mut selected = None;
    let mut seen = BTreeSet::new();
    for row in rows {
        fields(
            row,
            &[
                "id",
                "target-name",
                "host-name",
                "symbols-loaded",
                "thread-group",
                "ranges",
            ],
            &["id", "target-name", "host-name", "ranges"],
        )?;
        let id = text(row, "id")?;
        clean(id, 512)?;
        if !seen.insert(id.to_vec()) {
            return Err(Refusal::Duplicate);
        }
        let target = text(row, "target-name")?;
        let host = text(row, "host-name")?;
        clean(target, 512)?;
        clean(host, 512)?;
        if let Some(group) = row.get("thread-group")
            && group.as_const() != Some(b"i1")
        {
            return Err(Refusal::Process);
        }
        if let Some(compatibility_only) = row.get("symbols-loaded") {
            // The installed manual says this field conveys no useful evidence.
            // Both values parse; neither participates in an acceptance predicate.
            if !matches!(compatibility_only.as_const(), Some(b"0" | b"1")) {
                return Err(Refusal::Shape);
            }
        }
        let ranges = tuples(row.get("ranges").ok_or(Refusal::Shape)?, 8)?;
        for range in &ranges {
            fields(range, &["from", "to"], &["from", "to"])?;
            if hex(text(range, "from")?)? >= hex(text(range, "to")?)? {
                return Err(Refusal::Shape);
            }
        }
        if target.starts_with(b"memory://") || host.starts_with(b"memory://") {
            if selected.is_some() {
                return Err(Refusal::Duplicate);
            }
            if target != host || ranges.is_empty() {
                return Err(Refusal::Artifact);
            }
            selected = Some(parse_memory_uri(target, expected_pid, expected_bytes)?);
        } else if !target.starts_with(b"/") || !host.starts_with(b"/") {
            // This profile has one memory-backed GPU object, no file URI fallback.
            return Err(Refusal::Artifact);
        }
    }
    Ok(selected)
}

pub(super) fn parse_original_elf(
    line: &[u8],
    token: u64,
    selected: MemoryObject,
    expected_sha256: [u8; 32],
) -> Result<(), Refusal> {
    if selected.pid == 0
        || selected.bytes == 0
        || selected.bytes > MAX_ARTIFACT_BYTES
        || selected.address == 0
    {
        return Err(Refusal::Bound);
    }
    let result = result(line, token)?;
    fields(&result, &["memory"], &["memory"])?;
    let rows = tuples(result.get("memory").ok_or(Refusal::Shape)?, 1)?;
    let [row] = rows.as_slice() else {
        return Err(Refusal::Shape);
    };
    fields(
        row,
        &["begin", "offset", "end", "contents"],
        &["begin", "offset", "end", "contents"],
    )?;
    let end = selected
        .address
        .checked_add(selected.bytes as u64)
        .ok_or(Refusal::Bound)?;
    if hex(text(row, "begin")?)? != selected.address
        || hex(text(row, "offset")?)? != 0
        || hex(text(row, "end")?)? != end
    {
        return Err(Refusal::Artifact);
    }
    let contents = text(row, "contents")?;
    if contents.len() != selected.bytes.checked_mul(2).ok_or(Refusal::Bound)? {
        return Err(Refusal::Artifact);
    }
    let nibble = |b| match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err(Refusal::Shape),
    };
    let mut digest = Sha256::new();
    for pair in contents.chunks_exact(2) {
        digest.update([(nibble(pair[0])? << 4) | nibble(pair[1])?]);
    }
    let actual: [u8; 32] = digest.finalize().into();
    if actual != expected_sha256 {
        return Err(Refusal::Artifact);
    }
    Ok(())
}

pub(super) fn parse_host_stop(
    line: &[u8],
    breakpoint: &[u8],
    thread: &[u8],
    symbol: &str,
) -> Result<(), Refusal> {
    if !matches!(
        symbol,
        ENTRY_HOST_SYMBOL | PRE_HOST_SYMBOL | POST_HOST_SYMBOL
    ) || decimal(breakpoint)? == 0
        || decimal(thread)? == 0
    {
        return Err(Refusal::Stop);
    }
    let MiRecordV3::Async {
        token: None,
        kind: MiAsyncKindV3::Exec,
        class,
        results,
    } = record(line)?
    else {
        return Err(Refusal::Stop);
    };
    if class != "stopped" {
        return Err(Refusal::Stop);
    }
    fields(
        &results,
        &[
            "reason",
            "disp",
            "bkptno",
            "frame",
            "thread-id",
            "stopped-threads",
            "core",
        ],
        &["reason", "bkptno", "frame", "thread-id", "stopped-threads"],
    )?;
    if text(&results, "reason")? != b"breakpoint-hit"
        || text(&results, "bkptno")? != breakpoint
        || text(&results, "thread-id")? != thread
        || text(&results, "stopped-threads")? != b"all"
    {
        return Err(Refusal::Stop);
    }
    let frame = results
        .get("frame")
        .and_then(MiValueV3::as_tuple)
        .ok_or(Refusal::Shape)?;
    fields(
        frame,
        &[
            "addr", "func", "args", "file", "fullname", "line", "arch", "from",
        ],
        &["addr", "func", "args"],
    )?;
    if hex(text(frame, "addr")?)? == 0
        || text(frame, "func")? != symbol.as_bytes()
        || !frame
            .get("args")
            .and_then(MiValueV3::as_values)
            .is_some_and(|x| x.is_empty())
    {
        return Err(Refusal::Stop);
    }
    if let Some(arch) = frame.get("arch")
        && arch.as_const() != Some(b"i386:x86-64")
    {
        return Err(Refusal::Stop);
    }
    Ok(())
}

trait SplitMarker {
    fn split_marker(&self, marker: &[u8]) -> Option<(&[u8], &[u8])>;
}
impl SplitMarker for [u8] {
    fn split_marker(&self, marker: &[u8]) -> Option<(&[u8], &[u8])> {
        let index = self.windows(marker.len()).position(|w| w == marker)?;
        Some((&self[..index], &self[index + marker.len()..]))
    }
}
