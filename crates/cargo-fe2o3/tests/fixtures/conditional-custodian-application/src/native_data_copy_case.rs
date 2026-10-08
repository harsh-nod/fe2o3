//! Explicit opt-in DATA-copy case and inert output/version oracle.
use std::ffi::OsString;

pub const ELEMENTS: usize = 37;
pub const BYTES: usize = ELEMENTS * 4;

#[derive(Debug, Eq, PartialEq)]
pub struct Case {
    pub producer_source: String,
    pub device: u64,
}

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Case, String> {
    let mut args = args.into_iter();
    let mut next = || {
        args.next()
            .ok_or_else(|| "incomplete DATA-copy arguments".to_owned())?
            .into_string()
            .map_err(|_| "DATA-copy arguments must be UTF-8".to_owned())
    };
    if next()? != "--native-v5-data-copy" || next()? != "--producer-source" {
        return Err("requires --native-v5-data-copy --producer-source".into());
    }
    let producer_source = next()?;
    if producer_source.is_empty()
        || producer_source.len() > 4096
        || producer_source
            .bytes()
            .any(|b| matches!(b, 0 | b'\n' | b'\r'))
    {
        return Err("invalid DATA-copy producer source spelling".into());
    }
    if next()? != "--device" {
        return Err("requires one explicit device".into());
    }
    let spelling = next()?;
    let digits = spelling
        .strip_prefix("0x")
        .filter(|s| {
            s.len() == 16
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        })
        .ok_or("requires 0x and 16 lowercase hex digits")?;
    let device = u64::from_str_radix(digits, 16).map_err(|e| e.to_string())?;
    if device == 0 || args.next().is_some() {
        return Err("requires one nonzero device and no extra arguments".into());
    }
    Ok(Case {
        producer_source,
        device,
    })
}

/// Ordinary scalar observations supplied to an inert oracle, not execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Version {
    pub attempt: u64,
    pub lineage: u64,
}

#[derive(Debug)]
pub struct Verified {
    before: Version,
    after: Version,
}

pub fn verify(
    producer: &[u32],
    destination: &[u8],
    before: Version,
    after: Version,
) -> Result<Verified, String> {
    if producer.len() != ELEMENTS
        || producer
            .iter()
            .enumerate()
            .any(|(i, value)| *value != i as u32)
        || destination.len() != BYTES
        || destination
            .chunks_exact(4)
            .enumerate()
            .any(|(i, bytes)| bytes != (i as u32).to_le_bytes())
    {
        return Err(
            "DATA-copy producer or destination differs from the independent fill oracle".into(),
        );
    }
    // begin_write advances attempt_epoch; successful settlement publishes that
    // attempt as content_lineage, including after an earlier no-effect attempt.
    if before.lineage > before.attempt
        || before.attempt.checked_add(1) != Some(after.attempt)
        || after.lineage != after.attempt
    {
        return Err("DATA-copy destination did not settle exactly one successful writer".into());
    }
    Ok(Verified { before, after })
}

impl Verified {
    /// The caller invokes this only after actual Context/backend cleanup/refunds.
    /// The formatter itself does not certify those runtime obligations.
    pub fn fields(&self, device: u64) -> Result<String, String> {
        if device == 0 {
            return Err("zero device cannot identify a DATA-copy case".into());
        }
        Ok(format!(
            "\"device\":\"0x{device:016x}\",\"target\":\"gfx942:xnack-\",\"producer_elements\":{ELEMENTS},\"destination_bytes\":{BYTES},\"destination_attempt_before\":{},\"destination_lineage_before\":{},\"destination_attempt_after\":{},\"destination_lineage_after\":{},\"copy_path\":\"original-generated-DATA-to-device-local\",\"settled_native_launches\":1,\"settled_data_copies\":1",
            self.before.attempt, self.before.lineage, self.after.attempt, self.after.lineage,
        ))
    }
}

#[cfg(test)]
#[path = "native_data_copy_case/tests.rs"]
mod tests;
