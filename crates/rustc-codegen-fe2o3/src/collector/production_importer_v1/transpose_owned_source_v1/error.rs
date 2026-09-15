use super::*;

impl std::fmt::Display for PlanError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => write!(out, "transpose source custody rejected: {error:?}"),
            Self::Replay(error) => write!(out, "transpose source replay rejected: {error}"),
            Self::Ssa(error) => write!(out, "transpose source SSA rejected: {error}"),
            Self::Query(error) => write!(out, "transpose source use rejected: {error}"),
            Self::Typed(error) => write!(out, "transpose typed source custody rejected: {error}"),
        }
    }
}

impl std::error::Error for PlanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(_) => None,
            Self::Replay(error) => Some(error),
            Self::Ssa(error) => Some(error),
            Self::Query(error) => Some(error),
            Self::Typed(error) => Some(error),
        }
    }
}
