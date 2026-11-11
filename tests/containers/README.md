# Retired QEMU version matrix

The Debian 3.1/5.2/7.2 wrapper harness was removed. It selected an obsolete
`qapi-protocol --test minimal` target, silently skipped missing images, and
could report success after failing tests. Archived apt repositories and global
`/tmp` bind mounts are not the current development path.

Use `contrib/images` for images built from pinned local QEMU source,
`contrib/check-runtime.sh` for normal Rust checks, and the explicit live tests
in `docs/testing.md` / `docs/manager.md`. Older-QEMU compatibility work is
deferred; the previous harness remains available in Git history if needed.
