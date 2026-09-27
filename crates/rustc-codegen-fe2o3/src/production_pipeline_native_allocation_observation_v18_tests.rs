// Fixed-size diagnostic copies only. No layout/source observation grants authority.
use fe2o3_kernel_ir::{AccessMode, AddressSpace, ScalarType};

macro_rules! diagnostic_tag {
    ($name:ident, $input:path, $($variant:ident),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        pub(crate) enum $name { $($variant),+ }
        impl From<$input> for $name {
            fn from(value: $input) -> Self {
                use $input as Input;
                match value { $(Input::$variant => Self::$variant),+ }
            }
        }
    };
}
diagnostic_tag!(
    ScalarTag, ScalarType, Bool, I8, I16, I32, I64, I128, U8, U16, U32, U64, U128, Index, F16,
    Bf16, F32, F64
);
diagnostic_tag!(
    SpaceTag,
    AddressSpace,
    Private,
    Workgroup,
    Global,
    Constant,
    Generic
);
diagnostic_tag!(AccessTag, AccessMode, ReadOnly, WriteOnly, ReadWrite);

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum TypeTag {
    Unit,
    Scalar,
    StorageObject,
    Execution,
    Vector,
    Pointer,
    Slice,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum LayoutTag {
    Scalar,
    Vector,
    Pointer,
    Record,
    Union,
    Array,
    Slice,
    Variants,
}

fn required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(deserializer)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LayoutObservation {
    pub(crate) ordinal: u32,
    pub(crate) kind: LayoutTag,
    pub(crate) size: u64,
    pub(crate) alignment: u32,
    #[serde(deserialize_with = "required_option")]
    pub(crate) scalar: Option<ScalarTag>,
    pub(crate) members: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypeObservation {
    pub(crate) kind: TypeTag,
    #[serde(deserialize_with = "required_option")]
    pub(crate) scalar: Option<ScalarTag>,
    #[serde(deserialize_with = "required_option")]
    pub(crate) layout: Option<LayoutObservation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AllocationSourceObservation {
    pub(crate) root: usize,
    pub(crate) instance: usize,
    pub(crate) function: u32,
    pub(crate) input: [u32; 3],
    pub(crate) semantic_sha256: [u8; 32],
    // Existing public allocation queries do not expose local/generation identity.
    pub(crate) private_slot_identity_available: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AllocationObservation {
    pub(crate) element: TypeObservation,
    pub(crate) address_space: SpaceTag,
    pub(crate) alignment: u32,
    #[serde(deserialize_with = "required_option")]
    pub(crate) count: Option<u32>,
    pub(crate) result: u32,
    pub(crate) result_address_space: SpaceTag,
    pub(crate) result_access: AccessTag,
    pub(crate) result_pointee: TypeObservation,
    #[serde(deserialize_with = "required_option")]
    pub(crate) source: Option<AllocationSourceObservation>,
}

impl AllocationObservation {
    pub(crate) fn validate_shape(&self) -> Result<(), &'static str> {
        for ty in [self.element, self.result_pointee] {
            if (ty.kind == TypeTag::Scalar) != ty.scalar.is_some()
                || (ty.kind == TypeTag::StorageObject) != ty.layout.is_some()
            {
                return Err("native allocation diagnostic type/detail mismatch");
            }
            if let Some(layout) = ty.layout {
                if (layout.kind == LayoutTag::Scalar) != layout.scalar.is_some() {
                    return Err("native allocation diagnostic layout/detail mismatch");
                }
            }
        }
        if self
            .source
            .is_some_and(|source| source.private_slot_identity_available)
        {
            return Err("native allocation diagnostic does not expose private slot identity");
        }
        Ok(())
    }
}
