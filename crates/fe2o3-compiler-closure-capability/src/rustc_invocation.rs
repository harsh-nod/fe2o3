use std::fs::File;
use std::os::fd::RawFd;
use std::process::Command;

use fe2o3_rustc_invocation::{
    MAX_DESCRIPTOR_BYTES_V3, RustcInvocationDescriptorV3, decode_descriptor_v3,
    encode_descriptor_v3,
};

use crate::sealed_image::{CapabilityRole, ImageLength, SealedCapabilityImage};

const ROLE: CapabilityRole = CapabilityRole {
    name: "rustc-invocation capability",
    memfd_name: "fe2o3-rustc-invocation-capability-v1",
};
const LENGTH: ImageLength = ImageLength::Bounded {
    max: MAX_DESCRIPTOR_BYTES_V3,
};

/// Reserved descriptor used to pass the canonical invocation from a wrapper into rustc.
pub const RUSTC_INVOCATION_CHILD_FD_V1: RawFd = 199;

/// An immutable file capability containing one canonical V3 rustc invocation descriptor.
pub struct RustcInvocationCapabilityV1 {
    descriptor: RustcInvocationDescriptorV3,
    canonical_bytes: Vec<u8>,
    pub(super) image: SealedCapabilityImage,
}

impl RustcInvocationCapabilityV1 {
    /// Creates and seals the canonical encoding of one validated V3 invocation descriptor.
    pub fn create(descriptor: RustcInvocationDescriptorV3) -> Result<Self, String> {
        let canonical_bytes = encode_descriptor_v3(&descriptor)
            .map_err(|error| format!("cannot encode rustc-invocation capability: {error}"))?;
        let image = SealedCapabilityImage::create(&canonical_bytes, ROLE, LENGTH)?;
        let admitted = Self {
            descriptor,
            canonical_bytes,
            image,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits an owned close-on-exec descriptor carrying canonical V3 bytes.
    pub fn from_file(image: File) -> Result<Self, String> {
        Self::from_image(SealedCapabilityImage::from_file(image, ROLE, LENGTH)?)
    }

    /// Admits the invocation inherited at the canonical wrapper-to-rustc descriptor number.
    pub fn from_inherited_child() -> Result<Self, String> {
        Self::from_inherited_at(RUSTC_INVOCATION_CHILD_FD_V1)
    }

    /// Retains and admits an inherited, intentionally non-close-on-exec descriptor.
    pub fn from_inherited_at(child_fd: RawFd) -> Result<Self, String> {
        Self::from_image(SealedCapabilityImage::from_inherited_at(
            child_fd, ROLE, LENGTH,
        )?)
    }

    /// Returns the exact canonical descriptor carried by this capability.
    pub const fn descriptor(&self) -> &RustcInvocationDescriptorV3 {
        &self.descriptor
    }

    /// Revalidates file identity and transport invariants, then decodes and re-encodes exact V3 bytes.
    pub fn revalidate(&self) -> Result<(), String> {
        let bytes = self.image.read_exact_bytes()?;
        let descriptor = decode_canonical(&bytes)?;
        if bytes != self.canonical_bytes || descriptor != self.descriptor {
            return Err("rustc-invocation capability bytes changed".to_owned());
        }
        Ok(())
    }

    /// Clones the exact sealed descriptor for one broker or process-boundary transfer.
    pub fn try_clone_for_transfer(&self) -> Result<File, String> {
        self.revalidate()?;
        self.image.try_clone_for_transfer()
    }

    /// Installs this exact image at the canonical wrapper-to-rustc child descriptor.
    pub fn inherit_for_child(&self, command: &mut Command) -> Result<(), String> {
        self.inherit_for_child_at(command, RUSTC_INVOCATION_CHILD_FD_V1)
    }

    /// Installs this exact image at one unoccupied child descriptor.
    pub fn inherit_for_child_at(
        &self,
        command: &mut Command,
        child_fd: RawFd,
    ) -> Result<(), String> {
        self.revalidate()?;
        self.image.inherit_for_child_at(command, child_fd)
    }

    fn from_image(image: SealedCapabilityImage) -> Result<Self, String> {
        let canonical_bytes = image.read_exact_bytes()?;
        let descriptor = decode_canonical(&canonical_bytes)?;
        let admitted = Self {
            descriptor,
            canonical_bytes,
            image,
        };
        admitted.revalidate()?;
        Ok(admitted)
    }
}

fn decode_canonical(bytes: &[u8]) -> Result<RustcInvocationDescriptorV3, String> {
    let descriptor = decode_descriptor_v3(bytes)
        .map_err(|error| format!("rustc-invocation capability is not canonical V3: {error}"))?;
    let reencoded = encode_descriptor_v3(&descriptor)
        .map_err(|error| format!("cannot re-encode rustc-invocation capability: {error}"))?;
    if reencoded != bytes {
        return Err("rustc-invocation capability has noncanonical V3 bytes".to_owned());
    }
    Ok(descriptor)
}
