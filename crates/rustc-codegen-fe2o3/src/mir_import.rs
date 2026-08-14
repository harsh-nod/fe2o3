use crate::collector::CollectionResult;
use crate::rust_type_layout_general::{
    AdtKind, BackendRepresentationFacts, ScalarPrimitiveFacts, SourceScalarKind, TypeLayoutFacts,
    TypeLayoutKind, extract_general_layout,
};
use crate::semantic_features::{self, SessionRecognizedSemanticItem};
use crate::trusted_device_items::TrustedDeviceItem;
use dialect_mir::{MirAttr, MirOp, MirOpRecord, MirType};
use fe2o3_artifacts::{
    AbiKind as KernelAbiKind, AbiLayout, Access as KernelAccess,
    AddressSpace as KernelAddressSpace, AliasClass, ArgumentOwnership, BlockSize, Capability,
    Endianness, LaunchContract, Mutability as KernelMutability, PointerWidth, ScalarType,
    TargetIdentity,
};
use fe2o3_compiler_ffi::CodeObjectVersion;
use fe2o3_rustc_front::FunctionIdentityV1;
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiContractIdV1,
    DeviceFfiEffectsV1, DeviceFfiPhysicalAbiV1, derive_device_ffi_contract_id_v1,
    parse_device_ffi_effects_v1, parse_device_ffi_physical_abi_v1,
    validate_device_ffi_effect_abi_v1,
};
use rustc_abi::{CanonAbi, Reg, RegKind};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::mir::{
    AggregateKind, BasicBlock, BinOp, Body, ConstOperand, Local, NonDivergingIntrinsic, Operand,
    Place, ProjectionElem, Rvalue, SourceInfo, StatementKind, TerminatorKind, UnOp,
};
use rustc_middle::ty::{
    self, FloatTy, Instance, IntTy, Mutability, Ty, TyCtxt, TyKind, TypingEnv, UintTy,
};
use rustc_target::callconv::{ArgAttributes, ArgExtension, PassMode};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{self, Write};

const MIR_FUNCTION_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.mir-function-identity.v1\0";
#[allow(dead_code)]
const PORTABLE_MIR_SEMANTIC_DOMAIN_V2: &[u8] = b"fe2o3.portable-mir-semantic.v2\0";
#[allow(dead_code)]
const MAX_PORTABLE_MIR_TYPE_DEPTH_V2: usize = 64;

/// Stable policy inputs for one kernel's portable executable-MIR identity.
///
/// These values deliberately contain no compiler, Cargo, checkout, diagnostic,
/// or artifact observations. The digest is an admission-policy key only; it
/// does not authorize rustc MIR V2 ingestion or lowering.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub(crate) struct MirSemanticAdmissionInputsV2<'a> {
    kernel_export_name: &'a str,
    target: &'a TargetIdentity,
    abi: &'a AbiLayout,
    launch: &'a LaunchContract,
}

#[allow(dead_code)]
impl<'a> MirSemanticAdmissionInputsV2<'a> {
    pub(crate) const fn new(
        kernel_export_name: &'a str,
        target: &'a TargetIdentity,
        abi: &'a AbiLayout,
        launch: &'a LaunchContract,
    ) -> Self {
        Self {
            kernel_export_name,
            target,
            abi,
            launch,
        }
    }
}

/// Domain-separated SHA-256 identity of normalized portable MIR semantics.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[allow(dead_code)]
pub(crate) struct PortableMirSemanticDigestV2([u8; 32]);

