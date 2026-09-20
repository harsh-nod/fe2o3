//! Frozen node parser from 563f5a2b17f9ab0dcbee41c4c436a8e768515f47.
//! Only the file read is factored out so identical bytes reach both parsers.

use super::*;

pub(super) fn property_range(key: &str) -> Option<(u64, u64)> {
    let range = match key {
        "array_count" => (0, 4096),
        "caches_count" => (0, 16_384),
        "capability" | "capability2" | "hive_id" | "unique_id" => (0, u64::MAX),
        "cpu_core_id_base" | "simd_id_base" | "location_id" => (0, u32::MAX as u64),
        "cpu_cores_count" | "simd_count" => (0, 65_536),
        "cu_per_simd_array" | "max_waves_per_simd" | "simd_arrays_per_engine" => (0, 4096),
        "debug_prop" => (0, u64::MAX),
        "device_id" | "domain" | "vendor_id" => (0, u16::MAX as u64),
        "drm_render_minor" => (0, u32::MAX as u64),
        "fw_version" | "sdma_fw_version" => (0, u32::MAX as u64),
        "gds_size_in_kb" | "lds_size_in_kb" => (0, 1 << 30),
        "gfx_target_version" => (0, 999_999),
        "io_links_count" | "mem_banks_count" | "p2p_links_count" => (0, 4096),
        "local_mem_size" => (0, 1 << 60),
        "max_engine_clk_ccompute" | "max_engine_clk_fcompute" => (0, 100_000_000),
        "max_slots_scratch_cu" | "num_cp_queues" | "num_gws" => (0, 1 << 20),
        "num_sdma_engines" | "num_sdma_queues_per_engine" | "num_sdma_xgmi_engines" => (0, 4096),
        "num_xcc" => (0, 64),
        "simd_per_cu" => (0, 64),
        "wave_front_size" => (0, 128),
        _ => return None,
    };
    Some(range)
}

pub(super) fn parse(path: &Path, text: &str) -> Result<BTreeMap<String, u64>, TopologyError> {
    if !text.ends_with('\n') {
        return Err(TopologyError::MalformedPropertyLine {
            path: path.to_path_buf(),
            line: 1,
        });
    }
    let mut properties = BTreeMap::new();
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
        let Some((minimum, maximum)) = property_range(key) else {
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
        if value < minimum || value > maximum {
            return Err(TopologyError::PropertyOutOfRange {
                path: path.to_path_buf(),
                key: key.to_owned(),
                value,
                minimum,
                maximum,
            });
        }
        if properties.insert(key.to_owned(), value).is_some() {
            return Err(TopologyError::DuplicateProperty {
                path: path.to_path_buf(),
                key: key.to_owned(),
            });
        }
    }
    Ok(properties)
}
