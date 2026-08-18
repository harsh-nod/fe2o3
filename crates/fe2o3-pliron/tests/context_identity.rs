use fe2o3_pliron::{
    CONTEXT_IDENTITY_MARKER_KEY, ContextIdentityError, ensure_context_identity,
    require_context_identity,
};
use pliron::{context::Context, identifier::Identifier};

fn marker_key() -> Identifier {
    CONTEXT_IDENTITY_MARKER_KEY
        .try_into()
        .expect("fixed marker key is valid")
}

fn take_marker(context: &mut Context) -> Box<dyn std::any::Any> {
    let index = context
        .aux_data_map
        .remove(&marker_key())
        .expect("context identity marker exists");
    context
        .aux_data
        .remove(index)
        .expect("context identity marker is live")
}

fn install_marker(context: &mut Context, marker: Box<dyn std::any::Any>) {
    let index = context.aux_data.insert(marker);
    context.aux_data_map.insert(marker_key(), index);
}

#[test]
fn identities_are_idempotent_and_distinguish_fresh_contexts() {
    let mut first = Context::new();
    let mut second = Context::new();
    let first_identity = ensure_context_identity(&mut first).expect("first identity");
    let second_identity = ensure_context_identity(&mut second).expect("second identity");

    assert_ne!(first_identity, second_identity);
    assert_eq!(ensure_context_identity(&mut first), Ok(first_identity));
    assert_eq!(require_context_identity(&first), Ok(first_identity));
}

#[test]
fn moving_a_marker_to_an_unanchored_context_fails_closed() {
    let mut owner = Context::new();
    ensure_context_identity(&mut owner).expect("owner identity");
    let marker = take_marker(&mut owner);

    let mut foreign = Context::new();
    install_marker(&mut foreign, marker);
    assert_eq!(
        require_context_identity(&foreign),
        Err(ContextIdentityError::CorruptMarker)
    );
}

#[test]
fn moving_a_marker_cannot_replace_a_foreign_context_identity() {
    let mut owner = Context::new();
    let owner_identity = ensure_context_identity(&mut owner).expect("owner identity");
    let owner_marker = take_marker(&mut owner);

    let mut foreign = Context::new();
    let foreign_identity = ensure_context_identity(&mut foreign).expect("foreign identity");
    drop(take_marker(&mut foreign));
    install_marker(&mut foreign, owner_marker);

    assert_ne!(owner_identity, foreign_identity);
    assert_eq!(require_context_identity(&foreign), Ok(foreign_identity));
}

#[test]
fn foreign_and_dangling_locator_values_are_rejected() {
    let mut collision = Context::new();
    let foreign = collision.aux_data.insert(Box::new(9_u32));
    collision.aux_data_map.insert(marker_key(), foreign);
    assert_eq!(
        ensure_context_identity(&mut collision),
        Err(ContextIdentityError::MarkerCollision)
    );

    collision.aux_data.remove(foreign);
    assert_eq!(
        require_context_identity(&collision),
        Err(ContextIdentityError::CorruptMarker)
    );
}
