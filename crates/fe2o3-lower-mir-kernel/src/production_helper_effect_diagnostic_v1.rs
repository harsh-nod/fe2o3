#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum HelperEffectDecisionKindV1 {
    Missing,
    Incomplete,
    CompleteWithEffects,
    CompletePure,
}

/// Fixed-size diagnostic categories from an existing effect decision.
///
/// This deliberately omits multiplicity, atomic order/scope and operation
/// coordinates. It is not an effect summary, proof, admission or wire schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionHelperEffectDiagnosticV1 {
    decision: HelperEffectDecisionKindV1,
    physical: [u8; 8],
    compiler_order: u8,
}

impl ProductionHelperEffectDiagnosticV1 {
    fn from_decision(decision: Option<&fe2o3_kernel_ir::InterproceduralEffectDecisionV1>) -> Self {
        use fe2o3_kernel_ir::InterproceduralEffectDecisionV1 as Decision;
        let mut diagnostic = Self {
            decision: HelperEffectDecisionKindV1::Missing,
            physical: [0; 8],
            compiler_order: 0,
        };
        let Some(decision) = decision else {
            return diagnostic;
        };
        diagnostic.decision = match decision {
            Decision::Incomplete { .. } => HelperEffectDecisionKindV1::Incomplete,
            Decision::Complete(summary) if summary.is_pure() => {
                HelperEffectDecisionKindV1::CompletePure
            }
            Decision::Complete(_) => HelperEffectDecisionKindV1::CompleteWithEffects,
        };
        // The closed enums bound this existing set to 1,750 distinct effects:
        // 5*5 simple, 5^3 atomic, 5^2*2^5 synchronization, 5^2*2^5 fence.
        // Each address-space set has at most five members. No graph is walked
        // and no ledger-funded analysis, allocation or cloned summary is added.
        for effect in decision.summary().effects() {
            let (category, spaces) = match effect {
                MemoryEffect::Allocate(space) => (0, helper_effect_space_bit_v1(*space)),
                MemoryEffect::Read(space) => (1, helper_effect_space_bit_v1(*space)),
                MemoryEffect::Write(space) => (2, helper_effect_space_bit_v1(*space)),
                MemoryEffect::VolatileRead(space) => (3, helper_effect_space_bit_v1(*space)),
                MemoryEffect::VolatileWrite(space) => (4, helper_effect_space_bit_v1(*space)),
                MemoryEffect::Atomic { address_space, .. } => {
                    (5, helper_effect_space_bit_v1(*address_space))
                }
                MemoryEffect::Synchronize { address_spaces, .. } => (
                    6,
                    address_spaces
                        .iter()
                        .fold(0, |mask, space| mask | helper_effect_space_bit_v1(*space)),
                ),
                MemoryEffect::Fence { address_spaces, .. } => (
                    7,
                    address_spaces
                        .iter()
                        .fold(0, |mask, space| mask | helper_effect_space_bit_v1(*space)),
                ),
            };
            // Preserve the category even for an empty synchronization/fence set.
            diagnostic.physical[category] |= 0x80 | spaces;
        }
        let ordering = decision.summary().compiler_ordering();
        diagnostic.compiler_order = u8::from(ordering.has_ordered_verification_contract())
            | (u8::from(ordering.has_ordered_execution()) << 1)
            | (u8::from(ordering.has_ordered_region()) << 2);
        diagnostic
    }
}

fn helper_effect_space_bit_v1(space: AddressSpace) -> u8 {
    match space {
        AddressSpace::Private => 1,
        AddressSpace::Workgroup => 2,
        AddressSpace::Global => 4,
        AddressSpace::Constant => 8,
        AddressSpace::Generic => 16,
    }
}

fn fmt_helper_effect_names_v1(
    formatter: &mut fmt::Formatter<'_>,
    mask: u8,
    names: &[&str],
) -> fmt::Result {
    let mut separator = "";
    for (index, name) in names.iter().enumerate() {
        if mask & (1 << index) != 0 {
            formatter.write_str(separator)?;
            formatter.write_str(name)?;
            separator = ",";
        }
    }
    Ok(())
}

impl fmt::Display for ProductionHelperEffectDiagnosticV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self.decision {
            HelperEffectDecisionKindV1::Missing => "missing",
            HelperEffectDecisionKindV1::Incomplete => "incomplete",
            HelperEffectDecisionKindV1::CompleteWithEffects => "complete-with-effects",
            HelperEffectDecisionKindV1::CompletePure => "complete-and-pure",
        };
        write!(formatter, "effect decision={kind}; ")?;
        if self.decision == HelperEffectDecisionKindV1::Missing {
            return formatter.write_str(
                "physical=unavailable; compiler_order=unavailable; contributing operation=unavailable",
            );
        }
        let physical = if self.decision == HelperEffectDecisionKindV1::Incomplete {
            "partial_physical"
        } else {
            "physical"
        };
        write!(formatter, "{physical}=[")?;
        let mut separator = "";
        for (category, mask) in [
            "allocate",
            "read",
            "write",
            "volatile-read",
            "volatile-write",
            "atomic",
            "synchronize",
            "fence",
        ]
        .iter()
        .zip(self.physical)
        {
            if mask != 0 {
                write!(formatter, "{separator}{category}(")?;
                fmt_helper_effect_names_v1(
                    formatter,
                    mask,
                    &["private", "workgroup", "global", "constant", "generic"],
                )?;
                formatter.write_str(")")?;
                separator = ",";
            }
        }
        let ordering = if self.decision == HelperEffectDecisionKindV1::Incomplete {
            "partial_compiler_order"
        } else {
            "compiler_order"
        };
        write!(formatter, "]; {ordering}=[")?;
        fmt_helper_effect_names_v1(
            formatter,
            self.compiler_order,
            &["verification-contract", "execution", "region"],
        )?;
        formatter.write_str("]; contributing operation=unavailable")
    }
}

#[cfg(test)]
#[path = "production_helper_effect_diagnostic_v1_tests.rs"]
mod helper_effect_diagnostic_v1_tests;
