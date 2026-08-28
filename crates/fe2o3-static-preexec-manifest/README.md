# fe2o3 static pre-exec manifest

This crate is the canonical safe Rust encoder and structural validator for the
static pre-exec launcher's V1 descriptor manifest. Its 704-byte little-endian
wire format exactly matches
`tools/fe2o3-static-preexec-launcher/include/fe2o3_static_preexec_manifest.h`.

The codec validates all constraints represented by the record: versioning,
reserved bytes, parent identity bounds, descriptor count, ordered source file
descriptors, bounded and unique destinations, required standard descriptors,
zero inactive slots, object-validation classes, and non-aliasing object keys.
Ordinary objects use strict `fstat` identity. Process-pidfd objects additionally
require a live Linux pidfd in the launcher; multiple pidfds may share Linux's
anonymous-inode `fstat` key, so their exact target PID and process start time
must be bound by the receiving service. The launcher remains responsible for
seals, live snapshots, validation classes, descriptor access modes, and closed
unused descriptors.
