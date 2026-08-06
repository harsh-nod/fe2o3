use crate::{
    Capability, DigestBytes, IdentityText, Name, TargetIdentity,
    bundle::BundleIndexV1,
    encode::{capability_tag, code_object_format_tag, endianness_tag, pointer_width_tag},
};

pub const BUNDLE_INDEX_MAGIC: [u8; 8] = *b"FE2O3BI\0";
pub const BUNDLE_INDEX_VERSION: u16 = 1;
pub const MAX_BUNDLE_INDEX_BYTES: usize = 4 * 1024 * 1024;

impl BundleIndexV1 {
    /// Encodes this validated index using the canonical v1 binary format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.bytes(&BUNDLE_INDEX_MAGIC);
        writer.u16(BUNDLE_INDEX_VERSION);
        writer.u16(0);

        writer.u32(self.target_associations().len() as u32);
        for association in self.target_associations() {
            writer.digest(association.manifest_digest());
            writer.target(association.target());
        }

        writer.u32(self.payloads().len() as u32);
        for payload in self.payloads() {
            writer.digest(payload.digest());
            writer.u8(code_object_format_tag(payload.format()));
            writer.u64(payload.byte_len());
        }

        writer.u32(self.kernels().len() as u32);
        for kernel in self.kernels() {
            writer.digest(kernel.kernel_id());
            writer.name(kernel.symbol());
            writer.digest(kernel.manifest_digest());
            writer.u16(kernel.payload_digests().len() as u16);
            for digest in kernel.payload_digests() {
                writer.digest(*digest);
            }
        }

        debug_assert!(writer.bytes.len() <= MAX_BUNDLE_INDEX_BYTES);
        writer.bytes
    }
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(512),
        }
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn text(&mut self, value: &str) {
        self.u16(value.len() as u16);
        self.bytes(value.as_bytes());
    }

    fn name(&mut self, value: &Name) {
        self.text(value.as_str());
    }

    fn identity_text(&mut self, value: &IdentityText) {
        self.text(value.as_str());
    }

    fn digest(&mut self, value: DigestBytes) {
        self.bytes(value.as_bytes());
    }

    fn target(&mut self, target: &TargetIdentity) {
        self.identity_text(target.triple());
        self.identity_text(target.architecture());
        self.u8(pointer_width_tag(target.pointer_width()));
        self.u8(endianness_tag(target.endianness()));
        self.capabilities(target.capabilities());
    }

    fn capabilities(&mut self, capabilities: &[Capability]) {
        self.u16(capabilities.len() as u16);
        for capability in capabilities {
            self.u16(capability_tag(*capability));
        }
    }
}
