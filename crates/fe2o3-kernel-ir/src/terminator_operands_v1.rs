use crate::{Terminator, ValueId};

impl Terminator {
    pub fn operands(&self) -> Vec<ValueId> {
        let mut operands = Vec::with_capacity(self.operand_count());
        self.visit_operands(|operand| operands.push(operand));
        operands
    }

    /// Returns the exact number of SSA operands without allocating.
    pub fn operand_count(&self) -> usize {
        match self {
            Self::Branch { arguments, .. } => arguments.len(),
            Self::ConditionalBranch {
                then_arguments,
                else_arguments,
                ..
            } => 1 + then_arguments.len() + else_arguments.len(),
            Self::Switch {
                cases,
                default_arguments,
                ..
            } => {
                1 + cases.iter().map(|case| case.arguments.len()).sum::<usize>()
                    + default_arguments.len()
            }
            Self::IntegerSwitch {
                cases,
                default_arguments,
                ..
            } => {
                1 + cases.iter().map(|case| case.arguments.len()).sum::<usize>()
                    + default_arguments.len()
            }
            Self::Return { values } => values.len(),
            Self::Unreachable => 0,
        }
    }

    /// Visits SSA operands without allocating an operand list: selector first,
    /// followed by source-order edge arguments, then default-edge arguments.
    /// Stops immediately when the visitor returns an error.
    pub fn try_visit_operands<E>(
        &self,
        mut visitor: impl FnMut(ValueId) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Branch { arguments, .. } => {
                arguments.iter().copied().try_for_each(&mut visitor)?;
            }
            Self::ConditionalBranch {
                condition,
                then_arguments,
                else_arguments,
                ..
            } => {
                visitor(*condition)?;
                then_arguments.iter().copied().try_for_each(&mut visitor)?;
                else_arguments.iter().copied().try_for_each(&mut visitor)?;
            }
            Self::Switch {
                selector,
                cases,
                default_arguments,
                ..
            } => {
                visitor(*selector)?;
                for case in cases {
                    case.arguments.iter().copied().try_for_each(&mut visitor)?;
                }
                default_arguments
                    .iter()
                    .copied()
                    .try_for_each(&mut visitor)?;
            }
            Self::IntegerSwitch {
                selector,
                cases,
                default_arguments,
                ..
            } => {
                visitor(*selector)?;
                for case in cases {
                    case.arguments.iter().copied().try_for_each(&mut visitor)?;
                }
                default_arguments
                    .iter()
                    .copied()
                    .try_for_each(&mut visitor)?;
            }
            Self::Return { values } => {
                values.iter().copied().try_for_each(&mut visitor)?;
            }
            Self::Unreachable => {}
        }
        Ok(())
    }

    /// Visits every SSA operand in stable semantic order without allocating.
    pub fn visit_operands(&self, mut visitor: impl FnMut(ValueId)) {
        let result: Result<(), std::convert::Infallible> = self.try_visit_operands(|operand| {
            visitor(operand);
            Ok(())
        });
        match result {
            Ok(()) => {}
            Err(never) => match never {},
        }
    }
}
