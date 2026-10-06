//! Complete inert recipe encoding; no decoder can manufacture nominal custody.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirAggregateSsaActionV18 as Action, CanonicalKirAggregateSsaLeafTypeV18 as Leaf,
    CanonicalKirAggregateSsaMemoryEventV18 as Memory,
};
use fe2o3_kernel_ir::{ScalarType, VectorLayoutV12};

enum Target<'a> {
    Count,
    Write(&'a mut [u8]),
    Compare(&'a [u8]),
}
struct Writer<'a> {
    target: Target<'a>,
    cursor: usize,
}
impl Writer<'_> {
    fn raw(&mut self, bytes: &[u8], meter: &mut Meter<'_, '_>) -> Result<()> {
        meter.work(bytes.len())?;
        let end = add(self.cursor, bytes.len())?;
        match &mut self.target {
            Target::Count => {}
            Target::Write(out) => out
                .get_mut(self.cursor..end)
                .ok_or(Error::Inconsistent("prepaid witness extent"))?
                .copy_from_slice(bytes),
            Target::Compare(out) => {
                if out.get(self.cursor..end) != Some(bytes) {
                    return Err(Error::Inconsistent("full actual Policy12 witness bytes"));
                }
            }
        }
        self.cursor = end;
        Ok(())
    }
    fn word(&mut self, value: usize, meter: &mut Meter<'_, '_>) -> Result<()> {
        self.raw(
            &u64::try_from(value)
                .map_err(|_| Resource::Arithmetic)?
                .to_le_bytes(),
            meter,
        )
    }
    fn id(&mut self, value: &Identity, meter: &mut Meter<'_, '_>) -> Result<()> {
        self.raw(value.digest(), meter)?;
        self.raw(&value.canonical_length().to_le_bytes(), meter)
    }
    fn value(&mut self, value: fe2o3_kernel_ir::ValueId, meter: &mut Meter<'_, '_>) -> Result<()> {
        self.raw(&value.0.to_le_bytes(), meter)
    }
    fn leaf(&mut self, leaf: Leaf, meter: &mut Meter<'_, '_>) -> Result<()> {
        let (tag, scalar, lanes, layout, factor) = match leaf {
            Leaf::Scalar(s) => (0, s, 0u16, 0, 0u16),
            Leaf::Vector(v) => {
                let (layout, factor) = match v.layout {
                    VectorLayoutV12::Contiguous => (0, 0),
                    VectorLayoutV12::Interleaved { factor } => (1, factor),
                };
                (1, v.element, v.lanes, layout, factor)
            }
        };
        // Closed Policy12 codec tags, not Rust discriminants or Debug strings.
        let scalar = match scalar {
            ScalarType::Bool => 0,
            ScalarType::I8 => 1,
            ScalarType::I16 => 2,
            ScalarType::I32 => 3,
            ScalarType::I64 => 4,
            ScalarType::I128 => 5,
            ScalarType::U8 => 6,
            ScalarType::U16 => 7,
            ScalarType::U32 => 8,
            ScalarType::U64 => 9,
            ScalarType::U128 => 10,
            ScalarType::Index => 11,
            ScalarType::F16 => 12,
            ScalarType::Bf16 => 13,
            ScalarType::F32 => 14,
            ScalarType::F64 => 15,
        };
        self.raw(&[tag, scalar, layout], meter)?;
        self.raw(&lanes.to_le_bytes(), meter)?;
        self.raw(&factor.to_le_bytes(), meter)
    }
}
fn write(
    value: &OwnedAggregateFixedpointV18,
    out: &mut Writer<'_>,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    out.raw(b"fe2o3.aggregate-fixedpoint.v18.policy12\0", meter)?;
    // Wire revision 2 adds the concrete slot/event/phi-memory view. It cannot
    // reuse revision 1 generated-subject identity or an executed receipt.
    out.raw(&[12, 0, 2, 0, 18, 0], meter)?;
    out.word(AGGREGATE_FIXEDPOINT_MAX_ROUNDS_V18, meter)?;
    out.id(&value.input, meter)?;
    out.id(value.owner().identity(), meter)?;
    out.word(value.rounds.len(), meter)?;
    for round in &value.rounds {
        out.raw(&[u8::from(round.changed)], meter)?;
        out.id(round.scalar.owner().identity(), meter)?;
        out.id(round.aggregate.output().identity(), meter)?;
        let scalar = round.scalar.execution().canonical_bytes();
        out.word(scalar.len(), meter)?;
        out.raw(scalar, meter)?;
        let aggregate = round.aggregate.witness();
        out.id(aggregate.input_identity(), meter)?;
        out.word(aggregate.actions().len(), meter)?;
        for action in aggregate.actions() {
            match action {
                Action::Retain => out.raw(&[0], meter)?,
                Action::Remove => out.raw(&[1], meter)?,
                Action::Copy(value) => {
                    out.raw(&[2], meter)?;
                    out.value(*value, meter)?;
                }
            }
        }
        out.word(aggregate.parameters().len(), meter)?;
        for parameter in aggregate.parameters() {
            out.word(parameter.block, meter)?;
            out.value(parameter.value, meter)?;
            out.leaf(parameter.ty, meter)?;
        }
        out.word(aggregate.arguments().len(), meter)?;
        for argument in aggregate.arguments() {
            out.word(argument.edge, meter)?;
            out.value(argument.value, meter)?;
        }
        out.word(aggregate.conditions().len(), meter)?;
        for condition in aggregate.conditions() {
            out.raw(&[u8::from(condition.is_some())], meter)?;
            if let Some(value) = condition {
                out.value(*value, meter)?;
            }
        }
        out.word(aggregate.selected_allocations().len(), meter)?;
        for allocation in aggregate.selected_allocations() {
            out.word(*allocation, meter)?;
        }
        out.word(aggregate.planner_identities().len(), meter)?;
        for (function, plan) in aggregate.planner_identities() {
            out.word(*function, meter)?;
            out.raw(plan.as_bytes(), meter)?;
        }
        out.word(aggregate.memory_slots().len(), meter)?;
        for slot in aggregate.memory_slots() {
            out.raw(&[u8::from(slot.is_some())], meter)?;
            if let Some(slot) = slot {
                out.word(slot.allocation, meter)?;
                out.raw(&slot.layout.0.to_le_bytes(), meter)?;
                out.raw(&slot.offset.to_le_bytes(), meter)?;
                out.leaf(slot.ty, meter)?;
            }
        }
        out.word(aggregate.memory_events().len(), meter)?;
        for event in aggregate.memory_events() {
            match *event {
                Memory::None => out.raw(&[0], meter)?,
                Memory::Allocate { allocation } => {
                    out.raw(&[1], meter)?;
                    out.word(allocation, meter)?;
                }
                Memory::Project { allocation } => {
                    out.raw(&[2], meter)?;
                    out.word(allocation, meter)?;
                }
                Memory::Read {
                    slot,
                    output,
                    replacement,
                } => {
                    out.raw(&[3], meter)?;
                    out.word(slot, meter)?;
                    out.value(output, meter)?;
                    out.value(replacement, meter)?;
                }
                Memory::Write { slot, value } => {
                    out.raw(&[4], meter)?;
                    out.word(slot, meter)?;
                    out.value(value, meter)?;
                }
            }
        }
        out.word(aggregate.memory_parameters().len(), meter)?;
        for parameter in aggregate.memory_parameters() {
            out.word(parameter.parameter, meter)?;
            out.word(parameter.slot, meter)?;
        }
    }
    Ok(())
}
pub(super) fn encode(
    value: &OwnedAggregateFixedpointV18,
    meter: &mut Meter<'_, '_>,
) -> Result<Vec<u8>> {
    meter.reserve(size_of::<(Writer<'_>, Vec<u8>, Result<Vec<u8>>)>())?;
    let mut count = Writer {
        target: Target::Count,
        cursor: 0,
    };
    write(value, &mut count, meter)?;
    let length = count.cursor;
    drop(count);
    let (mut bytes, _) = meter.table(length)?;
    meter.work(length)?;
    bytes.resize(length, 0);
    let mut writer = Writer {
        target: Target::Write(&mut bytes),
        cursor: 0,
    };
    write(value, &mut writer, meter)?;
    if writer.cursor != length {
        return Err(Error::Inconsistent("complete witness write"));
    }
    Ok(bytes)
}
pub(super) fn compare(
    value: &OwnedAggregateFixedpointV18,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    meter.reserve(size_of::<Writer<'_>>() + size_of::<Result<()>>())?;
    let mut writer = Writer {
        target: Target::Compare(&value.canonical),
        cursor: 0,
    };
    write(value, &mut writer, meter)?;
    if writer.cursor != value.canonical.len() {
        return Err(Error::Inconsistent("no trailing witness bytes"));
    }
    Ok(())
}
