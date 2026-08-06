use fe2o3_host::PendingPublishedDirectLinkLoadAdmissionV1;

fn inspect(token: PendingPublishedDirectLinkLoadAdmissionV1) {
    let PendingPublishedDirectLinkLoadAdmissionV1 { inspection, .. } = token;
    let _ = inspection;
}

fn main() {}
