// Inputs are the actual locked record slice and stored poison flag, not
// precomputed validity receipts. The caller retains both account and token.
macro_rules! independent_retained_observation_body_v1 {
    ($records:ident, $poisoned:ident, $slot:ident, $owner:ident, $expected:ident) => {{
        if $poisoned {
            return false;
        }
        if $slot >= $records.len() {
            return false;
        }
        match &$records[$slot] {
            Some(record) => record.matches_retained_charge($owner, $expected),
            None => false,
        }
    }};
}
