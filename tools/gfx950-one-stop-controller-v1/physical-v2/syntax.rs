use crate::rocgdb_mi_parser_v3::{
    MiParserLimitsV3, MiRecordV3, MiResultsV3, MiValueV3, parse_mi_record_v3,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
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
    Deadline,
}
pub(crate) fn record(line: &[u8]) -> Result<MiRecordV3, Refusal> {
    parse_mi_record_v3(
        line,
        MiParserLimitsV3 {
            max_line_bytes: crate::MAX_LINE,
            max_string_bytes: 128 * 1024,
            max_fields: 512,
            max_depth: 12,
        },
    )
    .map_err(|_| Refusal::Syntax)
}
pub(crate) fn fields(r: &MiResultsV3, allowed: &[&str], required: &[&str]) -> Result<(), Refusal> {
    if r.keys().any(|k| !allowed.contains(&k.as_str()))
        || required.iter().any(|k| !r.contains_key(*k))
    {
        return Err(Refusal::Shape);
    }
    Ok(())
}
pub(crate) fn text<'a>(r: &'a MiResultsV3, k: &str) -> Result<&'a [u8], Refusal> {
    r.get(k).and_then(MiValueV3::as_const).ok_or(Refusal::Shape)
}
pub(crate) fn decimal(raw: &[u8]) -> Result<u64, Refusal> {
    if raw.is_empty()
        || raw.len() > 20
        || (raw.len() > 1 && raw[0] == b'0')
        || !raw.iter().all(u8::is_ascii_digit)
    {
        return Err(Refusal::Shape);
    }
    std::str::from_utf8(raw)
        .map_err(|_| Refusal::Shape)?
        .parse()
        .map_err(|_| Refusal::Bound)
}
pub(crate) fn positive(raw: &[u8]) -> Result<u64, Refusal> {
    let n = decimal(raw)?;
    if n == 0 { Err(Refusal::Shape) } else { Ok(n) }
}
pub(crate) fn signed(raw: &[u8]) -> Result<(), Refusal> {
    if let Some(x) = raw.strip_prefix(b"-") {
        if positive(x)? > 1u64 << 63 {
            return Err(Refusal::Bound);
        }
    } else if decimal(raw)? > i64::MAX as u64 {
        return Err(Refusal::Bound);
    }
    Ok(())
}
pub(crate) fn address(raw: &[u8]) -> Result<u64, Refusal> {
    let h = raw.strip_prefix(b"0x").ok_or(Refusal::Shape)?;
    if h.is_empty() || h.len() > 16 || !h.iter().all(u8::is_ascii_hexdigit) {
        return Err(Refusal::Shape);
    }
    u64::from_str_radix(std::str::from_utf8(h).map_err(|_| Refusal::Shape)?, 16)
        .map_err(|_| Refusal::Shape)
}
pub(crate) fn frame(r: &MiResultsV3, symbol: Option<&str>) -> Result<(), Refusal> {
    let f = r
        .get("frame")
        .and_then(MiValueV3::as_tuple)
        .ok_or(Refusal::Stop)?;
    fields(
        f,
        &[
            "addr", "func", "args", "file", "fullname", "line", "arch", "from",
        ],
        &["addr"],
    )?;
    if address(text(f, "addr")?)? == 0 {
        return Err(Refusal::Stop);
    }
    if let Some(s) = symbol
        && text(f, "func")? != s.as_bytes()
    {
        return Err(Refusal::Stop);
    }
    // Argument spelling is presentation only; it cannot mint checkpoint content.
    if let Some(args) = f.get("args")
        && args.as_values().is_none_or(|v| v.len() > 4)
    {
        return Err(Refusal::Shape);
    }
    Ok(())
}
pub(crate) fn entry_breakpoint(r: &MiResultsV3, max_hits: u64) -> Result<u64, Refusal> {
    fields(r, &["bkpt"], &["bkpt"])?;
    let b = r
        .get("bkpt")
        .and_then(MiValueV3::as_tuple)
        .ok_or(Refusal::Shape)?;
    fields(
        b,
        &[
            "number",
            "type",
            "disp",
            "enabled",
            "addr",
            "func",
            "at",
            "file",
            "fullname",
            "line",
            "thread-groups",
            "times",
            "original-location",
        ],
        &[
            "number",
            "type",
            "disp",
            "enabled",
            "addr",
            "times",
            "original-location",
        ],
    )?;
    if text(b, "type")? != b"breakpoint"
        || text(b, "disp")? != b"del"
        || text(b, "enabled")? != b"y"
        || text(b, "original-location")? != crate::ENTRY.as_bytes()
        || address(text(b, "addr")?)? == 0
        || decimal(text(b, "times")?)? > max_hits
    {
        return Err(Refusal::Stop);
    }
    match (b.get("func"), b.get("at")) {
        (Some(v), None) if v.as_const() == Some(crate::ENTRY.as_bytes()) => {}
        (None, Some(v))
            if v.as_const()
                .and_then(|x| x.strip_prefix(b"<"))
                .and_then(|x| x.strip_suffix(b">"))
                == Some(crate::ENTRY.as_bytes()) => {}
        _ => return Err(Refusal::Stop),
    }
    if let Some(groups) = b.get("thread-groups") {
        let xs = groups.as_values().ok_or(Refusal::Shape)?;
        if xs.len() != 1 || xs[0].as_const() != Some(b"i1") {
            return Err(Refusal::Process);
        }
    }
    positive(text(b, "number")?)
}
