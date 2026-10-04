use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    MatrixElement, TensorCoordinateExprV1, TensorElementPackingV1, TensorFragmentLayoutV1,
    TensorInstructionProfileV1, TensorLayoutContractV1, TensorLdsSwizzleV1, TensorMultiplicityV1,
    TensorOperandRoleV1, TensorSymbolicMapV1, TensorTailMaskV1,
};
use pliron::{
    builtin::op_interfaces::{NOpdsInterface, NRegionsInterface, NResultsInterface},
    combine::{Parser, count_min_max, parser::char::hex_digit},
    common_traits::Verify,
    context::Context,
    derive::{pliron_attr, pliron_op},
    op::Op,
    operation::Operation,
    parsable::{Parsable, ParseResult, StateStream},
    printable::{self, Printable},
    result::Result as PlironResult,
    verify_err, verify_err_noloc,
};

const AFFINE_MAP_KIND_V1: u32 = 1;
const OPAQUE_MAP_KIND_V1: u32 = 2;
const GFX950_FP8_SPLIT_K_MAP_KIND_V1: u32 = 3;

/// Stable compiler-derived identity for one tensor-capability value.
///
/// The identity carries no layout claim. The whole-function tensor-layout
/// analysis uses equal roots to connect producers, control-flow joins, and
/// consumers without depending on workload names.
#[pliron_attr(name = "kernel.tensor_value_root")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TensorValueRootAttr([u64; 4]);

impl TensorValueRootAttr {
    pub const fn new(words: [u64; 4]) -> Self {
        Self(words)
    }

    pub const fn words(&self) -> [u64; 4] {
        self.0
    }

    const fn is_zero(&self) -> bool {
        self.0[0] == 0 && self.0[1] == 0 && self.0[2] == 0 && self.0[3] == 0
    }
}

impl Verify for TensorValueRootAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if self.is_zero() {
            return verify_err_noloc!(
                "kernel.tensor_value_root cannot be the reserved all-zero identity"
            );
        }
        Ok(())
    }
}

impl Printable for TensorValueRootAttr {
    fn fmt(
        &self,
        _context: &Context,
        _state: &printable::State,
        formatter: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        write!(
            formatter,
            "{:016x}{:016x}{:016x}{:016x}",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

impl Parsable for TensorValueRootAttr {
    type Arg = ();
    type Parsed = Self;

    fn parse<'a>(
        state_stream: &mut StateStream<'a>,
        _arg: Self::Arg,
    ) -> ParseResult<'a, Self::Parsed> {
        let word = || {
            count_min_max::<String, _, _>(16, 16, hex_digit())
                .and_then(|digits| u64::from_str_radix(&digits, 16))
        };
        word()
            .and(word())
            .and(word())
            .and(word())
            .map(|(((first, second), third), fourth)| Self([first, second, third, fourth]))
            .parse_stream(state_stream)
            .into()
    }
}

/// Compiler-derived producer/consumer identities retained on one tensor site.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TensorDataflowRootsV1 {
    pub lhs: [u64; 4],
    pub rhs: [u64; 4],
    pub accumulator: [u64; 4],
    pub result: [u64; 4],
}

/// Exact control-participation claim retained on a cooperative tensor operation.
#[pliron_attr(name = "kernel.tensor_convergence", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TensorConvergenceAttr {
    UniformSubgroup,
    Divergent,
    UniformWorkgroup,
    Opaque,
}

/// Bounded instruction-wide tensor layout metadata.
#[pliron_attr(
    name = "kernel.tensor_instruction",
    format = "`<` $profile_kind `,` $profile_identity `,` $subgroup_width `,` $active_lanes `,` $tail_mask `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TensorInstructionAttr {
    profile_kind: u32,
    profile_identity: u32,
    subgroup_width: u32,
    active_lanes: u32,
    tail_mask: u32,
}

