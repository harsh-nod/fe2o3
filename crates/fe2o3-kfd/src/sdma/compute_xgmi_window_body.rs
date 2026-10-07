// Shared executable arithmetic only; no mapping, initialization, or DMA authority.
macro_rules! gfx942_compute_xgmi_window_bounds_v1 {
    ($source_len:expr, $destination_len:expr, $source_offset:expr, $destination_offset:expr, $bytes:expr) => {{
        let source_len = $source_len;
        let destination_len = $destination_len;
        let source_offset = $source_offset;
        let destination_offset = $destination_offset;
        let bytes = $bytes;
        bytes > 0
            && source_offset <= source_len
            && destination_offset <= destination_len
            && bytes <= source_len - source_offset
            && bytes <= destination_len - destination_offset
    }};
}

macro_rules! gfx942_compute_xgmi_window_packet_v1 {
    ($source_offset:expr, $destination_offset:expr, $bytes:expr, $relative:expr, $packet_bytes:expr) => {{
        let source_offset = $source_offset;
        let destination_offset = $destination_offset;
        let bytes = $bytes;
        let relative = $relative;
        let packet_bytes = $packet_bytes as u64;
        if packet_bytes == 0
            || relative > bytes
            || packet_bytes > bytes - relative
            || source_offset > u64::MAX - relative
            || destination_offset > u64::MAX - relative
        {
            None
        } else {
            let source = source_offset + relative;
            let destination = destination_offset + relative;
            if packet_bytes > u64::MAX - source || packet_bytes > u64::MAX - destination {
                None
            } else {
                Some((source, destination))
            }
        }
    }};
}
