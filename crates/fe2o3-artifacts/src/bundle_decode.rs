use std::fmt;

use crate::{
    BUNDLE_INDEX_MAGIC, BUNDLE_INDEX_VERSION, BundleIndexV1, BundleKernelIndexEntryV1,
    BundlePayloadReferenceV1, BundleTargetAssociationV1, BundleValidationError, Capability,
    CodeObjectFormat, DigestBytes, Endianness, IdentityText, MAX_BUNDLE_INDEX_BYTES,
    MAX_BUNDLE_KERNELS, MAX_BUNDLE_PAYLOAD_REFERENCES, MAX_BUNDLE_TARGET_ASSOCIATIONS,
    MAX_IDENTITY_TEXT_BYTES, MAX_KERNEL_PAYLOAD_REFERENCES, MAX_NAME_BYTES, Name, PointerWidth,
    TargetIdentity, ValidationError,
};

const CAPABILITY_COUNT: usize = 11;

impl BundleIndexV1 {
    /// Decodes a canonical index after bounded wire and reference validation.
    ///
    /// Successful decoding does not authenticate referenced manifests or
    /// payloads and does not grant authority to load or launch a kernel.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BundleDecodeError> {
        if bytes.len() > MAX_BUNDLE_INDEX_BYTES {
            return Err(BundleDecodeError::TooLarge {
                max: MAX_BUNDLE_INDEX_BYTES,
            });
        }

