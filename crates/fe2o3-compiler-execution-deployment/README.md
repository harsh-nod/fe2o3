# fe2o3 compiler-execution deployment boundary

This crate owns the canonical install manifest, descriptor-relative bundle
admission, and sealed source custody used before privileged compiler-execution
installation. Verification requires an expected manifest SHA-256 and git commit
from outside the bundle. No digest or commit read from the bundle can authorize
itself.

The returned value grants no installation, compiler, signing, publication,
loading, launch, execution, or GPU authority. It retains immutable sealed copies
of the exact admitted files for a later atomic installer.
