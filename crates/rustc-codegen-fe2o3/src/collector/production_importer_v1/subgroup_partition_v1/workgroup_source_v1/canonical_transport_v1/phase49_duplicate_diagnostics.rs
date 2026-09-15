//! Closed test evidence for a Rust borrow rejection, never importer evidence.
use serde_json::Value;

pub(super) const SUMMARY: &str = "fe2o3-phase-duplicate-rust-v1";
pub(super) const FILE: &str = "<phase49_duplicate_rust_source.rs>";
const MAX_OUTPUT: usize = 512 * 1024;
const MAX_LINE: usize = 64 * 1024;
const MAX_LINES: usize = 512;

pub(super) fn check(
    stderr: &[u8],
    test: &str,
    cpu: &str,
    source: &str,
) -> Result<(), &'static str> {
    if stderr.len() > MAX_OUTPUT || source.len() > MAX_LINE {
        return Err("duplicate Rust diagnostic output limit");
    }
    let expected_start = source
        .match_indices("&mut storage")
        .nth(1)
        .map(|(start, _)| start)
        .ok_or("duplicate Rust source lacks the second exact borrow")?;
    let expected_end = expected_start + "&mut storage".len();
    let text = std::str::from_utf8(stderr).map_err(|_| "duplicate Rust diagnostic UTF-8")?;
    let mut errors = 0usize;
    let mut aborts = 0usize;
    let mut summaries = 0usize;
    for (index, line) in text.lines().enumerate() {
        if index >= MAX_LINES || line.len() > MAX_LINE {
            return Err("duplicate Rust diagnostic line limit");
        }
        if line.is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(line).map_err(|_| "duplicate Rust diagnostic is not JSON")?;
        match value["$message_type"].as_str() {
            Some(SUMMARY) => {
                summaries += 1;
                if summaries != 1
                    || value["test"].as_str() != Some(test)
                    || value["cpu"].as_str() != Some(cpu)
                    || value["configured"].as_u64() != Some(1)
                    || value["after_analysis"].as_u64() != Some(0)
                    || value["fatal"].as_bool() != Some(true)
                {
                    return Err("duplicate Rust callback/fatal evidence mismatch");
                }
            }
            Some("diagnostic") => match value["level"].as_str() {
                Some("error") => match value["code"]["code"].as_str() {
                    Some("E0499") => {
                        errors += 1;
                        if errors != 1 {
                            return Err("duplicate Rust error count");
                        }
                        let spans = value["spans"]
                            .as_array()
                            .ok_or("duplicate Rust error lacks source span")?;
                        let mut primary = 0usize;
                        for span in spans {
                            if span["is_primary"].as_bool() != Some(true) {
                                continue;
                            }
                            primary += 1;
                            let start = span["byte_start"]
                                .as_u64()
                                .and_then(|v| usize::try_from(v).ok());
                            let end = span["byte_end"]
                                .as_u64()
                                .and_then(|v| usize::try_from(v).ok());
                            if span["file_name"].as_str() != Some(FILE)
                                || start != Some(expected_start)
                                || end != Some(expected_end)
                                || !start.zip(end).is_some_and(|(start, end)| {
                                    source.get(start..end) == Some("&mut storage")
                                })
                            {
                                return Err(
                                    "duplicate Rust error changed its exact storage borrow span",
                                );
                            }
                        }
                        if primary != 1 {
                            return Err("duplicate Rust primary span count");
                        }
                    }
                    Some(_) => return Err("duplicate Rust rejection contains another coded error"),
                    None => {
                        if !value["code"].is_null()
                            || !value["message"].as_str().is_some_and(|s| {
                                s == "aborting due to 1 previous error"
                                    || s.starts_with("aborting due to 1 previous error; ")
                            })
                        {
                            return Err("duplicate Rust rejection contains another uncoded error");
                        }
                        aborts += 1;
                    }
                },
                Some("warning" | "note" | "help" | "failure-note") => {}
                _ => return Err("duplicate Rust diagnostic level"),
            },
            _ => return Err("duplicate Rust diagnostic message type"),
        }
    }
    if (errors, aborts, summaries) != (1, 1, 1) {
        return Err("duplicate Rust rejection omitted exact error or callback evidence");
    }
    Ok(())
}

#[cfg(test)]
#[path = "phase49_duplicate_diagnostics_tests.rs"]
mod tests;
