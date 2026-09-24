//! Internal symbolic state. No invented device address bits or public debug encoding.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Half {
    Low,
    High,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AddressChain {
    pub pointer: PointerValue,
    pub pointer_generation: CompactSite,
    pub displacement: u64,
    pub displacement_generation: CompactSite,
    pub low_add: CompactSite,
}

/// Each value stays inline in the ordinary SSA table and actual CFG arguments.
/// Symbolic carries are deliberately not ScalarBits, even though canonical VCC is U64.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Value {
    Kernarg {
        half: Half,
        declaration: CompactSite,
        parameters: [ValueId; 5],
    },
    Output {
        half: Half,
        generation: CompactSite,
        pointer: PointerValue,
    },
    ScaledOffset {
        half: Half,
        generation: CompactSite,
        displacement: u64,
    },
    AddressLow(AddressChain),
    AddressHigh {
        chain: AddressChain,
        high_add: CompactSite,
    },
    CarryLow(AddressChain),
    CarryHigh {
        chain: AddressChain,
        high_add: CompactSite,
    },
}
impl Value {
    pub(super) const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::CarryLow(_) | Self::CarryHigh { .. } => ScalarType::U64,
            Self::Kernarg { .. }
            | Self::Output { .. }
            | Self::ScaledOffset { .. }
            | Self::AddressLow(_)
            | Self::AddressHigh { .. } => ScalarType::U32,
        }
    }
}

pub(super) fn kernarg_parameters(low: &RuntimeValue, high: &RuntimeValue) -> Option<[ValueId; 5]> {
    match (low, high) {
        (
            RuntimeValue::PhysicalEntry(Value::Kernarg {
                half: Half::Low,
                declaration: a,
                parameters: p,
            }),
            RuntimeValue::PhysicalEntry(Value::Kernarg {
                half: Half::High,
                declaration: b,
                parameters: q,
            }),
        ) if a == b && p == q => Some(*p),
        _ => None,
    }
}
pub(super) fn low_chain(
    base: &RuntimeValue,
    offset: &RuntimeValue,
    site: CompactSite,
) -> Option<AddressChain> {
    match (base, offset) {
        (
            RuntimeValue::PhysicalEntry(Value::Output {
                half: Half::Low,
                generation,
                pointer,
            }),
            RuntimeValue::PhysicalEntry(Value::ScaledOffset {
                half: Half::Low,
                generation: offset_generation,
                displacement,
            }),
        ) => Some(AddressChain {
            pointer: pointer.clone(),
            pointer_generation: *generation,
            displacement: *displacement,
            displacement_generation: *offset_generation,
            low_add: site,
        }),
        _ => None,
    }
}
pub(super) fn high_chain(
    base: &RuntimeValue,
    offset: &RuntimeValue,
    carry: &RuntimeValue,
) -> Option<AddressChain> {
    match (base, offset, carry) {
        (
            RuntimeValue::PhysicalEntry(Value::Output {
                half: Half::High,
                generation,
                pointer,
            }),
            RuntimeValue::PhysicalEntry(Value::ScaledOffset {
                half: Half::High,
                generation: offset_generation,
                displacement,
            }),
            RuntimeValue::PhysicalEntry(Value::CarryLow(chain)),
        ) if pointer == &chain.pointer
            && generation == &chain.pointer_generation
            && displacement == &chain.displacement
            && offset_generation == &chain.displacement_generation =>
        {
            Some(chain.clone())
        }
        _ => None,
    }
}
pub(super) fn store_chain(low: &RuntimeValue, high: &RuntimeValue) -> Option<AddressChain> {
    match (low, high) {
        (
            RuntimeValue::PhysicalEntry(Value::AddressLow(a)),
            RuntimeValue::PhysicalEntry(Value::AddressHigh { chain: b, .. }),
        ) if a == b => Some(a.clone()),
        _ => None,
    }
}

/// Fixed bounded result adapter, separate from the legacy <=2 scalar results.
/// No heap allocation, no widening of the legacy scalar operation dispatcher.
pub(super) struct Results {
    values: [Option<RuntimeValue>; 5],
    len: usize,
}
impl Results {
    pub(super) fn empty() -> Self {
        Self {
            values: [None, None, None, None, None],
            len: 0,
        }
    }
    pub(super) fn push(
        &mut self,
        value: RuntimeValue,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        let Some(slot) = self.values.get_mut(self.len) else {
            return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                "physical result bound",
            ));
        };
        *slot = Some(value);
        self.len += 1;
        Ok(())
    }
    pub(super) fn bind(
        self,
        engine: &Engine<'_, impl SimulationEventSinkV1>,
        values: &mut HashMap<ValueId, RuntimeValue>,
        definitions: &[ValueDef],
        site: &CompactSite,
    ) -> Result<(), SimulationExecutionErrorV1> {
        if definitions.len() != self.len {
            return Err(engine.at(
                *site,
                SimulationExecutionErrorKindV1::ResultArity {
                    expected: definitions.len(),
                    actual: self.len,
                },
            ));
        }
        for (definition, value) in definitions.iter().zip(self.values) {
            let value = value.ok_or_else(|| {
                engine.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant("physical result prefix"),
                )
            })?;
            bind_typed_value(engine, values, definition, value, site)?;
        }
        Ok(())
    }
}
