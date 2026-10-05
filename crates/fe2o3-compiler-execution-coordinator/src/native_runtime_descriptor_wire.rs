//! Native x86-64 syscall data shapes. These predicates grant no custody.
pub(super) const HEADER_BYTES: usize = 56;
pub(super) const CONTROL_BYTES: usize = 32;
pub(super) const MAX_IOVECS: usize = 64;
pub(super) const MAX_PAYLOAD: u64 = 1024 * 1024;
pub(super) const MSG_DONTWAIT: u64 = 0x40;
pub(super) const MSG_CMSG_CLOEXEC: u64 = 0x4000_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Header {
    pub(super) name: u64,
    pub(super) name_length: u32,
    pub(super) vectors: u64,
    pub(super) vector_count: u64,
    pub(super) control: u64,
    pub(super) control_length: u64,
    pub(super) flags: u32,
}

impl Header {
    pub(super) fn decode(bytes: &[u8; HEADER_BYTES]) -> Self {
        Self {
            name: word(bytes, 0),
            name_length: dword(bytes, 8),
            vectors: word(bytes, 16),
            vector_count: word(bytes, 24),
            control: word(bytes, 32),
            control_length: word(bytes, 40),
            flags: dword(bytes, 48),
        }
    }

    pub(super) fn stable_fields(self, after: Self) -> bool {
        self.name == after.name
            && self.vectors == after.vectors
            && self.vector_count == after.vector_count
            && self.control == after.control
    }

    pub(super) fn credential_layout(self, address: u64, vectors: &[u8]) -> bool {
        if self.name != 0
            || self.name_length != 0
            || self.control_length != CONTROL_BYTES as u64
            || self.vector_count > MAX_IOVECS as u64
            || vectors.len() != self.vector_count as usize * 16
        {
            return false;
        }
        let Some(header) = interval(address, HEADER_BYTES as u64) else {
            return false;
        };
        let Some(control) = interval(self.control, CONTROL_BYTES as u64) else {
            return false;
        };
        if overlaps(header, control) {
            return false;
        }
        let mut total = 0_u64;
        for vector in vectors.chunks_exact(16) {
            let length = word(vector, 8);
            let Some(next) = total.checked_add(length) else {
                return false;
            };
            total = next;
            if total > MAX_PAYLOAD {
                return false;
            }
            if length != 0 {
                let Some(destination) = interval(word(vector, 0), length) else {
                    return false;
                };
                if overlaps(destination, header) || overlaps(destination, control) {
                    return false;
                }
            }
        }
        true
    }
}

pub(super) fn credential_control(header: Header, bytes: &[u8; CONTROL_BYTES]) -> bool {
    // Linux cmsghdr: length, SOL_SOCKET, SCM_CREDENTIALS, then pid/uid/gid.
    // No sender identity is established by this shape predicate.
    header.flags & (0x08 | 0x20) == 0
        && matches!(header.control_length, 28 | 32)
        && word(bytes, 0) == 28
        && dword(bytes, 8) == 1
        && dword(bytes, 12) == 2
}

pub(super) fn interval(address: u64, length: u64) -> Option<(u64, u64)> {
    if address == 0 || length == 0 {
        return None;
    }
    Some((address, address.checked_add(length)?))
}

fn overlaps(left: (u64, u64), right: (u64, u64)) -> bool {
    left.0 < right.1 && right.0 < left.1
}

fn word(bytes: &[u8], offset: usize) -> u64 {
    u64::from_ne_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("fixed native word"),
    )
}

fn dword(bytes: &[u8], offset: usize) -> u32 {
    u32::from_ne_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("fixed native dword"),
    )
}

#[cfg(test)]
#[path = "native_runtime_descriptor_wire_tests.rs"]
mod tests;
