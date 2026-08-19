//! Bounded authenticated-fact model for one ordinary-Rust scalar kernel.
//!
//! This module is an integrity boundary, not a Rust or MIR parser. It joins
//! identities and facts that a rustc-facing collector observed in one session.
//! The resulting receipt contains no statements, expressions, executable
//! callback, or replacement body. A producer must derive every observation
//! from typed rustc APIs and keep its private same-session authority outside
//! this crate.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use sha2::{Digest as _, Sha256};

use crate::{
    FrontendSourceSpanV1, FrontendUnitV1, FunctionIdentityV1, FunctionRoleV1,
    KernelFrontendContractV1, StableTypeIdentityV1, encode_frontend_unit_v1,
    encode_kernel_frontend_contract_v1,
};

pub const MAX_SCALAR_IMPORT_FUNCTIONS_V1: usize = 128;
pub const MAX_SCALAR_IMPORT_CALLS_V1: usize = 512;
pub const MAX_SCALAR_IMPORT_ARGUMENTS_V1: usize = 32;
pub const MAX_SCALAR_IMPORT_UNSUPPORTED_OBSERVATIONS_V1: usize = 256;
pub const KERNEL_ITEM_ID_CANONICAL_BYTES_V1: usize = 112;
pub const KERNEL_INST_ID_CANONICAL_BYTES_V1: usize = 224;

const KERNEL_ITEM_MAGIC_V1: [u8; 8] = *b"F2KITEM1";
const KERNEL_INST_MAGIC_V1: [u8; 8] = *b"F2KINST1";
const KERNEL_IDENTITY_VERSION_V1: u16 = 1;
const KERNEL_ITEM_PAYLOAD_BYTES_V1: u32 = 96;
const KERNEL_INST_PAYLOAD_BYTES_V1: u32 = 208;
const KERNEL_IDENTITY_HEADER_BYTES_V1: usize = 16;
const IMPORT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3/ordinary-rust-scalar-import/v1";
const ROOT_INSTANCE_DOMAIN_V1: &[u8] = b"fe2o3/kernel-inst-function-identity/v1";