#[allow(dead_code)]
impl PortableMirSemanticDigestV2 {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    #[cfg(test)]
    fn to_hex(self) -> String {
        let mut encoded = String::with_capacity(64);
        for byte in self.0 {
            let _ = write!(encoded, "{byte:02x}");
        }
        encoded
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirModule {
    pub functions: Vec<MirFunction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunction {
    pub export_name: String,
    pub rust_path: String,
    pub kind: MirFunctionKind,
    pub typed_profile: Option<MirKernelProfile>,
    pub arg_count: usize,
    pub local_count: usize,
    pub locals: Vec<MirLocal>,
    pub blocks: Vec<MirBlock>,
    pub frontend_contract: Option<crate::collector::AuthenticatedKernelFrontendContractV1>,
    pub(crate) matrix_frontend_abi: Option<MatrixFrontendAbiV2>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirFunctionKind {
    KernelEntry,
    InternalHelper,
    DeviceFfiExport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirKernelProfile {
    VecAddRustcLayoutV2,
    GeneralScalarSliceRustcLayoutV3,
}

const MATRIX_SOURCE_ABI_DOMAIN_V2: &[u8] = fe2o3_kernel_ir::MATRIX_SOURCE_ABI_RECORD_DOMAIN_V2;
const ROW_SOFTMAX_PROVIDER_AUTHORITY_DOMAIN_V1: &[u8] =
    b"FE2O3/ROW-SOFTMAX-PROVIDER-AUTHORITY/V1\0";

pub(crate) const ROW_SOFTMAX_TRUSTED_ITEMS_V1: [TrustedDeviceItem; 8] = [
    TrustedDeviceItem::DisjointSlice,
    TrustedDeviceItem::ThreadIndex,
    TrustedDeviceItem::ThreadIndex1d,
    TrustedDeviceItem::ThreadIndexGet,
    TrustedDeviceItem::DisjointSliceGetMutAt,
    TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::Context),
    TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::ContextFromCompiler),
    TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::F32(
        fe2o3_kernel_ir::F32MathFunction::Exp,
    )),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RowSoftmaxProviderAuthorityV1 {
    pub(crate) provider: crate::trusted_device_items::ReviewedRowSoftmaxProviderDefinitionV1,
    pub(crate) definition_identities: Vec<[u8; 16]>,
    pub(crate) source_identities: Vec<[u8; 32]>,
    pub(crate) commitment: [u8; 32],
}

impl RowSoftmaxProviderAuthorityV1 {
    fn canonical_commitment(&self) -> Result<[u8; 32], &'static str> {
        if self.provider.crate_name != "fe2o3_device"
            || self.provider.stable_crate_id == 0
            || self.provider.crate_hash == [0; 16]
            || self.provider.cargo_metadata_build_observation == [0; 32]
            || self.provider.source_identity == [0; 32]
            || self.definition_identities.len() != ROW_SOFTMAX_TRUSTED_ITEMS_V1.len()
            || self.source_identities.len() != ROW_SOFTMAX_TRUSTED_ITEMS_V1.len()
            || self
                .definition_identities
                .iter()
                .any(|identity| identity == &[0; 16])
            || self
                .source_identities
                .iter()
                .any(|identity| identity == &[0; 32])
            || self.provider.source_identity != self.source_identities[0]
        {
            return Err("row-softmax provider authority is incomplete");
        }
        let mut digest = Sha256::new();
        row_provider_field(&mut digest, ROW_SOFTMAX_PROVIDER_AUTHORITY_DOMAIN_V1);
        row_provider_field(&mut digest, self.provider.crate_name.as_bytes());
        row_provider_field(&mut digest, &self.provider.stable_crate_id.to_le_bytes());
        row_provider_field(&mut digest, &self.provider.crate_hash);
        row_provider_field(&mut digest, &self.provider.cargo_metadata_build_observation);
        for ((item, definition), source) in ROW_SOFTMAX_TRUSTED_ITEMS_V1
            .iter()
            .zip(&self.definition_identities)
            .zip(&self.source_identities)
        {
            row_provider_field(&mut digest, item.canonical_path().as_bytes());
            row_provider_field(&mut digest, definition);
            row_provider_field(&mut digest, source);
        }
        Ok(digest.finalize().into())
    }

    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.canonical_commitment()? != self.commitment {
            return Err("row-softmax provider authority commitment mismatch");
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn canonical_for_test(cargo_metadata_build_observation: [u8; 32]) -> Self {
        let source_identities = [
            [4; 32], [5; 32], [5; 32], [5; 32], [4; 32], [6; 32], [6; 32], [6; 32],
        ];
        let mut authority = Self {
            provider: crate::trusted_device_items::ReviewedRowSoftmaxProviderDefinitionV1 {
                crate_name: "fe2o3_device".to_owned(),
                stable_crate_id: 1,
                crate_hash: [2; 16],
                cargo_metadata_build_observation,
                source_identity: [4; 32],
            },
            definition_identities: (0..ROW_SOFTMAX_TRUSTED_ITEMS_V1.len())
                .map(|index| [u8::try_from(index + 5).expect("small identity"); 16])
                .collect(),
            source_identities: source_identities.to_vec(),
            commitment: [0; 32],
        };
        authority.commitment = authority
            .canonical_commitment()
            .expect("synthetic row-softmax provider authority");
        authority
    }
}

fn row_provider_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MatrixRustPassModeV1 {
    CastI64,
    Indirect {
        pointee_size: u64,
        pointee_align: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MatrixFragmentLayoutV1 {
    pub(crate) repr_c: bool,
    pub(crate) size: u64,
    pub(crate) alignment: u64,
    pub(crate) field_count: u8,
    pub(crate) field_offset: u64,
    pub(crate) array_length: u64,
    pub(crate) array_stride: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MatrixBf16LayoutV1 {
    pub(crate) repr_transparent: bool,
    pub(crate) size: u64,
    pub(crate) alignment: u64,
    pub(crate) field_count: u8,
    pub(crate) field_offset: u64,
    pub(crate) scalar_bits: u64,
    pub(crate) scalar_unsigned: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MatrixArgAttributesV2 {
    pub(crate) regular: u8,
    pub(crate) extension: u8,
    pub(crate) pointee_size: u64,
    pub(crate) pointee_alignment: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MatrixRegV2 {
    pub(crate) kind: u8,
    pub(crate) size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MatrixPassModeV2 {
    Ignore,
    Direct(MatrixArgAttributesV2),
    Pair(MatrixArgAttributesV2, MatrixArgAttributesV2),
    Cast {
        pad_i32: bool,
        prefix: Box<[Option<MatrixRegV2>; 8]>,
        rest_offset: Option<u64>,
        rest: MatrixRegV2,
        rest_total: u64,
        rest_consecutive: bool,
        attrs: MatrixArgAttributesV2,
    },
    Indirect {
        attrs: MatrixArgAttributesV2,
        meta_attrs: Option<MatrixArgAttributesV2>,
        on_stack: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MatrixSourceTypeRoleV2 {
    Unit,
    DeviceMatrixReference,
    Bf16Fragment,
    F32Fragment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MatrixArgAbiFactsV2 {
    pub(crate) role: MatrixSourceTypeRoleV2,
    pub(crate) layout: TypeLayoutFacts,
    pub(crate) mode: MatrixPassModeV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MatrixFnAbiFactsV2 {
    pub(crate) convention: u8,
    pub(crate) c_variadic: bool,
    pub(crate) fixed_count: u32,
    pub(crate) can_unwind: bool,
    pub(crate) arguments: Vec<MatrixArgAbiFactsV2>,
    pub(crate) result: MatrixArgAbiFactsV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MatrixObservedSourceAbiV2 {
    pub(crate) provider: crate::trusted_device_items::ReviewedMatrixProviderObservationV2,
    pub(crate) definition_identities: Vec<[u8; 16]>,
    pub(crate) receiver_layout: TypeLayoutFacts,
    pub(crate) lhs_layout: TypeLayoutFacts,
    pub(crate) rhs_layout: TypeLayoutFacts,
    pub(crate) accumulator_layout: TypeLayoutFacts,
    pub(crate) result_layout: TypeLayoutFacts,
    pub(crate) kernel_abi: MatrixFnAbiFactsV2,
    pub(crate) method_abi: MatrixFnAbiFactsV2,
    pub(crate) kernel_source_structure: [MatrixSourceTypeRoleV2; 4],
    pub(crate) method_source_structure: [MatrixSourceTypeRoleV2; 5],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MatrixFrontendAbiV2 {
    pub(crate) digest: [u8; 32],
    pub(crate) observed_source: MatrixObservedSourceAbiV2,
    pub(crate) projected_kernarg: fe2o3_kernel_ir::MatrixProjectedKernargPolicyV1,
}

impl MatrixFrontendAbiV2 {
    fn from_observed(observed_source: MatrixObservedSourceAbiV2) -> Result<Self, &'static str> {
        let canonical_record = observed_source.canonical_record()?;
        Ok(Self {
            digest: Sha256::digest(&canonical_record).into(),
            observed_source,
            projected_kernarg: fe2o3_kernel_ir::MatrixProjectedKernargPolicyV1::canonical(),
        })
    }

    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        let canonical_record = self.observed_source.canonical_record()?;
        if Sha256::digest(&canonical_record).as_slice() != self.digest {
            return Err("matrix rustc source ABI observation digest mismatch");
        }
        self.projected_kernarg.validate()?;
        Ok(())
    }

    pub(crate) fn kernel_ir_binding(
        &self,
    ) -> Result<fe2o3_kernel_ir::MatrixFrontendBindingV2, &'static str> {
        self.validate()?;
        let provider = &self.observed_source.provider;
        let observed_source = fe2o3_kernel_ir::MatrixSourceAbiObservationV2::new_untrusted_claim(
            fe2o3_kernel_ir::MatrixProviderIdentityV2 {
                crate_name: provider.crate_name.clone(),
                stable_crate_id: provider.stable_crate_id,
                crate_hash: provider.crate_hash,
                cargo_metadata_build_observation: provider.cargo_metadata_build_observation,
                source_identity: provider.source_identity,
                definition_identities: self.observed_source.definition_identities.clone(),
            },
            self.observed_source.canonical_record()?,
        )?;
        if observed_source.digest != self.digest {
            return Err("matrix source ABI digest changed at the Kernel IR boundary");
        }
        Ok(fe2o3_kernel_ir::MatrixFrontendBindingV2 {
            observed_source,
            projected_kernarg: self.projected_kernarg.clone(),
        })
    }

    #[cfg(test)]
    pub(crate) fn canonical_for_test() -> Self {
        Self::from_observed(MatrixObservedSourceAbiV2::synthetic_for_test())
            .expect("synthetic matrix observation must serialize")
    }
}

impl MatrixObservedSourceAbiV2 {
    fn canonical_record(&self) -> Result<Vec<u8>, &'static str> {
        if self.provider.crate_name != "fe2o3_device"
            || self.provider.stable_crate_id == 0
            || self.provider.crate_hash == [0; 16]
            || self.provider.cargo_metadata_build_observation == [0; 32]
            || self.provider.source_identity == [0; 32]
            || self.definition_identities.len() != 6
            || self
                .definition_identities
                .iter()
                .any(|identity| identity == &[0; 16])
        {
            return Err("matrix rustc source ABI provider identity is incomplete");
        }
        let mut writer = MatrixSourceAbiWriterV2::new();
        writer.raw_bytes(MATRIX_SOURCE_ABI_DOMAIN_V2);
        writer.text(&self.provider.crate_name)?;
        writer.u64(self.provider.stable_crate_id);
        writer.bytes(&self.provider.crate_hash)?;
        writer.bytes(&self.provider.cargo_metadata_build_observation)?;
        writer.bytes(&self.provider.source_identity)?;
        writer.len(self.definition_identities.len())?;
        for identity in &self.definition_identities {
            writer.bytes(identity)?;
        }
        for layout in [
            &self.receiver_layout,
            &self.lhs_layout,
            &self.rhs_layout,
            &self.accumulator_layout,
            &self.result_layout,
        ] {
            writer.layout(layout)?;
        }
        writer.fn_abi(&self.kernel_abi)?;
        writer.fn_abi(&self.method_abi)?;
        for role in self.kernel_source_structure {
            writer.source_role(role);
        }
        for role in self.method_source_structure {
            writer.source_role(role);
        }
        Ok(writer.finish())
    }

    #[cfg(test)]
    fn synthetic_for_test() -> Self {
        fn layout(name: &str) -> TypeLayoutFacts {
            TypeLayoutFacts {
                rust_type: name.to_owned(),
                size_bytes: 0,
                abi_alignment_bytes: 1,
                unadjusted_abi_alignment_bytes: 1,
                maximum_requested_alignment_bytes: None,
                uninhabited: false,
                backend_representation: BackendRepresentationFacts::Memory,
                largest_niche: None,
                kind: TypeLayoutKind::Tuple(Vec::new()),
            }
        }
        fn arg(role: MatrixSourceTypeRoleV2, name: &str) -> MatrixArgAbiFactsV2 {
            MatrixArgAbiFactsV2 {
                role,
                layout: layout(name),
                mode: MatrixPassModeV2::Ignore,
            }
        }
        let provider = crate::trusted_device_items::ReviewedMatrixProviderObservationV2 {
            crate_name: "fe2o3_device".to_owned(),
            stable_crate_id: 1,
            crate_hash: [2; 16],
            cargo_metadata_build_observation: [3; 32],
            source_identity: [4; 32],
        };
        let kernel_abi = MatrixFnAbiFactsV2 {
            convention: 1,
            c_variadic: false,
            fixed_count: 3,
            can_unwind: false,
            arguments: vec![
                arg(MatrixSourceTypeRoleV2::Bf16Fragment, "lhs"),
                arg(MatrixSourceTypeRoleV2::Bf16Fragment, "rhs"),
                arg(MatrixSourceTypeRoleV2::F32Fragment, "accumulator"),
            ],
            result: arg(MatrixSourceTypeRoleV2::Unit, "unit"),
        };
        let method_abi = MatrixFnAbiFactsV2 {
            convention: 1,
            c_variadic: false,
            fixed_count: 4,
            can_unwind: false,
            arguments: vec![
                arg(MatrixSourceTypeRoleV2::DeviceMatrixReference, "receiver"),
                arg(MatrixSourceTypeRoleV2::Bf16Fragment, "lhs"),
                arg(MatrixSourceTypeRoleV2::Bf16Fragment, "rhs"),
                arg(MatrixSourceTypeRoleV2::F32Fragment, "accumulator"),
            ],
            result: arg(MatrixSourceTypeRoleV2::F32Fragment, "result"),
        };
        Self {
            provider,
            definition_identities: vec![[5; 16], [6; 16], [7; 16], [8; 16], [9; 16], [10; 16]],
            receiver_layout: layout("receiver"),
            lhs_layout: layout("lhs"),
            rhs_layout: layout("rhs"),
            accumulator_layout: layout("accumulator"),
            result_layout: layout("result"),
            kernel_abi,
            method_abi,
            kernel_source_structure: [
                MatrixSourceTypeRoleV2::Bf16Fragment,
                MatrixSourceTypeRoleV2::Bf16Fragment,
                MatrixSourceTypeRoleV2::F32Fragment,
                MatrixSourceTypeRoleV2::Unit,
            ],
            method_source_structure: [
                MatrixSourceTypeRoleV2::DeviceMatrixReference,
                MatrixSourceTypeRoleV2::Bf16Fragment,
                MatrixSourceTypeRoleV2::Bf16Fragment,
                MatrixSourceTypeRoleV2::F32Fragment,
                MatrixSourceTypeRoleV2::F32Fragment,
            ],
        }
    }
}

struct MatrixSourceAbiWriterV2 {
    bytes: Vec<u8>,
}

impl MatrixSourceAbiWriterV2 {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn boolean(&mut self, value: bool) {
        self.bytes.push(u8::from(value));
    }

    fn tag(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn len(&mut self, value: usize) -> Result<(), &'static str> {
        self.u32(u32::try_from(value).map_err(|_| "matrix source ABI record length overflowed")?);
        Ok(())
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), &'static str> {
        self.len(value.len())?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    fn raw_bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn text(&mut self, value: &str) -> Result<(), &'static str> {
        self.bytes(value.as_bytes())
    }

    fn optional_u64(&mut self, value: Option<u64>) {
        self.boolean(value.is_some());
        if let Some(value) = value {
            self.u64(value);
        }
    }

    fn optional_u128(&mut self, value: Option<u128>) {
        self.boolean(value.is_some());
        if let Some(value) = value {
            self.u128(value);
        }
    }

    fn source_role(&mut self, role: MatrixSourceTypeRoleV2) {
        self.tag(match role {
            MatrixSourceTypeRoleV2::Unit => 0,
            MatrixSourceTypeRoleV2::DeviceMatrixReference => 1,
            MatrixSourceTypeRoleV2::Bf16Fragment => 2,
            MatrixSourceTypeRoleV2::F32Fragment => 3,
        });
    }

    fn layout(&mut self, facts: &TypeLayoutFacts) -> Result<(), &'static str> {
        self.text(&facts.rust_type)?;
        self.u64(facts.size_bytes);
        self.u64(facts.abi_alignment_bytes);
        self.u64(facts.unadjusted_abi_alignment_bytes);
        self.optional_u64(facts.maximum_requested_alignment_bytes);
        self.boolean(facts.uninhabited);
        self.backend_representation(&facts.backend_representation);
        self.boolean(facts.largest_niche.is_some());
        if let Some(niche) = facts.largest_niche {
            self.u64(niche.offset_bytes);
            self.scalar_primitive(niche.primitive);
            self.u128(niche.valid_range_start);
            self.u128(niche.valid_range_end);
        }
        match &facts.kind {
            TypeLayoutKind::Scalar(source) => {
                self.tag(0);
                self.source_scalar(*source);
            }
            TypeLayoutKind::Pointer(pointer) => {
                self.tag(1);
                self.tag(match pointer.kind {
                    crate::rust_type_layout_general::PointerKind::SharedReference => 0,
                    crate::rust_type_layout_general::PointerKind::MutableReference => 1,
                    crate::rust_type_layout_general::PointerKind::ConstRaw => 2,
                    crate::rust_type_layout_general::PointerKind::MutRaw => 3,
                });
                self.u32(pointer.address_space);
                self.layout(&pointer.pointee)?;
            }
            TypeLayoutKind::Array(array) => {
                self.tag(2);
                self.u64(array.length);
                self.u64(array.stride_bytes);
                self.layout(&array.element)?;
            }
            TypeLayoutKind::Tuple(fields) => {
                self.tag(3);
                self.fields(fields)?;
            }
            TypeLayoutKind::Adt(adt) => {
                self.tag(4);
                self.text(&adt.definition)?;
                self.tag(match adt.kind {
                    AdtKind::Struct => 0,
                    AdtKind::Enum => 1,
                    AdtKind::Union => 2,
                });
                self.boolean(adt.representation.c);
                self.boolean(adt.representation.transparent);
                self.boolean(adt.representation.explicit_integer);
                self.optional_u64(adt.representation.packed_alignment_bytes);
                self.optional_u64(adt.representation.requested_alignment_bytes);
                self.boolean(adt.tag.is_some());
                if let Some(tag) = adt.tag {
                    self.u64(tag.offset_bytes);
                    self.scalar_layout(tag.scalar);
                    match tag.encoding {
                        crate::rust_type_layout_general::EnumTagEncodingFacts::Direct => {
                            self.tag(0)
                        }
                        crate::rust_type_layout_general::EnumTagEncodingFacts::Niche {
                            untagged_variant,
                            niche_variants_start,
                            niche_variants_end,
                            niche_start,
                        } => {
                            self.tag(1);
                            self.u32(untagged_variant);
                            self.u32(niche_variants_start);
                            self.u32(niche_variants_end);
                            self.u128(niche_start);
                        }
                    }
                }
                self.len(adt.variants.len())?;
                for variant in &adt.variants {
                    self.u32(variant.source_index);
                    self.text(&variant.name)?;
                    self.optional_u128(variant.discriminant_bits);
                    self.boolean(variant.discriminant_type.is_some());
                    if let Some(discriminant_type) = &variant.discriminant_type {
                        self.text(discriminant_type)?;
                    }
                    self.boolean(variant.discriminant_scalar.is_some());
                    if let Some(discriminant_scalar) = variant.discriminant_scalar {
                        self.source_scalar(discriminant_scalar);
                    }
                    self.boolean(variant.uninhabited);
                    self.fields(&variant.fields)?;
                }
            }
        }
        Ok(())
    }

    fn fields(
        &mut self,
        fields: &[crate::rust_type_layout_general::FieldLayoutFacts],
    ) -> Result<(), &'static str> {
        self.len(fields.len())?;
        for field in fields {
            self.len(field.source_index)?;
            self.len(field.memory_index)?;
            self.boolean(field.name.is_some());
            if let Some(name) = &field.name {
                self.text(name)?;
            }
            self.u64(field.offset_bytes);
            self.layout(&field.layout)?;
        }
        Ok(())
    }

    fn source_scalar(&mut self, scalar: SourceScalarKind) {
        match scalar {
            SourceScalarKind::Bool => self.tag(0),
            SourceScalarKind::Char => self.tag(1),
            SourceScalarKind::SignedInteger { bits } => {
                self.tag(2);
                self.u64(bits);
            }
            SourceScalarKind::UnsignedInteger { bits } => {
                self.tag(3);
                self.u64(bits);
            }
            SourceScalarKind::PointerSizedSignedInteger { bits } => {
                self.tag(4);
                self.u64(bits);
            }
            SourceScalarKind::PointerSizedUnsignedInteger { bits } => {
                self.tag(5);
                self.u64(bits);
            }
            SourceScalarKind::Float { bits } => {
                self.tag(6);
                self.u64(bits);
            }
        }
    }

    fn scalar_primitive(&mut self, primitive: ScalarPrimitiveFacts) {
        match primitive {
            ScalarPrimitiveFacts::Pointer { address_space } => {
                self.tag(0);
                self.u32(address_space);
            }
            ScalarPrimitiveFacts::Integer { bits, signed } => {
                self.tag(1);
                self.u64(bits);
                self.boolean(signed);
            }
            ScalarPrimitiveFacts::Float { bits } => {
                self.tag(2);
                self.u64(bits);
            }
        }
    }

    fn scalar_layout(&mut self, scalar: crate::rust_type_layout_general::ScalarLayoutFacts) {
        self.scalar_primitive(scalar.primitive);
        self.u64(scalar.size_bytes);
        self.u64(scalar.abi_alignment_bytes);
        self.boolean(scalar.initialized);
        self.u128(scalar.valid_range_start);
        self.u128(scalar.valid_range_end);
    }

    fn backend_representation(&mut self, backend: &BackendRepresentationFacts) {
        match backend {
            BackendRepresentationFacts::Scalar(scalar) => {
                self.tag(0);
                self.scalar_layout(*scalar);
            }
            BackendRepresentationFacts::ScalarPair {
                first,
                second,
                second_offset_bytes,
            } => {
                self.tag(1);
                self.scalar_layout(*first);
                self.scalar_layout(*second);
                self.u64(*second_offset_bytes);
            }
            BackendRepresentationFacts::Memory => self.tag(2),
        }
    }

    fn fn_abi(&mut self, abi: &MatrixFnAbiFactsV2) -> Result<(), &'static str> {
        self.tag(abi.convention);
        self.boolean(abi.c_variadic);
        self.u32(abi.fixed_count);
        self.boolean(abi.can_unwind);
        self.len(abi.arguments.len())?;
        for argument in &abi.arguments {
            self.arg_abi(argument)?;
        }
        self.arg_abi(&abi.result)
    }

    fn arg_abi(&mut self, argument: &MatrixArgAbiFactsV2) -> Result<(), &'static str> {
        self.source_role(argument.role);
        self.layout(&argument.layout)?;
        self.pass_mode(&argument.mode);
        Ok(())
    }

    fn attrs(&mut self, attrs: MatrixArgAttributesV2) {
        self.tag(attrs.regular);
        self.tag(attrs.extension);
        self.u64(attrs.pointee_size);
        self.optional_u64(attrs.pointee_alignment);
    }

    fn reg(&mut self, reg: MatrixRegV2) {
        self.tag(reg.kind);
        self.u64(reg.size);
    }

    fn pass_mode(&mut self, mode: &MatrixPassModeV2) {
        match mode {
            MatrixPassModeV2::Ignore => self.tag(0),
            MatrixPassModeV2::Direct(attrs) => {
                self.tag(1);
                self.attrs(*attrs);
            }
            MatrixPassModeV2::Pair(first, second) => {
                self.tag(2);
                self.attrs(*first);
                self.attrs(*second);
            }
            MatrixPassModeV2::Cast {
                pad_i32,
                prefix,
                rest_offset,
                rest,
                rest_total,
                rest_consecutive,
                attrs,
            } => {
                self.tag(3);
                self.boolean(*pad_i32);
                for reg in prefix.iter() {
                    self.boolean(reg.is_some());
                    if let Some(reg) = reg {
                        self.reg(*reg);
                    }
                }
                self.optional_u64(*rest_offset);
                self.reg(*rest);
                self.u64(*rest_total);
                self.boolean(*rest_consecutive);
                self.attrs(*attrs);
            }
            MatrixPassModeV2::Indirect {
                attrs,
                meta_attrs,
                on_stack,
            } => {
                self.tag(4);
                self.attrs(*attrs);
                self.boolean(meta_attrs.is_some());
                if let Some(meta_attrs) = meta_attrs {
                    self.attrs(*meta_attrs);
                }
                self.boolean(*on_stack);
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirLocal {
    pub index: usize,
    pub role: MirLocalRole,
    pub ty: MirImportedType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirLocalRole {
    Return,
    Arg,
    Temp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirImportedType {
    pub kind: MirType,
    pub rust: String,
    pub shape: MirTypeShape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTypeShape {
    Unit,
    Bool,
    I32,
    U32,
    I64,
    ISize,
    USize,
    F32,
    F64,
    F16,
    Bf16,
    Bf16x2,
    DeviceMath,
    Slice {
        element: Box<MirTypeShape>,
        mutable: bool,
    },
    DisjointSlice {
        element: Box<MirTypeShape>,
    },
    Reference {
        pointee: Box<MirTypeShape>,
        mutable: bool,
    },
    RawPointer {
        pointee: Box<MirTypeShape>,
        mutable: bool,
    },
    Adt {
        identity: String,
    },
    Tuple(Vec<MirTypeShape>),
    Unknown,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MirSourceLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBlock {
    pub index: usize,
    pub statements: Vec<MirStatement>,
    pub terminator: Option<MirTerminator>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStatement {
    pub index: usize,
    pub kind: MirStatementKind,
    pub destination: Option<MirPlaceRef>,
    pub operands: Vec<MirOperandRef>,
    pub rvalue: Option<MirRvalueKind>,
    /// Compatibility spelling consumed by the legacy record recognizer.
    pub operation: Option<String>,
    pub source: Option<MirSourceLocation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStatementKind {
    Assign,
    StorageLive,
    StorageDead,
    SetDiscriminant,
    Intrinsic,
    CopyNonOverlapping,
    Retag,
    Coverage,
    Nop,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPlaceRef {
    pub local: usize,
    pub projection: Vec<MirProjectionElem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirProjectionElem {
    Deref,
    Field(usize),
    Index {
        local: usize,
    },
    ConstantIndex {
        offset: u64,
        min_length: u64,
        from_end: bool,
    },
    Subslice {
        from: u64,
        to: u64,
        from_end: bool,
    },
    Downcast {
        variant: usize,
    },
    OpaqueCast,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirOperandRef {
    Place(MirPlaceRef),
    Constant {
        ty: MirImportedType,
        literal: MirConstant,
        /// Compatibility spelling consumed by the legacy record recognizer.
        value: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirConstant {
    Bool(bool),
    I32(i32),
    U32(u32),
    I64(i64),
    ISize(i64),
    USize(u64),
    F32Bits(u32),
    F64Bits(u64),
    Unevaluated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirTerminator {
    pub kind: MirTerminatorKind,
    pub source: Option<MirSourceLocation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCallee {
    identity: MirCalleeIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirExternalImport {
    contract_identity: DeviceFfiContractIdV1,
    symbol: String,
    target: String,
    code_object_version: u16,
    physical_abi: DeviceFfiPhysicalAbiV1,
    effects: DeviceFfiEffectsV1,
    semantic_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MirCalleeIdentity {
    SessionRecognized(SessionRecognizedSemanticItem),
    ExternalImport(MirExternalImport),
    RejectedTrustedProvider { path: String, marker: &'static str },
    Untrusted(String),
}

impl MirCallee {
    fn session_recognized(item: SessionRecognizedSemanticItem) -> Self {
        Self {
            identity: MirCalleeIdentity::SessionRecognized(item),
        }
    }

    #[cfg(test)]
    fn trusted(item: TrustedDeviceItem) -> Self {
        Self::session_recognized(SessionRecognizedSemanticItem::trusted_device_for_test(item))
    }

    fn untrusted(identity: String) -> Self {
        Self {
            identity: MirCalleeIdentity::Untrusted(identity),
        }
    }

    fn external_import(import: MirExternalImport) -> Self {
        Self {
            identity: MirCalleeIdentity::ExternalImport(import),
        }
    }

    fn rejected_trusted_provider(path: String, marker: &'static str) -> Self {
        Self {
            identity: MirCalleeIdentity::RejectedTrustedProvider { path, marker },
        }
    }

    pub(crate) fn identity(&self) -> &str {
        match &self.identity {
            MirCalleeIdentity::SessionRecognized(item) => item.canonical_path(),
            MirCalleeIdentity::ExternalImport(import) => &import.symbol,
            MirCalleeIdentity::RejectedTrustedProvider { path, .. } => path,
            MirCalleeIdentity::Untrusted(identity) => identity,
        }
    }

    pub(crate) fn session_recognized_item(&self) -> Option<SessionRecognizedSemanticItem> {
        match &self.identity {
            MirCalleeIdentity::SessionRecognized(item) => Some(*item),
            MirCalleeIdentity::ExternalImport(_)
            | MirCalleeIdentity::RejectedTrustedProvider { .. }
            | MirCalleeIdentity::Untrusted(_) => None,
        }
    }

    pub(crate) fn trusted_item(&self) -> Option<TrustedDeviceItem> {
        self.session_recognized_item()
            .map(SessionRecognizedSemanticItem::trusted_device_item)
    }

    pub(crate) fn external_import_evidence(&self) -> Option<&MirExternalImport> {
        match &self.identity {
            MirCalleeIdentity::ExternalImport(import) => Some(import),
            MirCalleeIdentity::SessionRecognized(_)
            | MirCalleeIdentity::RejectedTrustedProvider { .. }
            | MirCalleeIdentity::Untrusted(_) => None,
        }
    }

    pub(crate) fn rejected_provider_marker(&self) -> Option<&'static str> {
        match &self.identity {
            MirCalleeIdentity::RejectedTrustedProvider { marker, .. } => Some(*marker),
            MirCalleeIdentity::SessionRecognized(_)
            | MirCalleeIdentity::ExternalImport(_)
            | MirCalleeIdentity::Untrusted(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn trusted_for_test(item: TrustedDeviceItem) -> Self {
        Self::trusted(item)
    }

    #[cfg(test)]
    pub(crate) fn untrusted_for_test(identity: impl Into<String>) -> Self {
        Self::untrusted(identity.into())
    }

    #[cfg(test)]
    pub(crate) fn external_import_for_test(
        symbol: &str,
        physical_abi: &str,
        effects: &str,
    ) -> Self {
        let physical_abi = parse_device_ffi_physical_abi_v1(physical_abi)
            .expect("test external-import ABI must be canonical");
        let effects =
            parse_device_ffi_effects_v1(effects).expect("test external-import effects must parse");
        validate_device_ffi_effect_abi_v1(&effects, &physical_abi)
            .expect("test external-import ABI/effects must agree");
        Self::external_import(MirExternalImport {
            contract_identity: DeviceFfiContractIdV1::from_bytes([0x5a; 32]),
            symbol: symbol.to_owned(),
            target: "gfx942:xnack-".to_owned(),
            code_object_version: 5,
            physical_abi,
            effects,
            semantic_identity: "6b".repeat(32),
        })
    }
}

impl MirExternalImport {
    pub(crate) fn physical_abi(&self) -> &DeviceFfiPhysicalAbiV1 {
        &self.physical_abi
    }

    pub(crate) fn effects(&self) -> &DeviceFfiEffectsV1 {
        &self.effects
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSwitchTarget {
    pub value: u128,
    pub target: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTerminatorKind {
    Return,
    Unreachable,
    Goto {
        target: usize,
    },
    SwitchInt {
        discriminant: MirOperandRef,
        targets: Vec<MirSwitchTarget>,
        otherwise: usize,
    },
    Call {
        callee: Option<MirCallee>,
        target: Option<usize>,
        destination: Option<MirPlaceRef>,
        operands: Vec<MirOperandRef>,
    },
    Assert {
        condition: MirOperandRef,
        expected: bool,
        target: usize,
    },
    Drop {
        target: usize,
    },
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirRvalueKind {
    Use,
    Repeat,
    Ref,
    RawPointer,
    Cast,
    Binary(MirBinaryOp),
    Unary(MirUnaryOp),
    Discriminant,
    Aggregate,
    /// A rustc-authenticated construction of a payload-free enum variant.
    FieldlessEnumVariant(i64),
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitXor,
    BitAnd,
    BitOr,
    Shl,
    Shr,
    Eq,
    Lt,
    Le,
    Ne,
    Ge,
    Gt,
    Cmp,
    Offset,
    AddUnchecked,
    SubUnchecked,
    MulUnchecked,
    ShlUnchecked,
    ShrUnchecked,
    AddWithOverflow,
    SubWithOverflow,
    MulWithOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirUnaryOp {
    Not,
    Neg,
    PtrMetadata,
}

#[derive(Clone, Debug)]
struct CompilerFfiImports {
    entries: Vec<(crate::device_ffi::DeviceFfiSourceOwner, MirExternalImport)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirImportError {
    message: String,
}

impl MirImportError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for MirImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for MirImportError {}

impl CompilerFfiImports {
    fn from_collection<'tcx>(
        tcx: TyCtxt<'tcx>,
        collection: &CollectionResult<'tcx>,
    ) -> Result<Self, MirImportError> {
        let reconstructed = crate::compiler_ffi_adapter::adapt_collection_v1(tcx, collection)
            .map_err(|error| {
                MirImportError::new(format!(
                    "compiler FFI evidence could not be reconstructed before MIR import: {error}"
                ))
            })?;
        if reconstructed.as_ref() != collection.compiler_ffi_observation.as_ref() {
            return Err(MirImportError::new(
                "compiler FFI observation disagrees with the closed collection",
            ));
        }

        let imports = &collection.device_ffi.imports;
        if imports.is_empty() {
            return Ok(Self {
                entries: Vec::new(),
            });
        }
        let envelope = reconstructed.as_ref().ok_or_else(|| {
            MirImportError::new("reachable device FFI imports have no compiler envelope")
        })?;
        let envelope_symbols = envelope
            .directional_symbols()
            .imports()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let collected_symbols = imports
            .iter()
            .map(|entry| entry.contract.symbol.clone())
            .collect::<Vec<_>>();
        if envelope_symbols != collected_symbols {
            return Err(MirImportError::new(
                "compiler FFI import symbols disagree with the closed collection",
            ));
        }

        let envelope_target = envelope.target().to_string();
        let envelope_code_object_version =
            code_object_version_number(envelope.code_object_version());
        let mut entries = Vec::with_capacity(imports.len());
        for entry in imports {
            let contract = &entry.contract;
            if contract.direction != crate::device_ffi::DeviceFfiDirection::Import
                || entry.link_role_assertion.asserted_for_consistency_check()
                    != &crate::device_ffi::DeviceFfiLinkRole::RequiresExternalDefinition
            {
                return Err(MirImportError::new(format!(
                    "device FFI `{}` is not an external import",
                    contract.symbol
                )));
            }
            let asserted_code_object_version = *contract
                .code_object_version_assertion
                .asserted_for_consistency_check();
            let semantic_identity = contract
                .semantic_identity_assertion
                .asserted_for_consistency_check();
            let effects_text = contract.effects_assertion.asserted_for_consistency_check();
            entries.push((
                entry.owner.clone(),
                validate_external_import_fields(
                    contract.id,
                    &contract.symbol,
                    &contract.target,
                    asserted_code_object_version,
                    &contract.physical_abi,
                    effects_text,
                    semantic_identity,
                    &envelope_target,
                    envelope_code_object_version,
                )?,
            ));
        }
        Ok(Self { entries })
    }

    fn classify<'tcx>(
        &self,
        tcx: TyCtxt<'tcx>,
        def_id: rustc_hir::def_id::DefId,
    ) -> Option<MirExternalImport> {
        let def_path_hash = tcx.def_path_hash(def_id).0.to_le_bytes();
        self.entries.iter().find_map(|(owner, import)| {
            (owner.def_path_hash == def_path_hash
                && crate::device_ffi::source_owner_matches_instance(
                    tcx,
                    owner,
                    rustc_middle::ty::Instance::mono(tcx, def_id),
                ))
            .then(|| import.clone())
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_external_import_fields(
    contract_identity: DeviceFfiContractIdV1,
    symbol: &str,
    target: &str,
    code_object_version: u16,
    physical_abi_text: &str,
    effects_text: &str,
    semantic_identity: &str,
    envelope_target: &str,
    envelope_code_object_version: u16,
) -> Result<MirExternalImport, MirImportError> {
    if target != envelope_target {
        return Err(MirImportError::new(format!(
            "device FFI import `{symbol}` target `{target}` disagrees with compiler envelope target `{envelope_target}`"
        )));
    }
    if code_object_version != envelope_code_object_version {
        return Err(MirImportError::new(format!(
            "device FFI import `{symbol}` code-object version {code_object_version} disagrees with compiler envelope version {envelope_code_object_version}"
        )));
    }
    let derived_identity = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
        symbol,
        calling_convention: "C",
        code_object_version,
        target,
        physical_abi: physical_abi_text,
        effects: effects_text,
        semantic_identity,
    });
    if derived_identity != contract_identity {
        return Err(MirImportError::new(format!(
            "device FFI import `{symbol}` contract identity does not match its canonical fields"
        )));
    }
    let physical_abi = parse_device_ffi_physical_abi_v1(physical_abi_text).map_err(|_| {
        MirImportError::new(format!(
            "device FFI import `{symbol}` has a malformed physical ABI"
        ))
    })?;
    let effects = parse_device_ffi_effects_v1(effects_text).map_err(|_| {
        MirImportError::new(format!(
            "device FFI import `{symbol}` has malformed effects"
        ))
    })?;
    validate_device_ffi_effect_abi_v1(&effects, &physical_abi).map_err(|_| {
        MirImportError::new(format!(
            "device FFI import `{symbol}` effects disagree with its physical ABI"
        ))
    })?;
    if !effects.is_none() {
        return Err(MirImportError::new(format!(
            "device FFI import `{symbol}` declares effects that kernel IR cannot yet preserve"
        )));
    }
    Ok(MirExternalImport {
        contract_identity,
        symbol: symbol.to_owned(),
        target: target.to_owned(),
        code_object_version,
        physical_abi,
        effects,
        semantic_identity: semantic_identity.to_owned(),
    })
}

const fn code_object_version_number(version: CodeObjectVersion) -> u16 {
    match version {
        CodeObjectVersion::V4 => 4,
        CodeObjectVersion::V5 => 5,
        CodeObjectVersion::V6 => 6,
    }
}

fn import_matrix_source_abi_v2<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
) -> Result<Option<MatrixFrontendAbiV2>, MirImportError> {
    let argument_types = (1..=body.arg_count)
        .map(|index| body.local_decls[Local::from_usize(index)].ty)
        .collect::<Vec<_>>();
    let contains_matrix_fragment = argument_types.iter().any(|ty| {
        trusted_adt_item(tcx, *ty).is_some_and(|item| {
            matches!(
                item,
                TrustedDeviceItem::Bf16MfmaFragment | TrustedDeviceItem::F32AccumulatorFragment
            )
        })
    });
    if !contains_matrix_fragment {
        return Ok(None);
    }
    let [lhs, rhs, accumulator] = argument_types.as_slice() else {
        return Err(MirImportError::new(
            "matrix frontend source ABI requires exactly three source parameters",
        ));
    };
    for (role, ty, expected) in [
        ("lhs", *lhs, TrustedDeviceItem::Bf16MfmaFragment),
        ("rhs", *rhs, TrustedDeviceItem::Bf16MfmaFragment),
        (
            "accumulator",
            *accumulator,
            TrustedDeviceItem::F32AccumulatorFragment,
        ),
    ] {
        if trusted_adt_item(tcx, ty) != Some(expected) {
            return Err(MirImportError::new(format!(
                "matrix frontend {role} parameter is not the genuine external `{}` type",
                expected.canonical_path()
            )));
        }
    }
    if !matches!(body.local_decls[Local::from_usize(0)].ty.kind(), TyKind::Tuple(fields) if fields.is_empty())
    {
        return Err(MirImportError::new(
            "matrix frontend kernel must return the unit type",
        ));
    }

    let (bf16_fragment, bf16) = import_bf16_fragment_layout(tcx, *lhs)?;
    let (rhs_fragment, rhs_bf16) = import_bf16_fragment_layout(tcx, *rhs)?;
    if rhs_fragment != bf16_fragment || rhs_bf16 != bf16 {
        return Err(MirImportError::new(
            "matrix lhs and rhs fragment physical layouts disagree",
        ));
    }
    let _f32_fragment = import_f32_fragment_layout(tcx, *accumulator)?;
    let _rust_pass_modes = import_matrix_rust_fn_abi(tcx, instance, &argument_types)?;
    let method_instance = matrix_method_instance(tcx, body)?;
    validate_matrix_method_fn_abi(tcx, method_instance)?;

    let provider_definition = match lhs.kind() {
        TyKind::Adt(definition, _) => definition.did(),
        _ => unreachable!("trusted fragment identity requires an ADT"),
    };
    let provider_crate = provider_definition.krate;
    let provider =
        crate::trusted_device_items::reviewed_matrix_provider_observation(tcx, provider_definition)
            .map_err(MirImportError::new)?;
    let definition_identities = [
        TrustedDeviceItem::DeviceMatrix,
        TrustedDeviceItem::DeviceMatrixFromCompiler,
        TrustedDeviceItem::DeviceValue(dialect_amdgcn::DeviceValueDiagnosticItem::Bf16),
        TrustedDeviceItem::Bf16MfmaFragment,
        TrustedDeviceItem::F32AccumulatorFragment,
        TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
    ]
    .into_iter()
    .map(|item| {
        crate::trusted_device_items::definition(tcx, item)
            .filter(|def_id| def_id.krate == provider_crate)
            .map(|def_id| tcx.def_path_hash(def_id).0.to_le_bytes())
            .ok_or_else(|| {
                MirImportError::new(format!(
                    "matrix provider omitted reviewed definition `{}`",
                    item.canonical_path()
                ))
            })
    })
    .collect::<Result<Vec<_>, _>>()?;

    let method_roles = [
        MatrixSourceTypeRoleV2::DeviceMatrixReference,
        MatrixSourceTypeRoleV2::Bf16Fragment,
        MatrixSourceTypeRoleV2::Bf16Fragment,
        MatrixSourceTypeRoleV2::F32Fragment,
        MatrixSourceTypeRoleV2::F32Fragment,
    ];
    let kernel_roles = [
        MatrixSourceTypeRoleV2::Bf16Fragment,
        MatrixSourceTypeRoleV2::Bf16Fragment,
        MatrixSourceTypeRoleV2::F32Fragment,
        MatrixSourceTypeRoleV2::Unit,
    ];
    let kernel_abi = import_matrix_fn_abi_facts(tcx, instance, &kernel_roles)?;
    let method_abi = import_matrix_fn_abi_facts(tcx, method_instance, &method_roles)?;
    let receiver_layout = method_abi.arguments[0].layout.clone();
    let lhs_layout = extract_general_layout(tcx, *lhs).map_err(|error| {
        MirImportError::new(format!("failed to retain matrix lhs rustc layout: {error}"))
    })?;
    let rhs_layout = extract_general_layout(tcx, *rhs).map_err(|error| {
        MirImportError::new(format!("failed to retain matrix rhs rustc layout: {error}"))
    })?;
    let accumulator_layout = extract_general_layout(tcx, *accumulator).map_err(|error| {
        MirImportError::new(format!(
            "failed to retain matrix accumulator rustc layout: {error}"
        ))
    })?;
    let result_layout = method_abi.result.layout.clone();
    let observed_source = MatrixObservedSourceAbiV2 {
        provider,
        definition_identities,
        receiver_layout,
        lhs_layout,
        rhs_layout,
        accumulator_layout,
        result_layout,
        kernel_abi,
        method_abi,
        kernel_source_structure: kernel_roles,
        method_source_structure: method_roles,
    };
    let evidence =
        MatrixFrontendAbiV2::from_observed(observed_source).map_err(MirImportError::new)?;
    evidence.validate().map_err(MirImportError::new)?;
    Ok(Some(evidence))
}

fn trusted_adt_item(tcx: TyCtxt<'_>, ty: Ty<'_>) -> Option<TrustedDeviceItem> {
    let TyKind::Adt(definition, _) = ty.kind() else {
        return None;
    };
    semantic_features::classify(tcx, definition.did())
        .map(SessionRecognizedSemanticItem::trusted_device_item)
}

pub(crate) fn observe_row_softmax_provider_authority_v1(
    tcx: TyCtxt<'_>,
) -> Result<RowSoftmaxProviderAuthorityV1, MirImportError> {
    let mut provider: Option<crate::trusted_device_items::ReviewedRowSoftmaxProviderDefinitionV1> =
        None;
    let mut provider_crate = None;
    let mut definition_identities = Vec::with_capacity(ROW_SOFTMAX_TRUSTED_ITEMS_V1.len());
    let mut source_identities = Vec::with_capacity(ROW_SOFTMAX_TRUSTED_ITEMS_V1.len());
    for item in ROW_SOFTMAX_TRUSTED_ITEMS_V1 {
        let definition = crate::trusted_device_items::definition(tcx, item).ok_or_else(|| {
            MirImportError::new(format!(
                "row-softmax provider omitted reviewed definition `{}`",
                item.canonical_path()
            ))
        })?;
        if provider_crate.is_some_and(|expected| expected != definition.krate) {
            return Err(MirImportError::new(format!(
                "row-softmax definition `{}` came from a different provider crate",
                item.canonical_path()
            )));
        }
        provider_crate.get_or_insert(definition.krate);
        let observation =
            crate::trusted_device_items::reviewed_row_softmax_provider_definition(tcx, definition)
                .map_err(MirImportError::new)?;
        if let Some(expected) = &provider
            && (expected.crate_name != observation.crate_name
                || expected.stable_crate_id != observation.stable_crate_id
                || expected.crate_hash != observation.crate_hash
                || expected.cargo_metadata_build_observation
                    != observation.cargo_metadata_build_observation)
        {
            return Err(MirImportError::new(format!(
                "row-softmax definition `{}` changed provider identity",
                item.canonical_path()
            )));
        }
        definition_identities.push(tcx.def_path_hash(definition).0.to_le_bytes());
        source_identities.push(observation.source_identity);
        provider.get_or_insert(observation);
    }
    let mut authority = RowSoftmaxProviderAuthorityV1 {
        provider: provider.expect("the reviewed row-softmax item set is nonempty"),
        definition_identities,
        source_identities,
        commitment: [0; 32],
    };
    authority.commitment = authority
        .canonical_commitment()
        .map_err(MirImportError::new)?;
    authority.validate().map_err(MirImportError::new)?;
    Ok(authority)
}

fn import_bf16_fragment_layout<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Result<(MatrixFragmentLayoutV1, MatrixBf16LayoutV1), MirImportError> {
    let facts = extract_general_layout(tcx, ty).map_err(|error| {
        MirImportError::new(format!(
            "failed to extract Bf16MfmaFragment rustc layout: {error}"
        ))
    })?;
    let (fragment, element) = import_fragment_container(
        &facts,
        "Bf16MfmaFragment",
        MatrixFragmentLayoutV1 {
            repr_c: true,
            size: 8,
            alignment: 2,
            field_count: 1,
            field_offset: 0,
            array_length: 4,
            array_stride: 2,
        },
    )?;
    let bf16 = import_bf16_layout(element)?;
    Ok((fragment, bf16))
}

fn import_f32_fragment_layout<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Result<MatrixFragmentLayoutV1, MirImportError> {
    let facts = extract_general_layout(tcx, ty).map_err(|error| {
        MirImportError::new(format!(
            "failed to extract F32AccumulatorFragment rustc layout: {error}"
        ))
    })?;
    let (fragment, element) = import_fragment_container(
        &facts,
        "F32AccumulatorFragment",
        MatrixFragmentLayoutV1 {
            repr_c: true,
            size: 16,
            alignment: 4,
            field_count: 1,
            field_offset: 0,
            array_length: 4,
            array_stride: 4,
        },
    )?;
    validate_scalar_layout(
        element,
        "F32AccumulatorFragment element",
        SourceScalarKind::Float { bits: 32 },
        ScalarPrimitiveFacts::Float { bits: 32 },
        4,
        4,
        u128::from(u32::MAX),
    )?;
    Ok(fragment)
}

fn import_fragment_container<'a>(
    facts: &'a TypeLayoutFacts,
    role: &str,
    expected: MatrixFragmentLayoutV1,
) -> Result<(MatrixFragmentLayoutV1, &'a TypeLayoutFacts), MirImportError> {
    let TypeLayoutKind::Adt(adt) = &facts.kind else {
        return Err(matrix_layout_error(role, "outer type is not an ADT"));
    };
    let [variant] = adt.variants.as_slice() else {
        return Err(matrix_layout_error(
            role,
            "outer type does not have one variant",
        ));
    };
    let [field] = variant.fields.as_slice() else {
        return Err(matrix_layout_error(
            role,
            "outer type does not have one field",
        ));
    };
    let TypeLayoutKind::Array(array) = &field.layout.kind else {
        return Err(matrix_layout_error(role, "field is not an array"));
    };
    let observed = MatrixFragmentLayoutV1 {
        repr_c: adt.representation.c,
        size: facts.size_bytes,
        alignment: facts.abi_alignment_bytes,
        field_count: u8::try_from(variant.fields.len()).unwrap_or(u8::MAX),
        field_offset: field.offset_bytes,
        array_length: array.length,
        array_stride: array.stride_bytes,
    };
    if adt.kind != AdtKind::Struct
        || adt.representation.transparent
        || adt.tag.is_some()
        || !matches!(
            facts.backend_representation,
            BackendRepresentationFacts::Memory
        )
        || !matches!(
            field.layout.backend_representation,
            BackendRepresentationFacts::Memory
        )
        || facts.uninhabited
        || observed != expected
    {
        return Err(matrix_layout_error(
            role,
            "repr, size, alignment, field offset, or array layout drifted",
        ));
    }
    Ok((observed, &array.element))
}

fn import_bf16_layout(facts: &TypeLayoutFacts) -> Result<MatrixBf16LayoutV1, MirImportError> {
    let role = "Bf16MfmaFragment inner Bf16";
    let TypeLayoutKind::Adt(adt) = &facts.kind else {
        return Err(matrix_layout_error(role, "Bf16 is not an ADT"));
    };
    let [variant] = adt.variants.as_slice() else {
        return Err(matrix_layout_error(role, "Bf16 does not have one variant"));
    };
    let [field] = variant.fields.as_slice() else {
        return Err(matrix_layout_error(role, "Bf16 does not have one field"));
    };
    if adt.kind != AdtKind::Struct
        || !adt.representation.transparent
        || adt.representation.c
        || adt.tag.is_some()
        || facts.size_bytes != 2
        || facts.abi_alignment_bytes != 2
        || field.offset_bytes != 0
    {
        return Err(matrix_layout_error(
            role,
            "transparent wrapper representation, size, alignment, or field offset drifted",
        ));
    }
    validate_scalar_layout(
        &field.layout,
        role,
        SourceScalarKind::UnsignedInteger { bits: 16 },
        ScalarPrimitiveFacts::Integer {
            bits: 16,
            signed: false,
        },
        2,
        2,
        u128::from(u16::MAX),
    )?;
    validate_scalar_layout(
        facts,
        role,
        SourceScalarKind::UnsignedInteger { bits: 16 },
        ScalarPrimitiveFacts::Integer {
            bits: 16,
            signed: false,
        },
        2,
        2,
        u128::from(u16::MAX),
    )?;
    Ok(MatrixBf16LayoutV1 {
        repr_transparent: adt.representation.transparent,
        size: facts.size_bytes,
        alignment: facts.abi_alignment_bytes,
        field_count: u8::try_from(variant.fields.len()).unwrap_or(u8::MAX),
        field_offset: field.offset_bytes,
        scalar_bits: 16,
        scalar_unsigned: true,
    })
}

fn validate_scalar_layout(
    facts: &TypeLayoutFacts,
    role: &str,
    source: SourceScalarKind,
    primitive: ScalarPrimitiveFacts,
    size: u64,
    alignment: u64,
    valid_range_end: u128,
) -> Result<(), MirImportError> {
    let BackendRepresentationFacts::Scalar(scalar) = facts.backend_representation else {
        return Err(matrix_layout_error(
            role,
            "backend representation is not scalar",
        ));
    };
    if facts.size_bytes != size
        || facts.abi_alignment_bytes != alignment
        || scalar.primitive != primitive
        || scalar.size_bytes != size
        || scalar.abi_alignment_bytes != alignment
        || !scalar.initialized
        || scalar.valid_range_start != 0
        || scalar.valid_range_end != valid_range_end
        || (!matches!(facts.kind, TypeLayoutKind::Adt(_))
            && facts.kind != TypeLayoutKind::Scalar(source))
    {
        return Err(matrix_layout_error(
            role,
            "source scalar or backend scalar ABI drifted",
        ));
    }
    Ok(())
}

fn import_matrix_rust_fn_abi<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    argument_types: &[Ty<'tcx>],
) -> Result<[MatrixRustPassModeV1; 3], MirImportError> {
    let query = TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty()));
    let abi = tcx.fn_abi_of_instance(query).map_err(|error| {
        MirImportError::new(format!(
            "failed to compute matrix kernel rustc FnAbi: {error:?}"
        ))
    })?;
    if abi.conv != CanonAbi::Rust
        || abi.c_variadic
        || abi.fixed_count != 3
        || abi.args.len() != 3
        || !matches!(abi.ret.mode, PassMode::Ignore)
        || abi.ret.layout.size.bytes() != 0
    {
        return Err(MirImportError::new(
            "matrix kernel rustc FnAbi header is not exact Rust(args=3)->unit",
        ));
    }
    for (index, (argument, expected_ty)) in abi.args.iter().zip(argument_types).enumerate() {
        if argument.layout.ty != *expected_ty {
            return Err(MirImportError::new(format!(
                "matrix kernel rustc FnAbi argument {index} type disagrees with MIR"
            )));
        }
    }
    let lhs = import_cast_i64_mode(&abi.args[0].mode, "lhs")?;
    let rhs = import_cast_i64_mode(&abi.args[1].mode, "rhs")?;
    let accumulator = match &abi.args[2].mode {
        PassMode::Indirect {
            attrs,
            meta_attrs: None,
            on_stack: false,
        } if attrs.pointee_size.bytes() == 16
            && attrs.pointee_align.is_some_and(|align| align.bytes() == 4) =>
        {
            MatrixRustPassModeV1::Indirect {
                pointee_size: attrs.pointee_size.bytes(),
                pointee_align: attrs
                    .pointee_align
                    .expect("guard requires accumulator pointee alignment")
                    .bytes(),
            }
        }
        _ => {
            return Err(MirImportError::new(
                "matrix accumulator rustc FnAbi must be indirect(size=16, align=4)",
            ));
        }
    };
    Ok([lhs, rhs, accumulator])
}

fn matrix_method_instance<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
) -> Result<Instance<'tcx>, MirImportError> {
    let mut matches = body.basic_blocks.iter().filter_map(|block| {
        let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
            return None;
        };
        let Operand::Constant(constant) = func else {
            return None;
        };
        let TyKind::FnDef(def_id, args) = constant.const_.ty().kind() else {
            return None;
        };
        let instance = Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, args)
            .ok()
            .flatten()?;
        (semantic_features::classify(tcx, instance.def_id())
            .map(SessionRecognizedSemanticItem::trusted_device_item)
            == Some(TrustedDeviceItem::DeviceMatrixMultiplyAccumulate))
        .then_some(instance)
    });
    let instance = matches.next().ok_or_else(|| {
        MirImportError::new(
            "matrix source ABI observation requires one resolved multiply_accumulate call",
        )
    })?;
    if matches.next().is_some() {
        return Err(MirImportError::new(
            "matrix source ABI observation supports exactly one multiply_accumulate call",
        ));
    }
    Ok(instance)
}

fn validate_matrix_method_fn_abi<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<(), MirImportError> {
    let query = TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty()));
    let abi = tcx.fn_abi_of_instance(query).map_err(|error| {
        MirImportError::new(format!(
            "failed to compute matrix method rustc FnAbi: {error:?}"
        ))
    })?;
    if abi.conv != CanonAbi::Rust || abi.c_variadic || abi.fixed_count != 4 || abi.args.len() != 4 {
        return Err(MirImportError::new(
            "matrix method rustc FnAbi header is not exact Rust(args=4)",
        ));
    }
    require_matrix_source_type(
        tcx,
        abi.args[0].layout.ty,
        MatrixSourceTypeRoleV2::DeviceMatrixReference,
    )?;
    require_matrix_source_type(
        tcx,
        abi.args[1].layout.ty,
        MatrixSourceTypeRoleV2::Bf16Fragment,
    )?;
    require_matrix_source_type(
        tcx,
        abi.args[2].layout.ty,
        MatrixSourceTypeRoleV2::Bf16Fragment,
    )?;
    require_matrix_source_type(
        tcx,
        abi.args[3].layout.ty,
        MatrixSourceTypeRoleV2::F32Fragment,
    )?;
    require_matrix_source_type(tcx, abi.ret.layout.ty, MatrixSourceTypeRoleV2::F32Fragment)?;
    if !matches!(abi.args[0].mode, PassMode::Direct(_)) {
        return Err(MirImportError::new(
            "matrix method receiver rustc FnAbi must be direct",
        ));
    }
    import_cast_i64_mode(&abi.args[1].mode, "method lhs")?;
    import_cast_i64_mode(&abi.args[2].mode, "method rhs")?;
    require_indirect_fragment_mode(&abi.args[3].mode, "method accumulator")?;
    require_indirect_fragment_mode(&abi.ret.mode, "method result")?;
    Ok(())
}

fn require_indirect_fragment_mode(mode: &PassMode, role: &str) -> Result<(), MirImportError> {
    match mode {
        PassMode::Indirect {
            attrs,
            meta_attrs: None,
            on_stack: false,
        } if attrs.pointee_size.bytes() == 16
            && attrs.pointee_align.is_some_and(|align| align.bytes() == 4) =>
        {
            Ok(())
        }
        _ => Err(MirImportError::new(format!(
            "matrix {role} rustc FnAbi must be indirect(size=16, align=4)"
        ))),
    }
}

fn import_matrix_fn_abi_facts<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    roles: &[MatrixSourceTypeRoleV2],
) -> Result<MatrixFnAbiFactsV2, MirImportError> {
    let query = TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty()));
    let abi = tcx.fn_abi_of_instance(query).map_err(|error| {
        MirImportError::new(format!("failed to retain matrix rustc FnAbi: {error:?}"))
    })?;
    let Some((result_role, argument_roles)) = roles.split_last() else {
        return Err(MirImportError::new("matrix source ABI role list is empty"));
    };
    if abi.conv != CanonAbi::Rust
        || abi.c_variadic
        || abi.args.len() != argument_roles.len()
        || usize::try_from(abi.fixed_count).ok() != Some(argument_roles.len())
    {
        return Err(MirImportError::new(
            "matrix rustc FnAbi disagrees with the exact source parameter structure",
        ));
    }
    let arguments = abi
        .args
        .iter()
        .zip(argument_roles)
        .map(|(argument, role)| import_matrix_arg_abi(tcx, argument, *role))
        .collect::<Result<Vec<_>, _>>()?;
    let result = import_matrix_arg_abi(tcx, &abi.ret, *result_role)?;
    Ok(MatrixFnAbiFactsV2 {
        convention: 1,
        c_variadic: abi.c_variadic,
        fixed_count: abi.fixed_count,
        can_unwind: abi.can_unwind,
        arguments,
        result,
    })
}

fn import_matrix_arg_abi<'tcx>(
    tcx: TyCtxt<'tcx>,
    argument: &rustc_target::callconv::ArgAbi<'tcx, Ty<'tcx>>,
    role: MatrixSourceTypeRoleV2,
) -> Result<MatrixArgAbiFactsV2, MirImportError> {
    require_matrix_source_type(tcx, argument.layout.ty, role)?;
    let layout = extract_general_layout(tcx, argument.layout.ty).map_err(|error| {
        MirImportError::new(format!(
            "failed to retain matrix rustc FnAbi layout: {error}"
        ))
    })?;
    Ok(MatrixArgAbiFactsV2 {
        role,
        layout,
        mode: import_matrix_pass_mode(&argument.mode),
    })
}

fn require_matrix_source_type(
    tcx: TyCtxt<'_>,
    ty: Ty<'_>,
    role: MatrixSourceTypeRoleV2,
) -> Result<(), MirImportError> {
    let exact = match role {
        MatrixSourceTypeRoleV2::Unit => {
            matches!(ty.kind(), TyKind::Tuple(fields) if fields.is_empty())
        }
        MatrixSourceTypeRoleV2::DeviceMatrixReference => matches!(
            ty.kind(),
            TyKind::Ref(_, pointee, Mutability::Not)
                if trusted_adt_item(tcx, *pointee) == Some(TrustedDeviceItem::DeviceMatrix)
        ),
        MatrixSourceTypeRoleV2::Bf16Fragment => {
            trusted_adt_item(tcx, ty) == Some(TrustedDeviceItem::Bf16MfmaFragment)
        }
        MatrixSourceTypeRoleV2::F32Fragment => {
            trusted_adt_item(tcx, ty) == Some(TrustedDeviceItem::F32AccumulatorFragment)
        }
    };
    if exact {
        Ok(())
    } else {
        Err(MirImportError::new(format!(
            "matrix rustc FnAbi source type does not match role {role:?}"
        )))
    }
}

fn import_matrix_pass_mode(mode: &PassMode) -> MatrixPassModeV2 {
    match mode {
        PassMode::Ignore => MatrixPassModeV2::Ignore,
        PassMode::Direct(attrs) => MatrixPassModeV2::Direct(import_matrix_attrs(*attrs)),
        PassMode::Pair(first, second) => {
            MatrixPassModeV2::Pair(import_matrix_attrs(*first), import_matrix_attrs(*second))
        }
        PassMode::Cast { pad_i32, cast } => MatrixPassModeV2::Cast {
            pad_i32: *pad_i32,
            prefix: Box::new(cast.prefix.map(|reg| reg.map(import_matrix_reg))),
            rest_offset: cast.rest_offset.map(|offset| offset.bytes()),
            rest: import_matrix_reg(cast.rest.unit),
            rest_total: cast.rest.total.bytes(),
            rest_consecutive: cast.rest.is_consecutive,
            attrs: import_matrix_attrs(cast.attrs),
        },
        PassMode::Indirect {
            attrs,
            meta_attrs,
            on_stack,
        } => MatrixPassModeV2::Indirect {
            attrs: import_matrix_attrs(*attrs),
            meta_attrs: meta_attrs.map(import_matrix_attrs),
            on_stack: *on_stack,
        },
    }
}

fn import_matrix_attrs(attrs: ArgAttributes) -> MatrixArgAttributesV2 {
    MatrixArgAttributesV2 {
        regular: attrs.regular.bits(),
        extension: match attrs.arg_ext {
            ArgExtension::None => 0,
            ArgExtension::Zext => 1,
            ArgExtension::Sext => 2,
        },
        pointee_size: attrs.pointee_size.bytes(),
        pointee_alignment: attrs.pointee_align.map(|alignment| alignment.bytes()),
    }
}

fn import_matrix_reg(reg: Reg) -> MatrixRegV2 {
    MatrixRegV2 {
        kind: match reg.kind {
            RegKind::Integer => 0,
            RegKind::Float => 1,
            RegKind::Vector => 2,
        },
        size: reg.size.bytes(),
    }
}

fn import_cast_i64_mode(
    mode: &PassMode,
    role: &str,
) -> Result<MatrixRustPassModeV1, MirImportError> {
    let PassMode::Cast {
        pad_i32: false,
        cast,
    } = mode
    else {
        return Err(MirImportError::new(format!(
            "matrix {role} rustc FnAbi must be Cast(i64)"
        )));
    };
    if cast.prefix.iter().any(Option::is_some)
        || cast.rest_offset.is_some()
        || cast.rest.unit.kind != RegKind::Integer
        || cast.rest.unit.size.bytes() != 8
        || cast.rest.total.bytes() != 8
        || cast.rest.is_consecutive
    {
        return Err(MirImportError::new(format!(
            "matrix {role} rustc FnAbi cast target is not exact i64"
        )));
    }
    Ok(MatrixRustPassModeV1::CastI64)
}

fn matrix_layout_error(role: &str, detail: &str) -> MirImportError {
    MirImportError::new(format!("matrix frontend {role} layout rejected: {detail}"))
}

pub fn import_collection<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
) -> Result<MirModule, MirImportError> {
    let compiler_ffi_imports = CompilerFfiImports::from_collection(tcx, collection)?;
    let mut functions = Vec::new();
    for function in &collection.functions {
        let def_id = function.instance.def_id();
        if !tcx.is_mir_available(def_id) {
            continue;
        }

        let body = tcx.instance_mir(function.instance.def);
        let dead_branches = function.dead_branches.as_ref().ok_or_else(|| {
            MirImportError::new(format!(
                "collected function '{}' has no compiler dead-branch observation",
                tcx.def_path_str(def_id)
            ))
        })?;
        dead_branches
            .validate_against(tcx, function.instance, body)
            .map_err(|error| {
                MirImportError::new(format!(
                    "dead-branch evidence rejected before MIR import for '{}': {error}",
                    tcx.def_path_str(def_id)
                ))
            })?;
        let rust_path = imported_rust_path(tcx, def_id);
        let matrix_frontend_abi =
            if function.role == crate::collector::CollectedFunctionRole::KernelEntry {
                import_matrix_source_abi_v2(tcx, function.instance, body)?
            } else {
                None
            };
        functions.push(import_body(
            MirBodyImportContext {
                tcx,
                body,
                compiler_ffi_imports: &compiler_ffi_imports,
                dead_branches,
            },
            function.export_name.clone(),
            rust_path,
            import_function_kind(function.role),
            MirKernelMetadata {
                typed_profile: import_kernel_profile(function.typed_profile),
                frontend_contract: function.frontend_contract.clone(),
                matrix_frontend_abi,
            },
        ));
    }

    MirModule::from_functions_v1(functions)
}

fn imported_rust_path(tcx: TyCtxt<'_>, def_id: rustc_hir::def_id::DefId) -> String {
    let path = tcx.def_path_str(def_id);
    if def_id.krate == LOCAL_CRATE {
        format!("{}::{path}", tcx.crate_name(LOCAL_CRATE))
    } else {
        path
    }
}

fn import_function_kind(role: crate::collector::CollectedFunctionRole) -> MirFunctionKind {
    match role {
        crate::collector::CollectedFunctionRole::KernelEntry => MirFunctionKind::KernelEntry,
        crate::collector::CollectedFunctionRole::InternalHelper => MirFunctionKind::InternalHelper,
        crate::collector::CollectedFunctionRole::DeviceFfiExport => {
            MirFunctionKind::DeviceFfiExport
        }
    }
}

fn import_kernel_profile(
    profile: Option<crate::collector::TypedKernelProfile>,
) -> Option<MirKernelProfile> {
    profile.map(|profile| match profile {
        crate::collector::TypedKernelProfile::VecAddRustcLayoutV2 => {
            MirKernelProfile::VecAddRustcLayoutV2
        }
        crate::collector::TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. } => {
            MirKernelProfile::GeneralScalarSliceRustcLayoutV3
        }
    })
}

impl MirModule {
    fn from_functions_v1(mut functions: Vec<MirFunction>) -> Result<Self, MirImportError> {
        if functions.is_empty() {
            return Err(MirImportError::new(
                "MIR module contains no imported functions",
            ));
        }

        let mut exports = BTreeMap::new();
        let mut kernel_paths = BTreeMap::new();
        let mut identities = BTreeSet::new();
        let mut kernel_count = 0_usize;
        for function in &functions {
            function.validate_identity_fields_v1()?;
            if let Some(previous) = exports.insert(&function.export_name, &function.rust_path) {
                return Err(MirImportError::new(format!(
                    "duplicate function export `{}` for source functions `{previous}` and `{}`",
                    function.export_name, function.rust_path
                )));
            }

            let identity = function.source_identity_v1()?;
            if !identities.insert(identity) {
                return Err(MirImportError::new(format!(
                    "duplicate MIR function identity {} for `{}`",
                    function_identity_hex_v1(identity),
                    function.rust_path
                )));
            }

            if function.kind == MirFunctionKind::KernelEntry {
                kernel_count += 1;
                if let Some(previous) =
                    kernel_paths.insert(&function.rust_path, &function.export_name)
                {
                    return Err(MirImportError::new(format!(
                        "ambiguous kernel roots `{previous}` and `{}` select the same source function `{}`",
                        function.export_name, function.rust_path
                    )));
                }
            }
        }
        if kernel_count == 0 {
            return Err(MirImportError::new("MIR module contains no kernel root"));
        }

        functions.sort_by(|lhs, rhs| {
            lhs.kind
                .canonical_order_v1()
                .cmp(&rhs.kind.canonical_order_v1())
                .then_with(|| {
                    lhs.source_identity_bytes_v1()
                        .cmp(&rhs.source_identity_bytes_v1())
                })
                .then_with(|| lhs.export_name.cmp(&rhs.export_name))
                .then_with(|| lhs.rust_path.cmp(&rhs.rust_path))
        });
        Ok(Self { functions })
    }

    pub fn summary(&self) -> String {
        let record_count = self.dialect_records().len();
        let mut output = format!(
            "\n=== fe2o3 MIR import scaffold ({}, {record_count} op records) ===\n",
            MirOp::Module.name(),
        );
        for function in &self.functions {
            let kind = match function.kind {
                MirFunctionKind::KernelEntry => "kernel-entry",
                MirFunctionKind::InternalHelper => "internal-helper",
                MirFunctionKind::DeviceFfiExport => "device-ffi-export",
            };
            let _ = writeln!(
                output,
                "  [{kind}] {} ({})",
                function.export_name,
                MirOp::Func.name()
            );
            let _ = writeln!(output, "      path: {}", function.rust_path);
            if let Ok(identity) = function.source_identity_v1() {
                let _ = writeln!(
                    output,
                    "      source identity v1: {}",
                    function_identity_hex_v1(identity)
                );
            }
            let _ = writeln!(
                output,
                "      MIR:  {} bb, {} locals, {} args",
                function.blocks.len(),
                function.local_count,
                function.arg_count
            );
            for local in function
                .locals
                .iter()
                .filter(|local| local.role != MirLocalRole::Temp)
            {
                let role = match local.role {
                    MirLocalRole::Return => "return",
                    MirLocalRole::Arg => "arg",
                    MirLocalRole::Temp => "temp",
                };
                let _ = writeln!(
                    output,
                    "      local{}: {role} {} ({})",
                    local.index,
                    local.ty.kind.name(),
                    local.ty.rust
                );
            }
            for block in &function.blocks {
                let terminator = block
                    .terminator
                    .as_ref()
                    .map(|terminator| terminator.kind.summary())
                    .unwrap_or("missing terminator".to_string());
                let _ = writeln!(
                    output,
                    "      bb{} ({}): {} stmt(s), {terminator}",
                    block.index,
                    MirOp::Block.name(),
                    block.statements.len()
                );
                for statement in &block.statements {
                    if let Some(summary) = statement.summary() {
                        let _ = writeln!(output, "          {summary}");
                    }
                }
            }
        }
        output.push_str("===================================\n");
        output
    }

    pub fn dialect_records(&self) -> Vec<MirOpRecord> {
        let source_identities = self.source_identities_by_path_v1();
        let kernel_count = self
            .functions
            .iter()
            .filter(|function| function.kind == MirFunctionKind::KernelEntry)
            .count();
        let helper_count = self
            .functions
            .iter()
            .filter(|function| function.kind == MirFunctionKind::InternalHelper)
            .count();
        let mut records = vec![
            MirOpRecord::new(MirOp::Module)
                .with_attr(MirAttr::usize("functions", self.functions.len()))
                .with_attr(MirAttr::usize("kernel_roots", kernel_count))
                .with_attr(MirAttr::usize("internal_helpers", helper_count)),
        ];

        for function in &self.functions {
            // `kind` is a V1 compatibility field consumed by record_lowering.
            // The closed role remains authoritative on `MirFunction::kind`.
            let kind = match function.kind {
                MirFunctionKind::KernelEntry => "kernel",
                MirFunctionKind::InternalHelper | MirFunctionKind::DeviceFfiExport => "device",
            };
            let mut function_record = MirOpRecord::new(MirOp::Func)
                .with_attr(MirAttr::string("symbol", &function.export_name))
                .with_attr(MirAttr::string("kind", kind))
                .with_attr(MirAttr::string("rust_path", &function.rust_path))
                .with_attr(MirAttr::usize("args", function.arg_count))
                .with_attr(MirAttr::usize("locals", function.local_count))
                .with_attr(MirAttr::usize("blocks", function.blocks.len()));
            if let Ok(identity) = function.source_identity_v1() {
                function_record.attrs.push(MirAttr::string(
                    "source_identity_v1",
                    function_identity_hex_v1(identity),
                ));
            }
            records.push(function_record);

            for local in &function.locals {
                let role = match local.role {
                    MirLocalRole::Return => "return",
                    MirLocalRole::Arg => "arg",
                    MirLocalRole::Temp => "temp",
                };
                let op = match local.role {
                    MirLocalRole::Arg => MirOp::Arg,
                    MirLocalRole::Return | MirLocalRole::Temp => MirOp::Local,
                };
                records.push(
                    MirOpRecord::new(op)
                        .with_attr(MirAttr::string("function", &function.export_name))
                        .with_attr(MirAttr::usize("index", local.index))
                        .with_attr(MirAttr::string("role", role))
                        .with_attr(MirAttr::string("type", local.ty.kind.name()))
                        .with_attr(MirAttr::string("rust_type", &local.ty.rust)),
                );
            }

            for block in &function.blocks {
                records.push(
                    MirOpRecord::new(MirOp::Block)
                        .with_attr(MirAttr::string("function", &function.export_name))
                        .with_attr(MirAttr::usize("index", block.index))
                        .with_attr(MirAttr::usize("statements", block.statements.len())),
                );

                for statement in &block.statements {
                    records.push(statement.record(&function.export_name, block.index));
                    if let Some(record) =
                        statement.lowering_record(&function.export_name, block.index)
                    {
                        records.push(record);
                    }
                }

                if let Some(terminator) = &block.terminator {
                    records.push(terminator.kind.record(
                        &function.export_name,
                        block.index,
                        &source_identities,
                    ));
                }
            }
        }

        records
    }

    fn source_identities_by_path_v1(&self) -> BTreeMap<&str, Option<FunctionIdentityV1>> {
        let mut identities = BTreeMap::new();
        for function in &self.functions {
            let identity = function.source_identity_v1().ok();
            identities
                .entry(function.rust_path.as_str())
                .and_modify(|existing| *existing = None)
                .or_insert(identity);
        }
        identities
    }
}

impl MirFunction {
    fn validate_identity_fields_v1(&self) -> Result<(), MirImportError> {
        for (field, value) in [
            ("function export", self.export_name.as_str()),
            ("source function path", self.rust_path.as_str()),
        ] {
            if value.is_empty() {
                return Err(MirImportError::new(format!("{field} must not be empty")));
            }
            if value.chars().any(char::is_control) {
                return Err(MirImportError::new(format!(
                    "{field} `{value:?}` contains control characters"
                )));
            }
        }
        if self.kind != MirFunctionKind::KernelEntry && self.typed_profile.is_some() {
            return Err(MirImportError::new(format!(
                "non-kernel function `{}` carries a typed kernel profile",
                self.rust_path
            )));
        }
        if self.kind != MirFunctionKind::KernelEntry && self.matrix_frontend_abi.is_some() {
            return Err(MirImportError::new(format!(
                "non-kernel function `{}` carries matrix frontend ABI evidence",
                self.rust_path
            )));
        }
        if let Some(evidence) = &self.matrix_frontend_abi {
            evidence.validate().map_err(MirImportError::new)?;
        }
        Ok(())
    }

    fn source_identity_v1(&self) -> Result<FunctionIdentityV1, MirImportError> {
        FunctionIdentityV1::new(self.source_identity_bytes_v1()).map_err(|error| {
            MirImportError::new(format!(
                "invalid source identity for `{}`: {error}",
                self.rust_path
            ))
        })
    }

    fn source_identity_bytes_v1(&self) -> [u8; 32] {
        let mut digest = Sha256::new();
        append_identity_field_v1(&mut digest, MIR_FUNCTION_IDENTITY_DOMAIN_V1);
        append_identity_field_v1(&mut digest, self.rust_path.as_bytes());
        append_identity_field_v1(&mut digest, self.export_name.as_bytes());
        digest.finalize().into()
    }
}

impl MirFunctionKind {
    const fn canonical_order_v1(self) -> u8 {
        match self {
            Self::KernelEntry => 0,
            Self::InternalHelper => 1,
            Self::DeviceFfiExport => 2,
        }
    }
}

fn append_identity_field_v1(digest: &mut Sha256, field: &[u8]) {
    digest.update(u64::try_from(field.len()).unwrap_or(u64::MAX).to_le_bytes());
    digest.update(field);
}

fn function_identity_hex_v1(identity: FunctionIdentityV1) -> String {
    let mut encoded = String::with_capacity(64);
    for byte in identity.as_bytes() {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

#[allow(dead_code)]
impl MirModule {
    /// Computes the portable semantic identity of one kernel and its reachable
    /// local helper closure.
    ///
    /// Internal calls are resolved through compiler-authenticated collection
    /// paths, then encoded by stable export identity. The paths themselves are
    /// never included in the digest. This function produces no lowering or
    /// artifact authority.
    pub(crate) fn portable_semantic_digest_v2(
        &self,
        inputs: MirSemanticAdmissionInputsV2<'_>,
    ) -> Result<PortableMirSemanticDigestV2, MirImportError> {
        let (functions, functions_by_path) =
            self.portable_semantic_closure_v2(inputs.kernel_export_name)?;
        let mut encoder = PortableMirSemanticEncoderV2::new();
        encoder.target(inputs.target)?;
        encoder.abi(inputs.abi)?;
        encoder.launch(inputs.launch);
        encoder.text(inputs.kernel_export_name)?;
        encoder.len(functions.len())?;
        for function in functions {
            encoder.function(function, &functions_by_path)?;
        }
        Ok(encoder.finish())
    }

    /// Computes the path-independent semantic identity used by the collected
    /// scalar-control-flow V2 pilot.
    ///
    /// Collection and target admission remain separate authority checks. This
    /// digest binds the complete reachable portable-MIR closure while omitting
    /// checkout paths, source diagnostics, and build observations.
    pub(crate) fn collected_scalar_control_flow_digest_v2(
        &self,
        kernel_export_name: &str,
    ) -> Result<PortableMirSemanticDigestV2, MirImportError> {
        let (functions, functions_by_path) =
            self.portable_semantic_closure_v2(kernel_export_name)?;
        let mut encoder = PortableMirSemanticEncoderV2::new();
        encoder.text("fe2o3.collected-scalar-control-flow.v2")?;
        encoder.text(kernel_export_name)?;
        encoder.len(functions.len())?;
        for function in functions {
            encoder.function(function, &functions_by_path)?;
        }
        Ok(encoder.finish())
    }

    fn portable_semantic_closure_v2<'a>(
        &'a self,
        kernel_export_name: &str,
    ) -> Result<(Vec<&'a MirFunction>, BTreeMap<&'a str, &'a MirFunction>), MirImportError> {
        let mut functions_by_export = BTreeMap::new();
        let mut functions_by_path = BTreeMap::new();
        for function in &self.functions {
            if function.export_name.is_empty() {
                return Err(MirImportError::new(
                    "portable MIR function export must not be empty",
                ));
            }
            if functions_by_export
                .insert(function.export_name.as_str(), function)
                .is_some()
            {
                return Err(MirImportError::new(format!(
                    "portable MIR contains duplicate export `{}`",
                    function.export_name
                )));
            }
            if functions_by_path
                .insert(function.rust_path.as_str(), function)
                .is_some()
            {
                return Err(MirImportError::new(format!(
                    "portable MIR contains duplicate compiler path `{}`",
                    function.rust_path
                )));
            }
        }

        let root = functions_by_export
            .get(kernel_export_name)
            .copied()
            .ok_or_else(|| {
                MirImportError::new(format!(
                    "portable MIR kernel export `{kernel_export_name}` is absent"
                ))
            })?;
        if root.kind != MirFunctionKind::KernelEntry {
            return Err(MirImportError::new(format!(
                "portable MIR export `{kernel_export_name}` is not a kernel root"
            )));
        }

        let mut pending = vec![root];
        let mut reachable = BTreeMap::new();
        while let Some(function) = pending.pop() {
            let stable_key = (
                function.kind.canonical_order_v1(),
                function.export_name.as_str(),
            );
            if reachable.insert(stable_key, function).is_some() {
                continue;
            }
            for block in &function.blocks {
                let Some(MirTerminator {
                    kind:
                        MirTerminatorKind::Call {
                            callee: Some(callee),
                            ..
                        },
                    ..
                }) = &block.terminator
                else {
                    continue;
                };
                if let MirCalleeIdentity::Untrusted(path) = &callee.identity {
                    let target =
                        functions_by_path
                            .get(path.as_str())
                            .copied()
                            .ok_or_else(|| {
                                MirImportError::new(format!(
                                    "portable MIR cannot normalize unresolved callee `{path}`"
                                ))
                            })?;
                    pending.push(target);
                }
            }
        }

        Ok((reachable.into_values().collect(), functions_by_path))
    }
}

#[allow(dead_code)]
struct PortableMirSemanticEncoderV2 {
    digest: Sha256,
}

#[allow(dead_code)]
impl PortableMirSemanticEncoderV2 {
    fn new() -> Self {
        let mut digest = Sha256::new();
        digest.update(PORTABLE_MIR_SEMANTIC_DOMAIN_V2);
        Self { digest }
    }

    fn finish(self) -> PortableMirSemanticDigestV2 {
        PortableMirSemanticDigestV2(self.digest.finalize().into())
    }

    fn tag(&mut self, value: u8) {
        self.digest.update([value]);
    }

    fn boolean(&mut self, value: bool) {
        self.tag(u8::from(value));
    }

    fn usize(&mut self, value: usize) -> Result<(), MirImportError> {
        let value = u64::try_from(value)
            .map_err(|_| MirImportError::new("portable MIR index cannot be represented as u64"))?;
        self.u64(value);
        Ok(())
    }

    fn len(&mut self, value: usize) -> Result<(), MirImportError> {
        self.usize(value)
    }

    fn u16(&mut self, value: u16) {
        self.digest.update(value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.digest.update(value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.digest.update(value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.digest.update(value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.digest.update(value.to_le_bytes());
    }

    fn i64(&mut self, value: i64) {
        self.digest.update(value.to_le_bytes());
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), MirImportError> {
        self.len(value.len())?;
        self.digest.update(value);
        Ok(())
    }

    fn text(&mut self, value: &str) -> Result<(), MirImportError> {
        self.bytes(value.as_bytes())
    }

    fn target(&mut self, target: &TargetIdentity) -> Result<(), MirImportError> {
        self.text(target.triple().as_str())?;
        self.text(target.architecture().as_str())?;
        self.pointer_width(target.pointer_width());
        self.tag(match target.endianness() {
            Endianness::Little => 0,
            Endianness::Big => 1,
        });
        self.len(target.capabilities().len())?;
        for capability in target.capabilities() {
            self.tag(match capability {
                Capability::Subgroup => 0,
                Capability::Ballot => 1,
                Capability::Shuffle => 2,
                Capability::WorkgroupMemory => 3,
                Capability::MatrixMultiply => 4,
                Capability::AsyncCopy => 5,
                Capability::Atomics => 6,
                Capability::AmdWave => 7,
                Capability::AmdMfma => 8,
                Capability::AmdWmma => 9,
                Capability::AmdDsPermute => 10,
            });
        }
        Ok(())
    }

    fn pointer_width(&mut self, value: PointerWidth) {
        self.tag(match value {
            PointerWidth::Bits32 => 0,
            PointerWidth::Bits64 => 1,
        });
    }

    fn abi(&mut self, abi: &AbiLayout) -> Result<(), MirImportError> {
        self.u64(abi.size());
        self.u32(abi.alignment());
        self.pointer_width(abi.pointer_width());
        self.len(abi.fields().len())?;
        for field in abi.fields() {
            self.text(field.name().as_str())?;
            self.u64(field.offset());
            self.u64(field.size());
            self.u32(field.alignment());
            self.abi_kind(field.kind());
            self.tag(match field.mutability() {
                KernelMutability::Immutable => 0,
                KernelMutability::Mutable => 1,
            });
            self.tag(match field.access() {
                KernelAccess::ByValue => 0,
                KernelAccess::ReadOnly => 1,
                KernelAccess::WriteOnly => 2,
                KernelAccess::ReadWrite => 3,
            });
            self.tag(match field.address_space() {
                KernelAddressSpace::Value => 0,
                KernelAddressSpace::Global => 1,
                KernelAddressSpace::Constant => 2,
                KernelAddressSpace::Workgroup => 3,
                KernelAddressSpace::Private => 4,
                KernelAddressSpace::Generic => 5,
            });
            self.tag(match field.ownership() {
                ArgumentOwnership::ByValue => 0,
                ArgumentOwnership::SharedBorrow => 1,
                ArgumentOwnership::UniqueBorrow => 2,
                ArgumentOwnership::RawPointer => 3,
            });
            self.tag(match field.alias_class() {
                AliasClass::Value => 0,
                AliasClass::SharedReadOnly => 1,
                AliasClass::Exclusive => 2,
                AliasClass::SharedAtomic => 3,
                AliasClass::Unrestricted => 4,
            });
            // Opaque rustc type/layout identity bytes are build observations.
            // Portable MIR types and physical ABI shape carry the stable policy.
        }
        Ok(())
    }

    fn abi_kind(&mut self, kind: KernelAbiKind) {
        match kind {
            KernelAbiKind::Scalar(scalar) => {
                self.tag(0);
                self.tag(match scalar {
                    ScalarType::I8 => 0,
                    ScalarType::U8 => 1,
                    ScalarType::I16 => 2,
                    ScalarType::U16 => 3,
                    ScalarType::I32 => 4,
                    ScalarType::U32 => 5,
                    ScalarType::I64 => 6,
                    ScalarType::U64 => 7,
                    ScalarType::F16 => 8,
                    ScalarType::F32 => 9,
                    ScalarType::F64 => 10,
                });
            }
            KernelAbiKind::Pointer {
                pointee_size,
                pointee_alignment,
            } => {
                self.tag(1);
                self.u64(pointee_size);
                self.u32(pointee_alignment);
            }
            KernelAbiKind::Slice {
                element_size,
                element_alignment,
            } => {
                self.tag(2);
                self.u64(element_size);
                self.u32(element_alignment);
            }
        }
    }

    fn launch(&mut self, launch: &LaunchContract) {
        self.tag(launch.rank());
        match launch.block_size() {
            BlockSize::Any => self.tag(0),
            BlockSize::Exact(dimensions) => {
                self.tag(1);
                self.dimensions(dimensions);
            }
            BlockSize::AtMost(dimensions) => {
                self.tag(2);
                self.dimensions(dimensions);
            }
        }
        self.dimensions(launch.max_grid());
        self.u32(launch.static_shared_memory_bytes());
        self.u32(launch.max_dynamic_shared_memory_bytes());
    }

    fn dimensions(&mut self, dimensions: fe2o3_artifacts::Dimensions) {
        self.u32(dimensions.x());
        self.u32(dimensions.y());
        self.u32(dimensions.z());
    }

    fn function(
        &mut self,
        function: &MirFunction,
        functions_by_path: &BTreeMap<&str, &MirFunction>,
    ) -> Result<(), MirImportError> {
        self.text(&function.export_name)?;
        self.tag(function.kind.canonical_order_v1());
        self.kernel_profile(function.typed_profile);
        if let Some(evidence) = &function.matrix_frontend_abi {
            evidence.validate().map_err(MirImportError::new)?;
            // Rustc provider and ABI build observations are retained on the
            // imported function and Kernel IR operation, not folded into the
            // portable semantic policy identity.
            self.text("fe2o3.matrix-source-abi-required.v2")?;
        }
        self.usize(function.arg_count)?;
        self.usize(function.local_count)?;
        self.len(function.locals.len())?;
        for local in &function.locals {
            self.usize(local.index)?;
            self.tag(match local.role {
                MirLocalRole::Return => 0,
                MirLocalRole::Arg => 1,
                MirLocalRole::Temp => 2,
            });
            self.imported_type(&local.ty, 0)?;
        }
        self.len(function.blocks.len())?;
        for block in &function.blocks {
            self.block(block, functions_by_path)?;
        }
        // rust_path, frontend-contract compiler observations, and all source
        // diagnostics are intentionally absent from this transcript.
        Ok(())
    }

    fn kernel_profile(&mut self, profile: Option<MirKernelProfile>) {
        self.tag(match profile {
            None => 0,
            Some(MirKernelProfile::VecAddRustcLayoutV2) => 1,
            Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3) => 2,
        });
    }

    fn imported_type(&mut self, ty: &MirImportedType, depth: usize) -> Result<(), MirImportError> {
        self.tag(match ty.kind {
            MirType::I1 => 0,
            MirType::I32 => 1,
            MirType::I64 => 2,
            MirType::USize => 3,
            MirType::F32 => 4,
            MirType::F64 => 5,
            MirType::Ptr => 6,
            MirType::Slice => 7,
            MirType::DisjointSlice => 8,
            MirType::Unit => 9,
            MirType::Unknown => 10,
        });
        self.type_shape(&ty.shape, depth)
    }

    fn type_shape(&mut self, shape: &MirTypeShape, depth: usize) -> Result<(), MirImportError> {
        if depth >= MAX_PORTABLE_MIR_TYPE_DEPTH_V2 {
            return Err(MirImportError::new(
                "portable MIR type exceeds the semantic depth bound",
            ));
        }
        match shape {
            MirTypeShape::Unit => self.tag(0),
            MirTypeShape::Bool => self.tag(1),
            MirTypeShape::I32 => self.tag(2),
            MirTypeShape::U32 => self.tag(3),
            MirTypeShape::I64 => self.tag(4),
            MirTypeShape::ISize => self.tag(5),
            MirTypeShape::USize => self.tag(6),
            MirTypeShape::F32 => self.tag(7),
            MirTypeShape::F64 => self.tag(8),
            MirTypeShape::F16 => self.tag(9),
            MirTypeShape::Bf16 => self.tag(10),
            MirTypeShape::Bf16x2 => self.tag(11),
            MirTypeShape::DeviceMath => self.tag(12),
            MirTypeShape::Slice { element, mutable } => {
                self.tag(13);
                self.boolean(*mutable);
                self.type_shape(element, depth + 1)?;
            }
            MirTypeShape::DisjointSlice { element } => {
                self.tag(14);
                self.type_shape(element, depth + 1)?;
            }
            MirTypeShape::Reference { pointee, mutable } => {
                self.tag(15);
                self.boolean(*mutable);
                self.type_shape(pointee, depth + 1)?;
            }
            MirTypeShape::RawPointer { pointee, mutable } => {
                self.tag(16);
                self.boolean(*mutable);
                self.type_shape(pointee, depth + 1)?;
            }
            MirTypeShape::Adt { identity } => {
                self.tag(17);
                self.text(identity)?;
            }
            MirTypeShape::Tuple(fields) => {
                self.tag(18);
                self.len(fields.len())?;
                for field in fields {
                    self.type_shape(field, depth + 1)?;
                }
            }
            MirTypeShape::Unknown => self.tag(19),
        }
        Ok(())
    }

    fn block(
        &mut self,
        block: &MirBlock,
        functions_by_path: &BTreeMap<&str, &MirFunction>,
    ) -> Result<(), MirImportError> {
        self.usize(block.index)?;
        self.len(block.statements.len())?;
        for statement in &block.statements {
            self.statement(statement)?;
        }
        match &block.terminator {
            None => self.tag(0),
            Some(terminator) => {
                self.tag(1);
                self.terminator(&terminator.kind, functions_by_path)?;
            }
        }
        Ok(())
    }

    fn statement(&mut self, statement: &MirStatement) -> Result<(), MirImportError> {
        self.usize(statement.index)?;
        self.tag(match statement.kind {
            MirStatementKind::Assign => 0,
            MirStatementKind::StorageLive => 1,
            MirStatementKind::StorageDead => 2,
            MirStatementKind::SetDiscriminant => 3,
            MirStatementKind::Intrinsic => 4,
            MirStatementKind::CopyNonOverlapping => 5,
            MirStatementKind::Retag => 6,
            MirStatementKind::Coverage => 7,
            MirStatementKind::Nop => 8,
            MirStatementKind::Other => 9,
        });
        self.optional_place(statement.destination.as_ref())?;
        self.len(statement.operands.len())?;
        for operand in &statement.operands {
            self.operand(operand)?;
        }
        match statement.rvalue {
            None => self.tag(0),
            Some(rvalue) => {
                self.tag(1);
                self.rvalue(rvalue);
            }
        }
        // operation and source are compatibility/diagnostic observations.
        Ok(())
    }

    fn optional_place(&mut self, place: Option<&MirPlaceRef>) -> Result<(), MirImportError> {
        match place {
            None => self.tag(0),
            Some(place) => {
                self.tag(1);
                self.place(place)?;
            }
        }
        Ok(())
    }

    fn place(&mut self, place: &MirPlaceRef) -> Result<(), MirImportError> {
        self.usize(place.local)?;
        self.len(place.projection.len())?;
        for projection in &place.projection {
            match projection {
                MirProjectionElem::Deref => self.tag(0),
                MirProjectionElem::Field(field) => {
                    self.tag(1);
                    self.usize(*field)?;
                }
                MirProjectionElem::Index { local } => {
                    self.tag(2);
                    self.usize(*local)?;
                }
                MirProjectionElem::ConstantIndex {
                    offset,
                    min_length,
                    from_end,
                } => {
                    self.tag(3);
                    self.u64(*offset);
                    self.u64(*min_length);
                    self.boolean(*from_end);
                }
                MirProjectionElem::Subslice { from, to, from_end } => {
                    self.tag(4);
                    self.u64(*from);
                    self.u64(*to);
                    self.boolean(*from_end);
                }
                MirProjectionElem::Downcast { variant } => {
                    self.tag(5);
                    self.usize(*variant)?;
                }
                MirProjectionElem::OpaqueCast => self.tag(6),
                MirProjectionElem::Other => self.tag(7),
            }
        }
        Ok(())
    }

    fn operand(&mut self, operand: &MirOperandRef) -> Result<(), MirImportError> {
        match operand {
            MirOperandRef::Place(place) => {
                self.tag(0);
                self.place(place)?;
            }
            MirOperandRef::Constant { ty, literal, .. } => {
                self.tag(1);
                self.imported_type(ty, 0)?;
                self.constant(literal);
            }
        }
        Ok(())
    }

    fn constant(&mut self, constant: &MirConstant) {
        match constant {
            MirConstant::Bool(value) => {
                self.tag(0);
                self.boolean(*value);
            }
            MirConstant::I32(value) => {
                self.tag(1);
                self.i32(*value);
            }
            MirConstant::U32(value) => {
                self.tag(2);
                self.u32(*value);
            }
            MirConstant::I64(value) => {
                self.tag(3);
                self.i64(*value);
            }
            MirConstant::ISize(value) => {
                self.tag(4);
                self.i64(*value);
            }
            MirConstant::USize(value) => {
                self.tag(5);
                self.u64(*value);
            }
            MirConstant::F32Bits(value) => {
                self.tag(6);
                self.u32(*value);
            }
            MirConstant::F64Bits(value) => {
                self.tag(7);
                self.u64(*value);
            }
            MirConstant::Unevaluated => self.tag(8),
        }
    }

    fn rvalue(&mut self, rvalue: MirRvalueKind) {
        match rvalue {
            MirRvalueKind::Use => self.tag(0),
            MirRvalueKind::Repeat => self.tag(1),
            MirRvalueKind::Ref => self.tag(2),
            MirRvalueKind::RawPointer => self.tag(3),
            MirRvalueKind::Cast => self.tag(4),
            MirRvalueKind::Binary(operation) => {
                self.tag(5);
                self.binary(operation);
            }
            MirRvalueKind::Unary(operation) => {
                self.tag(6);
                self.unary(operation);
            }
            MirRvalueKind::Discriminant => self.tag(7),
            MirRvalueKind::Aggregate => self.tag(8),
            MirRvalueKind::Other => self.tag(9),
            MirRvalueKind::FieldlessEnumVariant(discriminant) => {
                self.tag(10);
                self.i64(discriminant);
            }
        }
    }

    fn binary(&mut self, operation: MirBinaryOp) {
        self.tag(match operation {
            MirBinaryOp::Add => 0,
            MirBinaryOp::Sub => 1,
            MirBinaryOp::Mul => 2,
            MirBinaryOp::Div => 3,
            MirBinaryOp::Rem => 4,
            MirBinaryOp::BitXor => 5,
            MirBinaryOp::BitAnd => 6,
            MirBinaryOp::BitOr => 7,
            MirBinaryOp::Shl => 8,
            MirBinaryOp::Shr => 9,
            MirBinaryOp::Eq => 10,
            MirBinaryOp::Lt => 11,
            MirBinaryOp::Le => 12,
            MirBinaryOp::Ne => 13,
            MirBinaryOp::Ge => 14,
            MirBinaryOp::Gt => 15,
            MirBinaryOp::Cmp => 16,
            MirBinaryOp::Offset => 17,
            MirBinaryOp::AddUnchecked => 18,
            MirBinaryOp::SubUnchecked => 19,
            MirBinaryOp::MulUnchecked => 20,
            MirBinaryOp::ShlUnchecked => 21,
            MirBinaryOp::ShrUnchecked => 22,
            MirBinaryOp::AddWithOverflow => 23,
            MirBinaryOp::SubWithOverflow => 24,
            MirBinaryOp::MulWithOverflow => 25,
        });
    }

    fn unary(&mut self, operation: MirUnaryOp) {
        self.tag(match operation {
            MirUnaryOp::Not => 0,
            MirUnaryOp::Neg => 1,
            MirUnaryOp::PtrMetadata => 2,
        });
    }

    fn terminator(
        &mut self,
        terminator: &MirTerminatorKind,
        functions_by_path: &BTreeMap<&str, &MirFunction>,
    ) -> Result<(), MirImportError> {
        match terminator {
            MirTerminatorKind::Return => self.tag(0),
            MirTerminatorKind::Unreachable => self.tag(1),
            MirTerminatorKind::Goto { target } => {
                self.tag(2);
                self.usize(*target)?;
            }
            MirTerminatorKind::SwitchInt {
                discriminant,
                targets,
                otherwise,
            } => {
                self.tag(3);
                self.operand(discriminant)?;
                self.len(targets.len())?;
                for target in targets {
                    self.u128(target.value);
                    self.usize(target.target)?;
                }
                self.usize(*otherwise)?;
            }
            MirTerminatorKind::Call {
                callee,
                target,
                destination,
                operands,
            } => {
                self.tag(4);
                self.callee(callee.as_ref(), functions_by_path)?;
                match target {
                    None => self.tag(0),
                    Some(target) => {
                        self.tag(1);
                        self.usize(*target)?;
                    }
                }
                self.optional_place(destination.as_ref())?;
                self.len(operands.len())?;
                for operand in operands {
                    self.operand(operand)?;
                }
            }
            MirTerminatorKind::Assert {
                condition,
                expected,
                target,
            } => {
                self.tag(5);
                self.operand(condition)?;
                self.boolean(*expected);
                self.usize(*target)?;
            }
            MirTerminatorKind::Drop { target } => {
                self.tag(6);
                self.usize(*target)?;
            }
            MirTerminatorKind::Other => self.tag(7),
        }
        Ok(())
    }

    fn callee(
        &mut self,
        callee: Option<&MirCallee>,
        functions_by_path: &BTreeMap<&str, &MirFunction>,
    ) -> Result<(), MirImportError> {
        let Some(callee) = callee else {
            self.tag(0);
            return Ok(());
        };
        match &callee.identity {
            MirCalleeIdentity::Untrusted(path) => {
                let target = functions_by_path.get(path.as_str()).ok_or_else(|| {
                    MirImportError::new(format!(
                        "portable MIR cannot encode unresolved callee `{path}`"
                    ))
                })?;
                self.tag(1);
                self.text(&target.export_name)?;
            }
            MirCalleeIdentity::SessionRecognized(item) => {
                self.tag(2);
                self.text(item.canonical_path())?;
            }
            MirCalleeIdentity::ExternalImport(import) => {
                self.tag(3);
                self.bytes(&import.contract_identity.as_bytes())?;
                self.text(&import.symbol)?;
                self.text(&import.target)?;
                self.u16(import.code_object_version);
                self.text(&import.semantic_identity)?;
            }
            MirCalleeIdentity::RejectedTrustedProvider { path, marker } => {
                return Err(MirImportError::new(format!(
                    "portable MIR rejects `{path}` at trusted-provider marker `{marker}`"
                )));
            }
        }
        Ok(())
    }
}

impl MirStatement {
    fn summary(&self) -> Option<String> {
        if self.kind != MirStatementKind::Assign {
            return None;
        }

        let destination = self
            .destination
            .as_ref()
            .map(MirPlaceRef::label)
            .unwrap_or_else(|| "_".to_string());
        let operation = self.operation.as_deref().unwrap_or(self.kind.name());
        let dialect_op = self.lowering_op().unwrap_or_else(|| self.dialect_op());
        if self.operands.is_empty() {
            return Some(format!(
                "stmt{}: {} {destination} = {operation}",
                self.index,
                dialect_op.name()
            ));
        }

        let operands = self
            .operands
            .iter()
            .map(MirOperandRef::label)
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "stmt{}: {} {destination} = {operation}({operands})",
            self.index,
            dialect_op.name()
        ))
    }

    fn record(&self, function: &str, block: usize) -> MirOpRecord {
        let mut record = MirOpRecord::new(self.dialect_op())
            .with_attr(MirAttr::string("function", function))
            .with_attr(MirAttr::usize("block", block))
            .with_attr(MirAttr::usize("index", self.index))
            .with_attr(MirAttr::string("kind", self.kind.name()))
            .with_attr(MirAttr::usize("operand_count", self.operands.len()));

        if let Some(destination) = &self.destination {
            record
                .attrs
                .push(MirAttr::usize("destination_local", destination.local));
            record
                .attrs
                .push(MirAttr::string("destination", destination.label()));
        }
        if let Some(operation) = &self.operation {
            record.attrs.push(MirAttr::string("operation", operation));
        }
        if !self.operands.is_empty() {
            let operands = self
                .operands
                .iter()
                .map(MirOperandRef::label)
                .collect::<Vec<_>>()
                .join(", ");
            record.attrs.push(MirAttr::string("operands", operands));
        }

        record
    }

    fn lowering_record(&self, function: &str, block: usize) -> Option<MirOpRecord> {
        let op = self.lowering_op()?;
        let mut record = MirOpRecord::new(op)
            .with_attr(MirAttr::string("function", function))
            .with_attr(MirAttr::usize("block", block))
            .with_attr(MirAttr::usize("statement", self.index))
            .with_attr(MirAttr::string("source", self.dialect_op().name()))
            .with_attr(MirAttr::usize("operand_count", self.operands.len()));

        if let Some(destination) = &self.destination {
            record
                .attrs
                .push(MirAttr::usize("destination_local", destination.local));
            record
                .attrs
                .push(MirAttr::string("destination", destination.label()));
        }
        if let Some(operation) = &self.operation {
            record.attrs.push(MirAttr::string("operation", operation));
        }
        if !self.operands.is_empty() {
            let operands = self
                .operands
                .iter()
                .map(MirOperandRef::label)
                .collect::<Vec<_>>()
                .join(", ");
            record.attrs.push(MirAttr::string("operands", operands));
        }

        Some(record)
    }

    fn dialect_op(&self) -> MirOp {
        match self.kind {
            MirStatementKind::Assign => MirOp::Assign,
            MirStatementKind::StorageLive
            | MirStatementKind::StorageDead
            | MirStatementKind::SetDiscriminant
            | MirStatementKind::Intrinsic
            | MirStatementKind::CopyNonOverlapping
            | MirStatementKind::Retag
            | MirStatementKind::Coverage
            | MirStatementKind::Nop
            | MirStatementKind::Other => MirOp::Statement,
        }
    }

    fn lowering_op(&self) -> Option<MirOp> {
        if self.kind != MirStatementKind::Assign {
            return None;
        }
        if self
            .destination
            .as_ref()
            .is_some_and(MirPlaceRef::is_memory_projection)
        {
            return Some(MirOp::Store);
        }

        match self.rvalue? {
            MirRvalueKind::Binary(
                MirBinaryOp::Add | MirBinaryOp::AddUnchecked | MirBinaryOp::AddWithOverflow,
            ) => Some(MirOp::Add),
            MirRvalueKind::Binary(
                MirBinaryOp::Sub | MirBinaryOp::SubUnchecked | MirBinaryOp::SubWithOverflow,
            ) => Some(MirOp::Sub),
            MirRvalueKind::Binary(
                MirBinaryOp::Mul | MirBinaryOp::MulUnchecked | MirBinaryOp::MulWithOverflow,
            ) => Some(MirOp::Mul),
            MirRvalueKind::Binary(MirBinaryOp::Div) => Some(MirOp::Div),
            MirRvalueKind::Binary(MirBinaryOp::Eq) => Some(MirOp::Eq),
            MirRvalueKind::Binary(MirBinaryOp::Lt) => Some(MirOp::Lt),
            MirRvalueKind::Binary(MirBinaryOp::Le) => Some(MirOp::Le),
            MirRvalueKind::Binary(MirBinaryOp::Ne) => Some(MirOp::Ne),
            MirRvalueKind::Binary(MirBinaryOp::Ge) => Some(MirOp::Ge),
            MirRvalueKind::Binary(MirBinaryOp::Gt) => Some(MirOp::Gt),
            MirRvalueKind::Binary(MirBinaryOp::Cmp) => Some(MirOp::Cmp),
            MirRvalueKind::Cast => Some(MirOp::Cast),
            MirRvalueKind::Binary(MirBinaryOp::Offset) => Some(MirOp::Gep),
            MirRvalueKind::Unary(MirUnaryOp::PtrMetadata) => Some(MirOp::SliceLen),
            MirRvalueKind::Use if self.operands.iter().any(MirOperandRef::is_memory_place) => {
                Some(MirOp::Load)
            }
            _ => None,
        }
    }
}

impl MirStatementKind {
    fn name(self) -> &'static str {
        match self {
            Self::Assign => "assign",
            Self::StorageLive => "storage_live",
            Self::StorageDead => "storage_dead",
            Self::SetDiscriminant => "set_discriminant",
            Self::Intrinsic => "intrinsic",
            Self::CopyNonOverlapping => "copy_nonoverlapping",
            Self::Retag => "retag",
            Self::Coverage => "coverage",
            Self::Nop => "nop",
            Self::Other => "other",
        }
    }
}

impl MirPlaceRef {
    fn local(local: Local) -> Self {
        Self {
            local: local.as_usize(),
            projection: Vec::new(),
        }
    }

    fn label(&self) -> String {
        let mut label = format!("local{}", self.local);
        for projection in &self.projection {
            label.push('.');
            label.push_str(&projection.label());
        }
        label
    }

    fn is_memory_projection(&self) -> bool {
        self.projection.iter().any(|projection| {
            matches!(
                projection,
                MirProjectionElem::Deref
                    | MirProjectionElem::Index { .. }
                    | MirProjectionElem::ConstantIndex { .. }
            )
        })
    }
}

impl MirProjectionElem {
    fn label(&self) -> String {
        match self {
            Self::Deref => "deref".to_string(),
            Self::Field(field) => format!("field{field}"),
            Self::Index { local } => format!("index_local{local}"),
            Self::ConstantIndex {
                offset,
                min_length,
                from_end,
            } => format!("constant_index{offset}_min{min_length}_from_end{from_end}"),
            Self::Subslice { from, to, from_end } => {
                format!("subslice{from}_{to}_from_end{from_end}")
            }
            Self::Downcast { variant } => format!("downcast{variant}"),
            Self::OpaqueCast => "opaque_cast".to_string(),
            Self::Other => "projection".to_string(),
        }
    }
}

impl MirOperandRef {
    fn label(&self) -> String {
        match self {
            Self::Place(place) => place.label(),
            Self::Constant { ty, value, .. } => format!("const:{}={value}", ty.kind.name()),
        }
    }

    fn is_memory_place(&self) -> bool {
        matches!(self, Self::Place(place) if place.is_memory_projection())
    }
}

impl MirTerminatorKind {
    fn record(
        &self,
        function: &str,
        block: usize,
        source_identities: &BTreeMap<&str, Option<FunctionIdentityV1>>,
    ) -> MirOpRecord {
        let mut record = MirOpRecord::new(self.dialect_op())
            .with_attr(MirAttr::string("function", function))
            .with_attr(MirAttr::usize("block", block));

        match self {
            Self::Goto { target } | Self::Assert { target, .. } | Self::Drop { target } => {
                record.attrs.push(MirAttr::usize("target", *target));
            }
            Self::SwitchInt { targets, .. } => {
                record
                    .attrs
                    .push(MirAttr::usize("targets", targets.len() + 1));
            }
            Self::Call {
                callee,
                target,
                destination,
                operands,
            } => {
                if let Some(callee) = callee {
                    record
                        .attrs
                        .push(MirAttr::string("callee", callee.identity()));
                    if let Some(Some(identity)) = source_identities.get(callee.identity()) {
                        record.attrs.push(MirAttr::string(
                            "callee_source_identity_v1",
                            function_identity_hex_v1(*identity),
                        ));
                    }
                    if let Some(import) = callee.external_import_evidence() {
                        record.attrs.push(MirAttr::string(
                            "device_ffi_contract",
                            import.contract_identity.to_hex(),
                        ));
                        record
                            .attrs
                            .push(MirAttr::string("device_ffi_target", &import.target));
                        record.attrs.push(MirAttr::usize(
                            "device_ffi_code_object_version",
                            usize::from(import.code_object_version),
                        ));
                        record.attrs.push(MirAttr::string(
                            "device_ffi_semantic_identity",
                            &import.semantic_identity,
                        ));
                    }
                }
                if let Some(target) = target {
                    record.attrs.push(MirAttr::usize("target", *target));
                }
                if let Some(destination) = destination {
                    record
                        .attrs
                        .push(MirAttr::usize("destination_local", destination.local));
                    record
                        .attrs
                        .push(MirAttr::string("destination", destination.label()));
                }
                record
                    .attrs
                    .push(MirAttr::usize("operand_count", operands.len()));
                if !operands.is_empty() {
                    let operands = operands
                        .iter()
                        .map(MirOperandRef::label)
                        .collect::<Vec<_>>()
                        .join(", ");
                    record.attrs.push(MirAttr::string("operands", operands));
                }
            }
            Self::Return | Self::Unreachable | Self::Other => {}
        }

        record
    }

    fn dialect_op(&self) -> MirOp {
        match self {
            Self::Return => MirOp::Return,
            Self::Unreachable => MirOp::Unreachable,
            Self::Goto { .. } => MirOp::Branch,
            Self::SwitchInt { .. } => MirOp::Switch,
            Self::Call { .. } => MirOp::Call,
            Self::Assert { .. } => MirOp::Assert,
            Self::Drop { .. } => MirOp::Drop,
            Self::Other => MirOp::Other,
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::Return => MirOp::Return.name().to_string(),
            Self::Unreachable => MirOp::Unreachable.name().to_string(),
            Self::Goto { target } => format!("{} -> bb{target}", MirOp::Branch.name()),
            Self::SwitchInt { targets, .. } => {
                format!("{} ({} target(s))", MirOp::Switch.name(), targets.len() + 1)
            }
            Self::Call { callee, target, .. } => {
                let callee = callee
                    .as_ref()
                    .map(MirCallee::identity)
                    .unwrap_or("<dynamic>");
                match target {
                    Some(target) => format!("{} {callee} -> bb{target}", MirOp::Call.name()),
                    None => format!("{} {callee} -> return", MirOp::Call.name()),
                }
            }
            Self::Assert { target, .. } => format!("{} -> bb{target}", MirOp::Assert.name()),
            Self::Drop { target } => format!("{} -> bb{target}", MirOp::Drop.name()),
            Self::Other => "other".to_string(),
        }
    }
}

struct MirKernelMetadata {
    typed_profile: Option<MirKernelProfile>,
    frontend_contract: Option<crate::collector::AuthenticatedKernelFrontendContractV1>,
    matrix_frontend_abi: Option<MatrixFrontendAbiV2>,
}

struct MirBodyImportContext<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    body: &'a Body<'tcx>,
    compiler_ffi_imports: &'a CompilerFfiImports,
    dead_branches: &'a crate::monomorphization_dead::CompilerDeadBranchObservationV1,
}

fn import_body<'tcx>(
    context: MirBodyImportContext<'_, 'tcx>,
    export_name: String,
    rust_path: String,
    kind: MirFunctionKind,
    kernel_metadata: MirKernelMetadata,
) -> MirFunction {
    let MirBodyImportContext {
        tcx,
        body,
        compiler_ffi_imports,
        dead_branches,
    } = context;
    let blocks = body
        .basic_blocks
        .iter_enumerated()
        .filter(|(index, _)| dead_branches.imports_block(index.as_usize()))
        .map(|(index, block)| MirBlock {
            index: index.as_usize(),
            statements: block
                .statements
                .iter()
                .enumerate()
                .map(|(statement_index, statement)| {
                    import_statement(tcx, statement_index, &statement.kind, statement.source_info)
                })
                .collect(),
            terminator: block.terminator.as_ref().map(|terminator| MirTerminator {
                kind: dead_branches
                    .selected_successor(index.as_usize())
                    .map_or_else(
                        || terminator_kind(tcx, &terminator.kind, compiler_ffi_imports),
                        |target| MirTerminatorKind::Goto { target },
                    ),
                source: Some(import_source_location(tcx, terminator.source_info)),
            }),
        })
        .collect();
    let locals = body
        .local_decls
        .iter_enumerated()
        .map(|(local, decl)| {
            let index = local.as_usize();
            let role = if index == 0 {
                MirLocalRole::Return
            } else if index <= body.arg_count {
                MirLocalRole::Arg
            } else {
                MirLocalRole::Temp
            };
            MirLocal {
                index,
                role,
                ty: import_type(tcx, decl.ty),
            }
        })
        .collect();

    MirFunction {
        export_name,
        rust_path,
        kind,
        typed_profile: kernel_metadata.typed_profile,
        arg_count: body.arg_count,
        local_count: body.local_decls.len(),
        locals,
        blocks,
        frontend_contract: kernel_metadata.frontend_contract,
        matrix_frontend_abi: kernel_metadata.matrix_frontend_abi,
    }
}

fn import_type<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> MirImportedType {
    let kind = match ty.kind() {
        TyKind::Bool => MirType::I1,
        TyKind::Int(IntTy::I32) => MirType::I32,
        TyKind::Uint(UintTy::U32) => MirType::I32,
        TyKind::Int(IntTy::I64) => MirType::I64,
        TyKind::Uint(UintTy::Usize) => MirType::USize,
        TyKind::Float(FloatTy::F32) => MirType::F32,
        TyKind::Float(FloatTy::F64) => MirType::F64,
        TyKind::Ref(_, pointee, _) => match pointee.kind() {
            TyKind::Slice(_) => MirType::Slice,
            _ => MirType::Ptr,
        },
        TyKind::RawPtr(_, _) => MirType::Ptr,
        TyKind::Adt(adt, _) => match semantic_features::classify(tcx, adt.did())
            .map(SessionRecognizedSemanticItem::trusted_device_item)
        {
            Some(TrustedDeviceItem::DisjointSlice) => MirType::DisjointSlice,
            Some(TrustedDeviceItem::DeviceValue(
                dialect_amdgcn::DeviceValueDiagnosticItem::Bf16x2,
            )) => MirType::I32,
            Some(
                TrustedDeviceItem::DeviceValue(_)
                | TrustedDeviceItem::DeviceMath(dialect_amdgcn::DeviceMathDiagnosticItem::Context),
            ) => MirType::Unknown,
            _ => MirType::Unknown,
        },
        TyKind::Tuple(elements) if elements.is_empty() => MirType::Unit,
        _ => MirType::Unknown,
    };

    MirImportedType {
        kind,
        rust: ty.to_string(),
        shape: import_type_shape(tcx, ty),
    }
}

fn import_type_shape<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> MirTypeShape {
    match ty.kind() {
        TyKind::Bool => MirTypeShape::Bool,
        TyKind::Int(IntTy::I32) => MirTypeShape::I32,
        TyKind::Uint(UintTy::U32) => MirTypeShape::U32,
        TyKind::Int(IntTy::I64) => MirTypeShape::I64,
        TyKind::Int(IntTy::Isize) => MirTypeShape::ISize,
        TyKind::Uint(UintTy::Usize) => MirTypeShape::USize,
        TyKind::Float(FloatTy::F32) => MirTypeShape::F32,
        TyKind::Float(FloatTy::F64) => MirTypeShape::F64,
        TyKind::Ref(_, pointee, mutability) => match pointee.kind() {
            TyKind::Slice(element) => MirTypeShape::Slice {
                element: Box::new(import_type_shape(tcx, *element)),
                mutable: *mutability == Mutability::Mut,
            },
            _ => MirTypeShape::Reference {
                pointee: Box::new(import_type_shape(tcx, *pointee)),
                mutable: *mutability == Mutability::Mut,
            },
        },
        TyKind::RawPtr(pointee, mutability) => MirTypeShape::RawPointer {
            pointee: Box::new(import_type_shape(tcx, *pointee)),
            mutable: *mutability == Mutability::Mut,
        },
        TyKind::Adt(adt, args) => match semantic_features::classify(tcx, adt.did())
            .map(SessionRecognizedSemanticItem::trusted_device_item)
        {
            Some(TrustedDeviceItem::DisjointSlice) => MirTypeShape::DisjointSlice {
                element: Box::new(import_type_shape(tcx, args.type_at(0))),
            },
            Some(item @ TrustedDeviceItem::ThreadIndex) => MirTypeShape::Adt {
                identity: item.canonical_path().to_owned(),
            },
            Some(
                item @ (TrustedDeviceItem::DeviceMatrix
                | TrustedDeviceItem::Bf16MfmaFragment
                | TrustedDeviceItem::F32AccumulatorFragment),
            ) => MirTypeShape::Adt {
                identity: item.canonical_path().to_owned(),
            },
            Some(TrustedDeviceItem::DeviceValue(
                dialect_amdgcn::DeviceValueDiagnosticItem::F16,
            )) => MirTypeShape::F16,
            Some(TrustedDeviceItem::DeviceValue(
                dialect_amdgcn::DeviceValueDiagnosticItem::Bf16,
            )) => MirTypeShape::Bf16,
            Some(TrustedDeviceItem::DeviceValue(
                dialect_amdgcn::DeviceValueDiagnosticItem::Bf16x2,
            )) => MirTypeShape::Bf16x2,
            Some(TrustedDeviceItem::DeviceMath(
                dialect_amdgcn::DeviceMathDiagnosticItem::Context,
            )) => MirTypeShape::DeviceMath,
            _ => MirTypeShape::Adt {
                identity: tcx.def_path_str(adt.did()),
            },
        },
        TyKind::Tuple(elements) if elements.is_empty() => MirTypeShape::Unit,
        TyKind::Tuple(elements) => MirTypeShape::Tuple(
            elements
                .iter()
                .map(|element| import_type_shape(tcx, element))
                .collect(),
        ),
        _ => MirTypeShape::Unknown,
    }
}

fn import_statement<'tcx>(
    tcx: TyCtxt<'tcx>,
    index: usize,
    kind: &StatementKind<'tcx>,
    source_info: SourceInfo,
) -> MirStatement {
    MirStatement {
        index,
        kind: statement_kind(kind),
        destination: statement_destination(kind),
        operands: statement_operands(tcx, kind),
        rvalue: statement_rvalue(tcx, kind),
        operation: statement_operation(kind),
        source: Some(import_source_location(tcx, source_info)),
    }
}

fn import_source_location(tcx: TyCtxt<'_>, source_info: SourceInfo) -> MirSourceLocation {
    let location = tcx.sess.source_map().lookup_char_pos(source_info.span.lo());
    MirSourceLocation {
        file: location
            .file
            .name
            .prefer_remapped_unconditionally()
            .to_string_lossy()
            .into_owned(),
        line: location.line,
        column: location.col.0 + 1,
    }
}

fn statement_kind(kind: &StatementKind<'_>) -> MirStatementKind {
    match kind {
        StatementKind::Assign(_) => MirStatementKind::Assign,
        StatementKind::StorageLive(_) => MirStatementKind::StorageLive,
        StatementKind::StorageDead(_) => MirStatementKind::StorageDead,
        StatementKind::SetDiscriminant { .. } => MirStatementKind::SetDiscriminant,
        StatementKind::Intrinsic(intrinsic) => match intrinsic.as_ref() {
            NonDivergingIntrinsic::CopyNonOverlapping(_) => MirStatementKind::CopyNonOverlapping,
            _ => MirStatementKind::Intrinsic,
        },
        StatementKind::Retag(_, _) => MirStatementKind::Retag,
        StatementKind::Coverage(_) => MirStatementKind::Coverage,
        StatementKind::Nop => MirStatementKind::Nop,
        _ => MirStatementKind::Other,
    }
}

fn statement_destination(kind: &StatementKind<'_>) -> Option<MirPlaceRef> {
    match kind {
        StatementKind::Assign(assign) => {
            let (place, _) = &**assign;
            Some(import_place(*place))
        }
        StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
            Some(MirPlaceRef::local(*local))
        }
        StatementKind::SetDiscriminant { place, .. } => Some(import_place(**place)),
        _ => None,
    }
}

fn statement_operands<'tcx>(tcx: TyCtxt<'tcx>, kind: &StatementKind<'tcx>) -> Vec<MirOperandRef> {
    match kind {
        StatementKind::Assign(assign) => {
            let (_, rvalue) = &**assign;
            rvalue_operands(tcx, rvalue)
        }
        StatementKind::Intrinsic(intrinsic) => match intrinsic.as_ref() {
            NonDivergingIntrinsic::CopyNonOverlapping(copy) => vec![
                import_operand(tcx, &copy.src),
                import_operand(tcx, &copy.dst),
                import_operand(tcx, &copy.count),
            ],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn statement_operation(kind: &StatementKind<'_>) -> Option<String> {
    let StatementKind::Assign(assign) = kind else {
        return None;
    };
    let (_, rvalue) = &**assign;
    Some(rvalue_operation(rvalue).to_string())
}

fn statement_rvalue<'tcx>(tcx: TyCtxt<'tcx>, kind: &StatementKind<'tcx>) -> Option<MirRvalueKind> {
    let StatementKind::Assign(assign) = kind else {
        return None;
    };
    let (_, rvalue) = &**assign;
    Some(import_rvalue_kind(tcx, rvalue))
}

fn rvalue_operands<'tcx>(tcx: TyCtxt<'tcx>, rvalue: &Rvalue<'tcx>) -> Vec<MirOperandRef> {
    match rvalue {
        Rvalue::Use(operand)
        | Rvalue::Repeat(operand, _)
        | Rvalue::Cast(_, operand, _)
        | Rvalue::UnaryOp(_, operand) => vec![import_operand(tcx, operand)],
        Rvalue::BinaryOp(_, operands) => vec![
            import_operand(tcx, &operands.0),
            import_operand(tcx, &operands.1),
        ],
        Rvalue::Ref(_, _, place) | Rvalue::RawPtr(_, place) | Rvalue::Discriminant(place) => {
            vec![MirOperandRef::Place(import_place(*place))]
        }
        Rvalue::Aggregate(_, operands) => operands
            .iter()
            .map(|operand| import_operand(tcx, operand))
            .collect(),
        _ => Vec::new(),
    }
}

fn rvalue_operation(rvalue: &Rvalue<'_>) -> &'static str {
    match rvalue {
        Rvalue::Use(_) => "use",
        Rvalue::Repeat(_, _) => "repeat",
        Rvalue::Ref(_, _, _) => "ref",
        Rvalue::RawPtr(_, _) => "raw_ptr",
        Rvalue::Cast(_, _, _) => "cast",
        Rvalue::BinaryOp(op, _) => bin_op_name(*op),
        Rvalue::UnaryOp(op, _) => unary_op_name(*op),
        Rvalue::Discriminant(_) => "discriminant",
        Rvalue::Aggregate(_, _) => "aggregate",
        _ => "other",
    }
}

fn import_rvalue_kind<'tcx>(tcx: TyCtxt<'tcx>, rvalue: &Rvalue<'tcx>) -> MirRvalueKind {
    match rvalue {
        Rvalue::Use(_) => MirRvalueKind::Use,
        Rvalue::Repeat(_, _) => MirRvalueKind::Repeat,
        Rvalue::Ref(_, _, _) => MirRvalueKind::Ref,
        Rvalue::RawPtr(_, _) => MirRvalueKind::RawPointer,
        Rvalue::Cast(_, _, _) => MirRvalueKind::Cast,
        Rvalue::BinaryOp(op, _) => MirRvalueKind::Binary(import_binary_op(*op)),
        Rvalue::UnaryOp(op, _) => MirRvalueKind::Unary(import_unary_op(*op)),
        Rvalue::Discriminant(_) => MirRvalueKind::Discriminant,
        Rvalue::Aggregate(kind, operands) => {
            fieldless_enum_discriminant(tcx, kind, operands.is_empty()).map_or(
                MirRvalueKind::Aggregate,
                MirRvalueKind::FieldlessEnumVariant,
            )
        }
        _ => MirRvalueKind::Other,
    }
}

fn fieldless_enum_discriminant<'tcx>(
    tcx: TyCtxt<'tcx>,
    kind: &AggregateKind<'tcx>,
    operands_empty: bool,
) -> Option<i64> {
    let AggregateKind::Adt(def_id, variant, _, _, active_field) = kind else {
        return None;
    };
    let adt = tcx.adt_def(*def_id);
    if !operands_empty
        || active_field.is_some()
        || !adt.is_enum()
        || adt
            .variants()
            .iter()
            .any(|variant| !variant.fields.is_empty())
    {
        return None;
    }
    i64::try_from(adt.discriminant_for_variant(tcx, *variant).val).ok()
}

fn import_operand<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> MirOperandRef {
    if let Some(place) = operand.place() {
        return MirOperandRef::Place(import_place(place));
    }

    let Operand::Constant(constant) = operand else {
        return MirOperandRef::Constant {
            ty: MirImportedType {
                kind: MirType::Unknown,
                rust: "<unknown>".to_string(),
                shape: MirTypeShape::Unknown,
            },
            literal: MirConstant::Unevaluated,
            value: "<unknown>".to_string(),
        };
    };

    MirOperandRef::Constant {
        ty: import_type(tcx, constant.const_.ty()),
        literal: import_constant(tcx, constant),
        value: constant_value_label(tcx, constant),
    }
}

fn import_constant<'tcx>(tcx: TyCtxt<'tcx>, constant: &ConstOperand<'tcx>) -> MirConstant {
    let typing_env = TypingEnv::fully_monomorphized();
    match constant.const_.ty().kind() {
        TyKind::Uint(UintTy::Usize) => constant
            .const_
            .try_eval_target_usize(tcx, typing_env)
            .map(MirConstant::USize)
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Int(IntTy::Isize) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::ISize(value.to_target_isize(tcx)))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Bool => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .and_then(|value| value.try_to_bool().ok())
            .map(MirConstant::Bool)
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Int(IntTy::I32) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::I32(value.to_i32()))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Uint(UintTy::U32) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::U32(value.to_u32()))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Int(IntTy::I64) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::I64(value.to_i64()))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Float(FloatTy::F32) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::F32Bits(value.to_u32()))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Float(FloatTy::F64) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::F64Bits(value.to_u64()))
            .unwrap_or(MirConstant::Unevaluated),
        _ => MirConstant::Unevaluated,
    }
}

fn constant_value_label<'tcx>(tcx: TyCtxt<'tcx>, constant: &ConstOperand<'tcx>) -> String {
    let debug = format!("{:?}", constant.const_);
    match constant.const_.ty().kind() {
        TyKind::Uint(UintTy::Usize) => constant
            .const_
            .try_eval_target_usize(tcx, TypingEnv::fully_monomorphized())
            .map(|value| format!("{debug};eval_u64={value}"))
            .unwrap_or(debug),
        TyKind::Int(IntTy::Isize) => constant
            .const_
            .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())
            .map(|value| format!("{debug};eval_i64={}", value.to_target_isize(tcx)))
            .unwrap_or(debug),
        _ => debug,
    }
}

fn import_place(place: Place<'_>) -> MirPlaceRef {
    MirPlaceRef {
        local: place.local.as_usize(),
        projection: place
            .projection
            .iter()
            .map(import_projection_elem)
            .collect(),
    }
}

fn import_projection_elem(element: ProjectionElem<Local, Ty<'_>>) -> MirProjectionElem {
    match element {
        ProjectionElem::Deref => MirProjectionElem::Deref,
        ProjectionElem::Field(field, _) => MirProjectionElem::Field(field.index()),
        ProjectionElem::Index(local) => MirProjectionElem::Index {
            local: local.as_usize(),
        },
        ProjectionElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        } => MirProjectionElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        },
        ProjectionElem::Subslice { from, to, from_end } => {
            MirProjectionElem::Subslice { from, to, from_end }
        }
        ProjectionElem::Downcast(_, variant) => MirProjectionElem::Downcast {
            variant: variant.index(),
        },
        ProjectionElem::OpaqueCast(_) => MirProjectionElem::OpaqueCast,
        _ => MirProjectionElem::Other,
    }
}

fn import_binary_op(op: BinOp) -> MirBinaryOp {
    match op {
        BinOp::Add => MirBinaryOp::Add,
        BinOp::Sub => MirBinaryOp::Sub,
        BinOp::Mul => MirBinaryOp::Mul,
        BinOp::Div => MirBinaryOp::Div,
        BinOp::Rem => MirBinaryOp::Rem,
        BinOp::BitXor => MirBinaryOp::BitXor,
        BinOp::BitAnd => MirBinaryOp::BitAnd,
        BinOp::BitOr => MirBinaryOp::BitOr,
        BinOp::Shl => MirBinaryOp::Shl,
        BinOp::Shr => MirBinaryOp::Shr,
        BinOp::Eq => MirBinaryOp::Eq,
        BinOp::Lt => MirBinaryOp::Lt,
        BinOp::Le => MirBinaryOp::Le,
        BinOp::Ne => MirBinaryOp::Ne,
        BinOp::Ge => MirBinaryOp::Ge,
        BinOp::Gt => MirBinaryOp::Gt,
        BinOp::Cmp => MirBinaryOp::Cmp,
        BinOp::Offset => MirBinaryOp::Offset,
        BinOp::AddUnchecked => MirBinaryOp::AddUnchecked,
        BinOp::SubUnchecked => MirBinaryOp::SubUnchecked,
        BinOp::MulUnchecked => MirBinaryOp::MulUnchecked,
        BinOp::ShlUnchecked => MirBinaryOp::ShlUnchecked,
        BinOp::ShrUnchecked => MirBinaryOp::ShrUnchecked,
        BinOp::AddWithOverflow => MirBinaryOp::AddWithOverflow,
        BinOp::SubWithOverflow => MirBinaryOp::SubWithOverflow,
        BinOp::MulWithOverflow => MirBinaryOp::MulWithOverflow,
    }
}

fn import_unary_op(op: UnOp) -> MirUnaryOp {
    match op {
        UnOp::Not => MirUnaryOp::Not,
        UnOp::Neg => MirUnaryOp::Neg,
        UnOp::PtrMetadata => MirUnaryOp::PtrMetadata,
    }
}

fn bin_op_name(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
        BinOp::Rem => "rem",
        BinOp::BitXor => "bitxor",
        BinOp::BitAnd => "bitand",
        BinOp::BitOr => "bitor",
        BinOp::Shl => "shl",
        BinOp::Shr => "shr",
        BinOp::Eq => "eq",
        BinOp::Lt => "lt",
        BinOp::Le => "le",
        BinOp::Ne => "ne",
        BinOp::Ge => "ge",
        BinOp::Gt => "gt",
        BinOp::Cmp => "cmp",
        BinOp::Offset => "offset",
        BinOp::AddUnchecked => "add_unchecked",
        BinOp::SubUnchecked => "sub_unchecked",
        BinOp::MulUnchecked => "mul_unchecked",
        BinOp::ShlUnchecked => "shl_unchecked",
        BinOp::ShrUnchecked => "shr_unchecked",
        BinOp::AddWithOverflow => "add_with_overflow",
        BinOp::SubWithOverflow => "sub_with_overflow",
        BinOp::MulWithOverflow => "mul_with_overflow",
    }
}

fn unary_op_name(op: UnOp) -> &'static str {
    match op {
        UnOp::Not => "not",
        UnOp::Neg => "neg",
        UnOp::PtrMetadata => "ptr_metadata",
    }
}

fn terminator_kind<'tcx>(
    tcx: TyCtxt<'tcx>,
    kind: &TerminatorKind<'tcx>,
    compiler_ffi_imports: &CompilerFfiImports,
) -> MirTerminatorKind {
    match kind {
        TerminatorKind::Return => MirTerminatorKind::Return,
        TerminatorKind::Unreachable => MirTerminatorKind::Unreachable,
        TerminatorKind::Goto { target } => MirTerminatorKind::Goto {
            target: target.as_usize(),
        },
        TerminatorKind::SwitchInt { discr, targets } => MirTerminatorKind::SwitchInt {
            discriminant: import_operand(tcx, discr),
            targets: targets
                .iter()
                .map(|(value, target)| MirSwitchTarget {
                    value,
                    target: target.as_usize(),
                })
                .collect(),
            otherwise: targets.otherwise().as_usize(),
        },
        TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            ..
        } => MirTerminatorKind::Call {
            callee: call_identity(tcx, func, compiler_ffi_imports),
            target: target.map(BasicBlock::as_usize),
            destination: Some(import_place(*destination)),
            operands: args
                .iter()
                .map(|arg| import_operand(tcx, &arg.node))
                .collect(),
        },
        TerminatorKind::Assert {
            cond,
            expected,
            target,
            ..
        } => MirTerminatorKind::Assert {
            condition: import_operand(tcx, cond),
            expected: *expected,
            target: target.as_usize(),
        },
        TerminatorKind::Drop { target, .. } => MirTerminatorKind::Drop {
            target: target.as_usize(),
        },
        _ => MirTerminatorKind::Other,
    }
}

