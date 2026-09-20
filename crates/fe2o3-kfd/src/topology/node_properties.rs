//! Fixed storage for the closed node-property schema; I/O stays in the parent reader.

#![forbid(unsafe_code)]

use super::{MAX_PROPERTY_LINES, Path, TopologyError};

const PROPERTY_COUNT: usize = 39;
const _: () = assert!(PROPERTY_COUNT <= u64::BITS as usize);

#[derive(Debug)]
pub(super) struct NodeProperties {
    values: [u64; PROPERTY_COUNT],
    seen: u64,
}

impl NodeProperties {
    pub(super) fn get(&self, key: &str) -> Option<&u64> {
        let (slot, _) = field(key)?;
        (self.seen & (1_u64 << slot) != 0).then(|| &self.values[slot])
    }
}

fn field(key: &str) -> Option<(usize, u64)> {
    Some(match key {
        "array_count" => (0, 4096),
        "caches_count" => (1, 16_384),
        "capability" => (2, u64::MAX),
        "capability2" => (3, u64::MAX),
        "cpu_core_id_base" => (4, u32::MAX as u64),
        "cpu_cores_count" => (5, 65_536),
        "cu_per_simd_array" => (6, 4096),
        "debug_prop" => (7, u64::MAX),
        "device_id" => (8, u16::MAX as u64),
        "domain" => (9, u16::MAX as u64),
        "drm_render_minor" => (10, u32::MAX as u64),
        "fw_version" => (11, u32::MAX as u64),
        "gds_size_in_kb" => (12, 1 << 30),
        "gfx_target_version" => (13, 999_999),
        "hive_id" => (14, u64::MAX),
        "io_links_count" => (15, 4096),
        "lds_size_in_kb" => (16, 1 << 30),
        "local_mem_size" => (17, 1 << 60),
        "location_id" => (18, u32::MAX as u64),
        "max_engine_clk_ccompute" => (19, 100_000_000),
        "max_engine_clk_fcompute" => (20, 100_000_000),
        "max_slots_scratch_cu" => (21, 1 << 20),
        "max_waves_per_simd" => (22, 4096),
        "mem_banks_count" => (23, 4096),
        "num_cp_queues" => (24, 1 << 20),
        "num_gws" => (25, 1 << 20),
        "num_sdma_engines" => (26, 4096),
        "num_sdma_queues_per_engine" => (27, 4096),
        "num_sdma_xgmi_engines" => (28, 4096),
        "num_xcc" => (29, 64),
        "p2p_links_count" => (30, 4096),
        "sdma_fw_version" => (31, u32::MAX as u64),
        "simd_arrays_per_engine" => (32, 4096),
        "simd_count" => (33, 65_536),
        "simd_id_base" => (34, u32::MAX as u64),
        "simd_per_cu" => (35, 64),
        "unique_id" => (36, u64::MAX),
        "vendor_id" => (37, u16::MAX as u64),
        "wave_front_size" => (38, 128),
        _ => return None,
    })
}

pub(super) fn parse(path: &Path, text: &str) -> Result<NodeProperties, TopologyError> {
    if !text.ends_with('\n') {
        return Err(TopologyError::MalformedPropertyLine {
            path: path.to_path_buf(),
            line: 1,
        });
    }
    let mut properties = NodeProperties {
        values: [0; PROPERTY_COUNT],
        seen: 0,
    };
    for (index, line) in text.split_terminator('\n').enumerate() {
        let line_number = index + 1;
        if line_number > MAX_PROPERTY_LINES {
            return Err(TopologyError::TooManyProperties {
                path: path.to_path_buf(),
                maximum: MAX_PROPERTY_LINES,
            });
        }
        let mut fields = line.split(' ');
        let Some(key) = fields.next() else {
            return Err(TopologyError::MalformedPropertyLine {
                path: path.to_path_buf(),
                line: line_number,
            });
        };
        let Some(raw_value) = fields.next() else {
            return Err(TopologyError::MalformedPropertyLine {
                path: path.to_path_buf(),
                line: line_number,
            });
        };
        if fields.next().is_some()
            || key.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            || raw_value.is_empty()
            || raw_value.starts_with('0') && raw_value != "0"
            || !raw_value.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(TopologyError::MalformedPropertyLine {
                path: path.to_path_buf(),
                line: line_number,
            });
        }
        let Some((slot, maximum)) = field(key) else {
            return Err(TopologyError::UnknownProperty {
                path: path.to_path_buf(),
                key: key.to_owned(),
            });
        };
        let value = raw_value
            .parse::<u64>()
            .map_err(|_| TopologyError::MalformedPropertyLine {
                path: path.to_path_buf(),
                line: line_number,
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
        // Syntax and range checks precede duplicates, including malformed repeated keys.
        let bit = 1_u64 << slot;
        if properties.seen & bit != 0 {
            return Err(TopologyError::DuplicateProperty {
                path: path.to_path_buf(),
                key: key.to_owned(),
            });
        }
        properties.seen |= bit;
        properties.values[slot] = value;
    }
    // Missing keys remain absent; GPU admission checks them in its existing order.
    Ok(properties)
}
