macro_rules! domain_retained_credit_dispatch_body_v1 {
    ($this:ident, $root:ident, $slot:ident, $owner:ident, $expected:ident) => {{
        if !Arc::ptr_eq(&$this.root, $root) {
            return false;
        }
        let Ok(state) = $this.root.state.lock() else {
            return false;
        };
        domain_retained_observation_v1(
            &state.nodes,
            state.max_depth,
            &state.records,
            state.poisoned,
            $this.key,
            $slot,
            $owner,
            $expected,
        )
    }};
}
