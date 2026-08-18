//! Internal canonical identity-preimage encoder.

use alloc::vec::Vec;

use crate::{HOST_API_SCHEMA_VERSION_V1, HostDigestV1};

const PREIMAGE_MAGIC_V1: &[u8; 8] = b"F2HOSTP1";

pub(crate) struct EncoderV1 {
    bytes: Vec<u8>,
}

impl EncoderV1 {
    pub(crate) fn new(domain: &[u8]) -> Self {
        let mut bytes = Vec::with_capacity(128);
        bytes.extend_from_slice(PREIMAGE_MAGIC_V1);
        bytes.extend_from_slice(&(domain.len() as u16).to_le_bytes());
        bytes.extend_from_slice(domain);
        bytes.extend_from_slice(&HOST_API_SCHEMA_VERSION_V1.to_le_bytes());
        Self { bytes }
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn usize_as_u16(&mut self, value: usize) {
        self.u16(value as u16);
    }

    pub(crate) fn digest(&mut self, digest: HostDigestV1) {
        self.bytes.extend_from_slice(digest.as_bytes());
    }

    pub(crate) fn optional_digest(&mut self, digest: Option<HostDigestV1>) {
        match digest {
            Some(digest) => {
                self.u8(1);
                self.digest(digest);
            }
            None => self.u8(0),
        }
    }

    pub(crate) fn text(&mut self, text: &str) {
        self.u16(text.len() as u16);
        self.bytes.extend_from_slice(text.as_bytes());
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