impl TensorInstructionAttr {
    fn from_contract(contract: &TensorLayoutContractV1, active_lanes: u32) -> Self {
        Self {
            profile_kind: match contract.profile {
                TensorInstructionProfileV1::Gfx942MfmaBf16F32M16N16K16Wave64 => 1,
                TensorInstructionProfileV1::Gfx950ScaledMfmaFp8E4M3F32M16N16K128Wave64 => 4,
                TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1F32M16N16K128Wave64 => 5,
                TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64 => 6,
                TensorInstructionProfileV1::IncompatibleWave32 => 2,
                TensorInstructionProfileV1::Opaque(_) => 3,
            },
            profile_identity: match contract.profile {
                TensorInstructionProfileV1::Opaque(identity) => identity,
                _ => 0,
            },
            subgroup_width: u32::from(contract.subgroup_width),
            active_lanes,
            tail_mask: match contract.tail_mask {
                TensorTailMaskV1::ExactPhysicalTile => 1,
                TensorTailMaskV1::ZeroFilledPredicateInputs => 2,
                TensorTailMaskV1::PredicateMask => 3,
                TensorTailMaskV1::Missing => 4,
                TensorTailMaskV1::Unsupported(code) => 0x100 + u32::from(code),
            },
        }
    }

    pub const fn active_lanes(&self) -> u32 {
        self.active_lanes
    }

    fn profile(&self) -> Result<TensorInstructionProfileV1, TensorLayoutDialectError> {
        Ok(match self.profile_kind {
            1 if self.profile_identity == 0 => {
                TensorInstructionProfileV1::Gfx942MfmaBf16F32M16N16K16Wave64
            }
            2 if self.profile_identity == 0 => TensorInstructionProfileV1::IncompatibleWave32,
            3 => TensorInstructionProfileV1::Opaque(self.profile_identity),
            4 if self.profile_identity == 0 => {
                TensorInstructionProfileV1::Gfx950ScaledMfmaFp8E4M3F32M16N16K128Wave64
            }
            5 if self.profile_identity == 0 => {
                TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1F32M16N16K128Wave64
            }
            6 if self.profile_identity == 0 => {
                TensorInstructionProfileV1::Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64
            }
            _ => return Err(TensorLayoutDialectError::NumericFieldOutOfRange),
        })
    }

    fn subgroup_width(&self) -> Result<u16, TensorLayoutDialectError> {
        self.subgroup_width
            .try_into()
            .map_err(|_| TensorLayoutDialectError::NumericFieldOutOfRange)
    }

    fn tail_mask(&self) -> Result<TensorTailMaskV1, TensorLayoutDialectError> {
        Ok(match self.tail_mask {
            1 => TensorTailMaskV1::ExactPhysicalTile,
            2 => TensorTailMaskV1::ZeroFilledPredicateInputs,
            3 => TensorTailMaskV1::PredicateMask,
            4 => TensorTailMaskV1::Missing,
            other => TensorTailMaskV1::Unsupported(
                other
                    .checked_sub(0x100)
                    .ok_or(TensorLayoutDialectError::NumericFieldOutOfRange)?
                    .try_into()
                    .map_err(|_| TensorLayoutDialectError::NumericFieldOutOfRange)?,
            ),
        })
    }
}

impl Verify for TensorInstructionAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if self.profile().is_err() || self.subgroup_width().is_err() || self.tail_mask().is_err() {
            return pliron::verify_err_noloc!(TensorLayoutDialectError::NumericFieldOutOfRange);
        }
        Ok(())
    }
}

