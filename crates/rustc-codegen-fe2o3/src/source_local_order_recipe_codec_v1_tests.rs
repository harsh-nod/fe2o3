//! Inert private draft, not an allocated public recipe schema or checked owner.
use serde::{Deserialize, Serialize};

pub(crate) const BYTE_CAP: usize = 8192;
pub(crate) const BINDING_CHANGED: &str = "local-order recipe item/instance binding changed";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InstanceBinding {
    pub(crate) function: [u8; 32],
    pub(crate) item: [u8; 32],
    pub(crate) monomorphization: [u8; 32],
    pub(crate) generic_types: [u8; 32],
    pub(crate) const_arguments: [u8; 32],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Order {
    SourceOrder,
    ReverseReady,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Origin {
    pub(crate) source: [u8; 32],
    pub(crate) semantic: [u8; 32],
    pub(crate) bound: [u8; 32],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Target {
    Gfx942XnackOffWave64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Shape {
    U32XorOrAndFourDistinctFormalParameters,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Recipe {
    private_draft_version: u8,
    target: Target,
    required: [u32; 3],
    maximum: [u32; 3],
    parameter_ordinals: [u32; 4],
    shape: Shape,
    binding: InstanceBinding,
    preference: Order,
    origin: Origin,
}

impl Recipe {
    /// Constructs inert intent only. Every use must bind against live rustc.
    pub(crate) fn new(binding: InstanceBinding, origin: Origin, preference: Order) -> Self {
        Self {
            private_draft_version: 1,
            target: Target::Gfx942XnackOffWave64,
            required: [64, 1, 1],
            maximum: [64, 1, 1],
            parameter_ordinals: [1, 2, 3, 4],
            shape: Shape::U32XorOrAndFourDistinctFormalParameters,
            binding,
            preference,
            origin,
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.private_draft_version != 1 {
            return Err("local-order recipe private draft version unsupported".into());
        }
        if self.required != [64, 1, 1]
            || self.maximum != [64, 1, 1]
            || self.parameter_ordinals != [1, 2, 3, 4]
        {
            return Err("local-order recipe exact profile mismatch".into());
        }
        Ok(())
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > BYTE_CAP {
            return Err("local-order recipe byte limit".into());
        }
        // The closed fixed-size shape bounds decoded allocation and nesting.
        // Derived struct visitors reject duplicate fields at every nesting level.
        let recipe: Self = serde_json::from_slice(bytes)
            .map_err(|_| "local-order recipe malformed or unknown field".to_string())?;
        recipe.validate()?;
        Ok(recipe)
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.is_empty() || bytes.len() > BYTE_CAP {
            return Err("local-order recipe byte limit".into());
        }
        Ok(bytes)
    }

    pub(crate) fn bind(&self, current: InstanceBinding) -> Result<Order, String> {
        self.validate()?;
        if self.binding != current {
            return Err(BINDING_CHANGED.into());
        }
        Ok(self.preference)
    }

    pub(crate) fn origin(&self) -> Origin {
        self.origin
    }
}

#[path = "source_local_order_recipe_codec_cases_v1_tests.rs"]
mod cases;
