//! Fixed-schema link parsing; file observations stay in the shared bounded reader.

#![forbid(unsafe_code)]

use super::{
    MAX_PROPERTY_BYTES, Path, RegularFileObservation, TopologyError, read_bounded_regular_into,
};

const KEYS: [&str; 13] = [
    "type",
    "version_major",
    "version_minor",
    "node_from",
    "node_to",
    "weight",
    "min_latency",
    "max_latency",
    "min_bandwidth",
    "max_bandwidth",
    "recommended_transfer_size",
    "recommended_sdma_engine_id_mask",
    "flags",
];

#[derive(Debug, Eq, PartialEq)]
pub(super) struct LinkProperties {
    pub(super) link_type: u32,
    pub(super) version_major: u32,
    pub(super) version_minor: u32,
    pub(super) node_from: u32,
    pub(super) node_to: u32,
    pub(super) weight: u32,
    pub(super) min_latency: u64,
    pub(super) max_latency: u64,
    pub(super) min_bandwidth: u64,
    pub(super) max_bandwidth: u64,
    pub(super) recommended_transfer_size: u64,
    pub(super) recommended_sdma_engine_id_mask: u64,
    pub(super) flags: u32,
}

pub(super) fn read(
    before: RegularFileObservation<'_>,
    bytes: &mut Vec<u8>,
) -> Result<LinkProperties, TopologyError> {
    let path = before.path;
    read_bounded_regular_into(before, MAX_PROPERTY_BYTES, bytes)?;
    let text =
        std::str::from_utf8(bytes).map_err(|_| TopologyError::InvalidUtf8(path.to_path_buf()))?;
    parse(path, text)
}

fn parse(path: &Path, text: &str) -> Result<LinkProperties, TopologyError> {
    if !text.ends_with('\n') {
        return Err(TopologyError::MalformedPropertyLine {
            path: path.to_path_buf(),
            line: 1,
        });
    }
    let mut values = [0_u64; KEYS.len()];
    let mut seen = 0_u16;
    for (index, line) in text.split_terminator('\n').enumerate() {
        if index >= KEYS.len() {
            return Err(TopologyError::TooManyProperties {
                path: path.to_path_buf(),
                maximum: KEYS.len(),
            });
        }
        let Some((key, raw_value)) = line.split_once(' ') else {
            return Err(TopologyError::MalformedPropertyLine {
                path: path.to_path_buf(),
                line: index + 1,
            });
        };
        if key.is_empty()
            || raw_value.is_empty()
            || raw_value.contains(' ')
            || raw_value.starts_with('0') && raw_value != "0"
            || !raw_value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(TopologyError::MalformedPropertyLine {
                path: path.to_path_buf(),
                line: index + 1,
            });
        }
        let (slot, maximum) = match key {
            "type" => (0, u32::MAX as u64),
            "version_major" => (1, u32::MAX as u64),
            "version_minor" => (2, u32::MAX as u64),
            "node_from" => (3, u16::MAX as u64),
            "node_to" => (4, u16::MAX as u64),
            "weight" => (5, u32::MAX as u64),
            "min_latency" => (6, u64::MAX),
            "max_latency" => (7, u64::MAX),
            "min_bandwidth" => (8, u64::MAX),
            "max_bandwidth" => (9, u64::MAX),
            "recommended_transfer_size" => (10, u64::MAX),
            "recommended_sdma_engine_id_mask" => (11, u64::MAX),
            "flags" => (12, u32::MAX as u64),
            _ => {
                return Err(TopologyError::UnknownProperty {
                    path: path.to_path_buf(),
                    key: key.to_owned(),
                });
            }
        };
        let value = raw_value
            .parse::<u64>()
            .map_err(|_| TopologyError::MalformedPropertyLine {
                path: path.to_path_buf(),
                line: index + 1,
            })?;
        if value > maximum {
            return Err(TopologyError::PropertyOutOfRange {
                path: path.to_path_buf(),
                key: key.to_owned(),
                value,
                minimum: 0,
                maximum,
            });
        }
        // Range and numeric validation precede duplicate detection in the original parser.
        let bit = 1_u16 << slot;
        if seen & bit != 0 {
            return Err(TopologyError::DuplicateProperty {
                path: path.to_path_buf(),
                key: key.to_owned(),
            });
        }
        seen |= bit;
        values[slot] = value;
    }
    for (slot, key) in KEYS.iter().enumerate() {
        if seen & (1_u16 << slot) == 0 {
            return Err(TopologyError::MissingProperty {
                path: path.to_path_buf(),
                key,
            });
        }
    }
    Ok(LinkProperties {
        link_type: values[0] as u32,
        version_major: values[1] as u32,
        version_minor: values[2] as u32,
        node_from: values[3] as u32,
        node_to: values[4] as u32,
        weight: values[5] as u32,
        min_latency: values[6],
        max_latency: values[7],
        min_bandwidth: values[8],
        max_bandwidth: values[9],
        recommended_transfer_size: values[10],
        recommended_sdma_engine_id_mask: values[11],
        flags: values[12] as u32,
    })
}
