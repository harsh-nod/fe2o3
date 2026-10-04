//! Registered fixed-vector type metadata for bounded structural identity.
//!
//! This type-only surface does not enable vector operations or native lowering.

use pliron::{
    builtin::types::{FP16Type, FP32Type, FP64Type, IntegerType},
    common_traits::Verify,
    context::Context,
    derive::{pliron_attr, pliron_type},
    result::Result,
    r#type::TypeHandle,
    verify_err,
};

use crate::optimization_v1::BFloat16Type;

/// Largest target-neutral fixed vector admitted by the V12 dialect contract.
pub const MAX_FIXED_VECTOR_LANES_V12: u16 = 1024;

/// Validated fixed-vector lane count.
#[pliron_attr(name = "gpu.vector_lanes_v12", format = "$0")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VectorLaneCountAttrV12(pub u16);

impl Verify for VectorLaneCountAttrV12 {
    fn verify(&self, _context: &Context) -> Result<()> {
        if (2..=MAX_FIXED_VECTOR_LANES_V12).contains(&self.0) {
            Ok(())
        } else {
            verify_err!(
                pliron::location::Location::Unknown,
                "fixed-vector lane count must be in 2..={MAX_FIXED_VECTOR_LANES_V12}"
            )
        }
    }
}

/// Physical vector layout. Zero denotes contiguous lanes; any other value is
/// the exact interleave factor.
#[pliron_attr(name = "gpu.vector_layout_v12", format = "$0")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct VectorLayoutAttrV12(pub u16);

impl VectorLayoutAttrV12 {
    pub const CONTIGUOUS: Self = Self(0);

    pub const fn interleaved(factor: u16) -> Self {
        Self(factor)
    }

    pub const fn interleave_factor(self) -> Option<u16> {
        if self.0 == 0 { None } else { Some(self.0) }
    }
}

impl Verify for VectorLayoutAttrV12 {
    fn verify(&self, _context: &Context) -> Result<()> {
        if self.0 == 0 || self.0 >= 2 {
            Ok(())
        } else {
            verify_err!(
                pliron::location::Location::Unknown,
                "fixed-vector interleave factor cannot be one"
            )
        }
    }
}

/// First-class fixed-lane vector type with an explicit physical layout.
#[pliron_type(
    name = "gpu.fixed_vector_v12",
    format = "`<` $element `,` $lanes `,` $layout `>`",
    generate_get = true
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FixedVectorTypeV12 {
    element: TypeHandle,
    lanes: VectorLaneCountAttrV12,
    layout: VectorLayoutAttrV12,
}

impl FixedVectorTypeV12 {
    pub fn try_get(
        context: &Context,
        element: TypeHandle,
        lanes: u16,
        layout: VectorLayoutAttrV12,
    ) -> Option<pliron::r#type::TypedHandle<Self>> {
        valid_vector_descriptor(context, element, lanes, layout)
            .then(|| Self::get(context, element, VectorLaneCountAttrV12(lanes), layout))
    }

    pub const fn element(&self) -> TypeHandle {
        self.element
    }

    pub const fn lanes(&self) -> u16 {
        self.lanes.0
    }

    pub const fn layout(&self) -> VectorLayoutAttrV12 {
        self.layout
    }
}

impl Verify for FixedVectorTypeV12 {
    fn verify(&self, context: &Context) -> Result<()> {
        self.lanes.verify(context)?;
        self.layout.verify(context)?;
        if !valid_vector_descriptor(context, self.element, self.lanes.0, self.layout) {
            return verify_err!(
                pliron::location::Location::Unknown,
                "fixed-vector element, lane count, and layout are incompatible"
            );
        }
        Ok(())
    }
}

fn valid_vector_descriptor(
    context: &Context,
    element: TypeHandle,
    lanes: u16,
    layout: VectorLayoutAttrV12,
) -> bool {
    if !(2..=MAX_FIXED_VECTOR_LANES_V12).contains(&lanes) || !is_vector_element(context, element) {
        return false;
    }
    match layout.interleave_factor() {
        None => true,
        Some(factor) => factor >= 2 && factor < lanes && lanes.is_multiple_of(factor),
    }
}

fn is_vector_element(context: &Context, element: TypeHandle) -> bool {
    let raw = element.deref(context);
    raw.downcast_ref::<IntegerType>()
        .is_some_and(|integer| matches!(integer.width(), 8 | 16 | 32 | 64 | 128))
        || raw.is::<FP16Type>()
        || raw.is::<BFloat16Type>()
        || raw.is::<FP32Type>()
        || raw.is::<FP64Type>()
}
