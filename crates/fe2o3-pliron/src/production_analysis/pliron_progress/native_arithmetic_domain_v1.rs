use pliron::r#type::Typed as _;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeProgressDomainV1 {
    LegacyIndex,
    Fixed { width: u32, signed: bool },
    IndexUnknown,
}

impl NativeProgressDomainV1 {
    fn maximum(self) -> Option<u128> {
        match self {
            Self::Fixed { width: 128, .. } => Some(u128::MAX),
            Self::Fixed { width, .. } => Some((1_u128 << width) - 1),
            Self::LegacyIndex => Some(u64::MAX.into()),
            Self::IndexUnknown => None,
        }
    }

    fn ordered(self, bits: u128) -> Option<u128> {
        match self {
            Self::Fixed { width, signed } => {
                if bits > self.maximum()? {
                    return None;
                }
                Some(if signed {
                    bits ^ (1_u128 << (width - 1))
                } else {
                    bits
                })
            }
            Self::LegacyIndex => u64::try_from(bits).ok().map(u128::from),
            // Without target width custody only zero and one are known to
            // survive materialization for every possible nonzero width.
            Self::IndexUnknown => (bits <= 1).then_some(bits),
        }
    }

    fn positive_step(self, bits: u128) -> Option<u64> {
        if self.maximum().is_some_and(|maximum| bits > maximum) {
            return None;
        }
        if let Self::Fixed {
            width,
            signed: true,
        } = self
        {
            if bits & (1_u128 << (width - 1)) != 0 {
                return None;
            }
        }
        u64::try_from(bits).ok()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NativeProgressLiteralV1 {
    domain: NativeProgressDomainV1,
    bits: u128,
}

impl NativeProgressLiteralV1 {
    fn ordered(self) -> Option<u128> {
        self.domain.ordered(self.bits)
    }
}

fn native_progress_domain_v1(
    context: &Context,
    value: pliron::value::Value,
) -> Option<NativeProgressDomainV1> {
    #[cfg(test)]
    native_progress_scalar_observation_v1::domain();
    let ty = value.get_type(context);
    let ty = ty.deref(context);
    if ty.is::<dialect_kernel::IndexType>() {
        return Some(NativeProgressDomainV1::LegacyIndex);
    }
    if ty.is::<dialect_gpu::optimization_v1::IndexType>() {
        return Some(NativeProgressDomainV1::IndexUnknown);
    }
    let integer = ty.downcast_ref::<pliron::builtin::types::IntegerType>()?;
    let signed = match integer.signedness() {
        pliron::builtin::types::Signedness::Signed => true,
        pliron::builtin::types::Signedness::Unsigned => false,
        pliron::builtin::types::Signedness::Signless => return None,
    };
    matches!(integer.width(), 8 | 16 | 32 | 64 | 128).then_some(NativeProgressDomainV1::Fixed {
        width: integer.width(),
        signed,
    })
}

#[derive(Clone, Copy)]
struct NativeProgressCastV1 {
    from: NativeProgressDomainV1,
    to: NativeProgressDomainV1,
    sign_extend: bool,
}

impl NativeProgressCastV1 {
    fn extend(self, bits: u128) -> Option<u128> {
        let NativeProgressDomainV1::Fixed { width: from, .. } = self.from else {
            return None;
        };
        let NativeProgressDomainV1::Fixed { width: to, .. } = self.to else {
            return None;
        };
        if from >= to || bits > self.from.maximum()? {
            return None;
        }
        Some(if self.sign_extend && bits & (1_u128 << (from - 1)) != 0 {
            bits | (self.to.maximum()? ^ self.from.maximum()?)
        } else {
            bits
        })
    }
    fn interval(self, lower: u128, upper: u128) -> Option<(u128, u128)> {
        let NativeProgressDomainV1::Fixed { width, signed } = self.from else {
            return None;
        };
        let NativeProgressDomainV1::Fixed {
            signed: target_signed,
            ..
        } = self.to
        else {
            return None;
        };
        if lower > upper || upper > self.from.maximum()? {
            return None;
        }
        // Signed-to-unsigned widening has a discontinuity at numeric zero.
        // Crossing it loses interval precision but never understates the set.
        if signed
            && !target_signed
            && lower < (1_u128 << (width - 1))
            && upper >= (1_u128 << (width - 1))
        {
            return Some((0, self.to.maximum()?));
        }
        let lower = self.to.ordered(self.extend(self.from.ordered(lower)?)?)?;
        let upper = self.to.ordered(self.extend(self.from.ordered(upper)?)?)?;
        Some((lower, upper))
    }
}

fn native_progress_cast_v1(
    context: &Context,
    value: pliron::value::Value,
) -> Option<(pliron::value::Value, NativeProgressCastV1)> {
    use dialect_gpu::optimization_v1::{CastKindAttr, CastOp};
    let pointer = value.defining_op()?;
    if !native_progress_scalar_shape_v1(context, pointer, 1, 1) {
        return None;
    }
    let operation = Operation::get_op_dyn(pointer, context);
    let cast = operation.downcast_ref::<CastOp>()?;
    if cast.result(context) != value {
        return None;
    }
    let sign_extend = match cast.kind(context)? {
        CastKindAttr::ZeroExtend => false,
        CastKindAttr::SignExtend => true,
        _ => return None,
    };
    let source = cast.get_operand_value(context);
    let from = native_progress_domain_v1(context, source)?;
    let to = native_progress_domain_v1(context, value)?;
    match (from, to) {
        (
            NativeProgressDomainV1::Fixed { width: a, signed },
            NativeProgressDomainV1::Fixed { width: b, .. },
        ) if a < b && signed == sign_extend => Some((
            source,
            NativeProgressCastV1 {
                from,
                to,
                sign_extend,
            },
        )),
        _ => None,
    }
}

// Only four strictly widening casts can cross the closed five-width family.
// The APInt copy is short-lived and covered by the admitted Progress envelope.
fn native_progress_literal_v1(
    context: &Context,
    value: pliron::value::Value,
) -> Option<NativeProgressLiteralV1> {
    #[cfg(test)]
    native_progress_scalar_observation_v1::literal();
    let mut current = value;
    let mut casts = [None; 4];
    let mut count = 0;
    let mut literal = loop {
        let definition = current.defining_op()?;
        let operation = Operation::get_op_dyn(definition, context);
        if let Some(constant) = operation.downcast_ref::<IndexConstantOp>() {
            if constant.result(context) != current {
                return None;
            }
            break NativeProgressLiteralV1 {
                domain: NativeProgressDomainV1::LegacyIndex,
                bits: constant.value(context)?.into(),
            };
        }
        if let Some(constant) = operation.downcast_ref::<dialect_gpu::optimization_v1::ConstantOp>()
        {
            if !native_progress_scalar_shape_v1(context, definition, 0, 1)
                || constant.result(context) != current
            {
                return None;
            }
            let domain = native_progress_domain_v1(context, current)?;
            let attribute = constant.get_attr_gpu_constant_value(context)?;
            let bits = match domain {
                NativeProgressDomainV1::IndexUnknown => attribute
                    .downcast_ref::<dialect_gpu::optimization_v1::IndexAttr>()?
                    .0
                    .into(),
                NativeProgressDomainV1::Fixed { width, signed } => {
                    let integer =
                        attribute.downcast_ref::<pliron::builtin::attributes::IntegerAttr>()?;
                    let ty = integer.get_type().deref(context);
                    let attribute_type: pliron::r#type::TypeHandle = integer.get_type().into();
                    if ty.width() != width
                        || ty.is_signed() != signed
                        || attribute_type != current.get_type(context)
                        || integer.verify(context).is_err()
                    {
                        return None;
                    }
                    #[cfg(test)]
                    native_progress_scalar_observation_v1::copy_attempt();
                    let copied = integer.value();
                    if copied.bw() != width as usize {
                        return None;
                    }
                    let bits = copied.to_u128();
                    drop(copied);
                    #[cfg(test)]
                    native_progress_scalar_observation_v1::copied_literal();
                    bits
                }
                NativeProgressDomainV1::LegacyIndex => return None,
            };
            break NativeProgressLiteralV1 { domain, bits };
        }
        let (source, cast) = native_progress_cast_v1(context, current)?;
        *casts.get_mut(count)? = Some(cast);
        count += 1;
        current = source;
    };
    for cast in casts[..count].iter().rev() {
        let cast = (*cast)?;
        if literal.domain != cast.from {
            return None;
        }
        literal = NativeProgressLiteralV1 {
            domain: cast.to,
            bits: cast.extend(literal.bits)?,
        };
    }
    (literal.domain == native_progress_domain_v1(context, value)?).then_some(literal)
}

fn native_progress_bound_upper_v1(
    context: &Context,
    value: pliron::value::Value,
    domain: NativeProgressDomainV1,
) -> Option<u128> {
    let mut current = value;
    let mut casts = [None; 4];
    let mut count = 0;
    while let Some((source, cast)) = native_progress_cast_v1(context, current) {
        *casts.get_mut(count)? = Some(cast);
        count += 1;
        current = source;
    }
    let mut current_domain = native_progress_domain_v1(context, current)?;
    let (mut lower, mut upper) = match native_progress_literal_v1(context, current) {
        Some(literal) if literal.domain == current_domain => {
            let coordinate = literal.ordered()?;
            (coordinate, coordinate)
        }
        Some(_) => return None,
        None => (0, current_domain.maximum()?),
    };
    for cast in casts[..count].iter().rev() {
        let cast = (*cast)?;
        if cast.from != current_domain {
            return None;
        }
        (lower, upper) = cast.interval(lower, upper)?;
        current_domain = cast.to;
    }
    (current_domain == domain).then_some(upper)
}

fn native_progress_no_wrap_v1(
    context: &Context,
    bound: pliron::value::Value,
    domain: NativeProgressDomainV1,
    step: u64,
) -> bool {
    #[cfg(test)]
    native_progress_scalar_observation_v1::bound();
    if step == 0 || native_progress_domain_v1(context, bound) != Some(domain) {
        return false;
    }
    if step == 1 {
        return true;
    }
    let Some(maximum) = domain.maximum() else {
        return false;
    };
    let Some(upper) = native_progress_bound_upper_v1(context, bound, domain) else {
        return false;
    };
    upper == 0
        || maximum
            .checked_sub(u128::from(step) - 1)
            .is_some_and(|safe| upper <= safe)
}

fn native_progress_scalar_shape_v1(
    context: &Context,
    operation: Ptr<Operation>,
    operands: usize,
    results: usize,
) -> bool {
    let raw = operation.deref(context);
    raw.get_num_operands() == operands
        && raw.get_num_results() == results
        && raw.get_num_successors() == 0
        && raw.num_regions() == 0
}

fn native_progress_bool_v1(context: &Context, value: pliron::value::Value) -> bool {
    let handle = value.get_type(context);
    let ty = handle.deref(context);
    ty.downcast_ref::<pliron::builtin::types::IntegerType>()
        .is_some_and(|integer| {
            integer.width() == 1
                && integer.signedness() == pliron::builtin::types::Signedness::Signless
        })
}
