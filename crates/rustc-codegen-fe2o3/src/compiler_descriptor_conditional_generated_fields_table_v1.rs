//! Bounded projection scratch. This private table alone conveys no authority.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Occurrence {
    pub(super) canonical: FunctionOperationLocation,
    pub(super) ranked: (u32, u32),
    pub(super) parameter: u32,
}

const EMPTY: ConditionalGeneratedArgumentV1 = ConditionalGeneratedArgumentV1 {
    projection: ConditionalArgumentBindingV1 {
        canonical_parameter: 0,
        source_argument: 0,
        adjusted_argument: 0,
        semantic_local: 0,
        semantic_type: 0,
        generated_field: 0,
        role: Role::Input,
        source_type_identity: [0; 32],
        device_layout_identity: [0; 32],
    },
    canonical_value: ValueId(0),
};

pub(super) struct Scratch {
    arguments: [ConditionalGeneratedArgumentV1; MAX_CONDITIONAL_ARGUMENTS_V1],
    argument_count: usize,
    reads: [Option<Occurrence>; MAX_CONDITIONAL_READS_V1],
    read_arguments: [u16; MAX_CONDITIONAL_READS_V1],
    read_count: usize,
}

impl Scratch {
    pub(super) fn new() -> Self {
        Self {
            arguments: [EMPTY; MAX_CONDITIONAL_ARGUMENTS_V1],
            argument_count: 0,
            reads: [None; MAX_CONDITIONAL_READS_V1],
            read_arguments: [0; MAX_CONDITIONAL_READS_V1],
            read_count: 0,
        }
    }
    pub(super) fn arguments(&self) -> &[ConditionalGeneratedArgumentV1] {
        &self.arguments[..self.argument_count]
    }
    pub(super) fn read_arguments(&self) -> &[u16] {
        &self.read_arguments[..self.read_count]
    }
    pub(super) fn insert(
        &mut self,
        row: ConditionalGeneratedArgumentV1,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        budget.charge_work(
            self.argument_count
                .checked_mul(96)
                .and_then(|v| v.checked_add(8))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let key = row.projection.canonical_parameter;
        for previous in self.arguments() {
            if previous.projection.canonical_parameter == key {
                // Read occurrences may share one exact whole argument. An
                // output/input overlap or any coordinate substitution is not
                // a second association to deduplicate away.
                if *previous != row || row.projection.role != Role::Input {
                    return Err(Error::Mismatch(
                        "conflicting canonical argument association",
                    ));
                }
                return Ok(());
            }
            if previous.projection.generated_field == row.projection.generated_field
                || previous.projection.source_argument == row.projection.source_argument
                || previous.projection.adjusted_argument == row.projection.adjusted_argument
                || previous.canonical_value == row.canonical_value
                || previous.projection.semantic_local == row.projection.semantic_local
            {
                return Err(Error::Mismatch(
                    "substituted generated/source/value/local association",
                ));
            }
        }
        if self.argument_count == MAX_CONDITIONAL_ARGUMENTS_V1 {
            return Err(Error::Mismatch("bounded unique arguments"));
        }
        let slot = self
            .arguments()
            .partition_point(|r| r.projection.canonical_parameter < key);
        self.arguments
            .copy_within(slot..self.argument_count, slot + 1);
        self.arguments[slot] = row;
        self.argument_count += 1;
        Ok(())
    }
    pub(super) fn push_read(
        &mut self,
        row: Occurrence,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        budget.charge_work(
            self.read_count
                .checked_mul(8)
                .and_then(|v| v.checked_add(4))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if self.read_count == MAX_CONDITIONAL_READS_V1 {
            return Err(Error::Mismatch("bounded read occurrences"));
        }
        for previous in &self.reads[..self.read_count] {
            let previous = previous
                .as_ref()
                .ok_or(Error::Mismatch("complete read scratch"))?;
            if previous.canonical == row.canonical || previous.ranked == row.ranked {
                return Err(Error::Mismatch(
                    "duplicate canonical/ranked read occurrence",
                ));
            }
        }
        self.reads[self.read_count] = Some(row);
        self.read_count += 1;
        Ok(())
    }
    pub(super) fn finish(
        &mut self,
        output_parameter: u32,
        expected_reads: usize,
        budget: &mut Budget<'_>,
    ) -> Result<u16, Error> {
        budget.charge_work(
            self.argument_count
                .checked_mul(self.read_count + 1)
                .and_then(|v| v.checked_mul(8))
                .and_then(|v| v.checked_add(8))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if expected_reads != self.read_count {
            return Err(Error::Mismatch("complete read occurrence map"));
        }
        let mut output = None;
        for (index, row) in self.arguments().iter().enumerate() {
            if row.projection.role == Role::Output {
                if row.projection.canonical_parameter != output_parameter
                    || output.replace(index as u16).is_some()
                {
                    return Err(Error::Mismatch("one exact output association"));
                }
            } else if !self.reads[..self.read_count]
                .iter()
                .flatten()
                .any(|read| read.parameter == row.projection.canonical_parameter)
            {
                return Err(Error::Mismatch("unreferenced input association"));
            }
        }
        let output = output.ok_or(Error::Mismatch("missing output association"))?;
        for (index, read) in self.reads[..self.read_count].iter().enumerate() {
            let read = read
                .as_ref()
                .ok_or(Error::Mismatch("missing read occurrence"))?;
            let slot = self.arguments[..self.argument_count]
                .iter()
                .position(|row| {
                    row.projection.canonical_parameter == read.parameter
                        && row.projection.role == Role::Input
                })
                .ok_or(Error::Mismatch("missing input association"))?;
            self.read_arguments[index] = slot as u16;
        }
        Ok(output)
    }
}
