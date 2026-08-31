# fe2o3 compiler-execution deployment boundary

This crate owns the canonical install manifest, descriptor-relative bundle
admission, sealed source custody, and atomic offline-root publication boundary.
Verification requires an expected manifest SHA-256 and git commit from outside
the bundle. No digest or commit read from the bundle can authorize itself.

The verified value grants no installation, compiler, signing, publication,
loading, launch, execution, or GPU authority. It retains immutable sealed copies
of the manifest and exact admitted files. The effective-UID-zero installer
consumes that custody, creates the fixed root-owned 12-directory/14-file tree,
and publishes it beneath an exact mode-`0700` install parent with one durable
`RENAME_NOREPLACE`. The returned move-only installed-root value retains the
sealed deployment evidence so every installed object can be revalidated after
publication, while exposing identity metadata but no descriptor or service
authority.

The crate also prepares disposable-root qualification custody. It freshly
revalidates the installed root, admits an independently SHA-pinned SquashFS V4
base image through two reads into a sealed memfd, verifies the exact image
profile, and retains an empty root-owned qualification-parent descriptor. This
preparation grants no mount or execution authority. A second root-only
transaction creates and descriptor-retains the exact empty base/root,
upper/work, run/state, and evidence staging tree. Its fault campaign proves
every interrupted boundary restores an empty parent. Mount attachment, root
composition, and real root/distinct-UID systemd execution remain separate
deployment gates. The private-namespace attachment implementation now uses an
atomic read-only autoclear loop device and detached upstream Linux mount APIs,
then rechecks the composed deployment projection against sealed custody. It has
not yet run under real host root, so live mount and systemd qualification remain
open. See the
[disposable-root V1 contract](../../docs/compiler-execution-disposable-root-v1.md).
