use std::fmt;

use super::*;

/// Failure to decode the bounded canonical inert semantic MIR representation.
///
/// Decoding performs structural admission and exact canonical re-encoding. It
/// does not authenticate the producer or grant proof, compiler, artifact,
/// publication, load, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticMirDecodeErrorV1 {
    InputLimitExceeded {
        actual: u64,
        max: u64,
    },
    UnexpectedEnd {
        offset: usize,
        requested: usize,
    },
    InvalidMagic,
    UnsupportedVersion(u16),
    WireVersionMismatch {
        expected: SemanticMirWireVersionV1,
        actual: SemanticMirWireVersionV1,
    },
    UnsupportedProductionWireVersion(SemanticMirWireVersionV1),
    InvalidBoolean {
        offset: usize,
        value: u8,
    },
    InvalidTag {
        context: &'static str,
        offset: usize,
        value: u8,
    },
    AllocationFailed {
        context: &'static str,
    },
    LengthOverflow {
        context: &'static str,
    },
    TrailingBytes {
        offset: usize,
        trailing: usize,
    },
    NonCanonical,
    Validation(SemanticMirErrorV1),
}

impl fmt::Display for SemanticMirDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputLimitExceeded { actual, max } => {
                write!(
                    formatter,
                    "semantic MIR input uses {actual} bytes, exceeding {max}"
                )
            }
            Self::UnexpectedEnd { offset, requested } => write!(
                formatter,
                "semantic MIR input ended at byte {offset} while reading {requested} bytes"
            ),
            Self::InvalidMagic => formatter.write_str("invalid semantic MIR magic"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported semantic MIR version {version}")
            }
            Self::WireVersionMismatch { expected, actual } => write!(
                formatter,
                "semantic MIR wire version {actual:?} does not match required {expected:?}"
            ),
            Self::UnsupportedProductionWireVersion(actual) => write!(
                formatter,
                "semantic MIR wire version {actual:?} is outside the current production custody policy"
            ),
            Self::InvalidBoolean { offset, value } => {
                write!(
                    formatter,
                    "invalid semantic MIR boolean {value} at byte {offset}"
                )
            }
            Self::InvalidTag {
                context,
                offset,
                value,
            } => write!(
                formatter,
                "invalid semantic MIR {context} tag {value} at byte {offset}"
            ),
            Self::AllocationFailed { context } => {
                write!(formatter, "semantic MIR {context} allocation failed")
            }
            Self::LengthOverflow { context } => {
                write!(
                    formatter,
                    "semantic MIR {context} length does not fit this host"
                )
            }
            Self::TrailingBytes { offset, trailing } => write!(
                formatter,
                "semantic MIR input has {trailing} trailing bytes after byte {offset}"
            ),
            Self::NonCanonical => formatter.write_str("semantic MIR input is not canonical"),
            Self::Validation(error) => write!(formatter, "invalid semantic MIR model: {error}"),
        }
    }
}

impl std::error::Error for SemanticMirDecodeErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SemanticMirErrorV1> for SemanticMirDecodeErrorV1 {
    fn from(value: SemanticMirErrorV1) -> Self {
        Self::Validation(value)
    }
}

impl AdmittedInertSemanticMirV1 {
    /// Decodes the exact bounded minimal-compatible canonical representation.
    ///
    /// Declared resource counts are checked before their records are read, the
    /// automatic compatibility admission validator is run, and its canonical
    /// output must exactly equal `bytes`. Ordinary V3 envelopes without V3-only
    /// content remain noncanonical through this compatibility API. Use
    /// [`Self::decode_exact_v3_canonical`] when V3 is the selected custody
    /// schema. The returned value remains inert and grants no authority.
    pub fn decode_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_minimal_compatible_canonical(bytes, limits)
    }

    /// Precisely named form of [`Self::decode_canonical`]: accepts only the
    /// canonical encoding selected by minimal-compatible admission.
    pub fn decode_minimal_compatible_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(bytes, limits, CanonicalDecodePolicyV1::MinimalCompatible)
    }

    /// Decodes one exact canonical semantic-MIR value admitted by the current
    /// production custody policy.
    ///
    /// Production collection deliberately selects at least V5 even when an
    /// older schema could represent the same model. This decoder preserves the
    /// declared production schema instead of re-admitting under the minimum
    /// compatible version. It grants no producer, proof, compiler, artifact,
    /// publication, load, or launch authority.
    pub fn decode_current_production_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(bytes, limits, CanonicalDecodePolicyV1::CurrentProduction)
    }

    /// Decodes bytes that are canonical specifically under the closed V3 wire
    /// schema, including ordinary models with no V3-only rvalue.
    ///
    /// The declared wire field must itself be V3; V2 bytes cannot be relabeled
    /// by the caller. Parser-owned vector reservations are fallible. Parsing
    /// and exact reencoding are linear in independently bounded canonical bytes
    /// and structural resources; `ValidationWork` retains its existing scope
    /// over semantic admission traversal only.
    pub fn decode_exact_v3_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V3),
        )
    }

    /// Decodes bytes canonical specifically under the closed ownership-bearing
    /// V4 wire schema.
    pub fn decode_exact_v4_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V4),
        )
    }

    /// Decodes bytes canonical specifically under the closed V5 schema.
    pub fn decode_exact_v5_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V5),
        )
    }

    /// Decodes bytes canonical specifically under the closed V6 collective and LDS transpose
    /// schema.
    pub fn decode_exact_v6_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V6),
        )
    }

    /// Decodes bytes canonical specifically under the closed V7
    /// source-resource schema.
    pub fn decode_exact_v7_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V7),
        )
    }

    /// Decodes bytes canonical specifically under the closed V8 BF16 conversion schema.
    pub fn decode_exact_v8_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V8),
        )
    }

    /// Decodes bytes canonical specifically under the closed V9 schema for
    /// target-neutral workgroup reduction and the combined workgroup-pipeline
    /// plus BF16-conversion surface.
    pub fn decode_exact_v9_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V9),
        )
    }

    /// Decodes bytes canonical specifically under the closed V10 schema for
    /// target-neutral inclusive and exclusive workgroup scans.
    pub fn decode_exact_v10_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V10),
        )
    }

    /// Decodes bytes canonical specifically under the closed V11 schema that
    /// adds the compiler trap terminal at its unique tag.
    pub fn decode_exact_v11_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V11),
        )
    }

    /// Decodes bytes canonical specifically under the closed V12 schema that
    /// retains the V11 trap and adds checked scalar volatile loads.
    pub fn decode_exact_v12_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V12),
        )
    }

    fn decode_with_policy(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
        policy: CanonicalDecodePolicyV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        let actual = u64::try_from(bytes.len())
            .map_err(|_| SemanticMirDecodeErrorV1::LengthOverflow { context: "input" })?;
        let max = limits.limit(SemanticMirResourceV1::CanonicalBytes);
        if actual > max {
            return Err(SemanticMirDecodeErrorV1::InputLimitExceeded { actual, max });
        }

        let mut decoder = CanonicalDecoderV1::with_expected_wire_version(
            bytes,
            limits,
            policy.expected_wire_version(),
        );
        let request = decoder.request()?;
        let wire_version = decoder.wire_version;
        decoder.finish()?;
        let admitted = match policy {
            CanonicalDecodePolicyV1::MinimalCompatible => request.admit(limits)?,
            CanonicalDecodePolicyV1::Exact(expected) => {
                debug_assert_eq!(wire_version, expected);
                request.admit_for_wire_version(wire_version, limits)?
            }
            CanonicalDecodePolicyV1::CurrentProduction => {
                if !matches!(
                    wire_version,
                    SemanticMirWireVersionV1::V5
                        | SemanticMirWireVersionV1::V6
                        | SemanticMirWireVersionV1::V7
                        | SemanticMirWireVersionV1::V8
                        | SemanticMirWireVersionV1::V9
                        | SemanticMirWireVersionV1::V10
                        | SemanticMirWireVersionV1::V11
                        | SemanticMirWireVersionV1::V12
                ) {
                    return Err(SemanticMirDecodeErrorV1::UnsupportedProductionWireVersion(
                        wire_version,
                    ));
                }
                request.admit_for_wire_version(wire_version, limits)?
            }
        };
        if admitted.canonical_encoding() != bytes {
            return Err(SemanticMirDecodeErrorV1::NonCanonical);
        }
        Ok(admitted)
    }
}

#[derive(Clone, Copy)]
enum CanonicalDecodePolicyV1 {
    MinimalCompatible,
    Exact(SemanticMirWireVersionV1),
    CurrentProduction,
}

impl CanonicalDecodePolicyV1 {
    const fn expected_wire_version(self) -> Option<SemanticMirWireVersionV1> {
        match self {
            Self::Exact(version) => Some(version),
            Self::MinimalCompatible | Self::CurrentProduction => None,
        }
    }
}

#[derive(Default)]
struct DecodeTotalsV1 {
    locals: u64,
    blocks: u64,
    statements: u64,
    projections: u64,
    operands: u64,
    call_arguments: u64,
    switch_targets: u64,
    relocations: u64,
    constant_bytes: u64,
    link_symbol_bytes: u64,
}

struct CanonicalDecoderV1<'a> {
    bytes: &'a [u8],
    offset: usize,
    wire_version: SemanticMirWireVersionV1,
    expected_wire_version: Option<SemanticMirWireVersionV1>,
    limits: SemanticMirLimitsV1,
    totals: DecodeTotalsV1,
}

impl<'a> CanonicalDecoderV1<'a> {
    #[cfg(test)]
    fn new(bytes: &'a [u8], limits: SemanticMirLimitsV1) -> Self {
        Self::with_expected_wire_version(bytes, limits, None)
    }

    fn with_expected_wire_version(
        bytes: &'a [u8],
        limits: SemanticMirLimitsV1,
        expected_wire_version: Option<SemanticMirWireVersionV1>,
    ) -> Self {
        Self {
            bytes,
            offset: 0,
            wire_version: SemanticMirWireVersionV1::V6,
            expected_wire_version,
            limits,
            totals: DecodeTotalsV1::default(),
        }
    }

    fn request(&mut self) -> Result<InertSemanticMirRequestV1, SemanticMirDecodeErrorV1> {
        if self.raw(MAGIC.len())? != MAGIC {
            return Err(SemanticMirDecodeErrorV1::InvalidMagic);
        }
        let raw_version = self.u16()?;
        let version = SemanticMirWireVersionV1::from_u16(raw_version)
            .ok_or(SemanticMirDecodeErrorV1::UnsupportedVersion(raw_version))?;
        if let Some(expected) = self.expected_wire_version
            && version != expected
        {
            return Err(SemanticMirDecodeErrorV1::WireVersionMismatch {
                expected,
                actual: version,
            });
        }
        self.wire_version = version;
        let layout_identity = SemanticLayoutIdentityV1(self.identity()?);
        let architecture = match self.tag("target architecture")? {
            0 => SemanticTargetArchitectureV1::AmdGpuGfx942,
            _ => unreachable!(),
        };
        let object_size_bound_bytes = self.u64()?;
        let target = SemanticTargetDataLayoutV1 {
            identity: layout_identity,
            architecture,
            object_size_bound_bytes,
        };
        if target != SemanticTargetDataLayoutV1::gfx942(layout_identity) {
            return Err(SemanticMirErrorV1::InvalidTypeLayout.into());
        }

        let types = self.records("types", Some(SemanticMirResourceV1::Types), Self::ty)?;
        let allocations = self.records(
            "allocations",
            Some(SemanticMirResourceV1::Allocations),
            Self::allocation,
        )?;
        let statics = self.records(
            "statics",
            Some(SemanticMirResourceV1::Statics),
            Self::static_decl,
        )?;
        let vtables = self.records(
            "vtables",
            Some(SemanticMirResourceV1::VTables),
            Self::vtable,
        )?;
        let functions = self.records(
            "functions",
            Some(SemanticMirResourceV1::Functions),
            Self::function,
        )?;
        let callables = self.records(
            "callables",
            Some(SemanticMirResourceV1::Callables),
            Self::callable,
        )?;
        let roots = self.records("roots", Some(SemanticMirResourceV1::Roots), |decoder| {
            Ok(SemanticFunctionIdV1(decoder.u32()?))
        })?;

        InertSemanticMirRequestV1::new_with_callables(
            target,
            types,
            allocations,
            statics,
            vtables,
            functions,
            callables,
            roots,
        )
        .map_err(Into::into)
    }

