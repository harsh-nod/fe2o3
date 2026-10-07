// Borrowed fields from one locked state; no token or account identity is created.
macro_rules! domain_retained_observation_body_v1 {
    ($nodes:ident, $profile:ident, $records:ident, $poisoned:ident,
     $key:ident, $slot:ident, $owner:ident, $expected:ident) => {{
        if $poisoned || domain_path_v1($nodes, $profile, $key).is_none() {
            return false;
        }
        if $slot >= $records.len() {
            return false;
        }
        match &$records[$slot] {
            Some(record) => {
                record.leaf == $key && record.credit.matches_retained_charge($owner, $expected)
            }
            None => false,
        }
    }};
}