fn call_identity<'tcx>(
    tcx: TyCtxt<'tcx>,
    func: &Operand<'tcx>,
    compiler_ffi_imports: &CompilerFfiImports,
) -> Option<MirCallee> {
    let Operand::Constant(constant) = func else {
        return None;
    };
    let TyKind::FnDef(def_id, args) = constant.const_.ty().kind() else {
        return None;
    };
    let resolved_def_id =
        Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, args)
            .ok()
            .flatten()
            .map(|instance| instance.def_id())
            .unwrap_or(*def_id);
    Some(
        if let Some(item) = semantic_features::classify(tcx, resolved_def_id) {
            MirCallee::session_recognized(item)
        } else if let Some(marker) =
            crate::trusted_device_items::rejected_provider_marker(tcx, resolved_def_id)
        {
            MirCallee::rejected_trusted_provider(imported_rust_path(tcx, resolved_def_id), marker)
        } else if let Some(import) = compiler_ffi_imports
            .classify(tcx, resolved_def_id)
            .or_else(|| compiler_ffi_imports.classify(tcx, *def_id))
        {
            MirCallee::external_import(import)
        } else {
            MirCallee::untrusted(imported_rust_path(tcx, resolved_def_id))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::CollectedFunctionRole;

    #[test]
    fn collection_roles_map_to_mir_roles_without_name_inference() {
        assert_eq!(
            import_function_kind(CollectedFunctionRole::KernelEntry),
            MirFunctionKind::KernelEntry
        );
        assert_eq!(
            import_function_kind(CollectedFunctionRole::InternalHelper),
            MirFunctionKind::InternalHelper
        );
        assert_eq!(
            import_function_kind(CollectedFunctionRole::DeviceFfiExport),
            MirFunctionKind::DeviceFfiExport
        );
    }

    #[test]
    fn portable_semantic_digest_v2_is_deterministic_and_domain_stable() {
        let module = portable_semantic_module();
        let environment = portable_semantic_environment();
        let first = portable_digest(&module, &environment);
        let second = portable_digest(&module.clone(), &environment);

        assert_eq!(first, second);
        assert_ne!(first.as_bytes(), &[0; 32]);
        assert_eq!(
            first.to_hex(),
            "5dce95ed570b079957b04b5692c2ab0f897b6c7478505260692d88831ad14ff5"
        );
    }

    #[test]
    fn portable_semantic_digest_v2_binds_executable_mir_mutations() {
        let original = portable_semantic_module();
        let environment = portable_semantic_environment();
        let expected = portable_digest(&original, &environment);

        let mut cfg = original.clone();
        let MirTerminatorKind::Call { target, .. } =
            &mut cfg.functions[0].blocks[0].terminator.as_mut().unwrap().kind
        else {
            panic!("fixture call terminator");
        };
        *target = Some(0);
        assert_ne!(portable_digest(&cfg, &environment), expected, "CFG");

        let mut operand = original.clone();
        let MirOperandRef::Place(place) =
            &mut operand.functions[0].blocks[0].statements[0].operands[0]
        else {
            panic!("fixture place operand");
        };
        place.local = 2;
        assert_ne!(portable_digest(&operand, &environment), expected, "operand");

        let mut ty = original.clone();
        ty.functions[0].locals[1].ty.kind = MirType::I32;
        ty.functions[0].locals[1].ty.shape = MirTypeShape::I32;
        assert_ne!(portable_digest(&ty, &environment), expected, "type");

        let mut callee = original.clone();
        let MirTerminatorKind::Call { callee: target, .. } = &mut callee.functions[0].blocks[0]
            .terminator
            .as_mut()
            .unwrap()
            .kind
        else {
            panic!("fixture call terminator");
        };
        *target = Some(MirCallee::trusted_for_test(
            TrustedDeviceItem::ThreadIndex1d,
        ));
        assert_ne!(portable_digest(&callee, &environment), expected, "callee");

        let mut projection = original.clone();
        let MirOperandRef::Place(place) =
            &mut projection.functions[0].blocks[0].statements[0].operands[0]
        else {
            panic!("fixture place operand");
        };
        place.projection[1] = MirProjectionElem::Field(0);
        assert_ne!(
            portable_digest(&projection, &environment),
            expected,
            "projection"
        );

        let mut constant = original.clone();
        let MirOperandRef::Constant { literal, .. } =
            &mut constant.functions[0].blocks[0].statements[0].operands[1]
        else {
            panic!("fixture constant operand");
        };
        *literal = MirConstant::U32(8);
        assert_ne!(
            portable_digest(&constant, &environment),
            expected,
            "constant"
        );

        let mut profile = original;
        profile.functions[0].typed_profile = Some(MirKernelProfile::VecAddRustcLayoutV2);
        assert_ne!(portable_digest(&profile, &environment), expected, "profile");
    }

    #[test]
    fn portable_semantic_digest_v2_excludes_paths_diagnostics_and_build_observations() {
        let original = portable_semantic_module();
        let environment = portable_semantic_environment();
        let expected = portable_digest(&original, &environment);
        let mut changed = original;

        changed.functions[0].rust_path = "different_checkout::opaque_build_hash::alpha".to_owned();
        changed.functions[1].rust_path = "different_checkout::opaque_build_hash::helper".to_owned();
        let MirTerminatorKind::Call {
            callee: Some(callee),
            ..
        } = &mut changed.functions[0].blocks[0]
            .terminator
            .as_mut()
            .unwrap()
            .kind
        else {
            panic!("fixture internal call");
        };
        *callee = MirCallee::untrusted_for_test("different_checkout::opaque_build_hash::helper");

        changed.functions[0].locals[1].ty.rust =
            "diagnostic::DisplayOnly<toolchain_hash>".to_owned();
        let statement = &mut changed.functions[0].blocks[0].statements[0];
        statement.operation = Some("different diagnostic spelling".to_owned());
        statement.source = Some(MirSourceLocation {
            file: "/different/checkout/src/kernel.rs".to_owned(),
            line: 999,
            column: 41,
        });
        let MirOperandRef::Constant { value, .. } = &mut statement.operands[1] else {
            panic!("fixture constant operand");
        };
        *value = "debug-only constant rendering".to_owned();
        changed.functions[0].blocks[0]
            .terminator
            .as_mut()
            .unwrap()
            .source = Some(MirSourceLocation {
            file: "/another/checkout/generated.rs".to_owned(),
            line: 1,
            column: 1,
        });

        let dimensions = fe2o3_rustc_front::FrontendWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
        let launch = fe2o3_rustc_front::FrontendLaunchBoundsV1::new(
            Some(dimensions),
            Some(dimensions),
            Some(1),
        )
        .unwrap();
        let contract =
            fe2o3_rustc_front::KernelFrontendContractV1::new(Some(launch), None).unwrap();
        changed.functions[0].frontend_contract =
            Some(crate::collector::AuthenticatedKernelFrontendContractV1::for_test(contract));
        changed.functions.reverse();

        assert_eq!(portable_digest(&changed, &environment), expected);
    }

    #[test]
    fn portable_semantic_digest_v2_binds_structured_target_abi_and_launch_policy() {
        let module = portable_semantic_module();
        let environment = portable_semantic_environment();
        let expected = portable_digest(&module, &environment);

        let different_target = TargetIdentity::new(
            fe2o3_artifacts::IdentityText::new("amdgcn-amd-amdhsa").unwrap(),
            fe2o3_artifacts::IdentityText::new("gfx950:xnack-").unwrap(),
            PointerWidth::Bits64,
            Endianness::Little,
            vec![Capability::Atomics, Capability::AmdWave],
        )
        .unwrap();
        let target_environment = PortableSemanticEnvironment {
            target: different_target,
            abi: environment.abi.clone(),
            launch: environment.launch.clone(),
        };
        assert_ne!(portable_digest(&module, &target_environment), expected);

        let abi_environment = PortableSemanticEnvironment {
            target: environment.target.clone(),
            abi: AbiLayout::new(0, 1, PointerWidth::Bits32, Vec::new()).unwrap(),
            launch: environment.launch.clone(),
        };
        assert_ne!(portable_digest(&module, &abi_environment), expected);

        let launch_environment = PortableSemanticEnvironment {
            target: environment.target,
            abi: environment.abi,
            launch: LaunchContract::new(
                1,
                BlockSize::Exact(fe2o3_artifacts::Dimensions::new(64, 1, 1).unwrap()),
                fe2o3_artifacts::Dimensions::new(65_535, 1, 1).unwrap(),
                0,
                0,
            )
            .unwrap(),
        };
        assert_ne!(portable_digest(&module, &launch_environment), expected);
    }

    #[test]
    fn portable_semantic_digest_v2_rejects_unresolved_textual_callees() {
        let mut module = portable_semantic_module();
        let MirTerminatorKind::Call {
            callee: Some(callee),
            ..
        } = &mut module.functions[0].blocks[0]
            .terminator
            .as_mut()
            .unwrap()
            .kind
        else {
            panic!("fixture internal call");
        };
        *callee = MirCallee::untrusted_for_test("foreign::opaque::callee");
        let environment = portable_semantic_environment();

        let error = module
            .portable_semantic_digest_v2(MirSemanticAdmissionInputsV2::new(
                "alpha",
                &environment.target,
                &environment.abi,
                &environment.launch,
            ))
            .unwrap_err();
        assert!(error.to_string().contains("unresolved callee"));
    }

    #[test]
    fn summary_includes_function_and_block_shape() {
        let module = MirModule {
            functions: vec![MirFunction {
                export_name: "vecadd".to_string(),
                rust_path: "fe2o3_vecadd::fe2o3_kernel_vecadd".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                matrix_frontend_abi: None,
                arg_count: 3,
                local_count: 17,
                locals: vec![
                    MirLocal {
                        index: 0,
                        role: MirLocalRole::Return,
                        ty: MirImportedType {
                            kind: MirType::Unit,
                            rust: "()".to_string(),
                            shape: MirTypeShape::Unit,
                        },
                    },
                    MirLocal {
                        index: 1,
                        role: MirLocalRole::Arg,
                        ty: MirImportedType {
                            kind: MirType::Slice,
                            rust: "&[f32]".to_string(),
                            shape: MirTypeShape::Slice {
                                element: Box::new(MirTypeShape::F32),
                                mutable: false,
                            },
                        },
                    },
                ],
                blocks: vec![MirBlock {
                    index: 0,
                    statements: vec![
                        simple_statement(0, MirStatementKind::StorageLive),
                        simple_statement(1, MirStatementKind::Assign),
                    ],
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Goto { target: 1 },
                        source: None,
                    }),
                }],
            }],
        };

        let summary = module.summary();

        assert!(summary.contains("[kernel-entry] vecadd (mir.func)"));
        assert!(summary.contains("fe2o3_vecadd::fe2o3_kernel_vecadd"));
        assert!(summary.contains("1 bb, 17 locals, 3 args"));
        assert!(summary.contains("local1: arg mir.slice (&[f32])"));
        assert!(summary.contains("bb0 (mir.block): 2 stmt(s), mir.br -> bb1"));
    }

    #[test]
    fn dialect_records_include_function_blocks_and_terminators() {
        let module = MirModule {
            functions: vec![MirFunction {
                export_name: "vecadd".to_string(),
                rust_path: "fe2o3_vecadd::fe2o3_kernel_vecadd".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                matrix_frontend_abi: None,
                arg_count: 3,
                local_count: 17,
                locals: vec![MirLocal {
                    index: 1,
                    role: MirLocalRole::Arg,
                    ty: MirImportedType {
                        kind: MirType::Slice,
                        rust: "&[f32]".to_string(),
                        shape: MirTypeShape::Slice {
                            element: Box::new(MirTypeShape::F32),
                            mutable: false,
                        },
                    },
                }],
                blocks: vec![MirBlock {
                    index: 0,
                    statements: vec![MirStatement {
                        index: 0,
                        kind: MirStatementKind::Assign,
                        destination: Some(MirPlaceRef {
                            local: 3,
                            projection: Vec::new(),
                        }),
                        operands: vec![MirOperandRef::Place(MirPlaceRef {
                            local: 1,
                            projection: vec![
                                MirProjectionElem::Deref,
                                MirProjectionElem::Index { local: 2 },
                            ],
                        })],
                        rvalue: Some(MirRvalueKind::Use),
                        operation: Some("use".to_string()),
                        source: None,
                    }],
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Return,
                        source: None,
                    }),
                }],
            }],
        };

        let records = module.dialect_records();

        assert_eq!(records[0].op, MirOp::Module);
        assert_eq!(records[1].op, MirOp::Func);
        assert_eq!(record_string(&records[1], "kind"), Some("kernel"));
        assert_eq!(records[2].op, MirOp::Arg);
        assert_eq!(records[3].op, MirOp::Block);
        assert_eq!(records[4].op, MirOp::Assign);
        assert_eq!(records[5].op, MirOp::Load);
        assert_eq!(records[6].op, MirOp::Return);
        assert_eq!(record_usize(&records[4], "destination_local"), Some(3));
        assert_eq!(record_string(&records[4], "operation"), Some("use"));
        assert_eq!(
            record_string(&records[4], "operands"),
            Some("local1.deref.index_local2")
        );
        assert_eq!(record_usize(&records[5], "statement"), Some(0));
        assert_eq!(record_string(&records[5], "source"), Some("mir.assign"));
    }

    #[test]
    fn call_records_include_destination_and_operands() {
        let module = MirModule {
            functions: vec![MirFunction {
                export_name: "copy".to_string(),
                rust_path: "fe2o3_copy::fe2o3_kernel_copy".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                matrix_frontend_abi: None,
                arg_count: 1,
                local_count: 3,
                locals: Vec::new(),
                blocks: vec![MirBlock {
                    index: 0,
                    statements: Vec::new(),
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Call {
                            callee: Some(MirCallee::trusted_for_test(
                                TrustedDeviceItem::ThreadIndex1d,
                            )),
                            target: Some(1),
                            destination: Some(local_place(2)),
                            operands: vec![MirOperandRef::Place(local_place(1))],
                        },
                        source: None,
                    }),
                }],
            }],
        };

        let records = module.dialect_records();

        assert_eq!(records[3].op, MirOp::Call);
        assert_eq!(
            record_string(&records[3], "callee"),
            Some("fe2o3_device::thread::index_1d")
        );
        assert_eq!(record_usize(&records[3], "target"), Some(1));
        assert_eq!(record_usize(&records[3], "destination_local"), Some(2));
        assert_eq!(record_string(&records[3], "destination"), Some("local2"));
        assert_eq!(record_usize(&records[3], "operand_count"), Some(1));
        assert_eq!(record_string(&records[3], "operands"), Some("local1"));
        assert_eq!(record_string(&records[3], "trusted_callee"), None);
    }

    #[test]
    fn callee_identity_cannot_mismatch_trusted_authority() {
        let items = [
            TrustedDeviceItem::DisjointSlice,
            TrustedDeviceItem::ThreadIndex,
            TrustedDeviceItem::ThreadIndex1d,
            TrustedDeviceItem::ThreadIndexGet,
            TrustedDeviceItem::ThreadIndexOffset,
            TrustedDeviceItem::ThreadIndexOffsetSigned,
            TrustedDeviceItem::ThreadIndexStride,
            TrustedDeviceItem::ThreadIndexStrideOffset,
            TrustedDeviceItem::DisjointSliceGetMut,
            TrustedDeviceItem::DisjointSliceGetMutAt,
        ];

        for item in items {
            let trusted = MirCallee::trusted_for_test(item);
            assert_eq!(trusted.identity(), item.canonical_path());
            assert_eq!(trusted.trusted_item(), Some(item));

            let same_spelling = MirCallee::untrusted_for_test(item.canonical_path());
            assert_eq!(same_spelling.identity(), item.canonical_path());
            assert_eq!(same_spelling.trusted_item(), None);
        }
    }

    #[test]
    fn external_import_evidence_uses_the_contract_symbol_and_retains_provenance() {
        let callee = MirCallee::external_import_for_test(
            "external_device_add_v1",
            "C(u32[size=4,align=4])->u32[size=4,align=4]",
            "none",
        );
        let module = MirModule {
            functions: vec![MirFunction {
                export_name: "consumer".to_string(),
                rust_path: "tests::consumer".to_string(),
                kind: MirFunctionKind::KernelEntry,
                typed_profile: None,
                frontend_contract: None,
                matrix_frontend_abi: None,
                arg_count: 0,
                local_count: 1,
                locals: Vec::new(),
                blocks: vec![MirBlock {
                    index: 0,
                    statements: Vec::new(),
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Call {
                            callee: Some(callee),
                            target: Some(1),
                            destination: Some(local_place(0)),
                            operands: Vec::new(),
                        },
                        source: None,
                    }),
                }],
            }],
        };

        let records = module.dialect_records();
        let call = records
            .iter()
            .find(|record| record.op == MirOp::Call)
            .expect("call record");
        assert_eq!(
            record_string(call, "callee"),
            Some("external_device_add_v1")
        );
        assert_eq!(
            record_string(call, "device_ffi_target"),
            Some("gfx942:xnack-")
        );
        assert_eq!(
            record_usize(call, "device_ffi_code_object_version"),
            Some(5)
        );
        assert!(record_string(call, "device_ffi_contract").is_some());
        assert!(record_string(call, "device_ffi_semantic_identity").is_some());
    }

    #[test]
    fn ordinary_extern_spelling_has_no_external_import_evidence() {
        let callee = MirCallee::untrusted_for_test("external_device_add_v1");

        assert_eq!(callee.identity(), "external_device_add_v1");
        assert_eq!(callee.trusted_item(), None);
        assert_eq!(callee.external_import_evidence(), None);
    }

    #[test]
    fn external_import_fields_fail_closed_independently() {
        const SYMBOL: &str = "external_device_add_v1";
        const TARGET: &str = "gfx942:xnack-";
        const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
        const SEMANTIC: &str = "6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b";
        let identity = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
            symbol: SYMBOL,
            calling_convention: "C",
            code_object_version: 5,
            target: TARGET,
            physical_abi: ABI,
            effects: "none",
            semantic_identity: SEMANTIC,
        });
        assert!(
            validate_external_import_fields(
                identity, SYMBOL, TARGET, 5, ABI, "none", SEMANTIC, TARGET, 5,
            )
            .is_ok()
        );

        let wrong_target = validate_external_import_fields(
            identity, SYMBOL, "gfx1100", 5, ABI, "none", SEMANTIC, TARGET, 5,
        )
        .expect_err("target mismatch");
        assert!(wrong_target.to_string().contains("target"));
        let wrong_cov = validate_external_import_fields(
            identity, SYMBOL, TARGET, 6, ABI, "none", SEMANTIC, TARGET, 5,
        )
        .expect_err("code-object version mismatch");
        assert!(wrong_cov.to_string().contains("code-object version"));
        let wrong_identity = validate_external_import_fields(
            DeviceFfiContractIdV1::from_bytes([0x11; 32]),
            SYMBOL,
            TARGET,
            5,
            ABI,
            "none",
            SEMANTIC,
            TARGET,
            5,
        )
        .expect_err("identity mismatch");
        assert!(wrong_identity.to_string().contains("contract identity"));

        let malformed_abi = "C(u32)->u32";
        let malformed_abi_identity = derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
            direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
            symbol: SYMBOL,
            calling_convention: "C",
            code_object_version: 5,
            target: TARGET,
            physical_abi: malformed_abi,
            effects: "none",
            semantic_identity: SEMANTIC,
        });
        let malformed_abi_error = validate_external_import_fields(
            malformed_abi_identity,
            SYMBOL,
            TARGET,
            5,
            malformed_abi,
            "none",
            SEMANTIC,
            TARGET,
            5,
        )
        .expect_err("malformed ABI");
        assert!(malformed_abi_error.to_string().contains("physical ABI"));

        let incompatible_effects = "read_global";
        let incompatible_effects_identity =
            derive_device_ffi_contract_id_v1(DeviceFfiContractFieldsV1 {
                direction: DEVICE_FFI_DIRECTION_IMPORT_V1,
                symbol: SYMBOL,
                calling_convention: "C",
                code_object_version: 5,
                target: TARGET,
                physical_abi: ABI,
                effects: incompatible_effects,
                semantic_identity: SEMANTIC,
            });
        let effects_error = validate_external_import_fields(
            incompatible_effects_identity,
            SYMBOL,
            TARGET,
            5,
            ABI,
            incompatible_effects,
            SEMANTIC,
            TARGET,
            5,
        )
        .expect_err("effects/ABI mismatch");
        assert!(effects_error.to_string().contains("effects disagree"));

        let semantic_error = validate_external_import_fields(
            identity,
            SYMBOL,
            TARGET,
            5,
            ABI,
            "none",
            &"7c".repeat(32),
            TARGET,
            5,
        )
        .expect_err("semantic identity mismatch");
        assert!(semantic_error.to_string().contains("contract identity"));
    }

    #[test]
    fn assignments_classify_lowering_ops() {
        let arithmetic = MirStatement {
            index: 0,
            kind: MirStatementKind::Assign,
            destination: Some(local_place(3)),
            operands: vec![
                MirOperandRef::Place(local_place(1)),
                MirOperandRef::Place(local_place(2)),
            ],
            rvalue: Some(MirRvalueKind::Binary(MirBinaryOp::MulWithOverflow)),
            operation: Some("mul_with_overflow".to_string()),
            source: None,
        };
        let load = MirStatement {
            index: 1,
            kind: MirStatementKind::Assign,
            destination: Some(local_place(4)),
            operands: vec![MirOperandRef::Place(MirPlaceRef {
                local: 1,
                projection: vec![
                    MirProjectionElem::Deref,
                    MirProjectionElem::Index { local: 2 },
                ],
            })],
            rvalue: Some(MirRvalueKind::Use),
            operation: Some("use".to_string()),
            source: None,
        };
        let store = MirStatement {
            index: 2,
            kind: MirStatementKind::Assign,
            destination: Some(MirPlaceRef {
                local: 5,
                projection: vec![MirProjectionElem::Deref],
            }),
            operands: vec![MirOperandRef::Place(local_place(4))],
            rvalue: Some(MirRvalueKind::Use),
            operation: Some("use".to_string()),
            source: None,
        };
        let compare = MirStatement {
            index: 3,
            kind: MirStatementKind::Assign,
            destination: Some(local_place(6)),
            operands: vec![
                MirOperandRef::Place(local_place(1)),
                MirOperandRef::Place(local_place(2)),
            ],
            rvalue: Some(MirRvalueKind::Binary(MirBinaryOp::Lt)),
            operation: Some("lt".to_string()),
            source: None,
        };

        assert_eq!(arithmetic.lowering_op(), Some(MirOp::Mul));
        assert_eq!(load.lowering_op(), Some(MirOp::Load));
        assert_eq!(store.lowering_op(), Some(MirOp::Store));
        assert_eq!(compare.lowering_op(), Some(MirOp::Lt));
    }

    #[test]
    fn general_two_kernel_shared_helper_is_represented_once_by_source_identity() {
        let helper = general_two_kernel_function(
            "shared_helper",
            "tests::shared_helper",
            MirFunctionKind::InternalHelper,
            None,
        );
        let helper_identity = function_identity_hex_v1(helper.source_identity_v1().unwrap());
        let module = MirModule::from_functions_v1(vec![
            general_two_kernel_function(
                "zeta",
                "tests::kernel_zeta",
                MirFunctionKind::KernelEntry,
                Some("tests::shared_helper"),
            ),
            helper,
            general_two_kernel_function(
                "alpha",
                "tests::kernel_alpha",
                MirFunctionKind::KernelEntry,
                Some("tests::shared_helper"),
            ),
        ])
        .unwrap();

        assert_eq!(
            module
                .functions
                .iter()
                .filter(|function| function.kind == MirFunctionKind::KernelEntry)
                .count(),
            2
        );
        assert_eq!(
            module
                .functions
                .iter()
                .filter(|function| function.kind == MirFunctionKind::InternalHelper)
                .count(),
            1
        );

        let records = module.dialect_records();
        assert_eq!(record_usize(&records[0], "kernel_roots"), Some(2));
        assert_eq!(record_usize(&records[0], "internal_helpers"), Some(1));
        let helper_record = records
            .iter()
            .find(|record| {
                record.op == MirOp::Func && record_string(record, "symbol") == Some("shared_helper")
            })
            .unwrap();
        assert_eq!(
            record_string(helper_record, "source_identity_v1"),
            Some(helper_identity.as_str())
        );
        let calls = records
            .iter()
            .filter(|record| record.op == MirOp::Call)
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().all(|call| {
            record_string(call, "callee") == Some("tests::shared_helper")
                && record_string(call, "callee_source_identity_v1")
                    == Some(helper_identity.as_str())
        }));
    }

    #[test]
    fn general_two_kernel_serialization_is_deterministic_for_all_input_orders() {
        let functions = [
            general_two_kernel_function(
                "kernel_a",
                "tests::kernel_a",
                MirFunctionKind::KernelEntry,
                Some("tests::shared"),
            ),
            general_two_kernel_function(
                "kernel_b",
                "tests::kernel_b",
                MirFunctionKind::KernelEntry,
                Some("tests::shared"),
            ),
            general_two_kernel_function(
                "shared",
                "tests::shared",
                MirFunctionKind::InternalHelper,
                None,
            ),
        ];
        let permutations = [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ];
        let expected = MirModule::from_functions_v1(
            permutations[0]
                .iter()
                .map(|index| functions[*index].clone())
                .collect(),
        )
        .unwrap()
        .dialect_records();

        for permutation in permutations.into_iter().skip(1) {
            let records = MirModule::from_functions_v1(
                permutation
                    .iter()
                    .map(|index| functions[*index].clone())
                    .collect(),
            )
            .unwrap()
            .dialect_records();
            assert_eq!(records, expected);
        }
    }

    #[test]
    fn general_two_kernel_malformed_and_duplicate_roots_fail_closed() {
        let malformed =
            general_two_kernel_function("kernel", "", MirFunctionKind::KernelEntry, None);
        assert!(
            MirModule::from_functions_v1(vec![malformed])
                .unwrap_err()
                .to_string()
                .contains("source function path must not be empty")
        );

        let duplicate_export = MirModule::from_functions_v1(vec![
            general_two_kernel_function("same", "tests::first", MirFunctionKind::KernelEntry, None),
            general_two_kernel_function(
                "same",
                "tests::second",
                MirFunctionKind::KernelEntry,
                None,
            ),
        ])
        .unwrap_err();
        assert!(
            duplicate_export
                .to_string()
                .contains("duplicate function export `same`")
        );

        let ambiguous_source = MirModule::from_functions_v1(vec![
            general_two_kernel_function(
                "first",
                "tests::same_root",
                MirFunctionKind::KernelEntry,
                None,
            ),
            general_two_kernel_function(
                "second",
                "tests::same_root",
                MirFunctionKind::KernelEntry,
                None,
            ),
        ])
        .unwrap_err();
        assert!(
            ambiguous_source
                .to_string()
                .contains("select the same source function `tests::same_root`")
        );

        let helper_only = general_two_kernel_function(
            "helper",
            "tests::helper",
            MirFunctionKind::InternalHelper,
            None,
        );
        assert_eq!(
            MirModule::from_functions_v1(vec![helper_only])
                .unwrap_err()
                .to_string(),
            "MIR module contains no kernel root"
        );
    }

    #[test]
    fn general_two_kernel_order_places_canonical_roots_before_shared_helpers() {
        let module = MirModule::from_functions_v1(vec![
            general_two_kernel_function(
                "helper",
                "tests::helper",
                MirFunctionKind::InternalHelper,
                None,
            ),
            general_two_kernel_function(
                "second",
                "tests::second",
                MirFunctionKind::KernelEntry,
                Some("tests::helper"),
            ),
            general_two_kernel_function(
                "first",
                "tests::first",
                MirFunctionKind::KernelEntry,
                Some("tests::helper"),
            ),
        ])
        .unwrap();

        assert_eq!(module.functions[2].kind, MirFunctionKind::InternalHelper);
        let root_identities = module.functions[..2]
            .iter()
            .map(|function| function.source_identity_v1().unwrap())
            .collect::<Vec<_>>();
        assert!(root_identities.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[derive(Clone)]
    struct PortableSemanticEnvironment {
        target: TargetIdentity,
        abi: AbiLayout,
        launch: LaunchContract,
    }

    fn portable_semantic_environment() -> PortableSemanticEnvironment {
        PortableSemanticEnvironment {
            target: TargetIdentity::new(
                fe2o3_artifacts::IdentityText::new("amdgcn-amd-amdhsa").unwrap(),
                fe2o3_artifacts::IdentityText::new("gfx942:xnack-").unwrap(),
                PointerWidth::Bits64,
                Endianness::Little,
                vec![Capability::Atomics, Capability::AmdWave],
            )
            .unwrap(),
            abi: AbiLayout::new(0, 1, PointerWidth::Bits64, Vec::new()).unwrap(),
            launch: LaunchContract::new(
                1,
                BlockSize::AtMost(fe2o3_artifacts::Dimensions::new(256, 1, 1).unwrap()),
                fe2o3_artifacts::Dimensions::new(65_535, 1, 1).unwrap(),
                0,
                1_024,
            )
            .unwrap(),
        }
    }

    fn portable_digest(
        module: &MirModule,
        environment: &PortableSemanticEnvironment,
    ) -> PortableMirSemanticDigestV2 {
        module
            .portable_semantic_digest_v2(MirSemanticAdmissionInputsV2::new(
                "alpha",
                &environment.target,
                &environment.abi,
                &environment.launch,
            ))
            .unwrap()
    }

    fn portable_semantic_module() -> MirModule {
        let u32_ty = MirImportedType {
            kind: MirType::I32,
            rust: "u32".to_owned(),
            shape: MirTypeShape::U32,
        };
        MirModule {
            functions: vec![
                MirFunction {
                    export_name: "alpha".to_owned(),
                    rust_path: "checkout_a::build_hash_a::alpha".to_owned(),
                    kind: MirFunctionKind::KernelEntry,
                    typed_profile: Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3),
                    arg_count: 1,
                    local_count: 3,
                    locals: vec![
                        MirLocal {
                            index: 0,
                            role: MirLocalRole::Return,
                            ty: MirImportedType {
                                kind: MirType::Unit,
                                rust: "()".to_owned(),
                                shape: MirTypeShape::Unit,
                            },
                        },
                        MirLocal {
                            index: 1,
                            role: MirLocalRole::Arg,
                            ty: u32_ty.clone(),
                        },
                        MirLocal {
                            index: 2,
                            role: MirLocalRole::Temp,
                            ty: u32_ty.clone(),
                        },
                    ],
                    blocks: vec![
                        MirBlock {
                            index: 0,
                            statements: vec![MirStatement {
                                index: 0,
                                kind: MirStatementKind::Assign,
                                destination: Some(local_place(2)),
                                operands: vec![
                                    MirOperandRef::Place(MirPlaceRef {
                                        local: 1,
                                        projection: vec![
                                            MirProjectionElem::Deref,
                                            MirProjectionElem::Index { local: 2 },
                                        ],
                                    }),
                                    MirOperandRef::Constant {
                                        ty: u32_ty,
                                        literal: MirConstant::U32(7),
                                        value: "const 7_u32".to_owned(),
                                    },
                                ],
                                rvalue: Some(MirRvalueKind::Binary(MirBinaryOp::Add)),
                                operation: Some("add diagnostic".to_owned()),
                                source: None,
                            }],
                            terminator: Some(MirTerminator {
                                kind: MirTerminatorKind::Call {
                                    callee: Some(MirCallee::untrusted_for_test(
                                        "checkout_a::build_hash_a::helper",
                                    )),
                                    target: Some(1),
                                    destination: Some(local_place(0)),
                                    operands: vec![MirOperandRef::Place(local_place(2))],
                                },
                                source: None,
                            }),
                        },
                        MirBlock {
                            index: 1,
                            statements: Vec::new(),
                            terminator: Some(MirTerminator {
                                kind: MirTerminatorKind::Return,
                                source: None,
                            }),
                        },
                    ],
                    frontend_contract: None,
                    matrix_frontend_abi: None,
                },
                MirFunction {
                    export_name: "helper".to_owned(),
                    rust_path: "checkout_a::build_hash_a::helper".to_owned(),
                    kind: MirFunctionKind::InternalHelper,
                    typed_profile: None,
                    arg_count: 1,
                    local_count: 1,
                    locals: vec![MirLocal {
                        index: 0,
                        role: MirLocalRole::Return,
                        ty: MirImportedType {
                            kind: MirType::Unit,
                            rust: "()".to_owned(),
                            shape: MirTypeShape::Unit,
                        },
                    }],
                    blocks: vec![MirBlock {
                        index: 0,
                        statements: Vec::new(),
                        terminator: Some(MirTerminator {
                            kind: MirTerminatorKind::Return,
                            source: None,
                        }),
                    }],
                    frontend_contract: None,
                    matrix_frontend_abi: None,
                },
            ],
        }
    }

    fn general_two_kernel_function(
        export_name: &str,
        rust_path: &str,
        kind: MirFunctionKind,
        callee: Option<&str>,
    ) -> MirFunction {
        let blocks = if let Some(callee) = callee {
            vec![
                MirBlock {
                    index: 0,
                    statements: Vec::new(),
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Call {
                            callee: Some(MirCallee::untrusted_for_test(callee)),
                            target: Some(1),
                            destination: Some(local_place(0)),
                            operands: vec![MirOperandRef::Place(local_place(1))],
                        },
                        source: None,
                    }),
                },
                MirBlock {
                    index: 1,
                    statements: Vec::new(),
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Return,
                        source: None,
                    }),
                },
            ]
        } else {
            vec![MirBlock {
                index: 0,
                statements: Vec::new(),
                terminator: Some(MirTerminator {
                    kind: MirTerminatorKind::Return,
                    source: None,
                }),
            }]
        };
        MirFunction {
            export_name: export_name.to_owned(),
            rust_path: rust_path.to_owned(),
            kind,
            typed_profile: None,
            arg_count: 1,
            local_count: 2,
            locals: Vec::new(),
            blocks,
            frontend_contract: None,
            matrix_frontend_abi: None,
        }
    }

    fn simple_statement(index: usize, kind: MirStatementKind) -> MirStatement {
        MirStatement {
            index,
            kind,
            destination: None,
            operands: Vec::new(),
            rvalue: None,
            operation: None,
            source: None,
        }
    }

    fn local_place(local: usize) -> MirPlaceRef {
        MirPlaceRef {
            local,
            projection: Vec::new(),
        }
    }

    fn record_usize(record: &MirOpRecord, name: &'static str) -> Option<usize> {
        record.attrs.iter().find_map(|attr| {
            if attr.name == name
                && let dialect_mir::MirAttrValue::Usize(value) = &attr.value
            {
                return Some(*value);
            }
            None
        })
    }

    fn record_string<'a>(record: &'a MirOpRecord, name: &'static str) -> Option<&'a str> {
        record.attrs.iter().find_map(|attr| {
            if attr.name == name
                && let dialect_mir::MirAttrValue::String(value) = &attr.value
            {
                return Some(value.as_str());
            }
            None
        })
    }
}
