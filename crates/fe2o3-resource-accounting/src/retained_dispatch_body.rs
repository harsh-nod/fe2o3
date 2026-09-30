macro_rules! resource_retained_credit_dispatch_body_v1 {
    ($this:ident, $credits:ident, $expected:ident) => {{
        let Some(token) = &$credits.token else {
            return false;
        };
        match (&$this.0, &token.account) {
            (AccountHandle::Independent(account), TokenAccount::Independent(actual))
                if Arc::ptr_eq(account, actual) =>
            {
                let Ok(state) = account.state.lock() else {
                    return false;
                };
                independent_retained_observation_v1(
                    &state.records,
                    state.poisoned,
                    token.slot,
                    token.owner,
                    $expected,
                )
            }
            (AccountHandle::Domain(account), TokenAccount::Domain(root)) => {
                account.matches_retained_charge(root, token.slot, token.owner, $expected)
            }
            _ => false,
        }
    }};
}
