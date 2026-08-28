use std::fs::File;

use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1, CompilerExecutionClientProfileV1,
};

use crate::sealed_image::{CapabilityRole, ImageLength, SealedCapabilityImage};

const ROLE: CapabilityRole = CapabilityRole {
    name: "compiler-execution client-profile capability",
    memfd_name: "fe2o3-compiler-execution-client-profile-v1",
};
const LENGTH: ImageLength = ImageLength::Exact(COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1);

/// Immutable descriptor capability carrying one exact compiler-execution client profile.
///
/// The profile is public trust configuration, not authority. This value grants no compiler,
/// signing, publication, loading, launch, or execution operation.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<CompilerExecutionClientProfileCapabilityV1>();
/// ```
///
/// ```compile_fail
/// use std::os::fd::AsFd;
/// use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV1;
/// fn require_as_fd<T: AsFd>() {}
/// require_as_fd::<CompilerExecutionClientProfileCapabilityV1>();
/// ```
pub struct CompilerExecutionClientProfileCapabilityV1 {
    profile: CompilerExecutionClientProfileV1,
    image: SealedCapabilityImage,
}

impl CompilerExecutionClientProfileCapabilityV1 {
    /// Creates and seals the exact canonical client-profile image.
    pub fn create(profile: CompilerExecutionClientProfileV1) -> Result<Self, String> {
        let image = SealedCapabilityImage::create(profile.canonical_bytes(), ROLE, LENGTH)?;
        let admitted = Self { profile, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Admits an already transferred immutable client-profile image.
    pub fn from_file(image: File) -> Result<Self, String> {
        let image = SealedCapabilityImage::from_file(image, ROLE, LENGTH)?;
        let profile = decode(&image.read_exact_bytes()?)?;
        let admitted = Self { profile, image };
        admitted.revalidate()?;
        Ok(admitted)
    }

    /// Borrows the exact public client profile.
    pub const fn profile(&self) -> &CompilerExecutionClientProfileV1 {
        &self.profile
    }

    /// Revalidates descriptor identity, mode, seals, length, bytes, and canonical profile equality.
    pub fn revalidate(&self) -> Result<(), String> {
        let current = decode(&self.image.read_exact_bytes()?)?;
        if current != self.profile {
            return Err("compiler-execution client-profile capability bytes changed".to_owned());
        }
        Ok(())
    }

    /// Clones the same sealed descriptor for one authenticated broker transfer.
    pub fn try_clone_for_transfer(&self) -> Result<File, String> {
        self.revalidate()?;
        self.image.try_clone_for_transfer()
    }
}

fn decode(bytes: &[u8]) -> Result<CompilerExecutionClientProfileV1, String> {
    CompilerExecutionClientProfileV1::decode(bytes).map_err(|error| {
        format!("compiler-execution client-profile capability is not canonical: {error}")
    })
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::os::unix::fs::PermissionsExt;

    use ed25519_dalek::SigningKey;
    use fe2o3_compiler_execution_protocol::{
        CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
    };

    use super::*;
    use crate::sealed_image::{REQUIRED_SEALS, SealedCapabilityImage};

    fn profile(seed: u8) -> CompilerExecutionClientProfileV1 {
        let key = SigningKey::from_bytes(&[seed; 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 456).unwrap(),
            key.verifying_key().to_bytes(),
        )
        .unwrap();
        CompilerExecutionClientProfileV1::new(1_234, 5_678, policy).unwrap()
    }

    #[test]
    fn exact_profile_is_sealed_transferred_and_revalidated() {
        let expected = profile(7);
        let capability =
            CompilerExecutionClientProfileCapabilityV1::create(expected.clone()).unwrap();
        assert_eq!(capability.profile(), &expected);
        assert_eq!(
            rustix::fs::fcntl_get_seals(capability.image.as_file()).unwrap(),
            REQUIRED_SEALS
        );
        assert!(capability.image.as_file().set_len(0).is_err());
        let transferred = capability.try_clone_for_transfer().unwrap();
        let recovered = CompilerExecutionClientProfileCapabilityV1::from_file(transferred).unwrap();
        assert_eq!(recovered.profile(), &expected);
        recovered.revalidate().unwrap();
    }

    #[test]
    fn mutable_and_malformed_sealed_images_fail_closed() {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-compiler-execution-client-profile-hostile-{}",
            std::process::id()
        ));
        fs::write(&path, profile(7).canonical_bytes()).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
        assert!(
            CompilerExecutionClientProfileCapabilityV1::from_file(File::open(&path).unwrap())
                .is_err()
        );
        fs::remove_file(path).unwrap();

        let mut malformed = *profile(7).canonical_bytes();
        malformed[0] ^= 1;
        let malformed_image = SealedCapabilityImage::create(&malformed, ROLE, LENGTH).unwrap();
        assert!(
            CompilerExecutionClientProfileCapabilityV1::from_file(
                malformed_image.try_clone_for_transfer().unwrap()
            )
            .is_err()
        );
    }

    #[test]
    fn independently_sealed_profile_substitution_remains_distinct() {
        let first = CompilerExecutionClientProfileCapabilityV1::create(profile(7)).unwrap();
        let second = CompilerExecutionClientProfileCapabilityV1::create(profile(8)).unwrap();
        assert_ne!(first.profile(), second.profile());
        assert_ne!(first.profile().identity(), second.profile().identity());
    }
}
