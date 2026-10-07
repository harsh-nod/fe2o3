macro_rules! runtime_retained_credit_dispatch_body_v1 {
    ($this:ident, $device:ident, $credits:ident, $expected:ident) => {{
        $this.device == $device
            && match (&$this.inner, $credits) {
                (
                    RuntimeResourceCreditAccountInnerV1::General(account),
                    RuntimeRetainedResourceCreditsV1::General(credits),
                ) => account.matches_retained_charge_v1(credits, $expected),
                (
                    RuntimeResourceCreditAccountInnerV1::Composed(admission),
                    RuntimeRetainedResourceCreditsV1::Composed(credits),
                ) => {
                    let bytes = $expected.get(RuntimeResourceKindV1::RequestedAllocationBytes);
                    $expected == request_charge(bytes)
                        && admission
                            .account()
                            .matches_retained_charge_v1(credits, bytes)
                }
                _ => false,
            }
    }};
}