        let mut reader = Reader::new(bytes);
        if reader.array::<8>()? != BUNDLE_INDEX_MAGIC {
            return Err(BundleDecodeError::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != BUNDLE_INDEX_VERSION {
            return Err(BundleDecodeError::UnknownVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(BundleDecodeError::UnsupportedFlags(flags));
        }

        let target_count = reader.count_u32(
            "bundle target associations",
            1,
            MAX_BUNDLE_TARGET_ASSOCIATIONS,
        )?;
        let mut target_associations = Vec::with_capacity(target_count);
        for _ in 0..target_count {
            let manifest_digest = reader.digest()?;
            let target = reader.target()?;
            target_associations.push(BundleTargetAssociationV1::new(manifest_digest, target));
        }
        ensure_digest_order(
            &target_associations,
            BundleTargetAssociationV1::manifest_digest,
            "bundle target associations",
            "bundle manifest digest",
        )?;

        let payload_count = reader.count_u32(
            "bundle payload references",
            1,
            MAX_BUNDLE_PAYLOAD_REFERENCES,
        )?;
        let mut payloads = Vec::with_capacity(payload_count);
        for _ in 0..payload_count {
            payloads.push(BundlePayloadReferenceV1::new(
                reader.digest()?,
                reader.code_object_format()?,
                reader.u64()?,
            )?);
        }
        ensure_digest_order(
            &payloads,
            BundlePayloadReferenceV1::digest,
            "bundle payload references",
            "bundle payload digest",
        )?;

        let kernel_count = reader.count_u32("bundle kernels", 1, MAX_BUNDLE_KERNELS)?;
        let mut kernels = Vec::with_capacity(kernel_count);
        for _ in 0..kernel_count {
            let kernel_id = reader.digest()?;
            let symbol = reader.name()?;
            let manifest_digest = reader.digest()?;
            let payload_count = reader.count_u16(
                "kernel payload references",
                1,
                MAX_KERNEL_PAYLOAD_REFERENCES,
            )?;
            let mut payload_digests = Vec::with_capacity(payload_count);
            for _ in 0..payload_count {
                payload_digests.push(reader.digest()?);
            }
            ensure_digest_order(
                &payload_digests,
                |digest| *digest,
                "kernel payload references",
                "kernel payload reference",
            )?;
            kernels.push(BundleKernelIndexEntryV1::new(
                kernel_id,
                symbol,
                manifest_digest,
                payload_digests,
            )?);
        }
        ensure_digest_order(
            &kernels,
            BundleKernelIndexEntryV1::kernel_id,
            "bundle kernels",
            "bundle kernel ID",
        )?;

        if !reader.is_empty() {
            return Err(BundleDecodeError::TrailingBytes);
        }

        Ok(Self::new(target_associations, payloads, kernels)?)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BundleDecodeError {
    TooLarge {
        max: usize,
    },
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    UnknownTag {
        kind: &'static str,
        tag: u8,
    },
    UnknownCapability(u16),
    CountOutOfRange {
        field: &'static str,
        count: u64,
        min: usize,
        max: usize,
    },
    NonCanonicalOrder {
        field: &'static str,
    },
    TrailingBytes,
    Model(ValidationError),
    Validation(BundleValidationError),
}

impl fmt::Display for BundleDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(formatter, "bundle index exceeds {max} bytes"),
            Self::Truncated => write!(formatter, "bundle index is truncated"),
            Self::InvalidMagic => write!(formatter, "bundle index magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(formatter, "unsupported bundle index version {version}")
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported bundle index flags {flags:#x}")
            }
            Self::UnknownTag { kind, tag } => write!(formatter, "unknown {kind} tag {tag}"),
            Self::UnknownCapability(tag) => write!(formatter, "unknown capability tag {tag}"),
            Self::CountOutOfRange {
                field,
                count,
                min,
                max,
            } => write!(formatter, "{field} count {count} is outside {min}..={max}"),
            Self::NonCanonicalOrder { field } => {
                write!(formatter, "{field} entries are not in canonical order")
            }
            Self::TrailingBytes => write!(formatter, "bundle index contains trailing bytes"),
            Self::Model(error) => error.fmt(formatter),
            Self::Validation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for BundleDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Model(error) => Some(error),
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ValidationError> for BundleDecodeError {
    fn from(value: ValidationError) -> Self {
        Self::Model(value)
    }
}

impl From<BundleValidationError> for BundleDecodeError {
    fn from(value: BundleValidationError) -> Self {
        Self::Validation(value)
    }
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], BundleDecodeError> {
        if self.remaining.len() < count {
            return Err(BundleDecodeError::Truncated);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], BundleDecodeError> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, BundleDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, BundleDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, BundleDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, BundleDecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn count_u16(
        &mut self,
        field: &'static str,
        min: usize,
        max: usize,
    ) -> Result<usize, BundleDecodeError> {
        let count = u64::from(self.u16()?);
        validate_count(field, count, min, max)?;
        Ok(count as usize)
    }

    fn count_u32(
        &mut self,
        field: &'static str,
        min: usize,
        max: usize,
    ) -> Result<usize, BundleDecodeError> {
        let count = u64::from(self.u32()?);
        validate_count(field, count, min, max)?;
        Ok(count as usize)
    }

    fn text(&mut self, field: &'static str, max: usize) -> Result<&'a str, BundleDecodeError> {
        let count = self.count_u16(field, 1, max)?;
        let bytes = self.take(count)?;
        std::str::from_utf8(bytes).map_err(|_| ValidationError::InvalidText { field }.into())
    }

    fn name(&mut self) -> Result<Name, BundleDecodeError> {
        Ok(Name::new(self.text("name", MAX_NAME_BYTES)?)?)
    }

    fn identity_text(&mut self) -> Result<IdentityText, BundleDecodeError> {
        Ok(IdentityText::new(
            self.text("identity text", MAX_IDENTITY_TEXT_BYTES)?,
        )?)
    }

    fn digest(&mut self) -> Result<DigestBytes, BundleDecodeError> {
        Ok(DigestBytes::from_bytes(self.array()?))
    }

    fn target(&mut self) -> Result<TargetIdentity, BundleDecodeError> {
        Ok(TargetIdentity::new(
            self.identity_text()?,
            self.identity_text()?,
            self.pointer_width()?,
            self.endianness()?,
            self.capabilities()?,
        )?)
    }

    fn pointer_width(&mut self) -> Result<PointerWidth, BundleDecodeError> {
        match self.u8()? {
            0 => Ok(PointerWidth::Bits32),
            1 => Ok(PointerWidth::Bits64),
            tag => Err(BundleDecodeError::UnknownTag {
                kind: "pointer width",
                tag,
            }),
        }
    }

    fn endianness(&mut self) -> Result<Endianness, BundleDecodeError> {
        match self.u8()? {
            0 => Ok(Endianness::Little),
            1 => Ok(Endianness::Big),
            tag => Err(BundleDecodeError::UnknownTag {
                kind: "endianness",
                tag,
            }),
        }
    }

    fn capabilities(&mut self) -> Result<Vec<Capability>, BundleDecodeError> {
        let count = self.count_u16("target capabilities", 0, CAPABILITY_COUNT)?;
        let mut capabilities = Vec::with_capacity(count);
        for _ in 0..count {
            capabilities.push(capability_from_tag(self.u16()?)?);
        }
        ensure_ordered_values(&capabilities, "target capabilities", "target capability")?;
        Ok(capabilities)
    }

    fn code_object_format(&mut self) -> Result<CodeObjectFormat, BundleDecodeError> {
        match self.u8()? {
            0 => Ok(CodeObjectFormat::NativeExecutable),
            1 => Ok(CodeObjectFormat::RelocatableObject),
            2 => Ok(CodeObjectFormat::LlvmBitcode),
            3 => Ok(CodeObjectFormat::SpirV),
            tag => Err(BundleDecodeError::UnknownTag {
                kind: "code object format",
                tag,
            }),
        }
    }
}

fn validate_count(
    field: &'static str,
    count: u64,
    min: usize,
    max: usize,
) -> Result<(), BundleDecodeError> {
    if count < min as u64 || count > max as u64 {
        Err(BundleDecodeError::CountOutOfRange {
            field,
            count,
            min,
            max,
        })
    } else {
        Ok(())
    }
}

fn ensure_digest_order<T>(
    values: &[T],
    key: impl Fn(&T) -> DigestBytes,
    order_field: &'static str,
    duplicate_field: &'static str,
) -> Result<(), BundleDecodeError> {
    for pair in values.windows(2) {
        if key(&pair[0]) == key(&pair[1]) {
            return Err(BundleValidationError::Duplicate {
                field: duplicate_field,
            }
            .into());
        }
        if key(&pair[0]) > key(&pair[1]) {
            return Err(BundleDecodeError::NonCanonicalOrder { field: order_field });
        }
    }
    Ok(())
}

fn ensure_ordered_values<T: Ord>(
    values: &[T],
    order_field: &'static str,
    duplicate_field: &'static str,
) -> Result<(), BundleDecodeError> {
    for pair in values.windows(2) {
        if pair[0] == pair[1] {
            return Err(ValidationError::Duplicate {
                field: duplicate_field,
            }
            .into());
        }
        if pair[0] > pair[1] {
            return Err(BundleDecodeError::NonCanonicalOrder { field: order_field });
        }
    }
    Ok(())
}

fn capability_from_tag(tag: u16) -> Result<Capability, BundleDecodeError> {
    match tag {
        0 => Ok(Capability::Subgroup),
        1 => Ok(Capability::Ballot),
        2 => Ok(Capability::Shuffle),
        3 => Ok(Capability::WorkgroupMemory),
        4 => Ok(Capability::MatrixMultiply),
        5 => Ok(Capability::AsyncCopy),
        6 => Ok(Capability::Atomics),
        7 => Ok(Capability::AmdWave),
        8 => Ok(Capability::AmdMfma),
        9 => Ok(Capability::AmdWmma),
        10 => Ok(Capability::AmdDsPermute),
        _ => Err(BundleDecodeError::UnknownCapability(tag)),
    }
}
