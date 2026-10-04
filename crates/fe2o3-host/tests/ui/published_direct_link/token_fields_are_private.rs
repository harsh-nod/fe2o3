use fe2o3_host::ValidatedPublishedDirectLinkSelectionV1;

fn inspect(token: ValidatedPublishedDirectLinkSelectionV1) {
    let ValidatedPublishedDirectLinkSelectionV1 { selection, .. } = token;
    let _ = selection;
}

fn main() {}