    fn finish(&self) -> Result<(), SemanticMirDecodeErrorV1> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(SemanticMirDecodeErrorV1::TrailingBytes {
                offset: self.offset,
                trailing: self.bytes.len() - self.offset,
            })
        }
    }

    fn raw(&mut self, length: usize) -> Result<&'a [u8], SemanticMirDecodeErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(SemanticMirDecodeErrorV1::LengthOverflow { context: "field" })?;
        let Some(value) = self.bytes.get(self.offset..end) else {
            return Err(SemanticMirDecodeErrorV1::UnexpectedEnd {
                offset: self.offset,
                requested: length,
            });
        };
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], SemanticMirDecodeErrorV1> {
        self.raw(N)?
            .try_into()
            .map_err(|_| SemanticMirDecodeErrorV1::UnexpectedEnd {
                offset: self.offset,
                requested: N,
            })
    }

    fn identity(&mut self) -> Result<[u8; 32], SemanticMirDecodeErrorV1> {
        self.array()
    }

    fn u8(&mut self) -> Result<u8, SemanticMirDecodeErrorV1> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, SemanticMirDecodeErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, SemanticMirDecodeErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, SemanticMirDecodeErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn i64(&mut self) -> Result<i64, SemanticMirDecodeErrorV1> {
        Ok(i64::from_le_bytes(self.array()?))
    }

    fn u128(&mut self) -> Result<u128, SemanticMirDecodeErrorV1> {
        Ok(u128::from_le_bytes(self.array()?))
    }

    fn boolean(&mut self) -> Result<bool, SemanticMirDecodeErrorV1> {
        let offset = self.offset;
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(SemanticMirDecodeErrorV1::InvalidBoolean { offset, value }),
        }
    }

    fn tag(&mut self, context: &'static str) -> Result<u8, SemanticMirDecodeErrorV1> {
        let offset = self.offset;
        let value = self.u8()?;
        if value == 0 {
            Ok(value)
        } else {
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context,
                offset,
                value,
            })
        }
    }

    fn tagged(
        &mut self,
        context: &'static str,
        maximum: u8,
    ) -> Result<u8, SemanticMirDecodeErrorV1> {
        let offset = self.offset;
        let value = self.u8()?;
        if value <= maximum {
            Ok(value)
        } else {
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context,
                offset,
                value,
            })
        }
    }

    fn option<T>(
        &mut self,
        context: &'static str,
        decode: impl FnOnce(&mut Self) -> Result<T, SemanticMirDecodeErrorV1>,
    ) -> Result<Option<T>, SemanticMirDecodeErrorV1> {
        match self.tagged(context, 1)? {
            0 => Ok(None),
            1 => decode(self).map(Some),
            _ => unreachable!(),
        }
    }

    fn records<T>(
        &mut self,
        context: &'static str,
        resource: Option<SemanticMirResourceV1>,
        mut decode: impl FnMut(&mut Self) -> Result<T, SemanticMirDecodeErrorV1>,
    ) -> Result<Vec<T>, SemanticMirDecodeErrorV1> {
        let count = self.u32()?;
        if let Some(resource) = resource {
            self.charge(resource, u64::from(count))?;
        }
        let count = usize::try_from(count)
            .map_err(|_| SemanticMirDecodeErrorV1::LengthOverflow { context })?;
        if count > self.remaining() {
            return Err(SemanticMirDecodeErrorV1::UnexpectedEnd {
                offset: self.offset,
                requested: count,
            });
        }
        let mut values = Vec::new();
        for _ in 0..count {
            let value = decode(self)?;
            values
                .try_reserve(1)
                .map_err(|_| SemanticMirDecodeErrorV1::AllocationFailed { context })?;
            values.push(value);
        }
        Ok(values)
    }

    fn blob(
        &mut self,
        context: &'static str,
        resource: SemanticMirResourceV1,
    ) -> Result<Vec<u8>, SemanticMirDecodeErrorV1> {
        let length = self.u64()?;
        self.charge(resource, length)?;
        let length = usize::try_from(length)
            .map_err(|_| SemanticMirDecodeErrorV1::LengthOverflow { context })?;
        let source = self.raw(length)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| SemanticMirDecodeErrorV1::AllocationFailed { context })?;
        bytes.extend_from_slice(source);
        Ok(bytes)
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn require_count(
        &self,
        resource: SemanticMirResourceV1,
        actual: u64,
    ) -> Result<(), SemanticMirDecodeErrorV1> {
        let max = self.limits.limit(resource);
        if actual > max {
            Err(SemanticMirErrorV1::LimitExceeded {
                resource,
                actual,
                max,
            }
            .into())
        } else {
            Ok(())
        }
    }

    fn charge(
        &mut self,
        resource: SemanticMirResourceV1,
        amount: u64,
    ) -> Result<(), SemanticMirDecodeErrorV1> {
        let slot = match resource {
            SemanticMirResourceV1::Locals => &mut self.totals.locals,
            SemanticMirResourceV1::Blocks => &mut self.totals.blocks,
            SemanticMirResourceV1::Statements => &mut self.totals.statements,
            SemanticMirResourceV1::Projections => &mut self.totals.projections,
            SemanticMirResourceV1::Operands => &mut self.totals.operands,
            SemanticMirResourceV1::CallArguments => &mut self.totals.call_arguments,
            SemanticMirResourceV1::SwitchTargets => &mut self.totals.switch_targets,
            SemanticMirResourceV1::Relocations => &mut self.totals.relocations,
            SemanticMirResourceV1::ConstantBytes => &mut self.totals.constant_bytes,
            SemanticMirResourceV1::LinkSymbolBytes => &mut self.totals.link_symbol_bytes,
            _ => return self.require_count(resource, amount),
        };
        *slot = slot
            .checked_add(amount)
            .ok_or(SemanticMirErrorV1::ArithmeticOverflow { resource })?;
        let actual = *slot;
        self.require_count(resource, actual)
    }

    fn source(&mut self) -> Result<SemanticSourceProvenanceV1, SemanticMirDecodeErrorV1> {
        Ok(SemanticSourceProvenanceV1::new(
            self.source_origin()?,
            self.source_origin()?,
        ))
    }

    fn source_origin(
        &mut self,
    ) -> Result<Option<SemanticSourceOriginV1>, SemanticMirDecodeErrorV1> {
        self.option("source origin", |decoder| {
            SemanticSourceOriginV1::new(
                SemanticSourceFileIdentityV1(decoder.identity()?),
                decoder.u64()?,
                decoder.u64()?,
                decoder.u32()?,
                decoder.u32()?,
                decoder.u32()?,
                decoder.u32()?,
            )
            .map_err(Into::into)
        })
    }

    fn ty(&mut self) -> Result<SemanticTypeDeclV1, SemanticMirDecodeErrorV1> {
        let identity = SemanticTypeIdentityV1(self.identity()?);
        let layout_identity = SemanticLayoutIdentityV1(self.identity()?);
        let layout = self.type_layout()?;
        let abi_properties = SemanticTypeAbiPropertiesV1 {
            pass_indirectly_in_non_rustic_abis: self.boolean()?,
            has_unsized_foreign_tail: self.boolean()?,
            rustc_layout_is_noundef: self.boolean()?,
            first_pointee: self.optional_pointee_info()?,
            second_pointee: self.optional_pointee_info()?,
        };
        let shape_tag = self.tagged("type shape", 13)?;
        let rust_type_kind = if shape_tag == 13 {
            SemanticRustTypeKindV1::Str
        } else {
            SemanticRustTypeKindV1::Ordinary
        };
        let shape = match shape_tag {
            0 => SemanticTypeShapeV1::Unit,
            1 => SemanticTypeShapeV1::Never,
            2 => SemanticTypeShapeV1::Scalar(self.scalar_type()?),
            3 => SemanticTypeShapeV1::Pointer(SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1(self.u32()?),
                match self.tagged("pointer kind", 1)? {
                    0 => SemanticPointerKindV1::Raw,
                    1 => SemanticPointerKindV1::Reference,
                    _ => unreachable!(),
                },
                self.mutability()?,
                self.u32()?,
                self.u16()?,
                match self.tagged("pointer metadata", 2)? {
                    0 => SemanticPointerMetadataV1::None,
                    1 => SemanticPointerMetadataV1::SliceLength,
                    2 => SemanticPointerMetadataV1::VTable,
                    _ => unreachable!(),
                },
            )?),
            4 => SemanticTypeShapeV1::Array {
                element: SemanticTypeIdV1(self.u32()?),
                length: self.u64()?,
            },
            5 => SemanticTypeShapeV1::Tuple(self.type_list()?),
            6 => SemanticTypeShapeV1::Aggregate(self.type_list()?),
            7 => {
                let discriminant = SemanticTypeIdV1(self.u32()?);
                let variants = self.records("enum variants", None, |decoder| {
                    let discriminant_value = decoder.u128()?;
                    let uninhabited = decoder.boolean()?;
                    let fields = decoder.type_list()?;
                    Ok(SemanticEnumVariantV1::new_with_inhabitedness(
                        discriminant_value,
                        fields,
                        uninhabited,
                    ))
                })?;
                SemanticTypeShapeV1::enum_type(discriminant, variants)?
            }
            8 => SemanticTypeShapeV1::FunctionPointer {
                safety: match self.tagged("function safety", 1)? {
                    0 => SemanticFunctionSafetyV1::Safe,
                    1 => SemanticFunctionSafetyV1::Unsafe,
                    _ => unreachable!(),
                },
                extern_abi: self.extern_abi()?,
                c_variadic: self.boolean()?,
                arguments: self.type_list()?,
                return_type: SemanticTypeIdV1(self.u32()?),
            },
            9 => SemanticTypeShapeV1::Opaque,
            10 => SemanticTypeShapeV1::ValidityScalar(SemanticValidityScalarTypeV1::new(
                self.scalar_type()?,
                self.records("scalar validity ranges", None, |decoder| {
                    Ok(SemanticScalarValidityRangeV1::new(
                        decoder.u128()?,
                        decoder.u128()?,
                    ))
                })?,
            )?),
            11 => SemanticTypeShapeV1::Union(self.type_list()?),
            12 => SemanticTypeShapeV1::Slice {
                element: SemanticTypeIdV1(self.u32()?),
            },
            13 => SemanticTypeShapeV1::Opaque,
            _ => unreachable!(),
        };
        Ok(
            SemanticTypeDeclV1::new(identity, layout_identity, layout, shape)
                .with_rustc_abi_properties(abi_properties)
                .with_rust_type_kind(rust_type_kind),
        )
    }

    fn type_layout(&mut self) -> Result<SemanticTypeLayoutV1, SemanticMirDecodeErrorV1> {
        let rustc_size_bytes = self.u64()?;
        let encoded_size = self.option("type size", Self::u64)?;
        let alignment_bytes = self.u64()?;
        let fields = self.fields_shape()?;
        let variants = self.rustc_variants()?;
        let uninhabited = self.boolean()?;
        let backend_repr = self.backend_repr()?;
        let largest_niche = self.option("largest niche", Self::layout_niche)?;
        let max_repr_alignment_bytes = self.option("maximum repr alignment", Self::u64)?;
        let unadjusted_abi_alignment_bytes = self.u64()?;
        let randomization_seed = self.u64()?;
        let details = self.layout_details()?;
        let layout = SemanticTypeLayoutV1::with_exact_rustc_layout(
            rustc_size_bytes,
            alignment_bytes,
            fields,
            variants,
            backend_repr,
            largest_niche,
            uninhabited,
            max_repr_alignment_bytes,
            unadjusted_abi_alignment_bytes,
            randomization_seed,
            details,
        )?;
        if layout.size_bytes() != encoded_size {
            return Err(SemanticMirErrorV1::InvalidTypeLayout.into());
        }
        Ok(layout)
    }

    fn optional_pointee_info(
        &mut self,
    ) -> Result<Option<SemanticAbiPointeeInfoV1>, SemanticMirDecodeErrorV1> {
        self.option("ABI pointee info", |decoder| {
            let kind = match decoder.tagged("ABI pointee kind", 3)? {
                0 => SemanticAbiPointeeKindV1::Raw,
                1 => SemanticAbiPointeeKindV1::SharedReference {
                    frozen: decoder.boolean()?,
                },
                2 => SemanticAbiPointeeKindV1::MutableReference {
                    unpin: decoder.boolean()?,
                },
                3 => SemanticAbiPointeeKindV1::Box {
                    unpin: decoder.boolean()?,
                    global: decoder.boolean()?,
                },
                _ => unreachable!(),
            };
            SemanticAbiPointeeInfoV1::new(kind, decoder.u64()?, decoder.u64()?).map_err(Into::into)
        })
    }

    fn fields_shape(&mut self) -> Result<SemanticFieldsShapeV1, SemanticMirDecodeErrorV1> {
        match self.tagged("fields shape", 3)? {
            0 => Ok(SemanticFieldsShapeV1::Primitive),
            1 => SemanticFieldsShapeV1::union(self.u64()?).map_err(Into::into),
            2 => Ok(SemanticFieldsShapeV1::array(self.u64()?, self.u64()?)),
            3 => {
                let offsets = self.records("field offsets", None, Self::u64)?;
                let memory_order = self.records("field memory order", None, Self::u32)?;
                SemanticFieldsShapeV1::arbitrary(offsets, memory_order).map_err(Into::into)
            }
            _ => unreachable!(),
        }
    }

    fn backend_repr(&mut self) -> Result<SemanticBackendReprV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("backend representation", 4)? {
            0 => SemanticBackendReprV1::memory(self.boolean()?),
            1 => SemanticBackendReprV1::scalar(self.backend_scalar()?),
            2 => SemanticBackendReprV1::scalar_pair(self.backend_scalar()?, self.backend_scalar()?),
            3 => SemanticBackendReprV1::simd_vector(self.backend_scalar()?, self.u64()?),
            4 => SemanticBackendReprV1::simd_scalable_vector(self.backend_scalar()?, self.u64()?),
            _ => unreachable!(),
        })
    }

    fn backend_scalar(&mut self) -> Result<SemanticBackendScalarV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("backend scalar", 1)? {
            0 => SemanticBackendScalarV1::initialized(
                self.backend_primitive()?,
                SemanticScalarValidityRangeV1::new(self.u128()?, self.u128()?),
            ),
            1 => SemanticBackendScalarV1::union(self.backend_primitive()?),
            _ => unreachable!(),
        })
    }

    fn backend_primitive(
        &mut self,
    ) -> Result<SemanticBackendPrimitiveV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("backend primitive", 2)? {
            0 => SemanticBackendPrimitiveV1::integer(self.boolean()?, self.u16()?, self.u64()?),
            1 => SemanticBackendPrimitiveV1::float(self.u16()?, self.u64()?),
            2 => SemanticBackendPrimitiveV1::pointer(self.u32()?, self.u64()?, self.u64()?),
            _ => unreachable!(),
        })
    }

    fn layout_niche(&mut self) -> Result<SemanticLayoutNicheV1, SemanticMirDecodeErrorV1> {
        SemanticLayoutNicheV1::new(
            self.u64()?,
            self.backend_primitive()?,
            SemanticScalarValidityRangeV1::new(self.u128()?, self.u128()?),
        )
        .map_err(Into::into)
    }

    fn layout_details(&mut self) -> Result<SemanticTypeLayoutDetailsV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("layout details", 1)? {
            0 => SemanticTypeLayoutDetailsV1::None,
            1 => SemanticTypeLayoutDetailsV1::Aggregate(self.aggregate_layout()?),
            _ => unreachable!(),
        })
    }

    fn rustc_variants(&mut self) -> Result<SemanticRustcVariantsV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("rustc variants", 2)? {
            0 => SemanticRustcVariantsV1::Empty,
            1 => SemanticRustcVariantsV1::Single { index: self.u32()? },
            2 => {
                let variants =
                    self.records("rustc enum variants", None, Self::enum_variant_layout)?;
                let encoding = self.enum_encoding()?;
                SemanticRustcVariantsV1::Multiple(Box::new(SemanticEnumLayoutV1::new(
                    variants, encoding,
                )?))
            }
            _ => unreachable!(),
        })
    }

    fn enum_variant_layout(
        &mut self,
    ) -> Result<SemanticEnumVariantLayoutV1, SemanticMirDecodeErrorV1> {
        SemanticEnumVariantLayoutV1::from_rustc(
            self.u32()?,
            self.u64()?,
            self.u64()?,
            self.fields_shape()?,
            self.backend_repr()?,
            self.option("variant largest niche", Self::layout_niche)?,
            self.boolean()?,
            self.option("variant maximum repr alignment", Self::u64)?,
            self.u64()?,
            self.u64()?,
            self.aggregate_layout()?,
        )
        .map_err(Into::into)
    }

    fn aggregate_layout(&mut self) -> Result<SemanticAggregateLayoutV1, SemanticMirDecodeErrorV1> {
        let field_offsets = self.records("aggregate field offsets", None, Self::u64)?;
        let padding = self.records("aggregate padding", None, |decoder| {
            SemanticPaddingV1::new(decoder.u64()?, decoder.u64()?).map_err(Into::into)
        })?;
        SemanticAggregateLayoutV1::new(field_offsets, padding).map_err(Into::into)
    }

    fn enum_encoding(&mut self) -> Result<SemanticEnumEncodingV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("enum encoding", 1)? {
            0 => SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                self.u32()?,
                self.u64()?,
                self.backend_scalar()?,
            )),
            1 => {
                let tag_field = self.u32()?;
                let path = self.records("niche path", None, |decoder| {
                    Ok(match decoder.tagged("niche path component", 1)? {
                        0 => SemanticNichePathComponentV1::Field(decoder.u32()?),
                        1 => SemanticNichePathComponentV1::ArrayElement(decoder.u64()?),
                        _ => unreachable!(),
                    })
                })?;
                let source = SemanticNicheSourceV1::new(path, self.u64()?)?;
                let source_niche = self.layout_niche()?;
                let tag = self.backend_scalar()?;
                SemanticEnumEncodingV1::Niche(SemanticNicheEnumEncodingV1::new(
                    tag_field,
                    source,
                    source_niche,
                    tag,
                    self.u32()?,
                    self.u32()?,
                    self.u32()?,
                    self.u128()?,
                )?)
            }
            _ => unreachable!(),
        })
    }

    fn scalar_type(&mut self) -> Result<SemanticScalarTypeV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("scalar type", 3)? {
            0 => SemanticScalarTypeV1::Bool,
            1 => SemanticScalarTypeV1::Char,
            2 => SemanticScalarTypeV1::Integer {
                signed: self.boolean()?,
                bits: self.u16()?,
            },
            3 => SemanticScalarTypeV1::Float { bits: self.u16()? },
            _ => unreachable!(),
        })
    }

    fn mutability(&mut self) -> Result<SemanticMutabilityV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("mutability", 1)? {
            0 => SemanticMutabilityV1::Immutable,
            1 => SemanticMutabilityV1::Mutable,
            _ => unreachable!(),
        })
    }

    fn type_list(&mut self) -> Result<SemanticAggregateTypeV1, SemanticMirDecodeErrorV1> {
        SemanticAggregateTypeV1::new(self.records("type list", None, |decoder| {
            Ok(SemanticTypeIdV1(decoder.u32()?))
        })?)
        .map_err(Into::into)
    }

    fn allocation(&mut self) -> Result<SemanticAllocationDeclV1, SemanticMirDecodeErrorV1> {
        let identity = SemanticAllocationIdentityV1(self.identity()?);
        let address_space = self.u32()?;
        let bytes = self.blob("allocation bytes", SemanticMirResourceV1::ConstantBytes)?;
        let initialized_mask = self.blob(
            "allocation initialized mask",
            SemanticMirResourceV1::ConstantBytes,
        )?;
        let alignment_bytes = self.u64()?;
        let mutable = self.boolean()?;
        let relocations = self.records(
            "relocations",
            Some(SemanticMirResourceV1::Relocations),
            |decoder| {
                let byte_offset = decoder.u64()?;
                let width_bytes = decoder.u8()?;
                let address_space = decoder.u32()?;
                let addend = decoder.i64()?;
                let target = match decoder.tagged("relocation target", 3)? {
                    0 => SemanticRelocationTargetV1::Allocation(SemanticAllocationIdV1(
                        decoder.u32()?,
                    )),
                    1 => SemanticRelocationTargetV1::Callable(SemanticCallableIdV1(decoder.u32()?)),
                    2 => SemanticRelocationTargetV1::Static(SemanticStaticIdV1(decoder.u32()?)),
                    3 => SemanticRelocationTargetV1::VTable(SemanticVTableIdV1(decoder.u32()?)),
                    _ => unreachable!(),
                };
                SemanticRelocationV1::new_in_address_space(
                    byte_offset,
                    width_bytes,
                    address_space,
                    addend,
                    target,
                )
                .map_err(Into::into)
            },
        )?;
        SemanticAllocationDeclV1::new_in_address_space(
            identity,
            address_space,
            bytes,
            initialized_mask,
            alignment_bytes,
            mutable,
            relocations,
        )
        .map_err(Into::into)
    }

    fn link_symbol(&mut self) -> Result<SemanticLinkSymbolV1, SemanticMirDecodeErrorV1> {
        SemanticLinkSymbolV1::new(self.blob("link symbol", SemanticMirResourceV1::LinkSymbolBytes)?)
            .map_err(Into::into)
    }

    fn static_decl(&mut self) -> Result<SemanticStaticDeclV1, SemanticMirDecodeErrorV1> {
        let identity = SemanticStaticIdentityV1(self.identity()?);
        let source = self.source()?;
        let ty = SemanticTypeIdV1(self.u32()?);
        let mutable = self.boolean()?;
        let address_space = self.u32()?;
        let definition = match self.tagged("static definition", 1)? {
            0 => SemanticStaticDefinitionV1::Defined {
                initializer: SemanticAllocationIdV1(self.u32()?),
            },
            1 => SemanticStaticDefinitionV1::ExternalRequired {
                symbol: self.link_symbol()?,
            },
            _ => unreachable!(),
        };
        let export_symbol = self.option("static export symbol", Self::link_symbol)?;
        let mut declaration =
            SemanticStaticDeclV1::new(identity, source, ty, mutable, address_space, definition);
        if let Some(export_symbol) = export_symbol {
            declaration = declaration.with_export_symbol(export_symbol);
        }
        Ok(declaration)
    }

    fn vtable(&mut self) -> Result<SemanticVTableDeclV1, SemanticMirDecodeErrorV1> {
        let identity = SemanticVTableIdentityV1(self.identity()?);
        let concrete_type = SemanticTypeIdV1(self.u32()?);
        let dyn_type = SemanticTypeIdV1(self.u32()?);
        let primary_trait_ref = SemanticTraitRefIdentityV1(self.identity()?);
        let dyn_predicates = self.records("vtable dyn predicates", None, |decoder| {
            Ok(SemanticDynPredicateIdentityV1(decoder.identity()?))
        })?;
        let trait_identity = SemanticVTableTraitIdentityV1::new(primary_trait_ref, dyn_predicates)?;
        let drop_glue = self.option("vtable drop glue", |decoder| {
            Ok(SemanticFunctionIdV1(decoder.u32()?))
        })?;
        let header = SemanticVTableHeaderV1::new(drop_glue, self.u64()?, self.u64()?)?;
        let slots = self.records("vtable slots", None, |decoder| {
            Ok(match decoder.tagged("vtable slot", 2)? {
                0 => SemanticVTableSlotV1::Vacant,
                1 => SemanticVTableSlotV1::Method(SemanticFunctionIdV1(decoder.u32()?)),
                2 => SemanticVTableSlotV1::TraitVPtr {
                    trait_ref: SemanticTraitRefIdentityV1(decoder.identity()?),
                    target: SemanticVTableIdV1(decoder.u32()?),
                },
                _ => unreachable!(),
            })
        })?;
        let allocation = SemanticAllocationIdV1(self.u32()?);
        SemanticVTableDeclV1::new_with_trait_identity_and_slots(
            identity,
            concrete_type,
            dyn_type,
            trait_identity,
            header,
            slots,
            allocation,
        )
        .map_err(Into::into)
    }

    fn function(&mut self) -> Result<SemanticFunctionDeclV1, SemanticMirDecodeErrorV1> {
        let identity = SemanticFunctionIdentityV1(self.identity()?);
        let role = match self.tagged("function role", 3)? {
            0 => SemanticFunctionRoleV1::KernelRoot,
            1 => SemanticFunctionRoleV1::InternalHelper,
            2 => SemanticFunctionRoleV1::DeviceFfiExport,
            3 => SemanticFunctionRoleV1::DropGlue(SemanticTypeIdV1(self.u32()?)),
            _ => unreachable!(),
        };
        let export = match self.tagged("function export", 2)? {
            0 => None,
            1 => Some(SemanticFunctionExportV1::Kernel(self.kernel_entry()?)),
            2 => Some(SemanticFunctionExportV1::DeviceFfi {
                export_symbol: self.link_symbol()?,
            }),
            _ => unreachable!(),
        };
        let item_definition_identity = SemanticItemDefinitionIdentityV1(self.identity()?);
        let monomorphization_identity = SemanticMonomorphizationIdentityV1(self.identity()?);
        let generic_type_arguments_identity =
            SemanticGenericTypeArgumentsIdentityV1(self.identity()?);
        let const_generic_arguments_identity =
            SemanticConstGenericArgumentsIdentityV1(self.identity()?);
        let source = self.source()?;
        let abi = self.abi()?;
        let locals = self.records("locals", Some(SemanticMirResourceV1::Locals), |decoder| {
            let identity = SemanticLocalIdentityV1(decoder.identity()?);
            let ty = SemanticTypeIdV1(decoder.u32()?);
            let role = match decoder.tagged("local role", 2)? {
                0 => SemanticLocalRoleV1::Return,
                1 => SemanticLocalRoleV1::Argument(decoder.u32()?),
                2 => SemanticLocalRoleV1::Temporary,
                _ => unreachable!(),
            };
            Ok(SemanticLocalDeclV1::new(
                identity,
                ty,
                role,
                decoder.source()?,
            ))
        })?;
        let entry = SemanticBlockIdV1(self.u32()?);
        let blocks = self.records(
            "blocks",
            Some(SemanticMirResourceV1::Blocks),
            Self::basic_block,
        )?;
        let mut function = SemanticFunctionDeclV1::new(
            identity,
            role,
            item_definition_identity,
            monomorphization_identity,
            generic_type_arguments_identity,
            const_generic_arguments_identity,
            source,
            abi,
            locals,
            entry,
            blocks,
        )?;
        match export {
            Some(SemanticFunctionExportV1::Kernel(entry)) => {
                function = function.with_kernel_entry(entry);
            }
            Some(SemanticFunctionExportV1::DeviceFfi { export_symbol }) => {
                function = function.with_device_ffi_export_symbol(export_symbol);
            }
            None => {}
        }
        Ok(function)
    }

    fn basic_block(&mut self) -> Result<SemanticBasicBlockV1, SemanticMirDecodeErrorV1> {
        let identity = SemanticBlockIdentityV1(self.identity()?);
        let source = self.source()?;
        let statements = self.records(
            "statements",
            Some(SemanticMirResourceV1::Statements),
            |decoder| {
                Ok(SemanticStatementV1::new(
                    decoder.source()?,
                    decoder.statement()?,
                ))
            },
        )?;
        let terminator = SemanticTerminatorV1::new(self.source()?, self.terminator()?);
        SemanticBasicBlockV1::new(identity, source, statements, terminator).map_err(Into::into)
    }

    fn callable(&mut self) -> Result<SemanticCallableDeclV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("callable", 2)? {
            0 => SemanticCallableDeclV1::defined(SemanticFunctionIdV1(self.u32()?)),
            1 => {
                let binding = self.non_body_binding()?;
                let contract = SemanticDeviceFfiImportContractV1::new(
                    SemanticDeviceFfiContractIdentityV1(self.identity()?),
                    self.link_symbol()?,
                    match self.tag("device FFI target")? {
                        0 => SemanticDeviceFfiTargetV1::AmdGpuGfx942XnackMinus,
                        _ => unreachable!(),
                    },
                    match self.tag("code object version")? {
                        0 => SemanticCodeObjectVersionV1::V6,
                        _ => unreachable!(),
                    },
                    SemanticDeviceFfiPhysicalAbiIdentityV1(self.identity()?),
                    SemanticDeviceFfiEffectsV1::new(self.u16()?)?,
                    SemanticDeviceFfiSemanticIdentityV1(self.identity()?),
                );
                SemanticCallableDeclV1::DeviceFfiImport { binding, contract }
            }
            2 => SemanticCallableDeclV1::CompilerIntrinsic {
                binding: self.non_body_binding()?,
                operation: self.compiler_intrinsic()?,
                operation_identity: SemanticCompilerIntrinsicIdentityV1(self.identity()?),
            },
            _ => unreachable!(),
        })
    }

    fn non_body_binding(
        &mut self,
    ) -> Result<SemanticNonBodyCallableBindingV1, SemanticMirDecodeErrorV1> {
        Ok(SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1(self.identity()?),
            SemanticItemDefinitionIdentityV1(self.identity()?),
            SemanticMonomorphizationIdentityV1(self.identity()?),
            SemanticGenericTypeArgumentsIdentityV1(self.identity()?),
            SemanticConstGenericArgumentsIdentityV1(self.identity()?),
            self.source()?,
            self.abi()?,
        ))
    }

    fn kernel_entry(&mut self) -> Result<SemanticKernelEntryV1, SemanticMirDecodeErrorV1> {
        let export_symbol = self.link_symbol()?;
        let kernel_binding_identity = SemanticKernelBindingIdentityV1(self.identity()?);
        let launch = self.option("kernel launch bounds", |decoder| {
            SemanticKernelLaunchBoundsV1::new(
                decoder.optional_workgroup_dimensions()?,
                decoder.optional_workgroup_dimensions()?,
                decoder.option("minimum resident workgroups", Self::u16)?,
            )
            .map_err(Into::into)
        })?;
        let unsafe_assembly = self.option("unsafe assembly", |decoder| {
            let target = match decoder.tag("unsafe assembly target")? {
                0 => SemanticUnsafeAssemblyTargetV1::AmdGpuGfx942,
                _ => unreachable!(),
            };
            SemanticUnsafeAssemblyDeclarationV1::new(
                target,
                decoder.u16()?,
                decoder.u16()?,
                decoder.u16()?,
            )
            .map_err(Into::into)
        })?;
        let reachable_assembly = self.option("reachable assembly", |decoder| {
            SemanticReachableAssemblyV1::new(
                decoder.u32()?,
                decoder.u16()?,
                decoder.u16()?,
                decoder.u16()?,
            )
            .map_err(Into::into)
        })?;
        let resources = if self.wire_version >= SemanticMirWireVersionV1::V7 {
            self.option("kernel resources", |decoder| {
                SemanticKernelResourceContractV1::new(decoder.u32()?, decoder.u32()?)
                    .map_err(Into::into)
            })?
        } else {
            None
        };
        Ok(SemanticKernelEntryV1::new(
            export_symbol,
            kernel_binding_identity,
            SemanticKernelSourceContractV1::new_with_resources(
                launch,
                resources,
                unsafe_assembly,
                reachable_assembly,
            )?,
        ))
    }

    fn optional_workgroup_dimensions(
        &mut self,
    ) -> Result<Option<SemanticWorkgroupDimensionsV1>, SemanticMirDecodeErrorV1> {
        self.option("workgroup dimensions", |decoder| {
            SemanticWorkgroupDimensionsV1::new([decoder.u32()?, decoder.u32()?, decoder.u32()?])
                .map_err(Into::into)
        })
    }

    fn abi(&mut self) -> Result<SemanticFunctionAbiV1, SemanticMirDecodeErrorV1> {
        let identity = SemanticAbiIdentityV1(self.identity()?);
        let layout_identity = SemanticLayoutIdentityV1(self.identity()?);
        let canon_abi = self.canon_abi()?;
        let extern_abi = self.extern_abi()?;
        let can_unwind = self.boolean()?;
        let c_variadic = self.boolean()?;
        let fixed_count = self.u32()?;
        let source_input_types = self.records(
            "source ABI inputs",
            Some(SemanticMirResourceV1::CallArguments),
            |decoder| Ok(SemanticTypeIdV1(decoder.u32()?)),
        )?;
        let source_argument_ownership = if self.wire_version >= SemanticMirWireVersionV1::V4 {
            self.records(
                "source ABI argument ownership",
                // Ownership is one-to-one metadata for the source inputs charged
                // immediately above, not another set of call arguments.
                None,
                |decoder| {
                    Ok(match decoder.tagged("source ABI argument ownership", 5)? {
                        0 => SemanticSourceArgumentOwnershipV1::Unspecified,
                        1 => SemanticSourceArgumentOwnershipV1::ByValue,
                        2 => SemanticSourceArgumentOwnershipV1::SharedBorrow,
                        3 => SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                        4 => SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                        5 => SemanticSourceArgumentOwnershipV1::RawPointer,
                        _ => unreachable!(),
                    })
                },
            )?
        } else {
            let mut ownership = Vec::new();
            ownership
                .try_reserve_exact(source_input_types.len())
                .map_err(|_| SemanticMirDecodeErrorV1::AllocationFailed {
                    context: "legacy source ABI argument ownership",
                })?;
            ownership.resize(
                source_input_types.len(),
                SemanticSourceArgumentOwnershipV1::Unspecified,
            );
            ownership
        };
        let source_output_type = SemanticTypeIdV1(self.u32()?);
        let arguments = self.records(
            "ABI arguments",
            Some(SemanticMirResourceV1::CallArguments),
            |decoder| {
                let role = match decoder.tagged("ABI argument role", 2)? {
                    0 => SemanticAbiArgumentRoleV1::Source,
                    1 => SemanticAbiArgumentRoleV1::RustCallTupleField(decoder.u32()?),
                    2 => SemanticAbiArgumentRoleV1::Hidden(
                        SemanticAbiHiddenArgumentRoleV1::CallerLocation,
                    ),
                    _ => unreachable!(),
                };
                let value = decoder.abi_value()?;
                Ok(match role {
                    SemanticAbiArgumentRoleV1::Source => SemanticAbiArgumentV1::source(value),
                    SemanticAbiArgumentRoleV1::RustCallTupleField(field) => {
                        SemanticAbiArgumentV1::rust_call_tuple_field(field, value)
                    }
                    SemanticAbiArgumentRoleV1::Hidden(role) => {
                        SemanticAbiArgumentV1::hidden(role, value)
                    }
                })
            },
        )?;
        let return_value = self.abi_value()?;
        SemanticFunctionAbiV1::from_rustc_with_source_signature(
            identity,
            layout_identity,
            canon_abi,
            extern_abi,
            can_unwind,
            c_variadic,
            fixed_count,
            source_input_types,
            source_output_type,
            arguments,
            return_value,
        )
        .and_then(|abi| abi.with_source_argument_ownership(source_argument_ownership))
        .map_err(Into::into)
    }

    fn canon_abi(&mut self) -> Result<SemanticCanonAbiV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("canonical ABI", 8)? {
            0 => SemanticCanonAbiV1::Rust,
            1 => SemanticCanonAbiV1::C,
            2 => SemanticCanonAbiV1::RustCold,
            3 => SemanticCanonAbiV1::GpuKernel,
            4 => SemanticCanonAbiV1::RustPreserveNone,
            5 => SemanticCanonAbiV1::Custom,
            6 => SemanticCanonAbiV1::Arm(match self.tagged("ARM ABI", 2)? {
                0 => SemanticArmCallV1::Aapcs,
                1 => SemanticArmCallV1::CCmseNonSecureCall,
                2 => SemanticArmCallV1::CCmseNonSecureEntry,
                _ => unreachable!(),
            }),
            7 => SemanticCanonAbiV1::Interrupt(match self.tagged("interrupt ABI", 5)? {
                0 => SemanticInterruptKindV1::Avr,
                1 => SemanticInterruptKindV1::AvrNonBlocking,
                2 => SemanticInterruptKindV1::Msp430,
                3 => SemanticInterruptKindV1::RiscvMachine,
                4 => SemanticInterruptKindV1::RiscvSupervisor,
                5 => SemanticInterruptKindV1::X86,
                _ => unreachable!(),
            }),
            8 => SemanticCanonAbiV1::X86(match self.tagged("x86 ABI", 5)? {
                0 => SemanticX86CallV1::Fastcall,
                1 => SemanticX86CallV1::Stdcall,
                2 => SemanticX86CallV1::SysV64,
                3 => SemanticX86CallV1::Thiscall,
                4 => SemanticX86CallV1::Vectorcall,
                5 => SemanticX86CallV1::Win64,
                _ => unreachable!(),
            }),
            _ => unreachable!(),
        })
    }

    fn extern_abi(&mut self) -> Result<SemanticExternAbiV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("extern ABI", 9)? {
            0 => SemanticExternAbiV1::C {
                unwind: self.boolean()?,
            },
            1 => SemanticExternAbiV1::System {
                unwind: self.boolean()?,
            },
            2 => SemanticExternAbiV1::Cdecl {
                unwind: self.boolean()?,
            },
            3 => SemanticExternAbiV1::Rust,
            4 => SemanticExternAbiV1::RustCall,
            5 => SemanticExternAbiV1::RustCold,
            6 => SemanticExternAbiV1::RustPreserveNone,
            7 => SemanticExternAbiV1::Unadjusted,
            8 => SemanticExternAbiV1::Custom,
            9 => SemanticExternAbiV1::GpuKernel,
            _ => unreachable!(),
        })
    }

    fn abi_value(&mut self) -> Result<SemanticAbiValueV1, SemanticMirDecodeErrorV1> {
        let source_ty = SemanticTypeIdV1(self.u32()?);
        let adjusted = self.option("adjusted ABI type", |decoder| {
            Ok(SemanticAbiAdjustedTypeV1::new(
                SemanticTypeIdV1(decoder.u32()?),
                SemanticLayoutIdentityV1(decoder.identity()?),
                decoder.type_layout()?,
            ))
        })?;
        let pointee_override = self.optional_pointee_info()?;
        let mode = self.abi_pass_mode()?;
        let mut value = match adjusted {
            Some(adjusted) => SemanticAbiValueV1::new_with_adjusted_type(source_ty, adjusted, mode),
            None => SemanticAbiValueV1::new(source_ty, mode),
        };
        if let Some(pointee_override) = pointee_override {
            value = value.with_pointee_override(pointee_override);
        }
        Ok(value)
    }

    fn abi_pass_mode(&mut self) -> Result<SemanticAbiPassModeV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("ABI pass mode", 4)? {
            0 => SemanticAbiPassModeV1::Ignore,
            1 => SemanticAbiPassModeV1::Direct(self.abi_attributes()?),
            2 => SemanticAbiPassModeV1::Pair {
                first: self.abi_attributes()?,
                second: self.abi_attributes()?,
            },
            3 => {
                let pad_i32 = self.boolean()?;
                let mut prefix = [None; 8];
                for register in &mut prefix {
                    *register = self.option("ABI cast prefix register", Self::abi_register)?;
                }
                let rest_offset_bytes = self.option("ABI cast rest offset", Self::u64)?;
                let unit = self.abi_register()?;
                let total_bytes = self.u64()?;
                let consecutive = self.boolean()?;
                let attributes = self.abi_attributes()?;
                SemanticAbiPassModeV1::cast(
                    pad_i32,
                    SemanticAbiCastV1::new(
                        prefix,
                        rest_offset_bytes,
                        SemanticAbiUniformV1::from_rustc(unit, total_bytes, consecutive)?,
                        attributes,
                    ),
                )
            }
            4 => SemanticAbiPassModeV1::Indirect {
                attributes: self.abi_attributes()?,
                metadata_attributes: self
                    .option("ABI metadata attributes", Self::abi_attributes)?,
                on_stack: self.boolean()?,
            },
            _ => unreachable!(),
        })
    }

    fn abi_attributes(&mut self) -> Result<SemanticAbiValueAttributesV1, SemanticMirDecodeErrorV1> {
        let regular = SemanticAbiRegularAttributesV1::from_rustc_bits(self.u8()?)?;
        let extension = match self.tagged("ABI extension", 2)? {
            0 => SemanticAbiExtensionV1::None,
            1 => SemanticAbiExtensionV1::ZeroExtend,
            2 => SemanticAbiExtensionV1::SignExtend,
            _ => unreachable!(),
        };
        let pointee_size_bytes = self.u64()?;
        let pointee_alignment_bytes = self.option("ABI pointee alignment", Self::u64)?;
        SemanticAbiValueAttributesV1::new(
            regular,
            extension,
            pointee_size_bytes,
            pointee_alignment_bytes,
        )
        .map_err(Into::into)
    }

    fn abi_register(&mut self) -> Result<SemanticAbiRegisterV1, SemanticMirDecodeErrorV1> {
        let kind = match self.tagged("ABI register kind", 2)? {
            0 => SemanticAbiRegisterKindV1::Integer,
            1 => SemanticAbiRegisterKindV1::Float,
            2 => SemanticAbiRegisterKindV1::Vector,
            _ => unreachable!(),
        };
        SemanticAbiRegisterV1::new(kind, self.u64()?).map_err(Into::into)
    }

    fn compiler_intrinsic(
        &mut self,
    ) -> Result<SemanticCompilerIntrinsicOperationV1, SemanticMirDecodeErrorV1> {
        let maximum_tag = if self.wire_version == SemanticMirWireVersionV1::V12 {
            65
        } else if self.wire_version == SemanticMirWireVersionV1::V11 {
            64
        } else if self.wire_version == SemanticMirWireVersionV1::V10 {
            63
        } else if self.wire_version == SemanticMirWireVersionV1::V9 {
            62
        } else if self.wire_version == SemanticMirWireVersionV1::V8 {
            55
        } else if self.wire_version >= SemanticMirWireVersionV1::V6 {
            58
        } else if self.wire_version >= SemanticMirWireVersionV1::V5 {
            44
        } else {
            36
        };
        Ok(match self.tagged("compiler intrinsic", maximum_tag)? {
            0 => SemanticCompilerIntrinsicOperationV1::ThreadIndex(self.axis()?),
            1 => SemanticCompilerIntrinsicOperationV1::WorkgroupIndex(self.axis()?),
            2 => SemanticCompilerIntrinsicOperationV1::WorkgroupDimension(self.axis()?),
            3 => SemanticCompilerIntrinsicOperationV1::GridDimension(self.axis()?),
            4 => SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier,
            5 => SemanticCompilerIntrinsicOperationV1::WaveBarrier,
            6 => SemanticCompilerIntrinsicOperationV1::FabsF32,
            7 => SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
            },
            8 => SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                index_witness: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
            },
            9 => SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                disjoint_slice: SemanticTypeIdV1(self.u32()?),
                index_witness: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
            },
            10 => SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
                input_witness: SemanticTypeIdV1(self.u32()?),
                output_witness: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                index_space: self.disjoint_index_space()?,
            },
            11 => SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
                input_witness: SemanticTypeIdV1(self.u32()?),
                output_witness: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                input_space: self.disjoint_index_space()?,
                output_space: self.disjoint_index_space()?,
                offset: self.u64()?,
            },
            12 => SemanticCompilerIntrinsicOperationV1::DisjointIndexGet {
                index_witness: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                index_space: self.disjoint_index_space()?,
            },
            13 => SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
                input_witness: SemanticTypeIdV1(self.u32()?),
                output_witness: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                input_space: self.disjoint_index_space()?,
                output_space: self.disjoint_index_space()?,
                offset: self.u64()?,
            },
            14 => SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                disjoint_slice: SemanticTypeIdV1(self.u32()?),
                index_witness: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                index_space: self.disjoint_index_space()?,
            },
            15 => SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent {
                grid_leader: SemanticTypeIdV1(self.u32()?),
            },
            16 => SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                disjoint_slice: SemanticTypeIdV1(self.u32()?),
                grid_leader: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
            },
            17 => SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock {
                input_witness: SemanticTypeIdV1(self.u32()?),
                output_block: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                input_space: self.disjoint_index_space()?,
                output_space: self.disjoint_index_space()?,
                lanes_per_block: self.u64()?,
                elements_per_lane: self.u64()?,
            },
            18 => SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
                disjoint_slice: SemanticTypeIdV1(self.u32()?),
                block_witness: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                index_space: self.disjoint_index_space()?,
                lanes_per_block: self.u64()?,
                elements_per_lane: self.u64()?,
            },
            19 => SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
                disjoint_slice: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                index_space: self.disjoint_index_space()?,
            },
            20 => SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent {
                context: SemanticTypeIdV1(self.u32()?),
            },
            21 | 22 => {
                return Err(SemanticMirDecodeErrorV1::InvalidTag {
                    context: "retired raw matrix fragment compiler intrinsic",
                    offset: self.offset - 1,
                    value: self.bytes[self.offset - 1],
                });
            }
            23 => SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                fragment: SemanticTypeIdV1(self.u32()?),
                values: SemanticTypeIdV1(self.u32()?),
            },
            24 => SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                context: SemanticTypeIdV1(self.u32()?),
                lhs_fragment: SemanticTypeIdV1(self.u32()?),
                rhs_fragment: SemanticTypeIdV1(self.u32()?),
                accumulator_fragment: SemanticTypeIdV1(self.u32()?),
                lhs: self.mfma_operand_contract()?,
                rhs: self.mfma_operand_contract()?,
                accumulator: self.mfma_accumulator_contract()?,
            },
            25 => SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedTiled2d {
                input_witness: SemanticTypeIdV1(self.u32()?),
                output_tile: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                input_space: self.disjoint_index_space()?,
                output_space: self.disjoint_index_space()?,
                lanes_per_tile: self.u64()?,
                tile_rows: self.u64()?,
                tile_columns: self.u64()?,
                elements_per_lane: self.u64()?,
            },
            26 => SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
                disjoint_slice: SemanticTypeIdV1(self.u32()?),
                tile_witness: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                index_space: self.disjoint_index_space()?,
                lanes_per_tile: self.u64()?,
                tile_rows: self.u64()?,
                tile_columns: self.u64()?,
                elements_per_lane: self.u64()?,
            },
            27 => SemanticCompilerIntrinsicOperationV1::CollectiveContextCurrent {
                context: SemanticTypeIdV1(self.u32()?),
            },
            28 => SemanticCompilerIntrinsicOperationV1::SubgroupReduceF32 {
                context: SemanticTypeIdV1(self.u32()?),
                width: self.u32()?,
                kind: match self.tagged("subgroup reduction kind", 1)? {
                    0 => SemanticSubgroupReductionKindV1::Sum,
                    1 => SemanticSubgroupReductionKindV1::Maximum,
                    _ => unreachable!(),
                },
            },
            29 => SemanticCompilerIntrinsicOperationV1::MathContextCurrent {
                context: SemanticTypeIdV1(self.u32()?),
            },
            30 => SemanticCompilerIntrinsicOperationV1::MathF32 {
                context: SemanticTypeIdV1(self.u32()?),
                function: match self.tagged("f32 math function", 12)? {
                    0 => SemanticF32MathFunctionV1::Sqrt,
                    1 => SemanticF32MathFunctionV1::FusedMultiplyAdd,
                    2 => SemanticF32MathFunctionV1::Floor,
                    3 => SemanticF32MathFunctionV1::Ceil,
                    4 => SemanticF32MathFunctionV1::Truncate,
                    5 => SemanticF32MathFunctionV1::RoundTiesEven,
                    6 => SemanticF32MathFunctionV1::Sin,
                    7 => SemanticF32MathFunctionV1::Cos,
                    8 => SemanticF32MathFunctionV1::Exp,
                    9 => SemanticF32MathFunctionV1::Exp2,
                    10 => SemanticF32MathFunctionV1::Ln,
                    11 => SemanticF32MathFunctionV1::Log2,
                    12 => SemanticF32MathFunctionV1::Log10,
                    _ => unreachable!(),
                },
            },
            31 => SemanticCompilerIntrinsicOperationV1::ColdPath,
            32 => SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
                lane: SemanticTypeIdV1(self.u32()?),
                wave_width: self.u32()?,
            },
            33 => SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor {
                result: SemanticTypeIdV1(self.u32()?),
                view: SemanticTypeIdV1(self.u32()?),
                error: SemanticTypeIdV1(self.u32()?),
                role: self.mfma_role()?,
                storage_layout: self.mfma_storage_layout()?,
            },
            34 => SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad {
                option_fragment: SemanticTypeIdV1(self.u32()?),
                view: SemanticTypeIdV1(self.u32()?),
                lane: SemanticTypeIdV1(self.u32()?),
                fragment: SemanticTypeIdV1(self.u32()?),
                contract: self.mfma_operand_contract()?,
                storage_layout: self.mfma_storage_layout()?,
            },
            35 => SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
                lane: SemanticTypeIdV1(self.u32()?),
                fragment: SemanticTypeIdV1(self.u32()?),
                contract: self.mfma_accumulator_contract()?,
            },
            36 => SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
                fragment: SemanticTypeIdV1(self.u32()?),
                view: SemanticTypeIdV1(self.u32()?),
                lane: SemanticTypeIdV1(self.u32()?),
                contract: self.mfma_operand_contract()?,
                storage_layout: self.mfma_storage_layout()?,
            },
            37 => SemanticCompilerIntrinsicOperationV1::StridedReadView2DFromSharedSlice {
                result: SemanticTypeIdV1(self.u32()?),
                view: SemanticTypeIdV1(self.u32()?),
                error: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
            },
            38 => SemanticCompilerIntrinsicOperationV1::StridedReadView2DLoadOr {
                view: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
            },
            39 => SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedRowStriped2d {
                input_witness: SemanticTypeIdV1(self.u32()?),
                output_stripe: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                input_space: self.disjoint_index_space()?,
                output_space: self.disjoint_index_space()?,
                lanes_per_row: self.u64()?,
                elements_per_lane: self.u64()?,
            },
            40 => SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut {
                disjoint_slice: SemanticTypeIdV1(self.u32()?),
                stripe_witness: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                index_space: self.disjoint_index_space()?,
                lanes_per_row: self.u64()?,
                elements_per_lane: self.u64()?,
            },
            41 => SemanticCompilerIntrinsicOperationV1::Gfx950Fp8MatrixViewRowMajor {
                result: SemanticTypeIdV1(self.u32()?),
                view: SemanticTypeIdV1(self.u32()?),
                error: SemanticTypeIdV1(self.u32()?),
                role: self.mfma_role()?,
                storage_layout: self.mfma_storage_layout()?,
            },
            42 => SemanticCompilerIntrinsicOperationV1::Gfx950Fp8MatrixLoadM16K128 {
                fragment: SemanticTypeIdV1(self.u32()?),
                view: SemanticTypeIdV1(self.u32()?),
                lane: SemanticTypeIdV1(self.u32()?),
                contract: self.mfma_operand_contract()?,
                storage_layout: self.mfma_storage_layout()?,
            },
            43 => SemanticCompilerIntrinsicOperationV1::Gfx950Fp4MatrixViewRowMajor {
                result: SemanticTypeIdV1(self.u32()?),
                view: SemanticTypeIdV1(self.u32()?),
                error: SemanticTypeIdV1(self.u32()?),
                role: self.mfma_role()?,
                storage_layout: self.mfma_storage_layout()?,
            },
            44 => SemanticCompilerIntrinsicOperationV1::Gfx950Fp4MatrixLoadM16K128 {
                fragment: SemanticTypeIdV1(self.u32()?),
                view: SemanticTypeIdV1(self.u32()?),
                lane: SemanticTypeIdV1(self.u32()?),
                contract: self.mfma_operand_contract()?,
                storage_layout: self.mfma_storage_layout()?,
            },
            45 => SemanticCompilerIntrinsicOperationV1::SubgroupBroadcastF32 {
                context: SemanticTypeIdV1(self.u32()?),
                width: self.u32()?,
            },
            46 => SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeCurrent {
                tile: SemanticTypeIdV1(self.u32()?),
                lane: SemanticTypeIdV1(self.u32()?),
                format: self.gfx950_lds_transpose_format()?,
            },
            47 => SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeStage {
                input_tile: SemanticTypeIdV1(self.u32()?),
                output_tile: SemanticTypeIdV1(self.u32()?),
                view: SemanticTypeIdV1(self.u32()?),
                format: self.gfx950_lds_transpose_format()?,
            },
            48 => SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposePublish {
                input_tile: SemanticTypeIdV1(self.u32()?),
                output_tile: SemanticTypeIdV1(self.u32()?),
                format: self.gfx950_lds_transpose_format()?,
            },
            49 => SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeRead {
                tile: SemanticTypeIdV1(self.u32()?),
                fragment: SemanticTypeIdV1(self.u32()?),
                contract: self.mfma_operand_contract()?,
                format: self.gfx950_lds_transpose_format()?,
            },
            50 => SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupContextCurrent {
                context: SemanticTypeIdV1(self.u32()?),
            },
            51 => SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupReduceF32 {
                context: SemanticTypeIdV1(self.u32()?),
                width: self.u32()?,
                kind: match self.tagged("gfx950 subgroup reduction kind", 1)? {
                    0 => SemanticSubgroupReductionKindV1::Sum,
                    1 => SemanticSubgroupReductionKindV1::Maximum,
                    _ => unreachable!(),
                },
            },
            52 => SemanticCompilerIntrinsicOperationV1::DynamicLdsExactCurrent {
                scope: SemanticTypeIdV1(self.u32()?),
                dynamic_lds: SemanticTypeIdV1(self.u32()?),
                element_storage: SemanticTypeIdV1(self.u32()?),
                elements: self.u64()?,
            },
            53 => SemanticCompilerIntrinsicOperationV1::WorkgroupReduceSum {
                workgroup: SemanticTypeIdV1(self.u32()?),
                context: SemanticTypeIdV1(self.u32()?),
                scratch: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
            },
            54 => SemanticCompilerIntrinsicOperationV1::DynamicLdsIntoCollectiveRawParts {
                dynamic_lds: SemanticTypeIdV1(self.u32()?),
                raw_parts: SemanticTypeIdV1(self.u32()?),
                element_storage: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
            },
            55 if self.wire_version == SemanticMirWireVersionV1::V8 => self.bf16_conversion()?,
            55 => SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
                scope: SemanticTypeIdV1(self.u32()?),
                pipeline: SemanticTypeIdV1(self.u32()?),
                buffers: self.u32()?,
                elements: self.u64()?,
                prefetch_distance: self.u32()?,
            },
            56 => SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineEvent {
                pipeline: SemanticTypeIdV1(self.u32()?),
                event: match self.tagged("workgroup pipeline event", 5)? {
                    0 => SemanticWorkgroupPipelineEventV1::Stage,
                    1 => SemanticWorkgroupPipelineEventV1::Commit,
                    2 => SemanticWorkgroupPipelineEventV1::Wait,
                    3 => SemanticWorkgroupPipelineEventV1::Consume,
                    4 => SemanticWorkgroupPipelineEventV1::Discard,
                    5 => SemanticWorkgroupPipelineEventV1::Release,
                    _ => unreachable!(),
                },
            },
            57 => SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite {
                pipeline: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
            },
            58 => SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead {
                pipeline: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
            },
            59 => self.bf16_conversion()?,
            60 => {
                let disjoint_slice = SemanticTypeIdV1(self.u32()?);
                let witness = SemanticTypeIdV1(self.u32()?);
                let element = SemanticTypeIdV1(self.u32()?);
                let raw_index = SemanticTypeIdV1(self.u32()?);
                let index_space = self.disjoint_index_space()?;
                let kind = match self.tagged("write-only disjoint-slice write kind", 5)? {
                    0 => SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint: false },
                    1 => SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint: true },
                    2 => SemanticWriteOnlyDisjointWriteKindV1::GridExclusive,
                    3 => SemanticWriteOnlyDisjointWriteKindV1::Block {
                        lanes_per_block: self.u64()?,
                        elements_per_lane: self.u64()?,
                    },
                    4 => SemanticWriteOnlyDisjointWriteKindV1::Tiled2d {
                        lanes_per_tile: self.u64()?,
                        tile_rows: self.u64()?,
                        tile_columns: self.u64()?,
                        elements_per_lane: self.u64()?,
                    },
                    5 => SemanticWriteOnlyDisjointWriteKindV1::RowStriped2d {
                        lanes_per_row: self.u64()?,
                        elements_per_lane: self.u64()?,
                    },
                    _ => unreachable!(),
                };
                SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
                    disjoint_slice,
                    witness,
                    element,
                    raw_index,
                    index_space,
                    kind,
                }
            }
            61 => SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceLen {
                disjoint_slice: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
                raw_index: SemanticTypeIdV1(self.u32()?),
                index_space: self.disjoint_index_space()?,
            },
            62 => SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupReduceSum {
                context: SemanticTypeIdV1(self.u32()?),
                dynamic_lds: SemanticTypeIdV1(self.u32()?),
                element_storage: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
            },
            63 => SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupScanSum {
                context: SemanticTypeIdV1(self.u32()?),
                dynamic_lds: SemanticTypeIdV1(self.u32()?),
                element_storage: SemanticTypeIdV1(self.u32()?),
                element: SemanticTypeIdV1(self.u32()?),
                kind: match self.tagged("workgroup scan kind", 1)? {
                    0 => SemanticWorkgroupScanKindV1::Inclusive,
                    1 => SemanticWorkgroupScanKindV1::Exclusive,
                    _ => unreachable!(),
                },
            },
            64 => SemanticCompilerIntrinsicOperationV1::Trap,
            65 => SemanticCompilerIntrinsicOperationV1::MemoryVolatileLoad {
                element: SemanticTypeIdV1(self.u32()?),
            },
            _ => unreachable!(),
        })
    }

    fn bf16_conversion(
        &mut self,
    ) -> Result<SemanticCompilerIntrinsicOperationV1, SemanticMirDecodeErrorV1> {
        Ok(SemanticCompilerIntrinsicOperationV1::Bf16Conversion {
            kind: match self.tagged("BF16 conversion kind", 3)? {
                0 => SemanticBf16ConversionKindV1::FromBits,
                1 => SemanticBf16ConversionKindV1::ToBits,
                2 => SemanticBf16ConversionKindV1::FromF32RoundTiesEven,
                3 => SemanticBf16ConversionKindV1::ToF32,
                _ => unreachable!(),
            },
            input: SemanticTypeIdV1(self.u32()?),
            output: SemanticTypeIdV1(self.u32()?),
        })
    }

    fn mfma_role(&mut self) -> Result<SemanticMfmaOperandRoleV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("MFMA operand role", 1)? {
            0 => SemanticMfmaOperandRoleV1::A,
            1 => SemanticMfmaOperandRoleV1::B,
            _ => unreachable!(),
        })
    }

    fn gfx950_lds_transpose_format(
        &mut self,
    ) -> Result<SemanticGfx950LdsTransposeFormatV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("gfx950 LDS transpose format", 1)? {
            0 => SemanticGfx950LdsTransposeFormatV1::Fp4E2M1,
            1 => SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
            _ => unreachable!(),
        })
    }

    fn mfma_storage_layout(
        &mut self,
    ) -> Result<SemanticMfmaStorageLayoutV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("MFMA storage layout", 1)? {
            0 => SemanticMfmaStorageLayoutV1::RowMajor,
            1 => SemanticMfmaStorageLayoutV1::LdsXor4,
            _ => unreachable!(),
        })
    }

    fn mfma_operand_contract(
        &mut self,
    ) -> Result<SemanticMfmaOperandContractV1, SemanticMirDecodeErrorV1> {
        let role = self.mfma_role()?;
        let profile = match self.tagged("MFMA profile", 2)? {
            0 => SemanticMfmaProfileV1::Bf16F32M16N16K16,
            1 => SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
            2 => SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128,
            _ => unreachable!(),
        };
        let register_distribution = match self.tagged("MFMA register distribution", 1)? {
            0 => SemanticMfmaRegisterDistributionV1::Tile16x16,
            1 => SemanticMfmaRegisterDistributionV1::Gfx950M16N16K128,
            _ => unreachable!(),
        };
        let wave_width = self.u32()?;
        Ok(SemanticMfmaOperandContractV1 {
            role,
            profile,
            register_distribution,
            wave_width,
        })
    }

    fn mfma_accumulator_contract(
        &mut self,
    ) -> Result<SemanticMfmaAccumulatorContractV1, SemanticMirDecodeErrorV1> {
        let profile = match self.tagged("MFMA profile", 2)? {
            0 => SemanticMfmaProfileV1::Bf16F32M16N16K16,
            1 => SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
            2 => SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128,
            _ => unreachable!(),
        };
        let distribution = match self.tagged("MFMA accumulator distribution", 0)? {
            0 => SemanticMfmaAccumulatorDistributionV1::RowMajor,
            _ => unreachable!(),
        };
        let wave_width = self.u32()?;
        Ok(SemanticMfmaAccumulatorContractV1 {
            profile,
            distribution,
            wave_width,
        })
    }

    fn disjoint_index_space(
        &mut self,
    ) -> Result<SemanticDisjointIndexSpaceV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("disjoint index space", 5)? {
            0 => SemanticDisjointIndexSpaceV1::Index1d,
            1 => SemanticDisjointIndexSpaceV1::ShiftedIndex1d {
                offset: self.u64()?,
            },
            2 => SemanticDisjointIndexSpaceV1::GridExclusive,
            3 => SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                lanes_per_block: self.u64()?,
                elements_per_lane: self.u64()?,
            },
            4 => SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
                lanes_per_tile: self.u64()?,
                tile_rows: self.u64()?,
                tile_columns: self.u64()?,
                elements_per_lane: self.u64()?,
            },
            5 => SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
                lanes_per_row: self.u64()?,
                elements_per_lane: self.u64()?,
            },
            _ => unreachable!(),
        })
    }

    fn axis(&mut self) -> Result<SemanticAxisV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("axis", 2)? {
            0 => SemanticAxisV1::X,
            1 => SemanticAxisV1::Y,
            2 => SemanticAxisV1::Z,
            _ => unreachable!(),
        })
    }

    fn statement(&mut self) -> Result<SemanticStatementKindV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("statement", 9)? {
            0 => SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                self.place()?,
                self.rvalue()?,
            )),
            1 => SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                self.place()?,
                self.operand()?,
                self.volatility()?,
                self.optional_atomic_access()?,
            )),
            2 => {
                let destination = self.place()?;
                let address = self.place()?;
                let value = self.operand()?;
                let operation = match self.tagged("atomic RMW operation", 10)? {
                    0 => SemanticAtomicRmwOpV1::Exchange,
                    1 => SemanticAtomicRmwOpV1::Add,
                    2 => SemanticAtomicRmwOpV1::Subtract,
                    3 => SemanticAtomicRmwOpV1::BitAnd,
                    4 => SemanticAtomicRmwOpV1::BitNand,
                    5 => SemanticAtomicRmwOpV1::BitOr,
                    6 => SemanticAtomicRmwOpV1::BitXor,
                    7 => SemanticAtomicRmwOpV1::SignedMaximum,
                    8 => SemanticAtomicRmwOpV1::SignedMinimum,
                    9 => SemanticAtomicRmwOpV1::UnsignedMaximum,
                    10 => SemanticAtomicRmwOpV1::UnsignedMinimum,
                    _ => unreachable!(),
                };
                SemanticStatementKindV1::AtomicRmw(SemanticAtomicRmwV1::new(
                    destination,
                    address,
                    value,
                    operation,
                    self.atomic_access()?,
                ))
            }
            3 => SemanticStatementKindV1::AtomicCompareExchange(
                SemanticAtomicCompareExchangeV1::new(
                    self.place()?,
                    self.place()?,
                    self.operand()?,
                    self.operand()?,
                    self.atomic_access()?,
                    self.atomic_ordering()?,
                    self.boolean()?,
                ),
            ),
            4 => SemanticStatementKindV1::SetDiscriminant {
                place: self.place()?,
                variant_index: self.u32()?,
            },
            5 => SemanticStatementKindV1::Deinitialize(self.place()?),
            6 => SemanticStatementKindV1::StorageLive(SemanticLocalIdV1(self.u32()?)),
            7 => SemanticStatementKindV1::StorageDead(SemanticLocalIdV1(self.u32()?)),
            8 => SemanticStatementKindV1::Nop,
            9 => SemanticStatementKindV1::Assume(self.operand()?),
            _ => unreachable!(),
        })
    }

    fn place(&mut self) -> Result<SemanticPlaceV1, SemanticMirDecodeErrorV1> {
        let local = SemanticLocalIdV1(self.u32()?);
        let projections = self.records(
            "projections",
            Some(SemanticMirResourceV1::Projections),
            |decoder| {
                let kind = match decoder.tagged("projection", 7)? {
                    0 => SemanticProjectionKindV1::Dereference,
                    1 => SemanticProjectionKindV1::Field(decoder.u32()?),
                    2 => SemanticProjectionKindV1::Index(SemanticLocalIdV1(decoder.u32()?)),
                    3 => SemanticProjectionKindV1::ConstantIndex {
                        offset: decoder.u64()?,
                        minimum_length: decoder.u64()?,
                        from_end: decoder.boolean()?,
                    },
                    4 => SemanticProjectionKindV1::Subslice {
                        from: decoder.u64()?,
                        to: decoder.u64()?,
                        from_end: decoder.boolean()?,
                    },
                    5 => SemanticProjectionKindV1::Downcast(decoder.u32()?),
                    6 => SemanticProjectionKindV1::OpaqueCast,
                    7 => SemanticProjectionKindV1::Subtype,
                    _ => unreachable!(),
                };
                SemanticProjectionV1::new(kind, SemanticTypeIdV1(decoder.u32()?))
                    .map_err(Into::into)
            },
        )?;
        let ty = SemanticTypeIdV1(self.u32()?);
        SemanticPlaceV1::new(local, projections, ty).map_err(Into::into)
    }

    fn operand(&mut self) -> Result<SemanticOperandV1, SemanticMirDecodeErrorV1> {
        self.charge(SemanticMirResourceV1::Operands, 1)?;
        self.operand_uncharged()
    }

    fn operand_uncharged(&mut self) -> Result<SemanticOperandV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("operand", 2)? {
            0 => SemanticOperandV1::Copy(self.place()?),
            1 => SemanticOperandV1::Move(self.place()?),
            2 => SemanticOperandV1::Constant(self.constant()?),
            _ => unreachable!(),
        })
    }

    fn constant(&mut self) -> Result<SemanticConstantV1, SemanticMirDecodeErrorV1> {
        let ty = SemanticTypeIdV1(self.u32()?);
        let value = match self.tagged("constant", 4)? {
            0 => SemanticConstantValueV1::ZeroSized,
            1 => SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(
                self.u128()?,
                self.u8()?,
            )?),
            2 => SemanticConstantValueV1::Bytes(SemanticConstantBytesV1::new(
                self.blob("constant bytes", SemanticMirResourceV1::ConstantBytes)?,
            )?),
            3 => {
                let byte_offset = self.u64()?;
                let provenance = self.pointer_provenance()?;
                let metadata = match self.tagged("pointer metadata value", 2)? {
                    0 => SemanticPointerValueMetadataV1::None,
                    1 => SemanticPointerValueMetadataV1::SliceLength(self.u64()?),
                    2 => SemanticPointerValueMetadataV1::VTable(SemanticVTableIdV1(self.u32()?)),
                    _ => unreachable!(),
                };
                SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new_with_metadata(
                    byte_offset,
                    provenance,
                    metadata,
                ))
            }
            4 => SemanticConstantValueV1::Callable(SemanticCallableIdV1(self.u32()?)),
            _ => unreachable!(),
        };
        Ok(SemanticConstantV1::new(ty, value))
    }

    fn pointer_provenance(
        &mut self,
    ) -> Result<SemanticPointerProvenanceV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("pointer provenance", 3)? {
            0 => SemanticPointerProvenanceV1::Allocation(SemanticAllocationIdV1(self.u32()?)),
            1 => SemanticPointerProvenanceV1::Callable(SemanticCallableIdV1(self.u32()?)),
            2 => SemanticPointerProvenanceV1::Static(SemanticStaticIdV1(self.u32()?)),
            3 => SemanticPointerProvenanceV1::ExposedAddress,
            _ => unreachable!(),
        })
    }

    fn rvalue(&mut self) -> Result<SemanticRvalueV1, SemanticMirDecodeErrorV1> {
        let result_type = SemanticTypeIdV1(self.u32()?);
        let maximum_tag = if self.wire_version != SemanticMirWireVersionV1::V2 {
            11
        } else {
            9
        };
        let kind = match self.tagged("rvalue", maximum_tag)? {
            0 => SemanticRvalueKindV1::Use(self.operand()?),
            1 => SemanticRvalueKindV1::Unary {
                operation: match self.tagged("unary operation", 2)? {
                    0 => SemanticUnaryOpV1::Not,
                    1 => SemanticUnaryOpV1::Negate,
                    2 => SemanticUnaryOpV1::PointerMetadata,
                    _ => unreachable!(),
                },
                operand: self.operand()?,
            },
            2 => SemanticRvalueKindV1::Binary {
                operation: self.binary_operation()?,
                left: self.operand()?,
                right: self.operand()?,
            },
            3 => SemanticRvalueKindV1::Cast {
                kind: match self.tagged("cast kind", 5)? {
                    0 => SemanticCastKindV1::Integer,
                    1 => SemanticCastKindV1::Float,
                    2 => SemanticCastKindV1::Pointer,
                    3 => SemanticCastKindV1::PointerExposeProvenance,
                    4 => SemanticCastKindV1::PointerWithExposedProvenance,
                    5 => SemanticCastKindV1::Transmute,
                    _ => unreachable!(),
                },
                operand: self.operand()?,
            },
            4 => SemanticRvalueKindV1::Borrow {
                kind: match self.tagged("borrow kind", 2)? {
                    0 => SemanticBorrowKindV1::Shared,
                    1 => SemanticBorrowKindV1::Mutable,
                    2 => SemanticBorrowKindV1::Fake,
                    _ => unreachable!(),
                },
                place: self.place()?,
            },
            5 => SemanticRvalueKindV1::AddressOf {
                mutability: self.mutability()?,
                place: self.place()?,
            },
            6 => SemanticRvalueKindV1::Length(self.place()?),
            7 => SemanticRvalueKindV1::Discriminant(self.place()?),
            8 => {
                let aggregate_kind = match self.tagged("aggregate kind", 3)? {
                    0 => SemanticAggregateKindV1::Array,
                    1 => SemanticAggregateKindV1::Tuple,
                    2 => SemanticAggregateKindV1::Aggregate,
                    3 => SemanticAggregateKindV1::EnumVariant(self.u32()?),
                    _ => unreachable!(),
                };
                let operands = self.records("aggregate operands", None, Self::operand)?;
                SemanticRvalueKindV1::Aggregate(SemanticAggregateRvalueV1::new(
                    aggregate_kind,
                    operands,
                )?)
            }
            9 => SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                self.place()?,
                self.volatility()?,
                self.optional_atomic_access()?,
            )),
            10 => {
                let operation = match self.tagged("checked binary operation", 2)? {
                    0 => SemanticCheckedBinaryOpV1::Add,
                    1 => SemanticCheckedBinaryOpV1::Subtract,
                    2 => SemanticCheckedBinaryOpV1::Multiply,
                    _ => unreachable!(),
                };
                self.charge(SemanticMirResourceV1::Operands, 2)?;
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    operation,
                    self.operand_uncharged()?,
                    self.operand_uncharged()?,
                ))
            }
            11 => {
                let operation = match self.tagged("unchecked binary operation", 2)? {
                    0 => SemanticUncheckedBinaryOpV1::Add,
                    1 => SemanticUncheckedBinaryOpV1::Subtract,
                    2 => SemanticUncheckedBinaryOpV1::Multiply,
                    _ => unreachable!(),
                };
                self.charge(SemanticMirResourceV1::Operands, 2)?;
                SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
                    operation,
                    self.operand_uncharged()?,
                    self.operand_uncharged()?,
                ))
            }
            _ => unreachable!(),
        };
        Ok(SemanticRvalueV1::new(result_type, kind))
    }

    fn binary_operation(&mut self) -> Result<SemanticBinaryOpV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("binary operation", 16)? {
            0 => SemanticBinaryOpV1::Add,
            1 => SemanticBinaryOpV1::Subtract,
            2 => SemanticBinaryOpV1::Multiply,
            3 => SemanticBinaryOpV1::Divide,
            4 => SemanticBinaryOpV1::Remainder,
            5 => SemanticBinaryOpV1::BitXor,
            6 => SemanticBinaryOpV1::BitAnd,
            7 => SemanticBinaryOpV1::BitOr,
            8 => SemanticBinaryOpV1::ShiftLeft,
            9 => SemanticBinaryOpV1::ShiftRight,
            10 => SemanticBinaryOpV1::Equal,
            11 => SemanticBinaryOpV1::LessThan,
            12 => SemanticBinaryOpV1::LessOrEqual,
            13 => SemanticBinaryOpV1::NotEqual,
            14 => SemanticBinaryOpV1::GreaterOrEqual,
            15 => SemanticBinaryOpV1::GreaterThan,
            16 => SemanticBinaryOpV1::Offset,
            _ => unreachable!(),
        })
    }

    fn volatility(&mut self) -> Result<SemanticVolatilityV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("volatility", 1)? {
            0 => SemanticVolatilityV1::NonVolatile,
            1 => SemanticVolatilityV1::Volatile,
            _ => unreachable!(),
        })
    }

    fn optional_atomic_access(
        &mut self,
    ) -> Result<Option<SemanticAtomicAccessV1>, SemanticMirDecodeErrorV1> {
        self.option("atomic access", Self::atomic_access)
    }

    fn atomic_access(&mut self) -> Result<SemanticAtomicAccessV1, SemanticMirDecodeErrorV1> {
        Ok(SemanticAtomicAccessV1::new(
            self.atomic_ordering()?,
            match self.tagged("atomic scope", 4)? {
                0 => SemanticAtomicScopeV1::SingleThread,
                1 => SemanticAtomicScopeV1::Workgroup,
                2 => SemanticAtomicScopeV1::Agent,
                3 => SemanticAtomicScopeV1::Device,
                4 => SemanticAtomicScopeV1::System,
                _ => unreachable!(),
            },
        ))
    }

    fn atomic_ordering(&mut self) -> Result<SemanticAtomicOrderingV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("atomic ordering", 4)? {
            0 => SemanticAtomicOrderingV1::Relaxed,
            1 => SemanticAtomicOrderingV1::Release,
            2 => SemanticAtomicOrderingV1::Acquire,
            3 => SemanticAtomicOrderingV1::AcquireRelease,
            4 => SemanticAtomicOrderingV1::SequentiallyConsistent,
            _ => unreachable!(),
        })
    }

    fn terminator(&mut self) -> Result<SemanticTerminatorKindV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("terminator", 11)? {
            0 => SemanticTerminatorKindV1::Goto(self.edge()?),
            1 => {
                let discriminant = self.operand()?;
                let values = self.records(
                    "switch targets",
                    Some(SemanticMirResourceV1::SwitchTargets),
                    |decoder| {
                        Ok(SemanticSwitchTargetV1::new(
                            decoder.u128()?,
                            decoder.edge()?,
                        ))
                    },
                )?;
                let otherwise = self.edge()?;
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant,
                    targets: SemanticSwitchTargetsV1::new(values, otherwise)?,
                }
            }
            2 => {
                let callee = SemanticCallableIdV1(self.u32()?);
                let arguments = self.records(
                    "call arguments",
                    Some(SemanticMirResourceV1::CallArguments),
                    Self::operand,
                )?;
                let variadic_argument_abis = self.records(
                    "variadic argument ABIs",
                    Some(SemanticMirResourceV1::CallArguments),
                    Self::abi_value,
                )?;
                let destination = self.option("call destination", |decoder| {
                    Ok(SemanticCallDestinationV1::new(
                        decoder.place()?,
                        decoder.edge()?,
                    ))
                })?;
                let unwind = self.unwind()?;
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
                        callee,
                        arguments,
                        variadic_argument_abis,
                        destination,
                        unwind,
                    )?,
                )
            }
            3 => {
                let callee = SemanticCallableIdV1(self.u32()?);
                let arguments = self.records(
                    "tail call arguments",
                    Some(SemanticMirResourceV1::CallArguments),
                    Self::operand,
                )?;
                let unwind = self.unwind()?;
                SemanticTerminatorKindV1::TailCall(SemanticDirectTailCallV1::new_callable(
                    callee, arguments, unwind,
                )?)
            }
            4 => SemanticTerminatorKindV1::Drop {
                place: self.place()?,
                drop_glue: SemanticFunctionIdV1(self.u32()?),
                target: self.edge()?,
                unwind: self.unwind()?,
            },
            5 => SemanticTerminatorKindV1::Assert {
                condition: self.operand()?,
                expected: self.boolean()?,
                message: self.assert_message()?,
                target: self.edge()?,
                unwind: self.unwind()?,
            },
            6 => SemanticTerminatorKindV1::FalseEdge {
                real_target: self.edge()?,
                imaginary_target: self.edge()?,
            },
            7 => SemanticTerminatorKindV1::Return,
            8 => SemanticTerminatorKindV1::UnwindResume,
            9 => SemanticTerminatorKindV1::UnwindTerminate,
            10 => SemanticTerminatorKindV1::Abort,
            11 => SemanticTerminatorKindV1::Unreachable,
            _ => unreachable!(),
        })
    }

    fn edge(&mut self) -> Result<SemanticControlFlowEdgeV1, SemanticMirDecodeErrorV1> {
        let role = match self.tagged("edge role", 11)? {
            0 => SemanticEdgeRoleV1::Goto,
            1 => SemanticEdgeRoleV1::SwitchValue,
            2 => SemanticEdgeRoleV1::SwitchOtherwise,
            3 => SemanticEdgeRoleV1::CallReturn,
            4 => SemanticEdgeRoleV1::CallUnwind,
            5 => SemanticEdgeRoleV1::TailCallUnwind,
            6 => SemanticEdgeRoleV1::DropReturn,
            7 => SemanticEdgeRoleV1::DropUnwind,
            8 => SemanticEdgeRoleV1::AssertSuccess,
            9 => SemanticEdgeRoleV1::AssertUnwind,
            10 => SemanticEdgeRoleV1::FalseEdgeReal,
            11 => SemanticEdgeRoleV1::FalseEdgeImaginary,
            _ => unreachable!(),
        };
        Ok(SemanticControlFlowEdgeV1::new(
            role,
            SemanticBlockIdV1(self.u32()?),
        ))
    }

    fn unwind(&mut self) -> Result<SemanticUnwindActionV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("unwind action", 3)? {
            0 => SemanticUnwindActionV1::Continue,
            1 => SemanticUnwindActionV1::Unreachable,
            2 => SemanticUnwindActionV1::Terminate,
            3 => SemanticUnwindActionV1::Cleanup(self.edge()?),
            _ => unreachable!(),
        })
    }

    fn assert_message(&mut self) -> Result<SemanticAssertMessageV1, SemanticMirDecodeErrorV1> {
        Ok(match self.tagged("assert message", 7)? {
            0 => SemanticAssertMessageV1::BoundsCheck {
                length: self.operand()?,
                index: self.operand()?,
            },
            1 => SemanticAssertMessageV1::Overflow {
                operation: self.binary_operation()?,
                left: self.operand()?,
                right: self.operand()?,
            },
            2 => SemanticAssertMessageV1::DivisionByZero(self.operand()?),
            3 => SemanticAssertMessageV1::RemainderByZero(self.operand()?),
            4 => SemanticAssertMessageV1::MisalignedPointerDereference {
                required_alignment: self.operand()?,
                found_alignment: self.operand()?,
            },
            5 => SemanticAssertMessageV1::NullPointerDereference,
            6 => SemanticAssertMessageV1::ResumedAfterReturn,
            7 => SemanticAssertMessageV1::ResumedAfterPanic,
            _ => unreachable!(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Debug;

    fn identity(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn minimal_request() -> InertSemanticMirRequestV1 {
        let ty_id = SemanticTypeIdV1::from_index(0);
        let layout = SemanticTypeLayoutV1::new_with_backend_repr(
            Some(4),
            4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
            )),
            false,
        )
        .unwrap();
        let ty = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1(identity(1)),
            SemanticLayoutIdentityV1(identity(2)),
            layout,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        );
        let mode = SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        );
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1(identity(3)),
            SemanticLayoutIdentityV1(identity(4)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![SemanticAbiValueV1::new(ty_id, mode.clone())],
            SemanticAbiValueV1::new(ty_id, mode),
        )
        .unwrap();
        let locals = vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1(identity(5)),
                ty_id,
                SemanticLocalRoleV1::Return,
                SemanticSourceProvenanceV1::unavailable(),
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1(identity(6)),
                ty_id,
                SemanticLocalRoleV1::Argument(0),
                SemanticSourceProvenanceV1::unavailable(),
            ),
        ];
        let block = SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(7)),
            SemanticSourceProvenanceV1::unavailable(),
            vec![],
            SemanticTerminatorV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticTerminatorKindV1::Return,
            ),
        )
        .unwrap();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1(identity(8)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1(identity(9)),
            SemanticMonomorphizationIdentityV1(identity(10)),
            SemanticGenericTypeArgumentsIdentityV1(identity(11)),
            SemanticConstGenericArgumentsIdentityV1(identity(12)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            vec![block],
        )
        .unwrap();
        InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1(identity(13))),
            vec![ty],
            vec![],
            vec![],
            vec![],
            vec![function],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
    }

    fn version_selection_request(
        operations: impl IntoIterator<Item = SemanticCompilerIntrinsicOperationV1>,
    ) -> InertSemanticMirRequestV1 {
        let request = minimal_request();
        let abi = request.functions[0].abi.clone();
        let mut callables = vec![SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )];
        callables.extend(
            operations
                .into_iter()
                .enumerate()
                .map(|(index, operation)| {
                    let tag = 120 + u8::try_from(index).unwrap();
                    SemanticCallableDeclV1::CompilerIntrinsic {
                        binding: SemanticNonBodyCallableBindingV1::new(
                            SemanticFunctionIdentityV1(identity(tag)),
                            SemanticItemDefinitionIdentityV1(identity(tag)),
                            SemanticMonomorphizationIdentityV1(identity(tag)),
                            SemanticGenericTypeArgumentsIdentityV1(identity(tag)),
                            SemanticConstGenericArgumentsIdentityV1(identity(tag)),
                            SemanticSourceProvenanceV1::unavailable(),
                            abi.clone(),
                        ),
                        operation,
                        operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(tag)),
                    }
                }),
        );
        InertSemanticMirRequestV1::new_with_callables(
            request.target,
            request.types.into_vec(),
            request.allocations.into_vec(),
            request.statics.into_vec(),
            request.vtables.into_vec(),
            request.functions.into_vec(),
            callables,
            request.roots.into_vec(),
        )
        .unwrap()
    }

    fn non_scan_trap_request() -> InertSemanticMirRequestV1 {
        let request = minimal_request();
        let never = SemanticTypeIdV1::from_index(u32::try_from(request.types.len()).unwrap());
        let mut types = request.types.into_vec();
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1(identity(130)),
            SemanticLayoutIdentityV1(identity(131)),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::Primitive,
                SemanticRustcVariantsV1::Empty,
                SemanticBackendReprV1::memory(true),
                None,
                true,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Never,
        ));
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1(identity(132)),
            SemanticLayoutIdentityV1(identity(133)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![],
            SemanticAbiValueV1::new(never, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let trap = SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1(identity(134)),
                SemanticItemDefinitionIdentityV1(identity(135)),
                SemanticMonomorphizationIdentityV1(identity(136)),
                SemanticGenericTypeArgumentsIdentityV1(identity(137)),
                SemanticConstGenericArgumentsIdentityV1(identity(138)),
                SemanticSourceProvenanceV1::unavailable(),
                abi,
            ),
            operation: SemanticCompilerIntrinsicOperationV1::Trap,
            operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(139)),
        };
        let mut callables = request.callables.into_vec();
        callables.push(trap);
        let mut functions = request.functions.into_vec();
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        functions[0].blocks[0] = SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1(identity(140)),
            SemanticSourceProvenanceV1::unavailable(),
            vec![],
            SemanticTerminatorV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticTerminatorKindV1::Call(call),
            ),
        )
        .unwrap();
        InertSemanticMirRequestV1::new_with_callables(
            request.target,
            types,
            request.allocations.into_vec(),
            request.statics.into_vec(),
            request.vtables.into_vec(),
            functions,
            callables,
            request.roots.into_vec(),
        )
        .unwrap()
    }

    fn component_round_trip<T: Debug + Eq>(
        value: T,
        encode: impl Fn(&mut CanonicalWriterV1, &T) -> Result<(), SemanticMirErrorV1>,
        decode: impl Fn(&mut CanonicalDecoderV1<'_>) -> Result<T, SemanticMirDecodeErrorV1>,
    ) {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode(&mut writer, &value).unwrap();
        let encoded = writer.finish();
        let mut decoder = CanonicalDecoderV1::new(&encoded, SemanticMirLimitsV1::default());
        let decoded = decode(&mut decoder).unwrap();
        decoder.finish().unwrap();
        assert_eq!(decoded, value);

        let mut reencoder = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode(&mut reencoder, &decoded).unwrap();
        assert_eq!(reencoder.finish(), encoded);
    }

    fn compiler_intrinsic_round_trip(
        operation: SemanticCompilerIntrinsicOperationV1,
        wire_version: SemanticMirWireVersionV1,
    ) -> Vec<u8> {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_compiler_intrinsic_operation(&mut writer, operation, wire_version).unwrap();
        let encoded = writer.finish();
        let mut decoder = CanonicalDecoderV1::new(&encoded, SemanticMirLimitsV1::default());
        decoder.wire_version = wire_version;
        let decoded = decoder.compiler_intrinsic().unwrap();
        decoder.finish().unwrap();
        assert_eq!(decoded, operation);

        let mut reencoder = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        encode_compiler_intrinsic_operation(&mut reencoder, decoded, wire_version).unwrap();
        assert_eq!(reencoder.finish(), encoded);
        encoded
    }

    #[test]
    fn canonical_model_round_trips_losslessly_without_authority() {
        let original = minimal_request()
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_canonical(
            original.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();

        assert_eq!(decoded.canonical_encoding(), original.canonical_encoding());
        assert_eq!(decoded.semantic_sha256(), original.semantic_sha256());
        assert_eq!(decoded.types(), original.types());
        assert_eq!(decoded.functions(), original.functions());
        assert_eq!(decoded.callables(), original.callables());
        assert_eq!(decoded.roots(), original.roots());
    }

    #[test]
    fn current_production_decoder_preserves_exact_v5_through_v12_custody() {
        let limits = SemanticMirLimitsV1::default();
        let admitted = [
            minimal_request().admit_exact_v5(limits).unwrap(),
            minimal_request().admit_exact_v6(limits).unwrap(),
            minimal_request().admit_exact_v7(limits).unwrap(),
            minimal_request().admit_exact_v8(limits).unwrap(),
            minimal_request().admit_exact_v9(limits).unwrap(),
            minimal_request().admit_exact_v10(limits).unwrap(),
            minimal_request().admit_exact_v11(limits).unwrap(),
            minimal_request().admit_exact_v12(limits).unwrap(),
        ];
        for original in admitted {
            let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
                original.canonical_encoding(),
                limits,
            )
            .unwrap();
            assert_eq!(decoded.wire_version(), original.wire_version());
            assert_eq!(decoded.canonical_encoding(), original.canonical_encoding());
            assert_eq!(decoded.semantic_sha256(), original.semantic_sha256());
        }

        let legacy = minimal_request().admit_exact_v4(limits).unwrap();
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_current_production_canonical(
                legacy.canonical_encoding(),
                limits,
            )
            .unwrap_err(),
            SemanticMirDecodeErrorV1::UnsupportedProductionWireVersion(
                SemanticMirWireVersionV1::V4
            )
        );
    }

    #[test]
    fn full_decoder_rejects_every_truncation_and_trailing_input() {
        let admitted = minimal_request()
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        let encoded = admitted.canonical_encoding();
        for end in 0..encoded.len() {
            assert!(
                AdmittedInertSemanticMirV1::decode_canonical(
                    &encoded[..end],
                    SemanticMirLimitsV1::default(),
                )
                .is_err(),
                "decoder accepted truncation at byte {end}"
            );
        }

        let mut trailing = encoded.to_vec();
        trailing.push(0);
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_canonical(&trailing, SemanticMirLimitsV1::default(),),
            Err(SemanticMirDecodeErrorV1::TrailingBytes { trailing: 1, .. })
        ));
    }

    #[test]
    fn single_bit_mutation_corpus_never_panics_or_decodes_noncanonically() {
        let admitted = minimal_request()
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        let encoded = admitted.canonical_encoding();
        for byte in 0..encoded.len() {
            for bit in 0..8 {
                let mut mutated = encoded.to_vec();
                mutated[byte] ^= 1 << bit;
                let result = std::panic::catch_unwind(|| {
                    AdmittedInertSemanticMirV1::decode_canonical(
                        &mutated,
                        SemanticMirLimitsV1::default(),
                    )
                });
                let decoded = result
                    .unwrap_or_else(|_| panic!("decoder panicked for byte {byte}, bit {bit}"));
                if let Ok(decoded) = decoded {
                    assert_eq!(
                        decoded.canonical_encoding(),
                        mutated,
                        "decoder accepted noncanonical byte {byte}, bit {bit}"
                    );
                }
            }
        }
    }

    #[test]
    fn full_decoder_rejects_envelope_noncanonical_and_input_limit_mutations() {
        let admitted = minimal_request()
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        let encoded = admitted.canonical_encoding();

        let mut invalid_magic = encoded.to_vec();
        invalid_magic[0] ^= 1;
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_canonical(
                &invalid_magic,
                SemanticMirLimitsV1::default(),
            )
            .unwrap_err(),
            SemanticMirDecodeErrorV1::InvalidMagic
        );

        let mut invalid_version = encoded.to_vec();
        invalid_version[MAGIC.len()..MAGIC.len() + 2].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_canonical(
                &invalid_version,
                SemanticMirLimitsV1::default(),
            )
            .unwrap_err(),
            SemanticMirDecodeErrorV1::UnsupportedVersion(u16::MAX)
        );

        let architecture_offset = MAGIC.len() + 2 + 32;
        let mut invalid_architecture = encoded.to_vec();
        invalid_architecture[architecture_offset] = 1;
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_canonical(
                &invalid_architecture,
                SemanticMirLimitsV1::default(),
            ),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "target architecture",
                ..
            })
        ));

        let mut private_noncanonical_target = encoded.to_vec();
        private_noncanonical_target[architecture_offset + 1..architecture_offset + 9]
            .copy_from_slice(&(1_u64 << 60).to_le_bytes());
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_canonical(
                &private_noncanonical_target,
                SemanticMirLimitsV1::default(),
            )
            .unwrap_err(),
            SemanticMirDecodeErrorV1::Validation(SemanticMirErrorV1::InvalidTypeLayout,)
        );

        let max = u64::try_from(encoded.len() - 1).unwrap();
        let limits = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::CanonicalBytes, max)
            .unwrap();
        assert_eq!(
            AdmittedInertSemanticMirV1::decode_canonical(encoded, limits).unwrap_err(),
            SemanticMirDecodeErrorV1::InputLimitExceeded {
                actual: u64::try_from(encoded.len()).unwrap(),
                max,
            }
        );
    }

    #[test]
    fn full_decoder_enforces_declared_and_aggregate_resource_limits() {
        let admitted = minimal_request()
            .admit(SemanticMirLimitsV1::default())
            .unwrap();
        let encoded = admitted.canonical_encoding();
        for (resource, actual) in [
            (SemanticMirResourceV1::Types, 1),
            (SemanticMirResourceV1::Functions, 1),
            (SemanticMirResourceV1::Callables, 1),
            (SemanticMirResourceV1::Roots, 1),
            (SemanticMirResourceV1::Locals, 2),
            (SemanticMirResourceV1::Blocks, 1),
            (SemanticMirResourceV1::CallArguments, 2),
        ] {
            let limits = SemanticMirLimitsV1::default()
                .with_limit(resource, actual - 1)
                .unwrap();
            assert!(matches!(
                AdmittedInertSemanticMirV1::decode_canonical(encoded, limits),
                Err(SemanticMirDecodeErrorV1::Validation(
                    SemanticMirErrorV1::LimitExceeded {
                        resource: observed,
                        actual: observed_actual,
                        max,
                    },
                )) if observed == resource && observed_actual > max
            ));
        }
    }

    #[test]
    fn nested_record_counts_are_charged_before_payload_decode() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.push(7);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.push(9);
        let limits = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::Locals, 1)
            .unwrap();
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);

        assert_eq!(
            decoder
                .records(
                    "first locals",
                    Some(SemanticMirResourceV1::Locals),
                    CanonicalDecoderV1::u8,
                )
                .unwrap(),
            vec![7]
        );
        assert!(matches!(
            decoder.records(
                "second locals",
                Some(SemanticMirResourceV1::Locals),
                CanonicalDecoderV1::u8,
            ),
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::Locals,
                    actual: 2,
                    max: 1,
                }
            ))
        ));
        assert_eq!(decoder.offset, 9, "the second payload was not read");
    }

    #[test]
    fn checked_operands_are_precharged_atomically_before_nested_payloads() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.push(10);
        bytes.push(0);
        bytes.push(u8::MAX);
        let limits = SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::Operands, 1)
            .unwrap();
        let mut decoder = CanonicalDecoderV1::new(&bytes, limits);

        assert_eq!(
            decoder.rvalue(),
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::Operands,
                    actual: 2,
                    max: 1,
                }
            ))
        );
        assert_eq!(
            decoder.offset, 6,
            "the hostile first operand tag must not be parsed"
        );
    }

    #[test]
    fn checked_nested_operand_and_constant_tags_fail_closed() {
        let prefix = || {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&0_u32.to_le_bytes());
            bytes.push(10);
            bytes.push(0);
            bytes
        };

        let mut invalid_operand = prefix();
        invalid_operand.push(u8::MAX);
        let mut decoder = CanonicalDecoderV1::new(&invalid_operand, SemanticMirLimitsV1::default());
        assert_eq!(
            decoder.rvalue(),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "operand",
                offset: 6,
                value: u8::MAX,
            })
        );

        let mut invalid_constant = prefix();
        invalid_constant.push(2);
        invalid_constant.extend_from_slice(&0_u32.to_le_bytes());
        invalid_constant.push(5);
        let mut decoder =
            CanonicalDecoderV1::new(&invalid_constant, SemanticMirLimitsV1::default());
        assert_eq!(
            decoder.rvalue(),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "constant",
                offset: 11,
                value: 5,
            })
        );
    }

    #[test]
    fn checked_hostile_byte_constant_is_bounded_before_payload_read() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.push(10);
        bytes.push(0);
        bytes.push(2);
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.push(2);
        bytes.extend_from_slice(&u64::MAX.to_le_bytes());
        let mut decoder = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());

        assert!(matches!(
            decoder.rvalue(),
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::ConstantBytes,
                    actual: u64::MAX,
                    max: HARD_MAX_CONSTANT_BYTES_V1,
                }
            ))
        ));
        assert_eq!(
            decoder.offset,
            bytes.len(),
            "only the hostile length field may be consumed"
        );
    }

    #[test]
    fn malformed_tags_booleans_lengths_and_decoder_resources_are_bounded() {
        let mut invalid_boolean = CanonicalDecoderV1::new(&[2], SemanticMirLimitsV1::default());
        assert_eq!(
            invalid_boolean.boolean(),
            Err(SemanticMirDecodeErrorV1::InvalidBoolean {
                offset: 0,
                value: 2,
            })
        );

        let mut invalid_tag = CanonicalDecoderV1::new(&[9], SemanticMirLimitsV1::default());
        assert!(matches!(
            invalid_tag.tagged("hostile", 3),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "hostile",
                offset: 0,
                value: 9,
            })
        ));

        let mut hostile_blob = u64::MAX.to_le_bytes().to_vec();
        hostile_blob.push(0);
        let mut invalid_length =
            CanonicalDecoderV1::new(&hostile_blob, SemanticMirLimitsV1::default());
        assert!(matches!(
            invalid_length.blob("hostile blob", SemanticMirResourceV1::ConstantBytes),
            Err(SemanticMirDecodeErrorV1::Validation(
                SemanticMirErrorV1::LimitExceeded {
                    resource: SemanticMirResourceV1::ConstantBytes,
                    ..
                }
            ))
        ));

        for resource in [
            SemanticMirResourceV1::Types,
            SemanticMirResourceV1::Functions,
            SemanticMirResourceV1::Callables,
            SemanticMirResourceV1::Allocations,
            SemanticMirResourceV1::Statics,
            SemanticMirResourceV1::VTables,
            SemanticMirResourceV1::Roots,
            SemanticMirResourceV1::Locals,
            SemanticMirResourceV1::Blocks,
            SemanticMirResourceV1::Statements,
            SemanticMirResourceV1::Projections,
            SemanticMirResourceV1::Operands,
            SemanticMirResourceV1::CallArguments,
            SemanticMirResourceV1::SwitchTargets,
            SemanticMirResourceV1::Relocations,
            SemanticMirResourceV1::ConstantBytes,
            SemanticMirResourceV1::LinkSymbolBytes,
            SemanticMirResourceV1::CanonicalBytes,
        ] {
            let limits = SemanticMirLimitsV1::default()
                .with_limit(resource, 0)
                .unwrap();
            let mut decoder = CanonicalDecoderV1::new(&[], limits);
            assert!(matches!(
                decoder.charge(resource, 1),
                Err(SemanticMirDecodeErrorV1::Validation(
                    SemanticMirErrorV1::LimitExceeded {
                        resource: observed,
                        actual: 1,
                        max: 0,
                    }
                )) if observed == resource
            ));
        }
    }

    #[test]
    fn every_type_layout_and_abi_wire_variant_round_trips_exactly() {
        for scalar in [
            SemanticScalarTypeV1::Bool,
            SemanticScalarTypeV1::Char,
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
            SemanticScalarTypeV1::Float { bits: 32 },
        ] {
            component_round_trip(
                scalar,
                |writer, value| encode_scalar_type(writer, *value),
                |decoder| decoder.scalar_type(),
            );
        }

        let primitives = [
            SemanticBackendPrimitiveV1::integer(true, 32, 4),
            SemanticBackendPrimitiveV1::float(32, 4),
            SemanticBackendPrimitiveV1::pointer(1, 8, 8),
        ];
        for primitive in primitives {
            component_round_trip(
                primitive,
                |writer, value| encode_backend_primitive(writer, *value),
                |decoder| decoder.backend_primitive(),
            );
        }

        let initialized = SemanticBackendScalarV1::initialized(
            primitives[0],
            SemanticScalarValidityRangeV1::new(0, u128::from(u32::MAX)),
        );
        let union = SemanticBackendScalarV1::union(primitives[0]);
        for scalar in [initialized, union] {
            component_round_trip(
                scalar,
                |writer, value| encode_backend_scalar(writer, *value),
                |decoder| decoder.backend_scalar(),
            );
        }

        for repr in [
            SemanticBackendReprV1::memory(true),
            SemanticBackendReprV1::scalar(initialized),
            SemanticBackendReprV1::scalar_pair(initialized, union),
            SemanticBackendReprV1::simd_vector(initialized, 4),
            SemanticBackendReprV1::simd_scalable_vector(initialized, 4),
        ] {
            component_round_trip(repr, encode_backend_repr, |decoder| decoder.backend_repr());
        }

        for fields in [
            SemanticFieldsShapeV1::Primitive,
            SemanticFieldsShapeV1::union(2).unwrap(),
            SemanticFieldsShapeV1::array(4, 8),
            SemanticFieldsShapeV1::arbitrary(vec![0, 4], vec![0, 1]).unwrap(),
        ] {
            component_round_trip(fields, encode_fields_shape, |decoder| {
                decoder.fields_shape()
            });
        }

        for pointee in [
            None,
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                    4,
                    4,
                )
                .unwrap(),
            ),
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                    4,
                    4,
                )
                .unwrap(),
            ),
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::Box {
                        unpin: true,
                        global: true,
                    },
                    0,
                    8,
                )
                .unwrap(),
            ),
        ] {
            component_round_trip(
                pointee,
                |writer, value| encode_optional_pointee_info(writer, *value),
                |decoder| decoder.optional_pointee_info(),
            );
        }

        let aggregate =
            SemanticAggregateLayoutV1::new(vec![0], vec![SemanticPaddingV1::new(4, 4).unwrap()])
                .unwrap();
        for details in [
            SemanticTypeLayoutDetailsV1::None,
            SemanticTypeLayoutDetailsV1::Aggregate(aggregate),
        ] {
            component_round_trip(details, encode_layout_details, |decoder| {
                decoder.layout_details()
            });
        }

        for abi in [
            SemanticCanonAbiV1::Rust,
            SemanticCanonAbiV1::C,
            SemanticCanonAbiV1::RustCold,
            SemanticCanonAbiV1::GpuKernel,
            SemanticCanonAbiV1::RustPreserveNone,
            SemanticCanonAbiV1::Custom,
            SemanticCanonAbiV1::Arm(SemanticArmCallV1::Aapcs),
            SemanticCanonAbiV1::Arm(SemanticArmCallV1::CCmseNonSecureCall),
            SemanticCanonAbiV1::Arm(SemanticArmCallV1::CCmseNonSecureEntry),
            SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::Avr),
            SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::AvrNonBlocking),
            SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::Msp430),
            SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::RiscvMachine),
            SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::RiscvSupervisor),
            SemanticCanonAbiV1::Interrupt(SemanticInterruptKindV1::X86),
            SemanticCanonAbiV1::X86(SemanticX86CallV1::Fastcall),
            SemanticCanonAbiV1::X86(SemanticX86CallV1::Stdcall),
            SemanticCanonAbiV1::X86(SemanticX86CallV1::SysV64),
            SemanticCanonAbiV1::X86(SemanticX86CallV1::Thiscall),
            SemanticCanonAbiV1::X86(SemanticX86CallV1::Vectorcall),
            SemanticCanonAbiV1::X86(SemanticX86CallV1::Win64),
        ] {
            component_round_trip(
                abi,
                |writer, value| encode_canon_abi(writer, *value),
                |decoder| decoder.canon_abi(),
            );
        }

        for abi in [
            SemanticExternAbiV1::C { unwind: false },
            SemanticExternAbiV1::C { unwind: true },
            SemanticExternAbiV1::System { unwind: false },
            SemanticExternAbiV1::System { unwind: true },
            SemanticExternAbiV1::Cdecl { unwind: false },
            SemanticExternAbiV1::Cdecl { unwind: true },
            SemanticExternAbiV1::Rust,
            SemanticExternAbiV1::RustCall,
            SemanticExternAbiV1::RustCold,
            SemanticExternAbiV1::RustPreserveNone,
            SemanticExternAbiV1::Unadjusted,
            SemanticExternAbiV1::Custom,
            SemanticExternAbiV1::GpuKernel,
        ] {
            component_round_trip(
                abi,
                |writer, value| encode_extern_abi(writer, *value),
                |decoder| decoder.extern_abi(),
            );
        }
    }

    #[test]
    fn every_intrinsic_axis_math_reduction_and_index_space_variant_round_trips_exactly() {
        let t = |index| SemanticTypeIdV1::from_index(index);
        let index_spaces = [
            SemanticDisjointIndexSpaceV1::Index1d,
            SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 3 },
            SemanticDisjointIndexSpaceV1::GridExclusive,
            SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                lanes_per_block: 16,
                elements_per_lane: 4,
            },
            SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
                lanes_per_tile: 64,
                tile_rows: 8,
                tile_columns: 8,
                elements_per_lane: 1,
            },
            SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
                lanes_per_row: 64,
                elements_per_lane: 64,
            },
        ];
        for space in index_spaces {
            component_round_trip(
                space,
                |writer, value| encode_disjoint_index_space(writer, *value),
                |decoder| decoder.disjoint_index_space(),
            );
        }
        for axis in [SemanticAxisV1::X, SemanticAxisV1::Y, SemanticAxisV1::Z] {
            component_round_trip(
                axis,
                |writer, value| encode_axis(writer, *value),
                |decoder| decoder.axis(),
            );
        }

        let lhs = SemanticMfmaOperandContractV1 {
            role: SemanticMfmaOperandRoleV1::A,
            profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
            register_distribution: SemanticMfmaRegisterDistributionV1::Tile16x16,
            wave_width: 64,
        };
        let rhs = SemanticMfmaOperandContractV1 {
            role: SemanticMfmaOperandRoleV1::B,
            ..lhs
        };
        let accumulator = SemanticMfmaAccumulatorContractV1 {
            profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
            distribution: SemanticMfmaAccumulatorDistributionV1::RowMajor,
            wave_width: 64,
        };
        let gfx950_rhs = SemanticMfmaOperandContractV1 {
            role: SemanticMfmaOperandRoleV1::B,
            profile: SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128,
            register_distribution: SemanticMfmaRegisterDistributionV1::Gfx950M16N16K128,
            wave_width: 64,
        };
        let mut operations = vec![
            SemanticCompilerIntrinsicOperationV1::ThreadIndex(SemanticAxisV1::X),
            SemanticCompilerIntrinsicOperationV1::WorkgroupIndex(SemanticAxisV1::Y),
            SemanticCompilerIntrinsicOperationV1::WorkgroupDimension(SemanticAxisV1::Z),
            SemanticCompilerIntrinsicOperationV1::GridDimension(SemanticAxisV1::X),
            SemanticCompilerIntrinsicOperationV1::DynamicLdsExactCurrent {
                scope: t(0),
                dynamic_lds: t(1),
                element_storage: t(2),
                elements: 64,
            },
            SemanticCompilerIntrinsicOperationV1::WorkgroupReduceSum {
                workgroup: t(0),
                context: t(1),
                scratch: t(2),
                element: t(3),
            },
            SemanticCompilerIntrinsicOperationV1::DynamicLdsIntoCollectiveRawParts {
                dynamic_lds: t(0),
                raw_parts: t(1),
                element_storage: t(2),
                element: t(3),
            },
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
                scope: t(0),
                pipeline: t(1),
                buffers: 3,
                elements: 64,
                prefetch_distance: 2,
            },
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite {
                pipeline: t(1),
                element: t(2),
            },
            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead {
                pipeline: t(1),
                element: t(2),
            },
            SemanticCompilerIntrinsicOperationV1::WorkgroupBarrier,
            SemanticCompilerIntrinsicOperationV1::WaveBarrier,
            SemanticCompilerIntrinsicOperationV1::FabsF32,
            SemanticCompilerIntrinsicOperationV1::ColdPath,
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: t(0),
                raw_index: t(1),
            },
            SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                index_witness: t(0),
                raw_index: t(1),
            },
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                disjoint_slice: t(0),
                index_witness: t(1),
                element: t(2),
                raw_index: t(3),
            },
            SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
                input_witness: t(0),
                output_witness: t(1),
                raw_index: t(2),
                index_space: index_spaces[0],
            },
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
                input_witness: t(0),
                output_witness: t(1),
                raw_index: t(2),
                input_space: index_spaces[0],
                output_space: index_spaces[1],
                offset: 3,
            },
            SemanticCompilerIntrinsicOperationV1::DisjointIndexGet {
                index_witness: t(0),
                raw_index: t(1),
                index_space: index_spaces[1],
            },
            SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
                input_witness: t(0),
                output_witness: t(1),
                raw_index: t(2),
                input_space: index_spaces[0],
                output_space: index_spaces[1],
                offset: 3,
            },
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                disjoint_slice: t(0),
                index_witness: t(1),
                element: t(2),
                raw_index: t(3),
                index_space: index_spaces[1],
            },
            SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader: t(0) },
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
                disjoint_slice: t(0),
                grid_leader: t(1),
                element: t(2),
                raw_index: t(3),
            },
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock {
                input_witness: t(0),
                output_block: t(1),
                raw_index: t(2),
                input_space: index_spaces[0],
                output_space: index_spaces[3],
                lanes_per_block: 16,
                elements_per_lane: 4,
            },
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
                disjoint_slice: t(0),
                block_witness: t(1),
                element: t(2),
                raw_index: t(3),
                index_space: index_spaces[3],
                lanes_per_block: 16,
                elements_per_lane: 4,
            },
            SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
                disjoint_slice: t(0),
                element: t(1),
                raw_index: t(2),
                index_space: index_spaces[1],
            },
            SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent { context: t(0) },
            SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent {
                lane: t(0),
                wave_width: 64,
            },
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor {
                result: t(0),
                view: t(1),
                error: t(2),
                role: SemanticMfmaOperandRoleV1::A,
                storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            },
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad {
                option_fragment: t(0),
                view: t(1),
                lane: t(2),
                fragment: t(3),
                contract: lhs,
                storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            },
            SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 {
                fragment: t(0),
                view: t(1),
                lane: t(2),
                contract: rhs,
                storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            },
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero {
                lane: t(0),
                fragment: t(1),
                contract: accumulator,
            },
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                fragment: t(0),
                values: t(1),
            },
            SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
                context: t(0),
                lhs_fragment: t(1),
                rhs_fragment: t(2),
                accumulator_fragment: t(3),
                lhs,
                rhs,
                accumulator,
            },
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedTiled2d {
                input_witness: t(0),
                output_tile: t(1),
                raw_index: t(2),
                input_space: index_spaces[0],
                output_space: index_spaces[4],
                lanes_per_tile: 64,
                tile_rows: 8,
                tile_columns: 8,
                elements_per_lane: 1,
            },
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
                disjoint_slice: t(0),
                tile_witness: t(1),
                element: t(2),
                raw_index: t(3),
                index_space: index_spaces[4],
                lanes_per_tile: 64,
                tile_rows: 8,
                tile_columns: 8,
                elements_per_lane: 1,
            },
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedRowStriped2d {
                input_witness: t(0),
                output_stripe: t(1),
                raw_index: t(2),
                input_space: index_spaces[0],
                output_space: index_spaces[5],
                lanes_per_row: 64,
                elements_per_lane: 64,
            },
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut {
                disjoint_slice: t(0),
                stripe_witness: t(1),
                element: t(2),
                raw_index: t(3),
                index_space: index_spaces[5],
                lanes_per_row: 64,
                elements_per_lane: 64,
            },
            SemanticCompilerIntrinsicOperationV1::CollectiveContextCurrent { context: t(0) },
            SemanticCompilerIntrinsicOperationV1::SubgroupReduceF32 {
                context: t(0),
                width: 64,
                kind: SemanticSubgroupReductionKindV1::Sum,
            },
            SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupContextCurrent { context: t(0) },
            SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupReduceF32 {
                context: t(0),
                width: 16,
                kind: SemanticSubgroupReductionKindV1::Maximum,
            },
            SemanticCompilerIntrinsicOperationV1::SubgroupBroadcastF32 {
                context: t(0),
                width: 16,
            },
            SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeCurrent {
                tile: t(1),
                lane: t(2),
                format: SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
            },
            SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeStage {
                input_tile: t(1),
                output_tile: t(2),
                view: t(3),
                format: SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
            },
            SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposePublish {
                input_tile: t(1),
                output_tile: t(2),
                format: SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
            },
            SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeRead {
                tile: t(1),
                fragment: t(2),
                contract: gfx950_rhs,
                format: SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
            },
            SemanticCompilerIntrinsicOperationV1::MathContextCurrent { context: t(0) },
        ];
        for kind in [
            SemanticSubgroupReductionKindV1::Sum,
            SemanticSubgroupReductionKindV1::Maximum,
        ] {
            operations.push(SemanticCompilerIntrinsicOperationV1::SubgroupReduceF32 {
                context: t(0),
                width: 64,
                kind,
            });
        }
        for event in [
            SemanticWorkgroupPipelineEventV1::Stage,
            SemanticWorkgroupPipelineEventV1::Commit,
            SemanticWorkgroupPipelineEventV1::Wait,
            SemanticWorkgroupPipelineEventV1::Consume,
            SemanticWorkgroupPipelineEventV1::Discard,
            SemanticWorkgroupPipelineEventV1::Release,
        ] {
            operations.push(
                SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineEvent {
                    pipeline: t(1),
                    event,
                },
            );
        }
        for function in [
            SemanticF32MathFunctionV1::Sqrt,
            SemanticF32MathFunctionV1::FusedMultiplyAdd,
            SemanticF32MathFunctionV1::Floor,
            SemanticF32MathFunctionV1::Ceil,
            SemanticF32MathFunctionV1::Truncate,
            SemanticF32MathFunctionV1::RoundTiesEven,
            SemanticF32MathFunctionV1::Sin,
            SemanticF32MathFunctionV1::Cos,
            SemanticF32MathFunctionV1::Exp,
            SemanticF32MathFunctionV1::Exp2,
            SemanticF32MathFunctionV1::Ln,
            SemanticF32MathFunctionV1::Log2,
            SemanticF32MathFunctionV1::Log10,
        ] {
            operations.push(SemanticCompilerIntrinsicOperationV1::MathF32 {
                context: t(0),
                function,
            });
        }
        for operation in operations {
            component_round_trip(
                operation,
                |writer, value| {
                    encode_compiler_intrinsic_operation(
                        writer,
                        *value,
                        SemanticMirWireVersionV1::V6,
                    )
                },
                |decoder| decoder.compiler_intrinsic(),
            );
        }
    }

    #[test]
    fn neutral_workgroup_reduce_retains_its_v9_encoding_under_v11_and_v12() {
        let operation = SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupReduceSum {
            context: SemanticTypeIdV1::from_index(0),
            dynamic_lds: SemanticTypeIdV1::from_index(1),
            element_storage: SemanticTypeIdV1::from_index(2),
            element: SemanticTypeIdV1::from_index(3),
        };
        let encoded = compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V9);
        assert_eq!(encoded[0], 62);
        assert_eq!(
            compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V11),
            encoded
        );
        assert_eq!(
            compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V12),
            encoded
        );

        for wire_version in [
            SemanticMirWireVersionV1::V6,
            SemanticMirWireVersionV1::V7,
            SemanticMirWireVersionV1::V8,
            SemanticMirWireVersionV1::V10,
        ] {
            let mut writer = CanonicalWriterV1::new(128);
            assert!(matches!(
                encode_compiler_intrinsic_operation(&mut writer, operation, wire_version),
                Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                    required: SemanticMirWireVersionV1::V9,
                    ..
                })
            ));
        }

        let mut decoder = CanonicalDecoderV1::new(&[63], SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V9;
        assert!(decoder.compiler_intrinsic().is_err());
    }

    #[test]
    fn neutral_workgroup_scan_kinds_retain_their_v10_encoding_under_v11_and_v12() {
        for (kind, tag) in [
            (SemanticWorkgroupScanKindV1::Inclusive, 0_u8),
            (SemanticWorkgroupScanKindV1::Exclusive, 1_u8),
        ] {
            let operation = SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupScanSum {
                context: SemanticTypeIdV1::from_index(0),
                dynamic_lds: SemanticTypeIdV1::from_index(1),
                element_storage: SemanticTypeIdV1::from_index(2),
                element: SemanticTypeIdV1::from_index(3),
                kind,
            };
            let encoded = compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V10);
            let mut frozen = vec![63];
            for index in 0_u32..4 {
                frozen.extend_from_slice(&index.to_le_bytes());
            }
            frozen.push(tag);
            assert_eq!(encoded, frozen);
            assert_eq!(
                compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V11),
                encoded,
            );
            assert_eq!(
                compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V12),
                encoded,
            );

            for wire_version in [
                SemanticMirWireVersionV1::V7,
                SemanticMirWireVersionV1::V8,
                SemanticMirWireVersionV1::V9,
            ] {
                let mut writer = CanonicalWriterV1::new(128);
                assert!(matches!(
                    encode_compiler_intrinsic_operation(&mut writer, operation, wire_version),
                    Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                        required: SemanticMirWireVersionV1::V10,
                        ..
                    })
                ));
            }
        }

        let mut malformed_kind = vec![63];
        for index in 0_u32..4 {
            malformed_kind.extend_from_slice(&index.to_le_bytes());
        }
        malformed_kind.push(2);
        for wire_version in [
            SemanticMirWireVersionV1::V10,
            SemanticMirWireVersionV1::V11,
            SemanticMirWireVersionV1::V12,
        ] {
            let mut decoder =
                CanonicalDecoderV1::new(&malformed_kind, SemanticMirLimitsV1::default());
            decoder.wire_version = wire_version;
            assert!(decoder.compiler_intrinsic().is_err());
        }

        let historical_volatile_tag_63 = [63, 7, 0, 0, 0];
        for wire_version in [
            SemanticMirWireVersionV1::V10,
            SemanticMirWireVersionV1::V11,
            SemanticMirWireVersionV1::V12,
        ] {
            let mut decoder = CanonicalDecoderV1::new(
                &historical_volatile_tag_63,
                SemanticMirLimitsV1::default(),
            );
            decoder.wire_version = wire_version;
            assert!(decoder.compiler_intrinsic().is_err());
        }

        let mut decoder = CanonicalDecoderV1::new(&[63], SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V9;
        assert!(decoder.compiler_intrinsic().is_err());
    }

    #[test]
    fn memory_volatile_load_round_trips_only_under_v12_tag_65() {
        let operation = SemanticCompilerIntrinsicOperationV1::MemoryVolatileLoad {
            element: SemanticTypeIdV1::from_index(7),
        };
        let encoded = compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V12);
        assert_eq!(encoded, [65, 7, 0, 0, 0]);

        for wire_version in [
            SemanticMirWireVersionV1::V9,
            SemanticMirWireVersionV1::V10,
            SemanticMirWireVersionV1::V11,
        ] {
            let mut writer = CanonicalWriterV1::new(128);
            assert_eq!(
                encode_compiler_intrinsic_operation(&mut writer, operation, wire_version),
                Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                    requested: wire_version,
                    required: SemanticMirWireVersionV1::V12,
                })
            );
            assert!(writer.finish().is_empty());
        }

        for wire_version in [SemanticMirWireVersionV1::V10, SemanticMirWireVersionV1::V11] {
            let mut legacy = CanonicalDecoderV1::new(&encoded, SemanticMirLimitsV1::default());
            legacy.wire_version = wire_version;
            assert!(matches!(
                legacy.compiler_intrinsic(),
                Err(SemanticMirDecodeErrorV1::InvalidTag {
                    context: "compiler intrinsic",
                    value: 65,
                    ..
                })
            ));
        }

        let mut v12_bad_tag = CanonicalDecoderV1::new(&[66], SemanticMirLimitsV1::default());
        v12_bad_tag.wire_version = SemanticMirWireVersionV1::V12;
        assert!(matches!(
            v12_bad_tag.compiler_intrinsic(),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "compiler intrinsic",
                value: 66,
                ..
            })
        ));

        let mut truncated = CanonicalDecoderV1::new(&[65], SemanticMirLimitsV1::default());
        truncated.wire_version = SemanticMirWireVersionV1::V12;
        assert!(matches!(
            truncated.compiler_intrinsic(),
            Err(SemanticMirDecodeErrorV1::UnexpectedEnd { .. })
        ));

        let mut trailing = encoded.to_vec();
        trailing.push(0);
        let mut v12 = CanonicalDecoderV1::new(&trailing, SemanticMirLimitsV1::default());
        v12.wire_version = SemanticMirWireVersionV1::V12;
        assert_eq!(v12.compiler_intrinsic().unwrap(), operation);
        assert!(matches!(
            v12.finish(),
            Err(SemanticMirDecodeErrorV1::TrailingBytes { trailing: 1, .. })
        ));
    }

    #[test]
    fn trap_round_trips_only_at_its_unique_v11_tag() {
        let operation = SemanticCompilerIntrinsicOperationV1::Trap;
        let encoded = compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V11);
        assert_eq!(encoded, [64]);

        for wire_version in [
            SemanticMirWireVersionV1::V7,
            SemanticMirWireVersionV1::V8,
            SemanticMirWireVersionV1::V9,
            SemanticMirWireVersionV1::V10,
        ] {
            let mut writer = CanonicalWriterV1::new(1);
            assert!(matches!(
                encode_compiler_intrinsic_operation(&mut writer, operation, wire_version),
                Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                    required: SemanticMirWireVersionV1::V11,
                    ..
                })
            ));

            let mut decoder = CanonicalDecoderV1::new(&[64], SemanticMirLimitsV1::default());
            decoder.wire_version = wire_version;
            assert!(decoder.compiler_intrinsic().is_err());
        }
        assert_eq!(
            compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V12),
            encoded
        );
    }

    #[test]
    fn non_scan_trap_selects_and_round_trips_current_production_v11() {
        let request = non_scan_trap_request();
        assert_eq!(
            minimum_wire_version(&request),
            SemanticMirWireVersionV1::V11
        );
        let admitted = request
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
        assert_eq!(admitted.wire_version(), SemanticMirWireVersionV1::V11);
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v10_canonical(
                admitted.canonical_encoding(),
                SemanticMirLimitsV1::default(),
            ),
            Err(SemanticMirDecodeErrorV1::WireVersionMismatch {
                expected: SemanticMirWireVersionV1::V10,
                actual: SemanticMirWireVersionV1::V11,
            })
        ));
        let exact = AdmittedInertSemanticMirV1::decode_exact_v11_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(exact.canonical_encoding(), admitted.canonical_encoding());
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
        assert!(decoded.callables().iter().any(|callable| matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Trap,
                ..
            }
        )));
    }

    #[test]
    fn every_bf16_conversion_variant_round_trips_only_under_v8() {
        for kind in [
            SemanticBf16ConversionKindV1::FromBits,
            SemanticBf16ConversionKindV1::ToBits,
            SemanticBf16ConversionKindV1::FromF32RoundTiesEven,
            SemanticBf16ConversionKindV1::ToF32,
        ] {
            let encoded = compiler_intrinsic_round_trip(
                SemanticCompilerIntrinsicOperationV1::Bf16Conversion {
                    kind,
                    input: SemanticTypeIdV1::from_index(0),
                    output: SemanticTypeIdV1::from_index(1),
                },
                SemanticMirWireVersionV1::V8,
            );
            assert_eq!(encoded[0], 55);
        }
    }

    #[test]
    fn literal_v6_v7_pipeline_and_v8_bf16_bytes_remain_canonical() {
        let pipeline = SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
            scope: SemanticTypeIdV1::from_index(0),
            pipeline: SemanticTypeIdV1::from_index(1),
            buffers: 3,
            elements: 64,
            prefetch_distance: 2,
        };
        let pipeline_bytes = [
            55, 0, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0,
        ];
        for wire_version in [SemanticMirWireVersionV1::V6, SemanticMirWireVersionV1::V7] {
            let mut decoder =
                CanonicalDecoderV1::new(&pipeline_bytes, SemanticMirLimitsV1::default());
            decoder.wire_version = wire_version;
            assert_eq!(decoder.compiler_intrinsic().unwrap(), pipeline);
            decoder.finish().unwrap();
            assert_eq!(
                compiler_intrinsic_round_trip(pipeline, wire_version),
                pipeline_bytes
            );
        }

        let bf16 = SemanticCompilerIntrinsicOperationV1::Bf16Conversion {
            kind: SemanticBf16ConversionKindV1::FromBits,
            input: SemanticTypeIdV1::from_index(0),
            output: SemanticTypeIdV1::from_index(1),
        };
        let bf16_v8_bytes = [55, 0, 0, 0, 0, 0, 1, 0, 0, 0];
        let mut decoder = CanonicalDecoderV1::new(&bf16_v8_bytes, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V8;
        assert_eq!(decoder.compiler_intrinsic().unwrap(), bf16);
        decoder.finish().unwrap();
        assert_eq!(
            compiler_intrinsic_round_trip(bf16, SemanticMirWireVersionV1::V8),
            bf16_v8_bytes
        );

        let bf16_v9 = compiler_intrinsic_round_trip(bf16, SemanticMirWireVersionV1::V9);
        assert_eq!(bf16_v9, [59, 0, 0, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(
            compiler_intrinsic_round_trip(pipeline, SemanticMirWireVersionV1::V9),
            pipeline_bytes
        );

        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert_eq!(
            encode_compiler_intrinsic_operation(
                &mut writer,
                pipeline,
                SemanticMirWireVersionV1::V8,
            ),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: SemanticMirWireVersionV1::V8,
                required: SemanticMirWireVersionV1::V9,
            })
        );
        assert!(writer.finish().is_empty());
    }

    #[test]
    fn minimum_wire_version_is_feature_compositional_at_the_v8_collision() {
        let pipeline = SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate {
            scope: SemanticTypeIdV1::from_index(0),
            pipeline: SemanticTypeIdV1::from_index(1),
            buffers: 3,
            elements: 64,
            prefetch_distance: 2,
        };
        let bf16 = SemanticCompilerIntrinsicOperationV1::Bf16Conversion {
            kind: SemanticBf16ConversionKindV1::FromBits,
            input: SemanticTypeIdV1::from_index(0),
            output: SemanticTypeIdV1::from_index(1),
        };
        assert_eq!(
            minimum_wire_version(&version_selection_request([pipeline])),
            SemanticMirWireVersionV1::V6
        );
        let mut resource_pipeline = version_selection_request([pipeline]);
        let resources = SemanticKernelResourceContractV1::new(256, 0).unwrap();
        let contract =
            SemanticKernelSourceContractV1::new_with_resources(None, Some(resources), None, None)
                .unwrap();
        let entry = SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"resource-pipeline".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1(identity(119)),
            contract,
        );
        resource_pipeline.functions[0] = resource_pipeline.functions[0]
            .clone()
            .with_kernel_entry(entry);
        assert_eq!(
            minimum_wire_version(&resource_pipeline),
            SemanticMirWireVersionV1::V7
        );
        assert_eq!(
            minimum_wire_version(&version_selection_request([bf16])),
            SemanticMirWireVersionV1::V8
        );
        assert_eq!(
            minimum_wire_version(&version_selection_request([pipeline, bf16])),
            SemanticMirWireVersionV1::V9
        );
    }

    #[test]
    fn v10_scan_and_v12_volatile_custody_is_compositional_and_exact() {
        let scan = SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupScanSum {
            context: SemanticTypeIdV1::from_index(0),
            dynamic_lds: SemanticTypeIdV1::from_index(1),
            element_storage: SemanticTypeIdV1::from_index(2),
            element: SemanticTypeIdV1::from_index(3),
            kind: SemanticWorkgroupScanKindV1::Inclusive,
        };
        let volatile = SemanticCompilerIntrinsicOperationV1::MemoryVolatileLoad {
            element: SemanticTypeIdV1::from_index(4),
        };
        let trap = SemanticCompilerIntrinsicOperationV1::Trap;
        let reduce = SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupReduceSum {
            context: SemanticTypeIdV1::from_index(0),
            dynamic_lds: SemanticTypeIdV1::from_index(1),
            element_storage: SemanticTypeIdV1::from_index(2),
            element: SemanticTypeIdV1::from_index(3),
        };
        let request = version_selection_request([scan, volatile]);
        assert_eq!(
            minimum_wire_version(&request),
            SemanticMirWireVersionV1::V12
        );
        assert_eq!(
            minimum_wire_version(&version_selection_request([scan, trap])),
            SemanticMirWireVersionV1::V11
        );
        for request in [
            version_selection_request([scan, reduce]),
            version_selection_request([reduce, scan]),
        ] {
            assert_eq!(
                minimum_wire_version(&request),
                SemanticMirWireVersionV1::V11
            );
        }
        for request in [
            version_selection_request([trap, volatile]),
            version_selection_request([volatile, trap]),
            version_selection_request([reduce, volatile]),
        ] {
            assert_eq!(
                minimum_wire_version(&request),
                SemanticMirWireVersionV1::V12
            );
        }

        let mut writer = CanonicalWriterV1::new(128);
        encode_compiler_intrinsic_operation(&mut writer, scan, SemanticMirWireVersionV1::V12)
            .unwrap();
        encode_compiler_intrinsic_operation(&mut writer, volatile, SemanticMirWireVersionV1::V12)
            .unwrap();
        let encoded = writer.finish();
        let mut decoder = CanonicalDecoderV1::new(&encoded, SemanticMirLimitsV1::default());
        decoder.wire_version = SemanticMirWireVersionV1::V12;
        assert_eq!(decoder.compiler_intrinsic().unwrap(), scan);
        assert_eq!(decoder.compiler_intrinsic().unwrap(), volatile);
        decoder.finish().unwrap();

        let limits = SemanticMirLimitsV1::default();
        let v11 = minimal_request().admit_exact_v11(limits).unwrap();
        let v12 = minimal_request().admit_exact_v12(limits).unwrap();
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v12_canonical(
                v11.canonical_encoding(),
                limits,
            ),
            Err(SemanticMirDecodeErrorV1::WireVersionMismatch {
                expected: SemanticMirWireVersionV1::V12,
                actual: SemanticMirWireVersionV1::V11,
            })
        ));
        assert!(matches!(
            AdmittedInertSemanticMirV1::decode_exact_v11_canonical(
                v12.canonical_encoding(),
                limits,
            ),
            Err(SemanticMirDecodeErrorV1::WireVersionMismatch {
                expected: SemanticMirWireVersionV1::V11,
                actual: SemanticMirWireVersionV1::V12,
            })
        ));
    }

    #[test]
    fn every_type_shape_enum_encoding_and_abi_pass_mode_round_trips_exactly() {
        let request = minimal_request();
        let layout = request.types[0].layout.clone();
        let t = |index| SemanticTypeIdV1::from_index(index);
        let empty = || SemanticAggregateTypeV1::new(vec![]).unwrap();
        let shapes = vec![
            SemanticTypeShapeV1::Unit,
            SemanticTypeShapeV1::Never,
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
            SemanticTypeShapeV1::ValidityScalar(
                SemanticValidityScalarTypeV1::new(
                    SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 8,
                    },
                    vec![SemanticScalarValidityRangeV1::new(1, u128::from(u8::MAX))],
                )
                .unwrap(),
            ),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    t(0),
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    t(0),
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Immutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::SliceLength,
                )
                .unwrap(),
            ),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    t(0),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::VTable,
                )
                .unwrap(),
            ),
            SemanticTypeShapeV1::Array {
                element: t(0),
                length: 4,
            },
            SemanticTypeShapeV1::Tuple(empty()),
            SemanticTypeShapeV1::Aggregate(empty()),
            SemanticTypeShapeV1::Enum {
                discriminant: t(0),
                variants: vec![SemanticEnumVariantV1::new_with_inhabitedness(
                    7,
                    empty(),
                    true,
                )]
                .into_boxed_slice(),
            },
            SemanticTypeShapeV1::FunctionPointer {
                safety: SemanticFunctionSafetyV1::Safe,
                extern_abi: SemanticExternAbiV1::Rust,
                c_variadic: false,
                arguments: empty(),
                return_type: t(0),
            },
            SemanticTypeShapeV1::FunctionPointer {
                safety: SemanticFunctionSafetyV1::Unsafe,
                extern_abi: SemanticExternAbiV1::C { unwind: true },
                c_variadic: true,
                arguments: SemanticAggregateTypeV1::new(vec![t(0)]).unwrap(),
                return_type: t(0),
            },
            SemanticTypeShapeV1::Opaque,
            SemanticTypeShapeV1::Union(empty()),
            SemanticTypeShapeV1::Slice { element: t(0) },
        ];
        assert_eq!(shapes.len(), 16);
        for (index, shape) in shapes.into_iter().enumerate() {
            let declaration = SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1(identity(u8::try_from(index + 20).unwrap())),
                SemanticLayoutIdentityV1(identity(u8::try_from(index + 40).unwrap())),
                layout.clone(),
                shape,
            );
            component_round_trip(declaration, encode_type, |decoder| decoder.ty());
        }

        let integer = SemanticBackendPrimitiveV1::integer(false, 8, 1);
        let scalar =
            SemanticBackendScalarV1::initialized(integer, SemanticScalarValidityRangeV1::new(0, 1));
        let direct =
            SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, scalar));
        let source = SemanticNicheSourceV1::new(
            vec![
                SemanticNichePathComponentV1::Field(0),
                SemanticNichePathComponentV1::ArrayElement(1),
            ],
            0,
        )
        .unwrap();
        let niche = SemanticLayoutNicheV1::new(
            0,
            integer,
            SemanticScalarValidityRangeV1::new(1, u128::from(u8::MAX)),
        )
        .unwrap();
        let niche_encoding = SemanticEnumEncodingV1::Niche(
            SemanticNicheEnumEncodingV1::new(0, source, niche, scalar, 0, 1, 1, 1).unwrap(),
        );
        for encoding in [direct.clone(), niche_encoding] {
            component_round_trip(encoding, encode_enum_encoding, |decoder| {
                decoder.enum_encoding()
            });
        }

        let variant = SemanticEnumVariantLayoutV1::from_rustc(
            0,
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap();
        for variants in [
            SemanticRustcVariantsV1::Empty,
            SemanticRustcVariantsV1::Single { index: 3 },
            SemanticRustcVariantsV1::Multiple(Box::new(
                SemanticEnumLayoutV1::new(vec![variant], direct).unwrap(),
            )),
        ] {
            component_round_trip(variants, encode_rustc_variants, |decoder| {
                decoder.rustc_variants()
            });
        }

        for capture in [
            None,
            Some(SemanticAbiPointerCaptureV1::CapturesNone),
            Some(SemanticAbiPointerCaptureV1::CapturesAddress),
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
        ] {
            for extension in [
                SemanticAbiExtensionV1::None,
                SemanticAbiExtensionV1::ZeroExtend,
                SemanticAbiExtensionV1::SignExtend,
            ] {
                let attributes = SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(true, capture, true, true, true, true),
                    extension,
                    8,
                    Some(8),
                )
                .unwrap();
                component_round_trip(
                    attributes,
                    |writer, value| encode_abi_attributes(writer, *value),
                    |decoder| decoder.abi_attributes(),
                );
            }
        }

        for (kind, size_bytes) in [
            (SemanticAbiRegisterKindV1::Integer, 4),
            (SemanticAbiRegisterKindV1::Float, 8),
            (SemanticAbiRegisterKindV1::Vector, 16),
        ] {
            let register = SemanticAbiRegisterV1::new(kind, size_bytes).unwrap();
            component_round_trip(
                register,
                |writer, value| encode_abi_register(writer, *value),
                |decoder| decoder.abi_register(),
            );
        }

        let attributes = SemanticAbiValueAttributesV1::plain();
        let register = SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 4).unwrap();
        let modes = vec![
            SemanticAbiPassModeV1::Ignore,
            SemanticAbiPassModeV1::Direct(attributes),
            SemanticAbiPassModeV1::Pair {
                first: attributes,
                second: attributes,
            },
            SemanticAbiPassModeV1::cast(
                true,
                SemanticAbiCastV1::new(
                    [Some(register), None, None, None, None, None, None, None],
                    Some(4),
                    SemanticAbiUniformV1::from_rustc(register, 8, true).unwrap(),
                    attributes,
                ),
            ),
            SemanticAbiPassModeV1::Indirect {
                attributes,
                metadata_attributes: Some(attributes),
                on_stack: true,
            },
        ];
        assert_eq!(modes.len(), 5);
        for (index, mode) in modes.into_iter().enumerate() {
            let mut value = if index == 3 {
                SemanticAbiValueV1::new_with_adjusted_type(
                    t(0),
                    SemanticAbiAdjustedTypeV1::new(
                        t(0),
                        SemanticLayoutIdentityV1(identity(80)),
                        layout.clone(),
                    ),
                    mode,
                )
            } else {
                SemanticAbiValueV1::new(t(0), mode)
            };
            if index == 4 {
                value = value.with_pointee_override(
                    SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap(),
                );
            }
            component_round_trip(value, encode_abi_value, |decoder| decoder.abi_value());
        }

        let ignored = SemanticAbiValueV1::new(t(0), SemanticAbiPassModeV1::Ignore);
        let rust_call = SemanticFunctionAbiV1::from_rustc_with_source_signature(
            SemanticAbiIdentityV1(identity(120)),
            SemanticLayoutIdentityV1(identity(121)),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::RustCall,
            false,
            false,
            0,
            vec![t(0)],
            t(0),
            vec![SemanticAbiArgumentV1::rust_call_tuple_field(
                0,
                ignored.clone(),
            )],
            ignored.clone(),
        )
        .unwrap();
        let hidden = SemanticFunctionAbiV1::from_rustc_with_source_signature(
            SemanticAbiIdentityV1(identity(122)),
            SemanticLayoutIdentityV1(identity(123)),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            vec![],
            t(0),
            vec![SemanticAbiArgumentV1::hidden(
                SemanticAbiHiddenArgumentRoleV1::CallerLocation,
                ignored.clone(),
            )],
            ignored,
        )
        .unwrap();
        for abi in [rust_call, hidden] {
            component_round_trip(
                abi,
                |writer, abi| encode_abi(writer, abi, SemanticMirWireVersionV1::V4),
                |decoder| decoder.abi(),
            );
        }
    }

    #[test]
    fn every_allocation_static_vtable_callable_and_kernel_contract_variant_round_trips() {
        let relocation_targets = [
            SemanticRelocationTargetV1::Allocation(SemanticAllocationIdV1::from_index(0)),
            SemanticRelocationTargetV1::Callable(SemanticCallableIdV1::from_index(0)),
            SemanticRelocationTargetV1::Static(SemanticStaticIdV1::from_index(0)),
            SemanticRelocationTargetV1::VTable(SemanticVTableIdV1::from_index(0)),
        ];
        let allocation = SemanticAllocationDeclV1::new_in_address_space(
            SemanticAllocationIdentityV1(identity(90)),
            1,
            vec![0; 4],
            vec![0x0f],
            4,
            true,
            relocation_targets
                .into_iter()
                .enumerate()
                .map(|(offset, target)| {
                    SemanticRelocationV1::new_in_address_space(
                        u64::try_from(offset).unwrap(),
                        1,
                        1,
                        0,
                        target,
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        component_round_trip(allocation, encode_allocation, |decoder| {
            decoder.allocation()
        });

        for declaration in [
            SemanticStaticDeclV1::new(
                SemanticStaticIdentityV1(identity(91)),
                SemanticSourceProvenanceV1::unavailable(),
                SemanticTypeIdV1::from_index(0),
                false,
                1,
                SemanticStaticDefinitionV1::Defined {
                    initializer: SemanticAllocationIdV1::from_index(0),
                },
            )
            .with_export_symbol(SemanticLinkSymbolV1::new(b"defined".to_vec()).unwrap()),
            SemanticStaticDeclV1::new(
                SemanticStaticIdentityV1(identity(92)),
                SemanticSourceProvenanceV1::unavailable(),
                SemanticTypeIdV1::from_index(0),
                false,
                1,
                SemanticStaticDefinitionV1::ExternalRequired {
                    symbol: SemanticLinkSymbolV1::new(b"external".to_vec()).unwrap(),
                },
            ),
        ] {
            component_round_trip(declaration, encode_static, |decoder| decoder.static_decl());
        }

        let predicates = vec![SemanticDynPredicateIdentityV1(identity(93))];
        let trait_identity = SemanticVTableTraitIdentityV1::new(
            SemanticTraitRefIdentityV1(identity(94)),
            predicates,
        )
        .unwrap();
        let vtable = SemanticVTableDeclV1::new_with_trait_identity_and_slots(
            SemanticVTableIdentityV1(identity(95)),
            SemanticTypeIdV1::from_index(0),
            SemanticTypeIdV1::from_index(0),
            trait_identity,
            SemanticVTableHeaderV1::new(Some(SemanticFunctionIdV1::from_index(0)), 8, 8).unwrap(),
            vec![
                SemanticVTableSlotV1::Vacant,
                SemanticVTableSlotV1::Method(SemanticFunctionIdV1::from_index(0)),
                SemanticVTableSlotV1::TraitVPtr {
                    trait_ref: SemanticTraitRefIdentityV1(identity(96)),
                    target: SemanticVTableIdV1::from_index(0),
                },
            ],
            SemanticAllocationIdV1::from_index(0),
        )
        .unwrap();
        component_round_trip(vtable, encode_vtable, |decoder| decoder.vtable());

        let request = minimal_request();
        let abi = request.functions[0].abi.clone();
        let binding = || {
            SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1(identity(100)),
                SemanticItemDefinitionIdentityV1(identity(101)),
                SemanticMonomorphizationIdentityV1(identity(102)),
                SemanticGenericTypeArgumentsIdentityV1(identity(103)),
                SemanticConstGenericArgumentsIdentityV1(identity(104)),
                SemanticSourceProvenanceV1::unavailable(),
                abi.clone(),
            )
        };
        let callables = vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::DeviceFfiImport {
                binding: binding(),
                contract: SemanticDeviceFfiImportContractV1::new(
                    SemanticDeviceFfiContractIdentityV1(identity(105)),
                    SemanticLinkSymbolV1::new(b"ffi".to_vec()).unwrap(),
                    SemanticDeviceFfiTargetV1::AmdGpuGfx942XnackMinus,
                    SemanticCodeObjectVersionV1::V6,
                    SemanticDeviceFfiPhysicalAbiIdentityV1(identity(106)),
                    SemanticDeviceFfiEffectsV1::new(
                        SemanticDeviceFfiEffectsV1::READ_GLOBAL
                            | SemanticDeviceFfiEffectsV1::WRITE_GLOBAL
                            | SemanticDeviceFfiEffectsV1::READ_WORKGROUP
                            | SemanticDeviceFfiEffectsV1::WRITE_WORKGROUP
                            | SemanticDeviceFfiEffectsV1::ATOMIC
                            | SemanticDeviceFfiEffectsV1::CONTROL_FLOW,
                    )
                    .unwrap(),
                    SemanticDeviceFfiSemanticIdentityV1(identity(107)),
                ),
            },
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding: binding(),
                operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex(SemanticAxisV1::X),
                operation_identity: SemanticCompilerIntrinsicIdentityV1(identity(108)),
            },
        ];
        for callable in callables {
            component_round_trip(
                callable,
                |writer, callable| encode_callable(writer, callable, SemanticMirWireVersionV1::V4),
                |decoder| decoder.callable(),
            );
        }

        let launch = SemanticKernelLaunchBoundsV1::new(
            Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
            Some(SemanticWorkgroupDimensionsV1::new([128, 1, 1]).unwrap()),
            Some(1),
        )
        .unwrap();
        let unsafe_assembly = SemanticUnsafeAssemblyDeclarationV1::new(
            SemanticUnsafeAssemblyTargetV1::AmdGpuGfx942,
            1,
            SemanticUnsafeAssemblyDeclarationV1::OPTION_NOMEM,
            0,
        )
        .unwrap();
        let reachable_assembly = SemanticReachableAssemblyV1::new(
            1,
            unsafe_assembly.operand_bits(),
            unsafe_assembly.option_bits(),
            unsafe_assembly.effect_bits(),
        )
        .unwrap();
        let entry = SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"kernel".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1(identity(109)),
            SemanticKernelSourceContractV1::new(
                Some(launch),
                Some(unsafe_assembly),
                Some(reachable_assembly),
            )
            .unwrap(),
        );
        component_round_trip(
            SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(b"kernel-empty-contract".to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1(identity(112)),
                SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
            ),
            |writer, entry| encode_kernel_entry(writer, entry, SemanticMirWireVersionV1::V6),
            |decoder| decoder.kernel_entry(),
        );
        component_round_trip(
            entry.clone(),
            |writer, entry| encode_kernel_entry(writer, entry, SemanticMirWireVersionV1::V6),
            |decoder| decoder.kernel_entry(),
        );

        let source_origin = SemanticSourceOriginV1::new(
            SemanticSourceFileIdentityV1(identity(110)),
            1,
            3,
            1,
            1,
            1,
            3,
        )
        .unwrap();
        for source in [
            SemanticSourceProvenanceV1::unavailable(),
            SemanticSourceProvenanceV1::new(Some(source_origin), None),
            SemanticSourceProvenanceV1::new(None, Some(source_origin)),
            SemanticSourceProvenanceV1::new(Some(source_origin), Some(source_origin)),
        ] {
            component_round_trip(
                source,
                |writer, value| encode_source(writer, *value),
                |decoder| decoder.source(),
            );
        }

        let mut base = request.functions[0].clone();
        base.locals = [
            base.locals[0].clone(),
            base.locals[1].clone(),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1(identity(111)),
                SemanticTypeIdV1::from_index(0),
                SemanticLocalRoleV1::Temporary,
                SemanticSourceProvenanceV1::new(Some(source_origin), Some(source_origin)),
            ),
        ]
        .into();
        base.source = SemanticSourceProvenanceV1::new(Some(source_origin), None);
        let functions = vec![
            base.clone().with_role(SemanticFunctionRoleV1::KernelRoot),
            base.clone()
                .with_role(SemanticFunctionRoleV1::KernelRoot)
                .with_kernel_entry(entry),
            base.clone()
                .with_role(SemanticFunctionRoleV1::InternalHelper),
            base.clone()
                .with_role(SemanticFunctionRoleV1::DeviceFfiExport)
                .with_device_ffi_export_symbol(
                    SemanticLinkSymbolV1::new(b"device-export".to_vec()).unwrap(),
                ),
            base.with_role(SemanticFunctionRoleV1::DropGlue(
                SemanticTypeIdV1::from_index(0),
            )),
        ];
        for function in functions {
            component_round_trip(
                function,
                |writer, function| encode_function(writer, function, SemanticMirWireVersionV1::V4),
                |decoder| decoder.function(),
            );
        }
    }

    #[test]
    fn every_place_operand_constant_rvalue_and_statement_variant_round_trips_exactly() {
        let t = SemanticTypeIdV1::from_index(0);
        let local = SemanticLocalIdV1::from_index(0);
        let base_place = || SemanticPlaceV1::new(local, vec![], t).unwrap();
        let projection_kinds = [
            SemanticProjectionKindV1::Dereference,
            SemanticProjectionKindV1::Field(0),
            SemanticProjectionKindV1::Index(local),
            SemanticProjectionKindV1::ConstantIndex {
                offset: 0,
                minimum_length: 1,
                from_end: false,
            },
            SemanticProjectionKindV1::Subslice {
                from: 0,
                to: 1,
                from_end: false,
            },
            SemanticProjectionKindV1::Downcast(0),
            SemanticProjectionKindV1::OpaqueCast,
            SemanticProjectionKindV1::Subtype,
        ];
        assert_eq!(projection_kinds.len(), 8);
        for kind in projection_kinds {
            let place =
                SemanticPlaceV1::new(local, vec![SemanticProjectionV1::new(kind, t).unwrap()], t)
                    .unwrap();
            component_round_trip(place, encode_place, |decoder| decoder.place());
        }

        for provenance in [
            SemanticPointerProvenanceV1::Allocation(SemanticAllocationIdV1::from_index(0)),
            SemanticPointerProvenanceV1::Callable(SemanticCallableIdV1::from_index(0)),
            SemanticPointerProvenanceV1::Static(SemanticStaticIdV1::from_index(0)),
            SemanticPointerProvenanceV1::ExposedAddress,
        ] {
            component_round_trip(
                provenance,
                |writer, value| encode_pointer_provenance(writer, *value),
                |decoder| decoder.pointer_provenance(),
            );
        }

        let constants = vec![
            SemanticConstantV1::new(t, SemanticConstantValueV1::ZeroSized),
            SemanticConstantV1::new(
                t,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 1).unwrap()),
            ),
            SemanticConstantV1::new(
                t,
                SemanticConstantValueV1::Bytes(
                    SemanticConstantBytesV1::new(vec![1, 2, 3]).unwrap(),
                ),
            ),
            SemanticConstantV1::new(
                t,
                SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new_with_metadata(
                    4,
                    SemanticPointerProvenanceV1::Allocation(SemanticAllocationIdV1::from_index(0)),
                    SemanticPointerValueMetadataV1::None,
                )),
            ),
            SemanticConstantV1::new(
                t,
                SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new_with_metadata(
                    4,
                    SemanticPointerProvenanceV1::Static(SemanticStaticIdV1::from_index(0)),
                    SemanticPointerValueMetadataV1::SliceLength(9),
                )),
            ),
            SemanticConstantV1::new(
                t,
                SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new_with_metadata(
                    4,
                    SemanticPointerProvenanceV1::Callable(SemanticCallableIdV1::from_index(0)),
                    SemanticPointerValueMetadataV1::VTable(SemanticVTableIdV1::from_index(0)),
                )),
            ),
            SemanticConstantV1::new(
                t,
                SemanticConstantValueV1::Callable(SemanticCallableIdV1::from_index(0)),
            ),
        ];
        for constant in constants {
            component_round_trip(constant, encode_constant, |decoder| decoder.constant());
        }

        for operand in [
            SemanticOperandV1::Copy(base_place()),
            SemanticOperandV1::Move(base_place()),
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                t,
                SemanticConstantValueV1::ZeroSized,
            )),
        ] {
            component_round_trip(operand, encode_operand, |decoder| decoder.operand());
        }

        let operand = || {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                t,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
            ))
        };
        let mut rvalues = vec![SemanticRvalueKindV1::Use(operand())];
        for operation in [
            SemanticUnaryOpV1::Not,
            SemanticUnaryOpV1::Negate,
            SemanticUnaryOpV1::PointerMetadata,
        ] {
            rvalues.push(SemanticRvalueKindV1::Unary {
                operation,
                operand: operand(),
            });
        }
        for operation in [
            SemanticBinaryOpV1::Add,
            SemanticBinaryOpV1::Subtract,
            SemanticBinaryOpV1::Multiply,
            SemanticBinaryOpV1::Divide,
            SemanticBinaryOpV1::Remainder,
            SemanticBinaryOpV1::BitXor,
            SemanticBinaryOpV1::BitAnd,
            SemanticBinaryOpV1::BitOr,
            SemanticBinaryOpV1::ShiftLeft,
            SemanticBinaryOpV1::ShiftRight,
            SemanticBinaryOpV1::Equal,
            SemanticBinaryOpV1::LessThan,
            SemanticBinaryOpV1::LessOrEqual,
            SemanticBinaryOpV1::NotEqual,
            SemanticBinaryOpV1::GreaterOrEqual,
            SemanticBinaryOpV1::GreaterThan,
            SemanticBinaryOpV1::Offset,
        ] {
            rvalues.push(SemanticRvalueKindV1::Binary {
                operation,
                left: operand(),
                right: operand(),
            });
        }
        for operation in [
            SemanticCheckedBinaryOpV1::Add,
            SemanticCheckedBinaryOpV1::Subtract,
            SemanticCheckedBinaryOpV1::Multiply,
        ] {
            rvalues.push(SemanticRvalueKindV1::CheckedBinary(
                SemanticCheckedBinaryRvalueV1::new(operation, operand(), operand()),
            ));
        }
        for kind in [
            SemanticCastKindV1::Integer,
            SemanticCastKindV1::Float,
            SemanticCastKindV1::Pointer,
            SemanticCastKindV1::PointerExposeProvenance,
            SemanticCastKindV1::PointerWithExposedProvenance,
            SemanticCastKindV1::Transmute,
        ] {
            rvalues.push(SemanticRvalueKindV1::Cast {
                kind,
                operand: operand(),
            });
        }
        for kind in [
            SemanticBorrowKindV1::Shared,
            SemanticBorrowKindV1::Mutable,
            SemanticBorrowKindV1::Fake,
        ] {
            rvalues.push(SemanticRvalueKindV1::Borrow {
                kind,
                place: base_place(),
            });
        }
        rvalues.extend([
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: base_place(),
            },
            SemanticRvalueKindV1::Length(base_place()),
            SemanticRvalueKindV1::Discriminant(base_place()),
        ]);
        for kind in [
            SemanticAggregateKindV1::Array,
            SemanticAggregateKindV1::Tuple,
            SemanticAggregateKindV1::Aggregate,
            SemanticAggregateKindV1::EnumVariant(2),
        ] {
            rvalues.push(SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(kind, vec![operand()]).unwrap(),
            ));
        }
        for (volatility, atomic) in [
            (SemanticVolatilityV1::NonVolatile, None),
            (
                SemanticVolatilityV1::Volatile,
                Some(SemanticAtomicAccessV1::new(
                    SemanticAtomicOrderingV1::Acquire,
                    SemanticAtomicScopeV1::System,
                )),
            ),
        ] {
            rvalues.push(SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                base_place(),
                volatility,
                atomic,
            )));
        }
        for kind in rvalues {
            component_round_trip(SemanticRvalueV1::new(t, kind), encode_rvalue, |decoder| {
                decoder.rvalue()
            });
        }

        for ordering in [
            SemanticAtomicOrderingV1::Relaxed,
            SemanticAtomicOrderingV1::Release,
            SemanticAtomicOrderingV1::Acquire,
            SemanticAtomicOrderingV1::AcquireRelease,
            SemanticAtomicOrderingV1::SequentiallyConsistent,
        ] {
            for scope in [
                SemanticAtomicScopeV1::SingleThread,
                SemanticAtomicScopeV1::Workgroup,
                SemanticAtomicScopeV1::Agent,
                SemanticAtomicScopeV1::Device,
                SemanticAtomicScopeV1::System,
            ] {
                component_round_trip(
                    SemanticAtomicAccessV1::new(ordering, scope),
                    |writer, value| encode_atomic_access(writer, *value),
                    |decoder| decoder.atomic_access(),
                );
            }
        }

        let access = SemanticAtomicAccessV1::new(
            SemanticAtomicOrderingV1::SequentiallyConsistent,
            SemanticAtomicScopeV1::System,
        );
        let mut statements = vec![
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                base_place(),
                SemanticRvalueV1::new(t, SemanticRvalueKindV1::Use(operand())),
            )),
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                base_place(),
                operand(),
                SemanticVolatilityV1::Volatile,
                Some(access),
            )),
        ];
        for operation in [
            SemanticAtomicRmwOpV1::Exchange,
            SemanticAtomicRmwOpV1::Add,
            SemanticAtomicRmwOpV1::Subtract,
            SemanticAtomicRmwOpV1::BitAnd,
            SemanticAtomicRmwOpV1::BitNand,
            SemanticAtomicRmwOpV1::BitOr,
            SemanticAtomicRmwOpV1::BitXor,
            SemanticAtomicRmwOpV1::SignedMaximum,
            SemanticAtomicRmwOpV1::SignedMinimum,
            SemanticAtomicRmwOpV1::UnsignedMaximum,
            SemanticAtomicRmwOpV1::UnsignedMinimum,
        ] {
            statements.push(SemanticStatementKindV1::AtomicRmw(
                SemanticAtomicRmwV1::new(base_place(), base_place(), operand(), operation, access),
            ));
        }
        statements.extend([
            SemanticStatementKindV1::AtomicCompareExchange(SemanticAtomicCompareExchangeV1::new(
                base_place(),
                base_place(),
                operand(),
                operand(),
                access,
                SemanticAtomicOrderingV1::Acquire,
                true,
            )),
            SemanticStatementKindV1::SetDiscriminant {
                place: base_place(),
                variant_index: 1,
            },
            SemanticStatementKindV1::Deinitialize(base_place()),
            SemanticStatementKindV1::StorageLive(local),
            SemanticStatementKindV1::StorageDead(local),
            SemanticStatementKindV1::Nop,
        ]);
        for statement in statements {
            component_round_trip(statement, encode_statement, |decoder| decoder.statement());
        }
    }

    #[test]
    fn every_edge_unwind_assertion_and_terminator_variant_round_trips_exactly() {
        let t = SemanticTypeIdV1::from_index(0);
        let block = SemanticBlockIdV1::from_index(0);
        let place = || SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], t).unwrap();
        let operand = || {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                t,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 1).unwrap()),
            ))
        };
        let edge_roles = [
            SemanticEdgeRoleV1::Goto,
            SemanticEdgeRoleV1::SwitchValue,
            SemanticEdgeRoleV1::SwitchOtherwise,
            SemanticEdgeRoleV1::CallReturn,
            SemanticEdgeRoleV1::CallUnwind,
            SemanticEdgeRoleV1::TailCallUnwind,
            SemanticEdgeRoleV1::DropReturn,
            SemanticEdgeRoleV1::DropUnwind,
            SemanticEdgeRoleV1::AssertSuccess,
            SemanticEdgeRoleV1::AssertUnwind,
            SemanticEdgeRoleV1::FalseEdgeReal,
            SemanticEdgeRoleV1::FalseEdgeImaginary,
        ];
        for role in edge_roles {
            component_round_trip(
                SemanticControlFlowEdgeV1::new(role, block),
                |writer, value| encode_edge(writer, *value),
                |decoder| decoder.edge(),
            );
        }
        let edge = |role| SemanticControlFlowEdgeV1::new(role, block);
        for unwind in [
            SemanticUnwindActionV1::Continue,
            SemanticUnwindActionV1::Unreachable,
            SemanticUnwindActionV1::Terminate,
            SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind)),
        ] {
            component_round_trip(
                unwind,
                |writer, value| encode_unwind(writer, *value),
                |decoder| decoder.unwind(),
            );
        }

        let assertions = vec![
            SemanticAssertMessageV1::BoundsCheck {
                length: operand(),
                index: operand(),
            },
            SemanticAssertMessageV1::Overflow {
                operation: SemanticBinaryOpV1::Add,
                left: operand(),
                right: operand(),
            },
            SemanticAssertMessageV1::DivisionByZero(operand()),
            SemanticAssertMessageV1::RemainderByZero(operand()),
            SemanticAssertMessageV1::MisalignedPointerDereference {
                required_alignment: operand(),
                found_alignment: operand(),
            },
            SemanticAssertMessageV1::NullPointerDereference,
            SemanticAssertMessageV1::ResumedAfterReturn,
            SemanticAssertMessageV1::ResumedAfterPanic,
        ];
        for assertion in assertions {
            component_round_trip(assertion, encode_assert_message, |decoder| {
                decoder.assert_message()
            });
        }

        let abi_value = SemanticAbiValueV1::new(t, SemanticAbiPassModeV1::Ignore);
        let terminators = vec![
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto)),
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: operand(),
                targets: SemanticSwitchTargetsV1::new(
                    vec![
                        SemanticSwitchTargetV1::new(0, edge(SemanticEdgeRoleV1::SwitchValue)),
                        SemanticSwitchTargetV1::new(1, edge(SemanticEdgeRoleV1::SwitchValue)),
                    ],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise),
                )
                .unwrap(),
            },
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
                    SemanticCallableIdV1::from_index(0),
                    vec![operand()],
                    vec![abi_value],
                    Some(SemanticCallDestinationV1::new(
                        place(),
                        edge(SemanticEdgeRoleV1::CallReturn),
                    )),
                    SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::CallUnwind)),
                )
                .unwrap(),
            ),
            SemanticTerminatorKindV1::TailCall(
                SemanticDirectTailCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    vec![operand()],
                    SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::TailCallUnwind)),
                )
                .unwrap(),
            ),
            SemanticTerminatorKindV1::Drop {
                place: place(),
                drop_glue: SemanticFunctionIdV1::from_index(0),
                target: edge(SemanticEdgeRoleV1::DropReturn),
                unwind: SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::DropUnwind)),
            },
            SemanticTerminatorKindV1::Assert {
                condition: operand(),
                expected: true,
                message: SemanticAssertMessageV1::NullPointerDereference,
                target: edge(SemanticEdgeRoleV1::AssertSuccess),
                unwind: SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::AssertUnwind)),
            },
            SemanticTerminatorKindV1::FalseEdge {
                real_target: edge(SemanticEdgeRoleV1::FalseEdgeReal),
                imaginary_target: edge(SemanticEdgeRoleV1::FalseEdgeImaginary),
            },
            SemanticTerminatorKindV1::Return,
            SemanticTerminatorKindV1::UnwindResume,
            SemanticTerminatorKindV1::UnwindTerminate,
            SemanticTerminatorKindV1::Abort,
            SemanticTerminatorKindV1::Unreachable,
        ];
        assert_eq!(terminators.len(), 12);
        for terminator in terminators {
            component_round_trip(terminator, encode_terminator, |decoder| {
                decoder.terminator()
            });
        }
    }
}
