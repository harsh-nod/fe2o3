//! Loop-state payload accounting on the inherited reference-extraction meter.

use super::*;
use reference_extraction_work_v1::add;

impl ReferenceExtractionWorkV1<'_> {
    pub(super) fn symbolic_units(
        &self,
        value: &ReferenceSymbolicValueV2,
    ) -> Result<usize, ReferenceBindingErrorV1> {
        self.charge(1)?;
        let expression = match value {
            ReferenceSymbolicValueV2::Scalar(expression)
            | ReferenceSymbolicValueV2::CheckedPair {
                value: expression, ..
            } => expression,
        };
        add(
            std::mem::size_of::<ReferenceSymbolicValueV2>(),
            self.expression(expression)?,
        )
    }

    pub(super) fn clone_symbolic(
        &self,
        value: &ReferenceSymbolicValueV2,
    ) -> Result<ReferenceSymbolicValueV2, ReferenceBindingErrorV1> {
        self.charge(self.symbolic_units(value)?)?;
        Ok(value.clone())
    }

    pub(super) fn environment_units(
        &self,
        environment: &ReferenceSymbolicEnvironmentV2,
    ) -> Result<usize, ReferenceBindingErrorV1> {
        let mut units = 0;
        for value in environment.values() {
            self.charge(1)?;
            units = add(
                units,
                std::mem::size_of::<(u32, ReferenceSymbolicValueV2)>(),
            )?;
            units = add(units, self.symbolic_units(value)?)?;
        }
        Ok(units)
    }

    pub(super) fn trace_units(
        &self,
        trace: &ReferenceLoopTraceV2,
    ) -> Result<usize, ReferenceBindingErrorV1> {
        self.charge(1)?;
        let mut units = add(
            std::mem::size_of::<ReferenceLoopTraceV2>(),
            self.environment_units(&trace.initial)?,
        )?;
        for environment in &trace.transitions {
            self.charge(1)?;
            units = add(units, std::mem::size_of::<ReferenceSymbolicEnvironmentV2>())?;
            units = add(units, self.environment_units(environment)?)?;
        }
        for expression in &trace.variants {
            units = add(units, self.expression(expression)?)?;
        }
        Ok(units)
    }

    pub(super) fn state_units(
        &self,
        state: &ReferenceSymbolicStateV2,
    ) -> Result<usize, ReferenceBindingErrorV1> {
        self.charge(1)?;
        let mut units = add(
            std::mem::size_of::<ReferenceSymbolicStateV2>(),
            self.environment_units(&state.environment)?,
        )?;
        units = add(units, self.clauses(&state.guard.clauses)?)?;
        for trace in state.traces.values() {
            units = add(units, std::mem::size_of::<(u32, u32)>())?;
            units = add(units, self.trace_units(trace)?)?;
        }
        Ok(units)
    }
}

#[cfg(test)]
#[path = "reference_loop_work_v1_tests.rs"]
mod tests;
