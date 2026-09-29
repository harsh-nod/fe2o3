macro_rules! dispatch_cancel_binding_body {
    ($syntax:ident, $owner:ident, $identity:ident) => {
        $syntax!({ $owner.generation.cancel_epoch($identity) })
    };
}
