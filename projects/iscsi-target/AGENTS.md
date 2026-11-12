# iSCSI target administration

This root-only CLI manages only dedicated qrab ZFS/LIO objects. Its paths,
portal and naming rules are fixed; there is no remote administration API or
caller-selected backing device. The `vm-*` subcommands are the fixed wrapper
behind the towl storage daemon; keep their argv exact and credentials on stdin.
`vm-image-import` is the exception: it reads a local file, so keep it
administrator-only and out of `disk::COMMANDS`/`dispatch`.
Read `contrib/storage/README.md` before host operations. Source changes and privileged installation/application are separate.

Preserve private credential state, exact ownership/GUID checks and fail-closed
fencing. A disabled TPG alone is insufficient: remove its owned ACL and LUN
export and wait for the kernel teardown barriers. Do not call timeouts or
Kubernetes state a target fence. Fence must retain data and rotate CHAP on
recovery. Never print credentials or put them in argv.