macro_rules! nonzero_identity {
    ($name:ident, $field:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            pub fn new(bytes: [u8; 32]) -> Result<Self, OrdinaryRustScalarValidationErrorV1> {
                if bytes == [0; 32] {
                    return Err(OrdinaryRustScalarValidationErrorV1::ZeroIdentity {
                        field: $field,
                    });
                }
                Ok(Self(bytes))
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

nonzero_identity!(RustItemDefinitionIdentityV1, "Rust item definition");
nonzero_identity!(
    ConcreteMonomorphizationIdentityV1,
    "concrete monomorphization"
);
nonzero_identity!(RustcSourceIdentityV1, "canonical source");
nonzero_identity!(RustcMirIdentityV1, "canonical MIR");

impl ConcreteMonomorphizationIdentityV1 {
    pub fn for_kernel_instance(instance: CanonicalKernelInstIdV1) -> Self {
        let mut digest = Sha256::new();
        append_digest_field(&mut digest, ROOT_INSTANCE_DOMAIN_V1);
        append_digest_field(&mut digest, instance.as_bytes());
        Self(digest.finalize().into())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalKernelItemIdV1([u8; KERNEL_ITEM_ID_CANONICAL_BYTES_V1]);

impl CanonicalKernelItemIdV1 {
    /// Builds the frozen V1 envelope from independently derived identity
    /// commitments. This only frames inert data; it does not authenticate the
    /// commitments or grant compiler authority.
    pub fn from_components(
        crate_identity: [u8; 32],
        rust_item_identity: [u8; 32],
        generic_definition_identity: [u8; 32],
    ) -> Result<Self, OrdinaryRustScalarValidationErrorV1> {
        let mut bytes = [0_u8; KERNEL_ITEM_ID_CANONICAL_BYTES_V1];
        bytes[..8].copy_from_slice(&KERNEL_ITEM_MAGIC_V1);
        bytes[8..10].copy_from_slice(&KERNEL_IDENTITY_VERSION_V1.to_le_bytes());
        bytes[12..16].copy_from_slice(&KERNEL_ITEM_PAYLOAD_BYTES_V1.to_le_bytes());
        bytes[16..48].copy_from_slice(&crate_identity);
        bytes[48..80].copy_from_slice(&rust_item_identity);
        bytes[80..112].copy_from_slice(&generic_definition_identity);
        RustItemDefinitionIdentityV1::new(rust_item_identity)?;
        Self::new(bytes)
    }

    pub fn new(
        bytes: [u8; KERNEL_ITEM_ID_CANONICAL_BYTES_V1],
    ) -> Result<Self, OrdinaryRustScalarValidationErrorV1> {
        validate_identity_envelope(
            &bytes,
            KERNEL_ITEM_MAGIC_V1,
            KERNEL_ITEM_PAYLOAD_BYTES_V1,
            "kernel item identity",
        )?;
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; KERNEL_ITEM_ID_CANONICAL_BYTES_V1] {
        &self.0
    }

    pub fn rust_item_identity(self) -> RustItemDefinitionIdentityV1 {
        let bytes = self.0
            [KERNEL_IDENTITY_HEADER_BYTES_V1 + 32..KERNEL_IDENTITY_HEADER_BYTES_V1 + 64]
            .try_into()
            .expect("fixed-width Rust item identity");
        RustItemDefinitionIdentityV1(bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalKernelInstIdV1([u8; KERNEL_INST_ID_CANONICAL_BYTES_V1]);

impl CanonicalKernelInstIdV1 {
    /// Builds the frozen V1 concrete-instance envelope from one canonical item
    /// and independently derived specialization commitments.
    pub fn from_components(
        item: CanonicalKernelItemIdV1,
        type_arguments_identity: [u8; 32],
        const_arguments_identity: [u8; 32],
        cfg_identity: [u8; 32],
    ) -> Result<Self, OrdinaryRustScalarValidationErrorV1> {
        for (field, identity) in [
            ("kernel type arguments", type_arguments_identity),
            ("kernel const arguments", const_arguments_identity),
            ("kernel cfg", cfg_identity),
        ] {
            if identity == [0; 32] {
                return Err(OrdinaryRustScalarValidationErrorV1::ZeroIdentity { field });
            }
        }
        let mut bytes = [0_u8; KERNEL_INST_ID_CANONICAL_BYTES_V1];
        bytes[..8].copy_from_slice(&KERNEL_INST_MAGIC_V1);
        bytes[8..10].copy_from_slice(&KERNEL_IDENTITY_VERSION_V1.to_le_bytes());
        bytes[12..16].copy_from_slice(&KERNEL_INST_PAYLOAD_BYTES_V1.to_le_bytes());
        bytes[16..128].copy_from_slice(item.as_bytes());
        bytes[128..160].copy_from_slice(&type_arguments_identity);
        bytes[160..192].copy_from_slice(&const_arguments_identity);
        bytes[192..224].copy_from_slice(&cfg_identity);
        Self::new(bytes)
    }

    pub fn new(
        bytes: [u8; KERNEL_INST_ID_CANONICAL_BYTES_V1],
    ) -> Result<Self, OrdinaryRustScalarValidationErrorV1> {
        validate_identity_envelope(
            &bytes,
            KERNEL_INST_MAGIC_V1,
            KERNEL_INST_PAYLOAD_BYTES_V1,
            "kernel instance identity",
        )?;
        let embedded: [u8; KERNEL_ITEM_ID_CANONICAL_BYTES_V1] = bytes
            [KERNEL_IDENTITY_HEADER_BYTES_V1
                ..KERNEL_IDENTITY_HEADER_BYTES_V1 + KERNEL_ITEM_ID_CANONICAL_BYTES_V1]
            .try_into()
            .expect("fixed-width embedded kernel item");
        CanonicalKernelItemIdV1::new(embedded)?;
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; KERNEL_INST_ID_CANONICAL_BYTES_V1] {
        &self.0
    }

    pub fn item(self) -> CanonicalKernelItemIdV1 {
        let bytes = self.0[KERNEL_IDENTITY_HEADER_BYTES_V1
            ..KERNEL_IDENTITY_HEADER_BYTES_V1 + KERNEL_ITEM_ID_CANONICAL_BYTES_V1]
            .try_into()
            .expect("fixed-width embedded kernel item");
        CanonicalKernelItemIdV1(bytes)
    }
}

fn validate_identity_envelope(
    bytes: &[u8],
    magic: [u8; 8],
    payload_bytes: u32,
    field: &'static str,
) -> Result<(), OrdinaryRustScalarValidationErrorV1> {
    if bytes[..8] != magic
        || u16::from_le_bytes(bytes[8..10].try_into().expect("fixed-width version"))
            != KERNEL_IDENTITY_VERSION_V1
        || u16::from_le_bytes(bytes[10..12].try_into().expect("fixed-width flags")) != 0
        || u32::from_le_bytes(bytes[12..16].try_into().expect("fixed-width length"))
            != payload_bytes
    {
        return Err(OrdinaryRustScalarValidationErrorV1::InvalidIdentityEnvelope { field });
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum FunctionImportRoleV1 {
    Kernel = 1,
    Helper = 2,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RustcFunctionKindV1 {
    OrdinaryItem = 1,
    Intrinsic = 2,
    Closure = 3,
    Coroutine = 4,
    Virtual = 5,
    FunctionPointer = 6,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RustcAbiPassModeV1 {
    Ignore = 1,
    Direct = 2,
    Pair = 3,
    Indirect = 4,
    Cast = 5,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum RustcCallingConventionV1 {
    Rust = 1,
    C = 2,
    System = 3,
    GpuKernel = 4,
    Other = 5,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcAbiValueV1 {
    rust_type: StableTypeIdentityV1,
    layout_identity: [u8; 32],
    size: u64,
    alignment: u64,
    pass_mode: RustcAbiPassModeV1,
}

impl RustcAbiValueV1 {
    pub fn new(
        rust_type: StableTypeIdentityV1,
        layout_identity: [u8; 32],
        size: u64,
        alignment: u64,
        pass_mode: RustcAbiPassModeV1,
    ) -> Result<Self, OrdinaryRustScalarValidationErrorV1> {
        if layout_identity == [0; 32] {
            return Err(OrdinaryRustScalarValidationErrorV1::ZeroIdentity {
                field: "rustc ABI layout",
            });
        }
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(OrdinaryRustScalarValidationErrorV1::InvalidAbiLayout);
        }
        if matches!(pass_mode, RustcAbiPassModeV1::Ignore) != (size == 0) {
            return Err(OrdinaryRustScalarValidationErrorV1::InvalidAbiLayout);
        }
        Ok(Self {
            rust_type,
            layout_identity,
            size,
            alignment,
            pass_mode,
        })
    }

    pub const fn rust_type(&self) -> StableTypeIdentityV1 {
        self.rust_type
    }

    pub const fn layout_identity(&self) -> &[u8; 32] {
        &self.layout_identity
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    pub const fn pass_mode(&self) -> RustcAbiPassModeV1 {
        self.pass_mode
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustcFnAbiFactsV1 {
    identity: [u8; 32],
    calling_convention: RustcCallingConventionV1,
    arguments: Vec<RustcAbiValueV1>,
    return_value: RustcAbiValueV1,
    c_variadic: bool,
    can_unwind: bool,
}

impl RustcFnAbiFactsV1 {
    pub fn new(
        identity: [u8; 32],
        calling_convention: RustcCallingConventionV1,
        arguments: Vec<RustcAbiValueV1>,
        return_value: RustcAbiValueV1,
        c_variadic: bool,
        can_unwind: bool,
    ) -> Result<Self, OrdinaryRustScalarValidationErrorV1> {
        if identity == [0; 32] {
            return Err(OrdinaryRustScalarValidationErrorV1::ZeroIdentity {
                field: "rustc FnAbi",
            });
        }
        if arguments.len() > MAX_SCALAR_IMPORT_ARGUMENTS_V1 {
            return Err(OrdinaryRustScalarValidationErrorV1::BoundExceeded {
                field: "rustc FnAbi arguments",
                actual: arguments.len(),
                max: MAX_SCALAR_IMPORT_ARGUMENTS_V1,
            });
        }
        Ok(Self {
            identity,
            calling_convention,
            arguments,
            return_value,
            c_variadic,
            can_unwind,
        })
    }

    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub const fn calling_convention(&self) -> RustcCallingConventionV1 {
        self.calling_convention
    }

    pub fn arguments(&self) -> &[RustcAbiValueV1] {
        &self.arguments
    }

    pub const fn return_value(&self) -> &RustcAbiValueV1 {
        &self.return_value
    }

    pub const fn is_c_variadic(&self) -> bool {
        self.c_variadic
    }

    pub const fn can_unwind(&self) -> bool {
        self.can_unwind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCallObservationV1 {
    callee: ConcreteMonomorphizationIdentityV1,
    source_span: FrontendSourceSpanV1,
}

impl DirectCallObservationV1 {
    pub const fn new(
        callee: ConcreteMonomorphizationIdentityV1,
        source_span: FrontendSourceSpanV1,
    ) -> Self {
        Self {
            callee,
            source_span,
        }
    }

    pub const fn callee(&self) -> ConcreteMonomorphizationIdentityV1 {
        self.callee
    }

    pub const fn source_span(&self) -> &FrontendSourceSpanV1 {
        &self.source_span
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReachableFunctionObservationV1 {
    frontend_identity: FunctionIdentityV1,
    role: FunctionImportRoleV1,
    item_identity: RustItemDefinitionIdentityV1,
    monomorphization: ConcreteMonomorphizationIdentityV1,
    source_identity: RustcSourceIdentityV1,
    mir_identity: RustcMirIdentityV1,
    source_span: FrontendSourceSpanV1,
    function_kind: RustcFunctionKindV1,
    is_concrete: bool,
    fn_abi: RustcFnAbiFactsV1,
    calls: Vec<DirectCallObservationV1>,
}

impl ReachableFunctionObservationV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        frontend_identity: FunctionIdentityV1,
        role: FunctionImportRoleV1,
        item_identity: RustItemDefinitionIdentityV1,
        monomorphization: ConcreteMonomorphizationIdentityV1,
        source_identity: RustcSourceIdentityV1,
        mir_identity: RustcMirIdentityV1,
        source_span: FrontendSourceSpanV1,
        function_kind: RustcFunctionKindV1,
        is_concrete: bool,
        fn_abi: RustcFnAbiFactsV1,
        mut calls: Vec<DirectCallObservationV1>,
    ) -> Result<Self, OrdinaryRustScalarValidationErrorV1> {
        if calls.len() > MAX_SCALAR_IMPORT_CALLS_V1 {
            return Err(OrdinaryRustScalarValidationErrorV1::BoundExceeded {
                field: "direct calls per function",
                actual: calls.len(),
                max: MAX_SCALAR_IMPORT_CALLS_V1,
            });
        }
        calls.sort_by(compare_calls);
        if calls.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(OrdinaryRustScalarValidationErrorV1::DuplicateDirectCall);
        }
        Ok(Self {
            frontend_identity,
            role,
            item_identity,
            monomorphization,
            source_identity,
            mir_identity,
            source_span,
            function_kind,
            is_concrete,
            fn_abi,
            calls,
        })
    }

    pub const fn frontend_identity(&self) -> FunctionIdentityV1 {
        self.frontend_identity
    }

    pub const fn role(&self) -> FunctionImportRoleV1 {
        self.role
    }

    pub const fn item_identity(&self) -> RustItemDefinitionIdentityV1 {
        self.item_identity
    }

    pub const fn monomorphization(&self) -> ConcreteMonomorphizationIdentityV1 {
        self.monomorphization
    }

    pub const fn source_identity(&self) -> RustcSourceIdentityV1 {
        self.source_identity
    }

    pub const fn mir_identity(&self) -> RustcMirIdentityV1 {
        self.mir_identity
    }

    pub const fn source_span(&self) -> &FrontendSourceSpanV1 {
        &self.source_span
    }

    pub const fn function_kind(&self) -> RustcFunctionKindV1 {
        self.function_kind
    }

    pub const fn is_concrete(&self) -> bool {
        self.is_concrete
    }

    pub const fn fn_abi(&self) -> &RustcFnAbiFactsV1 {
        &self.fn_abi
    }

    pub fn calls(&self) -> &[DirectCallObservationV1] {
        &self.calls
    }
}

fn compare_calls(
    left: &DirectCallObservationV1,
    right: &DirectCallObservationV1,
) -> std::cmp::Ordering {
    left.callee
        .cmp(&right.callee)
        .then_with(|| span_key(&left.source_span).cmp(&span_key(&right.source_span)))
}

fn span_key(span: &FrontendSourceSpanV1) -> (&str, (u32, u32), (u32, u32)) {
    (span.file(), span.start(), span.end())
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum UnsupportedRustBehaviorKindV1 {
    IndirectCall = 1,
    VirtualDispatch = 2,
    ClosureOrCoroutine = 3,
    Intrinsic = 4,
    ForeignCall = 5,
    InlineAssembly = 6,
    Unwind = 7,
    Panic = 8,
    Allocation = 9,
    DynamicDrop = 10,
    ThreadLocal = 11,
    MutableStatic = 12,
    TargetDependentType = 13,
    UnsupportedMirStatement = 14,
    UnsupportedMirTerminator = 15,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnsupportedRustBehaviorObservationV1 {
    function: ConcreteMonomorphizationIdentityV1,
    kind: UnsupportedRustBehaviorKindV1,
    source_span: FrontendSourceSpanV1,
}

impl UnsupportedRustBehaviorObservationV1 {
    pub const fn new(
        function: ConcreteMonomorphizationIdentityV1,
        kind: UnsupportedRustBehaviorKindV1,
        source_span: FrontendSourceSpanV1,
    ) -> Self {
        Self {
            function,
            kind,
            source_span,
        }
    }

    pub const fn function(&self) -> ConcreteMonomorphizationIdentityV1 {
        self.function
    }

    pub const fn kind(&self) -> UnsupportedRustBehaviorKindV1 {
        self.kind
    }

    pub const fn source_span(&self) -> &FrontendSourceSpanV1 {
        &self.source_span
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrdinaryRustScalarKernelObservationV1 {
    frontend_unit: FrontendUnitV1,
    kernel_item: CanonicalKernelItemIdV1,
    kernel_instance: CanonicalKernelInstIdV1,
    kernel_contract: KernelFrontendContractV1,
    functions: Vec<ReachableFunctionObservationV1>,
    unsupported: Vec<UnsupportedRustBehaviorObservationV1>,
}

impl OrdinaryRustScalarKernelObservationV1 {
    pub fn new(
        frontend_unit: FrontendUnitV1,
        kernel_item: CanonicalKernelItemIdV1,
        kernel_instance: CanonicalKernelInstIdV1,
        kernel_contract: KernelFrontendContractV1,
        functions: Vec<ReachableFunctionObservationV1>,
        unsupported: Vec<UnsupportedRustBehaviorObservationV1>,
    ) -> Result<Self, OrdinaryRustScalarValidationErrorV1> {
        if functions.len() > MAX_SCALAR_IMPORT_FUNCTIONS_V1 {
            return Err(OrdinaryRustScalarValidationErrorV1::BoundExceeded {
                field: "reachable functions",
                actual: functions.len(),
                max: MAX_SCALAR_IMPORT_FUNCTIONS_V1,
            });
        }
        if unsupported.len() > MAX_SCALAR_IMPORT_UNSUPPORTED_OBSERVATIONS_V1 {
            return Err(OrdinaryRustScalarValidationErrorV1::BoundExceeded {
                field: "unsupported observations",
                actual: unsupported.len(),
                max: MAX_SCALAR_IMPORT_UNSUPPORTED_OBSERVATIONS_V1,
            });
        }
        Ok(Self {
            frontend_unit,
            kernel_item,
            kernel_instance,
            kernel_contract,
            functions,
            unsupported,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarImportCallChainFrameV1 {
    function: ConcreteMonomorphizationIdentityV1,
    source_span: FrontendSourceSpanV1,
    call_site: Option<FrontendSourceSpanV1>,
}

impl ScalarImportCallChainFrameV1 {
    pub const fn function(&self) -> ConcreteMonomorphizationIdentityV1 {
        self.function
    }

    pub const fn source_span(&self) -> &FrontendSourceSpanV1 {
        &self.source_span
    }

    pub const fn call_site(&self) -> Option<&FrontendSourceSpanV1> {
        self.call_site.as_ref()
    }
}

/// Integrity-checked reconciliation of one rustc observation set.
///
/// This type deliberately does not implement `Clone`; consumers should retain
/// one custody path when a rustc integration wraps it in private authority.
/// Its fields are private, so external code cannot construct a receipt by
/// asserting digest values.
///
/// ```compile_fail
/// use fe2o3_rustc_front::AuthenticatedOrdinaryRustScalarKernelImportV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<AuthenticatedOrdinaryRustScalarKernelImportV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_rustc_front::AuthenticatedOrdinaryRustScalarKernelImportV1;
/// let _forged = AuthenticatedOrdinaryRustScalarKernelImportV1 {};
/// ```
pub struct AuthenticatedOrdinaryRustScalarKernelImportV1 {
    frontend_unit: FrontendUnitV1,
    kernel_item: CanonicalKernelItemIdV1,
    kernel_instance: CanonicalKernelInstIdV1,
    kernel_contract: KernelFrontendContractV1,
    functions: Vec<ReachableFunctionObservationV1>,
    root: ConcreteMonomorphizationIdentityV1,
    parent: BTreeMap<
        ConcreteMonomorphizationIdentityV1,
        (ConcreteMonomorphizationIdentityV1, FrontendSourceSpanV1),
    >,
    source_closure_identity: [u8; 32],
    mir_closure_identity: [u8; 32],
    import_identity: [u8; 32],
}

impl fmt::Debug for AuthenticatedOrdinaryRustScalarKernelImportV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthenticatedOrdinaryRustScalarKernelImportV1")
            .field("root", &self.root)
            .field("function_count", &self.functions.len())
            .field("source_closure_identity", &self.source_closure_identity)
            .field("mir_closure_identity", &self.mir_closure_identity)
            .field("import_identity", &self.import_identity)
            .finish_non_exhaustive()
    }
}

impl AuthenticatedOrdinaryRustScalarKernelImportV1 {
    pub const fn frontend_unit(&self) -> &FrontendUnitV1 {
        &self.frontend_unit
    }

    pub const fn kernel_item(&self) -> CanonicalKernelItemIdV1 {
        self.kernel_item
    }

    pub const fn kernel_instance(&self) -> CanonicalKernelInstIdV1 {
        self.kernel_instance
    }

    pub const fn kernel_contract(&self) -> KernelFrontendContractV1 {
        self.kernel_contract
    }

    pub fn functions(&self) -> &[ReachableFunctionObservationV1] {
        &self.functions
    }

    pub const fn root(&self) -> ConcreteMonomorphizationIdentityV1 {
        self.root
    }

    pub const fn source_closure_identity(&self) -> &[u8; 32] {
        &self.source_closure_identity
    }

    pub const fn mir_closure_identity(&self) -> &[u8; 32] {
        &self.mir_closure_identity
    }

    pub const fn import_identity(&self) -> &[u8; 32] {
        &self.import_identity
    }

    pub fn call_chain_to(
        &self,
        function: ConcreteMonomorphizationIdentityV1,
    ) -> Option<Vec<ScalarImportCallChainFrameV1>> {
        build_call_chain(self.root, function, &self.functions, &self.parent)
    }

    /// This integrity record is inert until a private rustc integration binds it
    /// to same-session compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// There is no executable body or callback in this receipt.
    pub const fn grants_execution_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u16)]
pub enum OrdinaryRustScalarDiagnosticCodeV1 {
    InvalidIdentityEnvelope = 1001,
    KernelItemMismatch = 1002,
    ZeroIdentity = 1003,
    KernelRootIdentityMismatch = 1004,
    BoundExceeded = 1101,
    KernelRootCount = 1102,
    FunctionSetMismatch = 1103,
    FunctionRoleMismatch = 1104,
    SignatureMismatch = 1105,
    DuplicateMonomorphization = 1106,
    DuplicateFrontendObservation = 1107,
    UnknownCallee = 1108,
    UnreachableHelper = 1109,
    RecursiveCall = 1110,
    DuplicateDirectCall = 1111,
    NonOrdinaryFunction = 1201,
    NonConcreteMonomorphization = 1202,
    UnsupportedFnAbi = 1203,
    UnsupportedLaunch = 1204,
    UnsupportedBehavior = 1205,
    InvalidAbiLayout = 1206,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OrdinaryRustScalarValidationErrorV1 {
    InvalidIdentityEnvelope {
        field: &'static str,
    },
    KernelItemMismatch,
    KernelRootIdentityMismatch,
    ZeroIdentity {
        field: &'static str,
    },
    BoundExceeded {
        field: &'static str,
        actual: usize,
        max: usize,
    },
    KernelRootCount {
        actual: usize,
    },
    MissingFunctionObservation {
        function: FunctionIdentityV1,
    },
    UnexpectedFunctionObservation {
        function: FunctionIdentityV1,
    },
    FunctionRoleMismatch {
        function: FunctionIdentityV1,
    },
    SignatureMismatch {
        function: FunctionIdentityV1,
    },
    DuplicateMonomorphization {
        function: ConcreteMonomorphizationIdentityV1,
    },
    DuplicateFrontendObservation {
        function: FunctionIdentityV1,
    },
    UnknownCallee {
        caller: ConcreteMonomorphizationIdentityV1,
        callee: ConcreteMonomorphizationIdentityV1,
    },
    UnreachableHelper {
        function: ConcreteMonomorphizationIdentityV1,
    },
    RecursiveCall {
        function: ConcreteMonomorphizationIdentityV1,
    },
    DuplicateDirectCall,
    NonOrdinaryFunction {
        function: ConcreteMonomorphizationIdentityV1,
        kind: RustcFunctionKindV1,
        call_chain: Vec<ScalarImportCallChainFrameV1>,
    },
    NonConcreteMonomorphization {
        function: ConcreteMonomorphizationIdentityV1,
        call_chain: Vec<ScalarImportCallChainFrameV1>,
    },
    UnsupportedFnAbi {
        function: ConcreteMonomorphizationIdentityV1,
        call_chain: Vec<ScalarImportCallChainFrameV1>,
    },
    UnsupportedLaunch,
    UnsupportedBehavior {
        kind: UnsupportedRustBehaviorKindV1,
        source_span: FrontendSourceSpanV1,
        call_chain: Vec<ScalarImportCallChainFrameV1>,
    },
    InvalidAbiLayout,
}

impl OrdinaryRustScalarValidationErrorV1 {
    pub const fn code(&self) -> OrdinaryRustScalarDiagnosticCodeV1 {
        match self {
            Self::InvalidIdentityEnvelope { .. } => {
                OrdinaryRustScalarDiagnosticCodeV1::InvalidIdentityEnvelope
            }
            Self::KernelItemMismatch => OrdinaryRustScalarDiagnosticCodeV1::KernelItemMismatch,
            Self::KernelRootIdentityMismatch => {
                OrdinaryRustScalarDiagnosticCodeV1::KernelRootIdentityMismatch
            }
            Self::ZeroIdentity { .. } => OrdinaryRustScalarDiagnosticCodeV1::ZeroIdentity,
            Self::BoundExceeded { .. } => OrdinaryRustScalarDiagnosticCodeV1::BoundExceeded,
            Self::KernelRootCount { .. } => OrdinaryRustScalarDiagnosticCodeV1::KernelRootCount,
            Self::MissingFunctionObservation { .. }
            | Self::UnexpectedFunctionObservation { .. } => {
                OrdinaryRustScalarDiagnosticCodeV1::FunctionSetMismatch
            }
            Self::FunctionRoleMismatch { .. } => {
                OrdinaryRustScalarDiagnosticCodeV1::FunctionRoleMismatch
            }
            Self::SignatureMismatch { .. } => OrdinaryRustScalarDiagnosticCodeV1::SignatureMismatch,
            Self::DuplicateMonomorphization { .. } => {
                OrdinaryRustScalarDiagnosticCodeV1::DuplicateMonomorphization
            }
            Self::DuplicateFrontendObservation { .. } => {
                OrdinaryRustScalarDiagnosticCodeV1::DuplicateFrontendObservation
            }
            Self::UnknownCallee { .. } => OrdinaryRustScalarDiagnosticCodeV1::UnknownCallee,
            Self::UnreachableHelper { .. } => OrdinaryRustScalarDiagnosticCodeV1::UnreachableHelper,
            Self::RecursiveCall { .. } => OrdinaryRustScalarDiagnosticCodeV1::RecursiveCall,
            Self::DuplicateDirectCall => OrdinaryRustScalarDiagnosticCodeV1::DuplicateDirectCall,
            Self::NonOrdinaryFunction { .. } => {
                OrdinaryRustScalarDiagnosticCodeV1::NonOrdinaryFunction
            }
            Self::NonConcreteMonomorphization { .. } => {
                OrdinaryRustScalarDiagnosticCodeV1::NonConcreteMonomorphization
            }
            Self::UnsupportedFnAbi { .. } => OrdinaryRustScalarDiagnosticCodeV1::UnsupportedFnAbi,
            Self::UnsupportedLaunch => OrdinaryRustScalarDiagnosticCodeV1::UnsupportedLaunch,
            Self::UnsupportedBehavior { .. } => {
                OrdinaryRustScalarDiagnosticCodeV1::UnsupportedBehavior
            }
            Self::InvalidAbiLayout => OrdinaryRustScalarDiagnosticCodeV1::InvalidAbiLayout,
        }
    }
}

impl fmt::Display for OrdinaryRustScalarValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "FE2O3-RUST-SCALAR-{:04}: ", self.code() as u16)?;
        match self {
            Self::InvalidIdentityEnvelope { field } => {
                write!(formatter, "{field} is not the exact canonical V1 envelope")
            }
            Self::KernelItemMismatch => formatter.write_str(
                "kernel instance does not embed the selected kernel item identity",
            ),
            Self::KernelRootIdentityMismatch => formatter.write_str(
                "kernel root does not match the selected concrete instance and Rust item identity",
            ),
            Self::ZeroIdentity { field } => write!(formatter, "{field} identity is all zero"),
            Self::BoundExceeded { field, actual, max } => {
                write!(formatter, "{field} bound exceeded: {actual} > {max}")
            }
            Self::KernelRootCount { actual } => {
                write!(formatter, "expected exactly one kernel root, found {actual}")
            }
            Self::MissingFunctionObservation { function } => {
                write!(formatter, "frontend function {function:?} has no rustc observation")
            }
            Self::UnexpectedFunctionObservation { function } => {
                write!(formatter, "rustc observation {function:?} is absent from the frontend unit")
            }
            Self::FunctionRoleMismatch { function } => {
                write!(formatter, "function {function:?} has inconsistent kernel/helper roles")
            }
            Self::SignatureMismatch { function } => {
                write!(formatter, "function {function:?} has inconsistent Rust signature and FnAbi")
            }
            Self::DuplicateMonomorphization { function } => {
                write!(formatter, "concrete monomorphization {function:?} is duplicated")
            }
            Self::DuplicateFrontendObservation { function } => {
                write!(formatter, "frontend function observation {function:?} is duplicated")
            }
            Self::UnknownCallee { caller, callee } => {
                write!(formatter, "function {caller:?} calls uncollected function {callee:?}")
            }
            Self::UnreachableHelper { function } => {
                write!(formatter, "helper monomorphization {function:?} is not kernel-reachable")
            }
            Self::RecursiveCall { function } => {
                write!(formatter, "recursive call cycle reaches {function:?}")
            }
            Self::DuplicateDirectCall => {
                formatter.write_str("direct-call observations contain an exact duplicate")
            }
            Self::NonOrdinaryFunction { function, kind, .. } => {
                write!(formatter, "function {function:?} is not an ordinary Rust item: {kind:?}")
            }
            Self::NonConcreteMonomorphization { function, .. } => {
                write!(formatter, "function {function:?} is not fully monomorphized")
            }
            Self::UnsupportedFnAbi { function, .. } => {
                write!(
                    formatter,
                    "function {function:?} does not have non-variadic, non-unwinding Rust FnAbi"
                )
            }
            Self::UnsupportedLaunch => formatter.write_str(
                "scalar milestone requires safe Rust and exact required/maximum workgroup [1, 1, 1]",
            ),
            Self::UnsupportedBehavior { kind, source_span, .. } => write!(
                formatter,
                "unsupported Rust behavior {kind:?} at {}:{}:{}",
                source_span.file(),
                source_span.start().0,
                source_span.start().1
            ),
            Self::InvalidAbiLayout => formatter.write_str(
                "rustc ABI value must have a nonzero power-of-two alignment and size/pass-mode agreement",
            ),
        }
    }
}

impl std::error::Error for OrdinaryRustScalarValidationErrorV1 {}

pub fn authenticate_ordinary_rust_scalar_kernel_v1(
    mut observation: OrdinaryRustScalarKernelObservationV1,
) -> Result<AuthenticatedOrdinaryRustScalarKernelImportV1, OrdinaryRustScalarValidationErrorV1> {
    if observation.kernel_instance.item() != observation.kernel_item {
        return Err(OrdinaryRustScalarValidationErrorV1::KernelItemMismatch);
    }
    validate_scalar_launch(observation.kernel_contract)?;

    observation
        .functions
        .sort_by_key(ReachableFunctionObservationV1::monomorphization);
    let total_calls = observation
        .functions
        .iter()
        .try_fold(0_usize, |count, function| {
            count.checked_add(function.calls.len())
        })
        .unwrap_or(usize::MAX);
    if total_calls > MAX_SCALAR_IMPORT_CALLS_V1 {
        return Err(OrdinaryRustScalarValidationErrorV1::BoundExceeded {
            field: "reachable direct calls",
            actual: total_calls,
            max: MAX_SCALAR_IMPORT_CALLS_V1,
        });
    }
    if let Some(pair) = observation
        .functions
        .windows(2)
        .find(|pair| pair[0].monomorphization == pair[1].monomorphization)
    {
        return Err(
            OrdinaryRustScalarValidationErrorV1::DuplicateMonomorphization {
                function: pair[0].monomorphization,
            },
        );
    }

    let roots = observation
        .functions
        .iter()
        .filter(|function| function.role == FunctionImportRoleV1::Kernel)
        .collect::<Vec<_>>();
    if roots.len() != 1 {
        return Err(OrdinaryRustScalarValidationErrorV1::KernelRootCount {
            actual: roots.len(),
        });
    }
    let root = roots[0].monomorphization;
    if root != ConcreteMonomorphizationIdentityV1::for_kernel_instance(observation.kernel_instance)
        || roots[0].item_identity != observation.kernel_item.rust_item_identity()
    {
        return Err(OrdinaryRustScalarValidationErrorV1::KernelRootIdentityMismatch);
    }

    reconcile_frontend_functions(&observation.frontend_unit, &observation.functions)?;
    let parent = validate_reachable_call_graph(root, &observation.functions)?;

    for function in &observation.functions {
        let call_chain = build_call_chain(
            root,
            function.monomorphization,
            &observation.functions,
            &parent,
        )
        .expect("validated function is reachable");
        if function.function_kind != RustcFunctionKindV1::OrdinaryItem {
            return Err(OrdinaryRustScalarValidationErrorV1::NonOrdinaryFunction {
                function: function.monomorphization,
                kind: function.function_kind,
                call_chain,
            });
        }
        if !function.is_concrete {
            return Err(
                OrdinaryRustScalarValidationErrorV1::NonConcreteMonomorphization {
                    function: function.monomorphization,
                    call_chain,
                },
            );
        }
        if function.fn_abi.calling_convention != RustcCallingConventionV1::Rust
            || function.fn_abi.c_variadic
            || function.fn_abi.can_unwind
        {
            return Err(OrdinaryRustScalarValidationErrorV1::UnsupportedFnAbi {
                function: function.monomorphization,
                call_chain,
            });
        }
    }

    observation.unsupported.sort_by(|left, right| {
        left.function
            .cmp(&right.function)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| span_key(&left.source_span).cmp(&span_key(&right.source_span)))
    });
    if let Some(unsupported) = observation.unsupported.first() {
        let call_chain =
            build_call_chain(root, unsupported.function, &observation.functions, &parent).ok_or(
                OrdinaryRustScalarValidationErrorV1::UnreachableHelper {
                    function: unsupported.function,
                },
            )?;
        return Err(OrdinaryRustScalarValidationErrorV1::UnsupportedBehavior {
            kind: unsupported.kind,
            source_span: unsupported.source_span.clone(),
            call_chain,
        });
    }

    let source_closure_identity = closure_identity(
        b"fe2o3/ordinary-rust-source-closure/v1",
        observation.functions.iter().map(|function| {
            (
                function.monomorphization,
                function.source_identity.as_bytes(),
            )
        }),
    );
    let mir_closure_identity = closure_identity(
        b"fe2o3/ordinary-rust-mir-closure/v1",
        observation
            .functions
            .iter()
            .map(|function| (function.monomorphization, function.mir_identity.as_bytes())),
    );
    let import_identity =
        calculate_import_identity(&observation, source_closure_identity, mir_closure_identity);

    Ok(AuthenticatedOrdinaryRustScalarKernelImportV1 {
        frontend_unit: observation.frontend_unit,
        kernel_item: observation.kernel_item,
        kernel_instance: observation.kernel_instance,
        kernel_contract: observation.kernel_contract,
        functions: observation.functions,
        root,
        parent,
        source_closure_identity,
        mir_closure_identity,
        import_identity,
    })
}

fn validate_scalar_launch(
    contract: KernelFrontendContractV1,
) -> Result<(), OrdinaryRustScalarValidationErrorV1> {
    let Some(launch) = contract.launch() else {
        return Err(OrdinaryRustScalarValidationErrorV1::UnsupportedLaunch);
    };
    if contract.unsafe_assembly().is_some()
        || launch.required().map(|value| value.as_array()) != Some([1, 1, 1])
        || launch.maximum().map(|value| value.as_array()) != Some([1, 1, 1])
        || launch.min_workgroups_per_compute_unit().is_some()
    {
        return Err(OrdinaryRustScalarValidationErrorV1::UnsupportedLaunch);
    }
    Ok(())
}

fn reconcile_frontend_functions(
    unit: &FrontendUnitV1,
    observations: &[ReachableFunctionObservationV1],
) -> Result<(), OrdinaryRustScalarValidationErrorV1> {
    let by_frontend = observations
        .iter()
        .map(|function| (function.frontend_identity, function))
        .collect::<BTreeMap<_, _>>();
    if by_frontend.len() != observations.len() {
        let mut seen = BTreeSet::new();
        let function = observations
            .iter()
            .find_map(|function| {
                (!seen.insert(function.frontend_identity)).then_some(function.frontend_identity)
            })
            .expect("map cardinality proves a duplicate frontend identity");
        return Err(OrdinaryRustScalarValidationErrorV1::DuplicateFrontendObservation { function });
    }
    for function in unit.functions() {
        let Some(observation) = by_frontend.get(&function.identity()) else {
            return Err(
                OrdinaryRustScalarValidationErrorV1::MissingFunctionObservation {
                    function: function.identity(),
                },
            );
        };
        let expected_role = match function.role() {
            FunctionRoleV1::Kernel => FunctionImportRoleV1::Kernel,
            FunctionRoleV1::Helper => FunctionImportRoleV1::Helper,
        };
        if observation.role != expected_role {
            return Err(OrdinaryRustScalarValidationErrorV1::FunctionRoleMismatch {
                function: function.identity(),
            });
        }
        let signature = function.signature();
        if signature.parameters().len() != observation.fn_abi.arguments.len()
            || signature
                .parameters()
                .iter()
                .zip(&observation.fn_abi.arguments)
                .any(|(rust_type, abi)| *rust_type != abi.rust_type)
            || signature.return_type() != observation.fn_abi.return_value.rust_type
        {
            return Err(OrdinaryRustScalarValidationErrorV1::SignatureMismatch {
                function: function.identity(),
            });
        }
    }
    let unit_identities = unit
        .functions()
        .iter()
        .map(|function| function.identity())
        .collect::<BTreeSet<_>>();
    if let Some(function) = observations
        .iter()
        .find(|function| !unit_identities.contains(&function.frontend_identity))
    {
        return Err(
            OrdinaryRustScalarValidationErrorV1::UnexpectedFunctionObservation {
                function: function.frontend_identity,
            },
        );
    }
    Ok(())
}

fn validate_reachable_call_graph(
    root: ConcreteMonomorphizationIdentityV1,
    functions: &[ReachableFunctionObservationV1],
) -> Result<
    BTreeMap<
        ConcreteMonomorphizationIdentityV1,
        (ConcreteMonomorphizationIdentityV1, FrontendSourceSpanV1),
    >,
    OrdinaryRustScalarValidationErrorV1,
> {
    let by_instance = functions
        .iter()
        .map(|function| (function.monomorphization, function))
        .collect::<BTreeMap<_, _>>();
    for function in functions {
        for call in &function.calls {
            if !by_instance.contains_key(&call.callee) {
                return Err(OrdinaryRustScalarValidationErrorV1::UnknownCallee {
                    caller: function.monomorphization,
                    callee: call.callee,
                });
            }
        }
    }

    reject_recursive_calls(root, &by_instance)?;
    let mut visited = BTreeSet::from([root]);
    let mut parent = BTreeMap::new();
    let mut pending = VecDeque::from([root]);
    while let Some(caller) = pending.pop_front() {
        let function = by_instance
            .get(&caller)
            .expect("validated root and collected callees");
        for call in &function.calls {
            if visited.insert(call.callee) {
                parent.insert(call.callee, (caller, call.source_span.clone()));
                pending.push_back(call.callee);
            }
        }
    }
    if let Some(function) = functions
        .iter()
        .find(|function| !visited.contains(&function.monomorphization))
    {
        return Err(OrdinaryRustScalarValidationErrorV1::UnreachableHelper {
            function: function.monomorphization,
        });
    }
    Ok(parent)
}

fn reject_recursive_calls(
    root: ConcreteMonomorphizationIdentityV1,
    functions: &BTreeMap<ConcreteMonomorphizationIdentityV1, &ReachableFunctionObservationV1>,
) -> Result<(), OrdinaryRustScalarValidationErrorV1> {
    fn visit(
        current: ConcreteMonomorphizationIdentityV1,
        functions: &BTreeMap<ConcreteMonomorphizationIdentityV1, &ReachableFunctionObservationV1>,
        active: &mut BTreeSet<ConcreteMonomorphizationIdentityV1>,
        done: &mut BTreeSet<ConcreteMonomorphizationIdentityV1>,
    ) -> Result<(), OrdinaryRustScalarValidationErrorV1> {
        if done.contains(&current) {
            return Ok(());
        }
        if !active.insert(current) {
            return Err(OrdinaryRustScalarValidationErrorV1::RecursiveCall { function: current });
        }
        for call in &functions
            .get(&current)
            .expect("all call targets preflighted")
            .calls
        {
            visit(call.callee, functions, active, done)?;
        }
        active.remove(&current);
        done.insert(current);
        Ok(())
    }

    visit(root, functions, &mut BTreeSet::new(), &mut BTreeSet::new())
}

fn build_call_chain(
    root: ConcreteMonomorphizationIdentityV1,
    target: ConcreteMonomorphizationIdentityV1,
    functions: &[ReachableFunctionObservationV1],
    parent: &BTreeMap<
        ConcreteMonomorphizationIdentityV1,
        (ConcreteMonomorphizationIdentityV1, FrontendSourceSpanV1),
    >,
) -> Option<Vec<ScalarImportCallChainFrameV1>> {
    let by_instance = functions
        .iter()
        .map(|function| (function.monomorphization, function))
        .collect::<BTreeMap<_, _>>();
    let mut reversed = Vec::new();
    let mut current = target;
    loop {
        let function = by_instance.get(&current)?;
        let call_site = parent.get(&current).map(|(_, span)| span.clone());
        reversed.push(ScalarImportCallChainFrameV1 {
            function: current,
            source_span: function.source_span.clone(),
            call_site,
        });
        if current == root {
            break;
        }
        current = parent.get(&current)?.0;
    }
    reversed.reverse();
    Some(reversed)
}

fn closure_identity<'a>(
    domain: &[u8],
    identities: impl ExactSizeIterator<Item = (ConcreteMonomorphizationIdentityV1, &'a [u8; 32])>,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    append_digest_field(&mut digest, domain);
    append_digest_field(&mut digest, &(identities.len() as u64).to_le_bytes());
    for (instance, identity) in identities {
        append_digest_field(&mut digest, instance.as_bytes());
        append_digest_field(&mut digest, identity);
    }
    digest.finalize().into()
}

fn calculate_import_identity(
    observation: &OrdinaryRustScalarKernelObservationV1,
    source_closure_identity: [u8; 32],
    mir_closure_identity: [u8; 32],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    append_digest_field(&mut digest, IMPORT_IDENTITY_DOMAIN_V1);
    append_digest_field(
        &mut digest,
        &encode_frontend_unit_v1(&observation.frontend_unit)
            .expect("previously validated frontend unit"),
    );
    append_digest_field(&mut digest, observation.kernel_item.as_bytes());
    append_digest_field(&mut digest, observation.kernel_instance.as_bytes());
    append_digest_field(
        &mut digest,
        &encode_kernel_frontend_contract_v1(observation.kernel_contract),
    );
    append_digest_field(&mut digest, &source_closure_identity);
    append_digest_field(&mut digest, &mir_closure_identity);
    for function in &observation.functions {
        append_function_commitment(&mut digest, function);
    }
    digest.finalize().into()
}

fn append_function_commitment(digest: &mut Sha256, function: &ReachableFunctionObservationV1) {
    append_digest_field(digest, function.frontend_identity.as_bytes());
    append_digest_field(digest, &[function.role as u8]);
    append_digest_field(digest, function.item_identity.as_bytes());
    append_digest_field(digest, function.monomorphization.as_bytes());
    append_digest_field(digest, function.source_identity.as_bytes());
    append_digest_field(digest, function.mir_identity.as_bytes());
    append_span_commitment(digest, &function.source_span);
    append_digest_field(
        digest,
        &[function.function_kind as u8, u8::from(function.is_concrete)],
    );
    append_digest_field(digest, &function.fn_abi.identity);
    append_digest_field(digest, &[function.fn_abi.calling_convention as u8]);
    append_digest_field(
        digest,
        &[
            u8::from(function.fn_abi.c_variadic),
            u8::from(function.fn_abi.can_unwind),
        ],
    );
    append_digest_field(
        digest,
        &(function.fn_abi.arguments.len() as u64).to_le_bytes(),
    );
    for argument in &function.fn_abi.arguments {
        append_abi_commitment(digest, argument);
    }
    append_abi_commitment(digest, &function.fn_abi.return_value);
    append_digest_field(digest, &(function.calls.len() as u64).to_le_bytes());
    for call in &function.calls {
        append_digest_field(digest, call.callee.as_bytes());
        append_span_commitment(digest, &call.source_span);
    }
}

fn append_abi_commitment(digest: &mut Sha256, value: &RustcAbiValueV1) {
    append_digest_field(digest, value.rust_type.as_bytes());
    append_digest_field(digest, &value.layout_identity);
    append_digest_field(digest, &value.size.to_le_bytes());
    append_digest_field(digest, &value.alignment.to_le_bytes());
    append_digest_field(digest, &[value.pass_mode as u8]);
}

fn append_span_commitment(digest: &mut Sha256, span: &FrontendSourceSpanV1) {
    append_digest_field(digest, span.file().as_bytes());
    append_digest_field(digest, &span.start().0.to_le_bytes());
    append_digest_field(digest, &span.start().1.to_le_bytes());
    append_digest_field(digest, &span.end().0.to_le_bytes());
    append_digest_field(digest, &span.end().1.to_le_bytes());
}

fn append_digest_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}
