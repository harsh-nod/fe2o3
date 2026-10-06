/// Exact claimed access guard. An explicit predicate has no CFG edge, including
/// when its bound comparison is also the complete predicate. This is inert data;
/// only the owning graph/source consumer can authenticate the stated equations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixedAccessGuardV86 {
    CfgEdge {
        edge: MixedEdgeV26,
        condition: MixedDefinitionV26,
    },
    ExplicitPredicate {
        condition: MixedDefinitionV26,
        bound_comparison: MixedDefinitionV26,
    },
}

impl MixedAccessGuardV86 {
    pub const fn condition(self) -> MixedDefinitionV26 {
        match self {
            Self::CfgEdge { condition, .. } | Self::ExplicitPredicate { condition, .. } => {
                condition
            }
        }
    }

    pub const fn edge(self) -> Option<MixedEdgeV26> {
        match self {
            Self::CfgEdge { edge, .. } => Some(edge),
            Self::ExplicitPredicate { .. } => None,
        }
    }

    fn validate(self, function: u32, path: MixedGuardPathV26) -> Format<()> {
        if self.condition().function() != function {
            return Err("mixed V86 guard function");
        }
        match (self, path) {
            (Self::CfgEdge { edge, .. }, MixedGuardPathV26::TrueEdge { successor, .. })
                if edge.function == function && u64::from(edge.successor) == successor =>
            {
                Ok(())
            }
            (
                Self::ExplicitPredicate {
                    bound_comparison, ..
                },
                MixedGuardPathV26::ExplicitPredicate,
            ) if bound_comparison.function() == function => Ok(()),
            _ => Err("mixed V86 guard kind or path"),
        }
    }
}

impl Wire for MixedAccessGuardV86 {
    const BYTES: usize = 1 + MixedEdgeV26::BYTES + 2 * MixedDefinitionV26::BYTES;

    fn read(reader: &mut Reader<'_>) -> Format<Self> {
        match u8::read(reader)? {
            0 => {
                let edge = MixedEdgeV26::read(reader)?;
                let condition = MixedDefinitionV26::read(reader)?;
                if <[u8; 17]>::read(reader)? != [0; 17] {
                    return Err("mixed V86 CFG guard padding");
                }
                Ok(Self::CfgEdge { edge, condition })
            }
            1 => {
                // Reserved bytes are padding, never a fabricated edge value.
                if <[u8; 12]>::read(reader)? != [0; 12] {
                    return Err("mixed V86 explicit guard padding");
                }
                Ok(Self::ExplicitPredicate {
                    condition: MixedDefinitionV26::read(reader)?,
                    bound_comparison: MixedDefinitionV26::read(reader)?,
                })
            }
            _ => Err("mixed V86 guard tag"),
        }
    }

    fn write(&self, output: &mut Output<'_>) {
        match *self {
            Self::CfgEdge { edge, condition } => {
                0u8.write(output);
                edge.write(output);
                condition.write(output);
                [0u8; 17].write(output);
            }
            Self::ExplicitPredicate {
                condition,
                bound_comparison,
            } => {
                1u8.write(output);
                [0u8; 12].write(output);
                condition.write(output);
                bound_comparison.write(output);
            }
        }
    }
}
