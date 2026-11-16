# Native link encryption prototype

The [root deployment kit](DEPLOYMENT.md) provides a guarded coordinator,
offline packaging, persistent native configuration and rollback. Its pinned
tools include nftables; the original prototype bundle below remains historical.

This directory packages the standard Linux MACsec/MKA supplicant for isolated
network-namespace experiments. It changes no production interface or service.
The kernel supports MACsec on the three nodes and storage gateway; native MKA
and Cilium integration still require verification before a production plan.

The published [immutable bundle](tools.ref) was built from qrab `48d26c9` and
pulled back by digest. Its binary hash matches the tested patched bundle.
Extract it with `podman create "$(cat contrib/network-encryption/tools.ref)" /unused`
and `podman cp` as below. No CAK or native service is included.
The same bundle is staged at `/var/tmp/qrab-link-tools-release-91129ff32d6d`
on kube01, kube02 and kube03; version and binary hash match on each node.

Build from the pinned qrab QEMU runtime. Package retrieval uses verified HTTPS,
package signatures and the existing strict APT helper. The bundle retains the
package versions, runtime dependency list and license files. It carries its own
loader/libraries so testing does not replace host packages or start services:

```sh
bash contrib/images/build.sh link-tools macsec-2.11-7
container=$(podman create zot.yapplesauce.com/qrab/link-tools:macsec-2.11-7 /unused)
podman cp "$container":/. /tmp/qrab-link-tools
podman rm "$container"
/tmp/qrab-link-tools/run-wpa -v
/tmp/qrab-link-tools/run-wpa -h
```

Use `macsec_linux` in a disposable network namespace. Keep generated CAKs and
supplicant configurations outside the repository with root-only permissions.
Do not use key-debug logging. Standard MKA must negotiate fresh session keys and
handle rekey/restart; a persistent manually configured static transmit key with
a reset packet number is unsuitable for production.

The prototype must verify peer discovery, wrong-key refusal, encrypted traffic,
plaintext/replay rejection, address preservation, restart/rekey and MTU behavior.
Production additionally requires Cilium host-firewall/device integration,
private bridge EAPOL forwarding, boot ordering and a tested rollback. The
physical storage/gateway needs a root execution path for those later steps.

The current isolated oracle runs without manager, QEMU, KVM or guest downloads:

```sh
sudo python3 contrib/network-encryption/check-macsec.py --tools /var/tmp/qrab-link-tools-20261116
```

It creates its switch and all three endpoints in private network namespaces,
keeps CAKs in a root-only `/run` directory and removes the namespaces afterward.
IPv4/IPv6 UDP, 8 KiB TCP on ports 179/3260, preserved source addresses, a wrong
CAK, plaintext injection, replay and key-server restart have passed on kube01.
A lower-link capture recorded 36 MACsec frames, six MKA frames and zero clear
IP frames; restart established a different session-key identifier. Advancing
the isolated transmit packet number near exhaustion also triggered automatic
MKA rekey and restored traffic with a different session key. The port checks
are synthetic TCP echo, not real iSCSI/BGP protocol or fencing tests.
The restart check requires nonempty, disjoint old/new session-key identifier
sets; merely retiring an old SA cannot satisfy it after automatic rekey.

The current deployed Cilium host firewall rejects MACsec/MKA traffic. A
separate fork change passes those frames only on explicitly named external
lower devices; the 24 compiled-kernel cases retain IP policy and the disabled
default. Native integration still needs upper-device attachment and live
verification. No production interface has been modified. See the
[deployment requirements and diagram](../../docs/node-link-encryption.md).

The deployable tool recipe pins Debian `2:2.11-7`, which contains the fixes for
[CVE-2026-78807](https://security-tracker.debian.org/tracker/CVE-2026-78807) and
the [configuration-write issue](https://security-tracker.debian.org/tracker/TEMP-1148985-A50803).
The initial `2:2.10-24` run established protocol feasibility only. All listed
checks were repeated successfully with `2:2.11-7`; no host packages were replaced.