/// Explicit symbolic lane/register-component mapping for one tensor operand.
#[pliron_attr(
    name = "kernel.tensor_fragment",
    format = "`<` $role `,` $rows `,` $columns `,` $element `,` $fragment_elements `,` $map_kind `,` $lane_modulus `,` $lane_divisor `,` $axis0_constant `,` $axis0_mod_scale `,` $axis0_div_scale `,` $axis0_component_scale `,` $axis0_tile_origin `,` $axis1_constant `,` $axis1_mod_scale `,` $axis1_div_scale `,` $axis1_component_scale `,` $axis1_tile_origin `,` $multiplicity `,` $broadcast_factor `,` $packing `,` $lds_swizzle `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TensorFragmentAttr {
    role: u32,
    rows: u32,
    columns: u32,
    element: u32,
    fragment_elements: u32,
    map_kind: u32,
    lane_modulus: u32,
    lane_divisor: u32,
    axis0_constant: u32,
    axis0_mod_scale: u32,
    axis0_div_scale: u32,
    axis0_component_scale: u32,
    axis0_tile_origin: u32,
    axis1_constant: u32,
    axis1_mod_scale: u32,
    axis1_div_scale: u32,
    axis1_component_scale: u32,
    axis1_tile_origin: u32,
    multiplicity: u32,
    broadcast_factor: u32,
    packing: u32,
    lds_swizzle: u32,
}

impl TensorFragmentAttr {
    fn from_fragment(fragment: TensorFragmentLayoutV1) -> Self {
        let (map_kind, lane_modulus, lane_divisor, axes) = match fragment.mapping {
            TensorSymbolicMapV1::LaneComponentAffine {
                lane_modulus,
                lane_divisor,
                axes,
            } => (
                AFFINE_MAP_KIND_V1,
                u32::from(lane_modulus),
                u32::from(lane_divisor),
                axes,
            ),
            TensorSymbolicMapV1::Opaque(identity) => (
                OPAQUE_MAP_KIND_V1,
                identity,
                0,
                [TensorCoordinateExprV1::new(0, 0, 0); 2],
            ),
            TensorSymbolicMapV1::Gfx950Fp8M16N16K128SplitK => (
                GFX950_FP8_SPLIT_K_MAP_KIND_V1,
                0,
                0,
                [TensorCoordinateExprV1::new(0, 0, 0); 2],
            ),
        };
        let [axis0, axis1] = axes;
        let (multiplicity, broadcast_factor) = match fragment.multiplicity {
            TensorMultiplicityV1::Unique => (1, 1),
            TensorMultiplicityV1::Broadcast { factor } => (2, u32::from(factor)),
        };
        Self {
            role: role_code(fragment.role),
            rows: u32::from(fragment.shape[0]),
            columns: u32::from(fragment.shape[1]),
            element: match fragment.element {
                MatrixElement::Bf16 => 1,
                MatrixElement::F32 => 2,
                MatrixElement::Fp8E4M3 => 3,
                MatrixElement::Fp4E2M1 => 4,
            },
            fragment_elements: u32::from(fragment.fragment_elements),
            map_kind,
            lane_modulus,
            lane_divisor,
            axis0_constant: u32::from(axis0.constant),
            axis0_mod_scale: u32::from(axis0.lane_mod_scale),
            axis0_div_scale: u32::from(axis0.lane_div_scale),
            axis0_component_scale: u32::from(axis0.component_scale),
            axis0_tile_origin: u32::from(axis0.tile_origin),
            axis1_constant: u32::from(axis1.constant),
            axis1_mod_scale: u32::from(axis1.lane_mod_scale),
            axis1_div_scale: u32::from(axis1.lane_div_scale),
            axis1_component_scale: u32::from(axis1.component_scale),
            axis1_tile_origin: u32::from(axis1.tile_origin),
            multiplicity,
            broadcast_factor,
            packing: match fragment.packing {
                TensorElementPackingV1::Bf16PairInI32 => 1,
                TensorElementPackingV1::F32Scalar => 2,
                TensorElementPackingV1::Fp8FourInI32 => 4,
                TensorElementPackingV1::Fp4EightInI32 => 5,
                TensorElementPackingV1::Unsupported(code) => 0x100 + u32::from(code),
            },
            lds_swizzle: match fragment.lds_swizzle {
                TensorLdsSwizzleV1::None => 1,
                TensorLdsSwizzleV1::Xor4 => 2,
                TensorLdsSwizzleV1::Unsupported(code) => 0x100 + u32::from(code),
            },
        }
    }

