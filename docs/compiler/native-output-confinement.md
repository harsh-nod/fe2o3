# Native Compiler Output Confinement

The native attempt selects output confinement on its original checkpointed
stage. Before profile READY, the child applies Landlock to the retained directory
binding at FD197. ABI3 or newer is required; unavailable enforcement aborts the
owned child. Installation work and scratch are prepaid on the request account.

The policy handles file writes, truncation, creation, removal and cross-directory
link/rename rights. Only regular-file and directory output operations beneath
the original directory are allowed. Device, socket, FIFO and symlink creation
remain denied. Captured arguments/environment are unchanged: other output,
incremental or temporary paths do not acquire permission from argv.

This is one prerequisite, not activation. Existing descriptors retain their
rights; device-open effects, imported descriptors, ioctl, executable mappings,
external writers and source identity require their independent checks. Arbitrary
hardlinks already present in the output hierarchy can alias external files;
path confinement does not prove that every alias of an inode is inside it.
Only the separate immutable-file contract protects approved executable bytes.
Arbitrary
user-source snapshots are not introduced: the compiler claim begins at captured
MIR, and approved executable files retain their existing immutable-file contract.
The controller must not admit opens from this staging flag alone.

The [kernel Landlock documentation](https://docs.kernel.org/userspace-api/landlock.html)
describes ABI3 truncation, inheritance and already-open descriptor limitations.
The isolated kernel test checks creation, truncation, rename/link escape,
exec inheritance and the inherited-descriptor limitation. Component tests do not
qualify an approved runtime, production compilation or artifact publication.
