// These executable arithmetic bodies are also checked by the packet-plan proof.
macro_rules! compute_xgmi_packet_count_body_v1 {
    ($total:ident, $cap:expr, $limit:expr) => {{
        let packet_bytes: u64 = $cap;
        let max_packets: u64 = $limit;
        if $total == 0 || $total > packet_bytes * max_packets {
            None
        } else {
            Some((($total - 1) / packet_bytes + 1) as usize)
        }
    }};
}

macro_rules! compute_xgmi_packet_at_body_v1 {
    ($total:ident, $count:ident, $index:ident, $cap:expr) => {{
        if $index >= $count {
            None
        } else {
            let packet_bytes: u64 = $cap;
            let offset = $index as u64 * packet_bytes;
            let remaining = $total - offset;
            let bytes = if remaining > packet_bytes {
                packet_bytes
            } else {
                remaining
            };
            Some((offset, bytes as u32))
        }
    }};
}
