//! Fixed secret staging shared by issuer and external-anchor custody.
use crate::{native_capability::Result, sealed_image::SealedCapabilityImage};
use zeroize::Zeroize;

pub(crate) struct SeedGuard<'a, const N: usize>(pub(crate) &'a mut [u8; N]);
impl<const N: usize> Drop for SeedGuard<'_, N> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

pub(crate) fn read_secret<const N: usize, T>(
    image: &SealedCapabilityImage,
    use_secret: impl FnOnce(&[u8; N]) -> Result<T>,
) -> Result<T> {
    image.validate_secret_fixed()?;
    let mut bytes = [0; N];
    with_secret(
        &mut bytes,
        |bytes| {
            image.read_fixed_into(bytes)?;
            image.validate_secret_fixed()
        },
        use_secret,
    )
}

// Guard before I/O: short reads and post-read refusal may already have written secrets.
pub(crate) fn with_secret<const N: usize, T>(
    bytes: &mut [u8; N],
    read: impl FnOnce(&mut [u8; N]) -> Result<()>,
    use_secret: impl FnOnce(&[u8; N]) -> Result<T>,
) -> Result<T> {
    let bytes = SeedGuard(bytes);
    read(bytes.0)?;
    use_secret(bytes.0)
}
