# QEMU build subprojects

These small source snapshots are exported with `git archive` from the exact
commits named by QEMU 11.1.3's Meson wrap files, then gzip-compressed with a
zero timestamp. Their original licenses are included in each archive.

`contrib/images/qemu-source.json` records the revisions and SHA-256 checksums.
`prepare-source.py` verifies both the snapshots and the QEMU wrap revisions,
then includes the package build files from QEMU's own source tree. The image
build uses `--disable-download`: it does not clone QEMU or any subproject from
upstream. QEMU itself is exported only from our pinned local Gitea checkout.

To update a snapshot, review and acquire the new revision explicitly, export
that revision with `git archive`, gzip it with `mtime=0`, and update the lock.
