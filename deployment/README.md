# fe2o3 protected compiler-execution deployment

These files define the sole systemd deployment for the root coordinator,
protected supervisor, and local reference external anchor. They require systemd
253 or newer because the service uses ordered `OpenFile=` activation.

Install the files under their matching system directories:

- `systemd/*.service` and `systemd/*.socket` under the system unit directory;
- `sysusers.d/*.conf` under the system sysusers directory; and
- `tmpfiles.d/*.conf` under the system tmpfiles directory.

Run `systemd-sysusers` and `systemd-tmpfiles --create`, then install the five
static service images as root-owned, root-group, single-link mode `0555` files
at the paths named by the service unit. None may carry a file capability or
POSIX ACL. Build and install the root provisioning command with
`scripts/build-static-compiler-execution-provisioner.sh`.

With both the service and socket stopped, provision one nonzero policy
generation:

```console
# /usr/libexec/fe2o3/fe2o3-compiler-execution-provision 1
```

The command resolves the actual `fe2o3-compiler` and `fe2o3-anchor` UID/GID
values, measures every fixed static image twice, creates missing 32-byte key
seeds from the kernel random source, derives the complete four-record graph, and
publishes each file with no-replace rename plus file and directory durability.
It is idempotent for identical inputs. Existing mismatched bytes, modes,
ownership, links, ACLs, capabilities, image paths, or generations fail closed
and are never replaced. Seeds use mode `0400`; public records use mode `0444`.
A partial first publication can be resumed with the same command and inputs.

Add only authorized Cargo users to the `fe2o3-compiler` group. Enabling
`fe2o3-compiler-execution.socket` starts the coordinator on first connection.
The coordinator independently validates the complete activation environment,
all 14 descriptors, every record relationship, both service identities, and
all filesystem policy before either child is released.

The bundled same-host external anchor remains qualification-only and carries no
production rollback authority. Production deployment still requires an
independently administered monotonic backend and separately protected key
custody.