    fn fragment(&self) -> Result<TensorFragmentLayoutV1, TensorLayoutDialectError> {
        let axis = |constant: u32,
                    lane_mod_scale: u32,
                    lane_div_scale: u32,
                    component_scale: u32,
                    tile_origin: u32|
         -> Result<TensorCoordinateExprV1, TensorLayoutDialectError> {
            Ok(TensorCoordinateExprV1 {
                constant: narrow(constant)?,
                lane_mod_scale: narrow(lane_mod_scale)?,
                lane_div_scale: narrow(lane_div_scale)?,
                component_scale: narrow(component_scale)?,
                tile_origin: match tile_origin {
                    0 => false,
                    1 => true,
                    _ => return Err(TensorLayoutDialectError::NumericFieldOutOfRange),
                },
            })
        };
        let mapping = match self.map_kind {
            AFFINE_MAP_KIND_V1 => TensorSymbolicMapV1::LaneComponentAffine {
                lane_modulus: narrow(self.lane_modulus)?,
                lane_divisor: narrow(self.lane_divisor)?,
                axes: [
                    axis(
                        self.axis0_constant,
                        self.axis0_mod_scale,
                        self.axis0_div_scale,
                        self.axis0_component_scale,
                        self.axis0_tile_origin,
                    )?,
                    axis(
                        self.axis1_constant,
                        self.axis1_mod_scale,
                        self.axis1_div_scale,
                        self.axis1_component_scale,
                        self.axis1_tile_origin,
                    )?,
                ],
            },
            OPAQUE_MAP_KIND_V1 => TensorSymbolicMapV1::Opaque(self.lane_modulus),
            GFX950_FP8_SPLIT_K_MAP_KIND_V1 => TensorSymbolicMapV1::Gfx950Fp8M16N16K128SplitK,
            _ => return Err(TensorLayoutDialectError::UnknownSymbolicMapKind),
        };
        Ok(TensorFragmentLayoutV1 {
            role: role_from_code(self.role)?,
            shape: [narrow(self.rows)?, narrow(self.columns)?],
            element: match self.element {
                1 => MatrixElement::Bf16,
                2 => MatrixElement::F32,
                3 => MatrixElement::Fp8E4M3,
                4 => MatrixElement::Fp4E2M1,
                _ => return Err(TensorLayoutDialectError::UnknownElement),
            },
            fragment_elements: self
                .fragment_elements
                .try_into()
                .map_err(|_| TensorLayoutDialectError::NumericFieldOutOfRange)?,
            mapping,
            multiplicity: match self.multiplicity {
                1 => TensorMultiplicityV1::Unique,
                2 => TensorMultiplicityV1::Broadcast {
                    factor: self
                        .broadcast_factor
                        .try_into()
                        .map_err(|_| TensorLayoutDialectError::NumericFieldOutOfRange)?,
                },
                _ => return Err(TensorLayoutDialectError::UnknownMultiplicity),
            },
            packing: match self.packing {
                1 => TensorElementPackingV1::Bf16PairInI32,
                2 => TensorElementPackingV1::F32Scalar,
                4 => TensorElementPackingV1::Fp8FourInI32,
                5 => TensorElementPackingV1::Fp4EightInI32,
                other => TensorElementPackingV1::Unsupported(
                    other
                        .checked_sub(0x100)
                        .ok_or(TensorLayoutDialectError::NumericFieldOutOfRange)?
                        .try_into()
                        .map_err(|_| TensorLayoutDialectError::NumericFieldOutOfRange)?,
                ),
            },
            lds_swizzle: match self.lds_swizzle {
                1 => TensorLdsSwizzleV1::None,
                2 => TensorLdsSwizzleV1::Xor4,
                other => TensorLdsSwizzleV1::Unsupported(
                    other
                        .checked_sub(0x100)
                        .ok_or(TensorLayoutDialectError::NumericFieldOutOfRange)?
                        .try_into()
                        .map_err(|_| TensorLayoutDialectError::NumericFieldOutOfRange)?,
                ),
            },
        })
    }
}

impl Verify for TensorFragmentAttr {
    fn verify(&self, _context: &Context) -> PlironResult<()> {
        if let Err(error) = self.fragment() {
            return pliron::verify_err_noloc!(error);
        }
        Ok(())
    }
}

fn narrow(value: u32) -> Result<u16, TensorLayoutDialectError> {
    value
        .try_into()
        .map_err(|_| TensorLayoutDialectError::NumericFieldOutOfRange)
}

const fn role_code(role: TensorOperandRoleV1) -> u32 {
    match role {
        TensorOperandRoleV1::A => 1,
        TensorOperandRoleV1::B => 2,
        TensorOperandRoleV1::Accumulator => 3,
    }
}

fn role_from_code(code: u32) -> Result<TensorOperandRoleV1, TensorLayoutDialectError> {
    match code {
        1 => Ok(TensorOperandRoleV1::A),
        2 => Ok(TensorOperandRoleV1::B),
        3 => Ok(TensorOperandRoleV1::Accumulator),
        _ => Err(TensorLayoutDialectError::UnknownRole),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TensorLayoutDialectError {
    NumericFieldOutOfRange,
    UnknownRole,
    UnknownElement,
    UnknownMultiplicity,
    UnknownSymbolicMapKind,
    MalformedOperation,
}

impl fmt::Display for TensorLayoutDialectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NumericFieldOutOfRange => "tensor layout numeric field is out of range",
            Self::UnknownRole => "tensor layout has an unknown operand role",
            Self::UnknownElement => "tensor layout has an unknown element type",
            Self::UnknownMultiplicity => "tensor layout has an unknown multiplicity",
            Self::UnknownSymbolicMapKind => "tensor layout has an unknown symbolic map kind",
            Self::MalformedOperation => "kernel.tensor_layout has a malformed payload",
        })
    }
}

impl Error for TensorLayoutDialectError {}

/// Workload-neutral cooperative tensor layout contract retained for verification.
#[pliron_op(
    name = "kernel.tensor_layout",
    format,
    interfaces = [NOpdsInterface<0>, NResultsInterface<0>, NRegionsInterface<0>],
    attributes = (
        kernel_tensor_instruction: TensorInstructionAttr,
        kernel_tensor_convergence: TensorConvergenceAttr,
        kernel_tensor_a: TensorFragmentAttr,
        kernel_tensor_b: TensorFragmentAttr,
        kernel_tensor_accumulator: TensorFragmentAttr,
        kernel_tensor_lhs_root: TensorValueRootAttr,
        kernel_tensor_rhs_root: TensorValueRootAttr,
        kernel_tensor_accumulator_root: TensorValueRootAttr,
        kernel_tensor_result_root: TensorValueRootAttr
    )
)]
/// One cooperative tensor-instruction occurrence and its declared contract.
///
/// This is not detachable metadata: one operation denotes one physical
/// instruction site in the ranked graph. Its attributes are claims that the
/// generic pass cross-checks for internal compatibility and CFG participation;
/// they do not authenticate source provenance or grant lowering authority.
pub struct TensorLayoutOp;

impl TensorLayoutOp {
    pub fn new(
        context: &mut Context,
        contract: &TensorLayoutContractV1,
        convergence: TensorConvergenceAttr,
        active_lanes: u32,
    ) -> Self {
        Self::new_impl(context, contract, convergence, active_lanes, None)
    }

    pub fn new_with_dataflow_roots(
        context: &mut Context,
        contract: &TensorLayoutContractV1,
        convergence: TensorConvergenceAttr,
        active_lanes: u32,
        roots: TensorDataflowRootsV1,
    ) -> Self {
        Self::new_impl(context, contract, convergence, active_lanes, Some(roots))
    }

    fn new_impl(
        context: &mut Context,
        contract: &TensorLayoutContractV1,
        convergence: TensorConvergenceAttr,
        active_lanes: u32,
        roots: Option<TensorDataflowRootsV1>,
    ) -> Self {
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_kernel_tensor_instruction(
            context,
            TensorInstructionAttr::from_contract(contract, active_lanes),
        );
        op.set_attr_kernel_tensor_convergence(context, convergence);
        op.set_attr_kernel_tensor_a(context, TensorFragmentAttr::from_fragment(contract.a));
        op.set_attr_kernel_tensor_b(context, TensorFragmentAttr::from_fragment(contract.b));
        op.set_attr_kernel_tensor_accumulator(
            context,
            TensorFragmentAttr::from_fragment(contract.accumulator),
        );
        if let Some(roots) = roots {
            op.set_attr_kernel_tensor_lhs_root(context, TensorValueRootAttr::new(roots.lhs));
            op.set_attr_kernel_tensor_rhs_root(context, TensorValueRootAttr::new(roots.rhs));
            op.set_attr_kernel_tensor_accumulator_root(
                context,
                TensorValueRootAttr::new(roots.accumulator),
            );
            op.set_attr_kernel_tensor_result_root(context, TensorValueRootAttr::new(roots.result));
        }
        op
    }

    pub fn active_lanes(&self, context: &Context) -> Option<u32> {
        self.get_attr_kernel_tensor_instruction(context)
            .map(|instruction| instruction.active_lanes())
    }

    pub fn convergence(&self, context: &Context) -> Option<TensorConvergenceAttr> {
        self.get_attr_kernel_tensor_convergence(context)
            .map(|convergence| *convergence)
    }

    pub fn contract(
        &self,
        context: &Context,
    ) -> Result<TensorLayoutContractV1, TensorLayoutDialectError> {
        let instruction = self
            .get_attr_kernel_tensor_instruction(context)
            .ok_or(TensorLayoutDialectError::MalformedOperation)?;
        Ok(TensorLayoutContractV1 {
            profile: instruction.profile()?,
            subgroup_width: instruction.subgroup_width()?,
            a: self
                .get_attr_kernel_tensor_a(context)
                .ok_or(TensorLayoutDialectError::MalformedOperation)?
                .fragment()?,
            b: self
                .get_attr_kernel_tensor_b(context)
                .ok_or(TensorLayoutDialectError::MalformedOperation)?
                .fragment()?,
            accumulator: self
                .get_attr_kernel_tensor_accumulator(context)
                .ok_or(TensorLayoutDialectError::MalformedOperation)?
                .fragment()?,
            tail_mask: instruction.tail_mask()?,
        })
    }

    pub fn dataflow_roots(
        &self,
        context: &Context,
    ) -> Result<Option<TensorDataflowRootsV1>, TensorLayoutDialectError> {
        let roots = [
            self.get_attr_kernel_tensor_lhs_root(context),
            self.get_attr_kernel_tensor_rhs_root(context),
            self.get_attr_kernel_tensor_accumulator_root(context),
            self.get_attr_kernel_tensor_result_root(context),
        ];
        match roots {
            [None, None, None, None] => Ok(None),
            [Some(lhs), Some(rhs), Some(accumulator), Some(result)] => {
                Ok(Some(TensorDataflowRootsV1 {
                    lhs: lhs.words(),
                    rhs: rhs.words(),
                    accumulator: accumulator.words(),
                    result: result.words(),
                }))
            }
            _ => Err(TensorLayoutDialectError::MalformedOperation),
        }
    }
}

impl Verify for TensorLayoutOp {
    fn verify(&self, context: &Context) -> PlironResult<()> {
        let operation = self.get_operation();
        let operation = operation.deref(context);
        if operation.get_num_operands() != 0
            || operation.get_num_results() != 0
            || operation.get_num_successors() != 0
            || operation.num_regions() != 0
            || !matches!(operation.attributes.0.len(), 5 | 9)
            || self.contract(context).is_err()
            || self.convergence(context).is_none()
            || self.dataflow_roots(context).is_err()
        {
            return verify_err!(
                self.loc(context),
                TensorLayoutDialectError::MalformedOperation
            );
        }
        Ok(())
    }
}
