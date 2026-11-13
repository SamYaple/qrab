# Guest networking

A VM has no network interface unless `spec.nics` names one. Each guest NIC is
backed by its own **NIC Pod**, `<vm>-nic-<name>`. The NIC Pod's IP is the
guest's address on that NIC, its Cilium identity is the NIC's identity, and its
labels carry the NIC's security groups. Guest traffic never enters the VM
Pod's network namespace.

```yaml
spec:
  nics:                       # at most 4, names unique
  - name: lan                 # DNS label, 1..15 characters
    securityGroups: [web, ssh]   # at most 8 DNS labels
```

## Why a Pod per NIC

- **The VM Pod's namespace stays private.** It holds the BMC (8080), Redfish
  (8081) and the manager's control socket. A guest on the same network
  namespace could reach those listeners, and policies selecting the VM Pod would
  have to treat guest traffic and BMC traffic as one identity.
- **Upstream Cilium gives one endpoint per Pod.** A second interface inside the
  VM Pod would share its identity and IP. A separate Pod gets its own
  endpoint, identity and IP from the node's Pod CIDR, which the cluster's BGP
  already advertises (native routing). Nothing like OVN, Multus or a second
  CNI is involved.
- **The `qrab` namespace enforces Pod Security restricted.** There is no
  hostPath, no added capability and no tun device, so the two Pods cannot share
  a Unix socket or a tap. They talk over TCP, and passt needs neither tun nor
  privileges.

## Data path

```
guest virtio-net
  → QEMU -netdev stream (TCP client, VM Pod)
  → NIC Pod :7700, qrab-manager nic (one connection at a time)
  → passt Unix socket (/run/nic/passt.sock)
  → passt: guest L2 ⇄ the NIC Pod's own TCP/UDP/ICMP sockets
  → Cilium (NIC Pod identity and policies)
  → BGP-routed Pod IP
```

QEMU's `stream` netdev and passt use the same framing (a 4-byte big-endian
length, then the Ethernet frame), so the relay copies bytes. QEMU runs with
`"reconnect-ms":1000` and the virtio-net device has no option ROM
(`romfile=`), so it never enters the boot order.

passt answers the guest's ARP, NDP and DHCP itself and hands it the NIC Pod's
own address and gateway. That DHCP serves only this guest; it is not network
DHCP. Outbound connections from the guest leave from the NIC Pod's IP. Inbound
connections to the NIC Pod IP on any TCP or UDP port, except the relay port
7700 and the kernel's ephemeral range (32768–60999), are forwarded to the
guest. This is passt's `-t ~7700 -u ~7700`; passt skips the ephemeral range
because those ports carry its own outbound connections.

The NIC Pod runs `qrab-manager nic --listen 0.0.0.0:7700 --socket
/run/nic/passt.sock`. It binds the relay listener, then starts

```
passt -f -q --no-ns-isolation --socket /run/nic/passt.sock -P /run/nic/passt.pid -t ~7700 -u ~7700
```

- **Exit behaviour.** If passt exits, the relay exits non-zero and Kubernetes
  restarts the container (the Pod keeps its IP).
- **Second connections.** A second TCP connection while one is active is
  closed at once, as passt does with a second client; QEMU retries.
- **Stale sessions.** TCP keepalive (5 s idle, 3 probes 2 s apart) frees the
  slot when a VM Pod vanishes without closing the connection.
- **Readiness.** `qrab-manager nic-ready` passes once passt's pidfile and
  socket exist and the listener is bound. It reads the `ready` record and
  `/proc`, and never connects to passt or 7700, because both serve a single
  client.

### Resource cost

passt holds one listening socket per forwarded port, about 72,000 sockets
(TCP and UDP). Their kernel memory, about 260 MiB, is charged to the NIC
container, which requests 320 MiB and is limited to 512 MiB. passt's own RSS is
about 16 MiB.

### The passt patch

The manager image builds passt from the Debian source
`0.0~git20250503.587980c-2+deb13u1`, taken from the base image's snapshot and
checked against `contrib/images/passt/source.sha256`. It adds
[`0001-no-ns-isolation.patch`](../contrib/images/passt/0001-no-ns-isolation.patch).

Upstream passt always isolates itself:

1. `unshare` into a user namespace, then into IPC, mount, UTS and PID
   namespaces;
2. `pivot_root` into an empty tmpfs.

containerd's RuntimeDefault seccomp profile refuses `unshare`, `mount`,
`umount2` and `setns` to a container without CAP_SYS_ADMIN. In a restricted
Pod, upstream passt therefore exits with "Couldn't create user namespace". The
patch adds the opt-in flag `--no-ns-isolation`, which skips only those steps.

passt still keeps the rest of its hardening:

- it switches UID and GID;
- it clamps and drops capabilities (none are present);
- it sets `no_new_privs`;
- it installs its own seccomp filter. Under the Pod's RuntimeDefault filter,
  `/proc/<pid>/status` shows `Seccomp: 2` and `Seccomp_filters: 2`.

The Pod itself supplies the remaining isolation: a separate network namespace,
read-only root, no capabilities, no service-account token and RuntimeDefault
seccomp. Without the flag, passt behaves exactly as upstream.
`images.lock.json` records the version and the patch hash.

## Security groups

Each entry in `securityGroups` becomes the label
`sg.qrab.yapplesauce.com/<group>: ""` on the NIC Pod. Security-group rules are
ordinary CiliumNetworkPolicies selecting those labels:

```yaml
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: sg-web
  namespace: qrab
spec:
  endpointSelector:
    matchLabels:
      sg.qrab.yapplesauce.com/web: ""
  ingress:
  - fromEntities: [world, cluster]
    toPorts:
    - ports:
      - {port: '80', protocol: TCP}
      - {port: '443', protocol: TCP}
  egress:
  - toEntities: [world]
```

Changing a NIC's `securityGroups` patches the NIC Pod's labels in place. Cilium
recomputes its identity; the guest, its IP and the VM Pod are untouched.

### Default allow, and a baseline

The operator's own policy for a NIC (below) never enables default deny. **A
NIC Pod that no security-group policy selects is default-allow** in both
directions: any source can reach every forwarded guest port, and the guest
can reach anything. That is Cilium's rule for an endpoint that no enforcing
policy selects. Install a default-deny baseline for every NIC Pod and open
only what each security group needs:

```yaml
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: vm-nic-baseline
  namespace: qrab
spec:
  endpointSelector:
    matchLabels:
      app.kubernetes.io/component: vm-nic
  # Empty rules: selecting the NIC Pods turns on default deny both ways.
  ingress:
  - {}
  egress:
  - toEndpoints:
    - matchLabels:
        k8s:io.kubernetes.pod.namespace: kube-system
        k8s:k8s-app: kube-dns
    toPorts:
    - ports:
      - {port: '53', protocol: UDP}
      - {port: '53', protocol: TCP}
```

The relay rule below keeps working under such a baseline.

## The relay policy

For each NIC the operator owns a CiliumNetworkPolicy `<vm>-nic-<name>-relay`
with two rules. Both set `enableDefaultDeny: {ingress: false, egress: false}`,
so they only add rules and never switch an endpoint into default deny.

1. **Rule for the NIC Pod.**
   - It allows TCP 7700 from this VM's Pod only (`app.kubernetes.io/component:
     vm` and `qrab.yapplesauce.com/owner-uid: <uid>`).
   - Its `ingressDeny` refuses 7700 from every other source: the world, host,
     remote-node, kube-apiserver, ingress, health, init and unmanaged
     entities; Pods in other namespaces; non-VM Pods; and other VMs' Pods.
   - Deny rules take precedence over allows, so no security-group rule, however
     broad, opens the relay port.
2. **Rule for the VM Pod.** It allows egress to its own NIC Pods on TCP 7700.
   This rule matters when another policy, such as `qrab-demo`'s in
   `console.yaml`, restricts the VM Pod's egress.

NIC Pods are labelled `app.kubernetes.io/component: vm-nic` and share
`app.kubernetes.io/instance: <vm>` with their VM Pod. A Service or policy meant
for the VM Pod must therefore also select `app.kubernetes.io/component: vm`, as
`console.yaml` does, so that NIC Pods never receive BMC or Redfish traffic.

## Addresses and replacement

- **Deterministic MACs.** The guest MAC is `02:` followed by five bytes of
  SHA-256 over the VM UID and NIC name. It is unicast and locally
  administered, and only QEMU knows it; passt learns it from the guest's
  frames.
- **Ordering.** The operator creates the NIC Pods first. It reports reason
  `NicPending` until every one is Ready with an IP. Only then does it render
  the VM's manager config with `nics: [{id, mac, host: <NIC Pod IP>, port:
  7700}]` and start the VM Pod. `status.nics` lists each NIC's `name`, `mac`,
  `podName`, `ip` and `ready`.
- **Replacing a NIC Pod.** NIC Pods hold no VM state, so a missing or Failed
  NIC Pod is recreated. A replaced NIC Pod normally gets a **new IP**. The new
  IP changes the VM's config, which triggers the operator's normal controlled
  VM Pod replacement (TPM and firmware identity are kept). The guest then gets
  the new address from passt's DHCP. A container restart inside the same Pod
  keeps the IP; QEMU reconnects and the guest does not restart.
- **While the VM is off.** NIC Pods stay up when `running: false`, so a
  restart keeps the IPs.
- **Removal.** Removing a NIC deletes its Pod and policy. Deleting the VM
  garbage-collects both through owner references.

The planned fix for the IP change is a delegated IPAM range per VM NIC plus an
FRR-announced /32, so an address follows the guest across NIC Pod replacement
and migration. It is not implemented.

## Limits

- passt is a userspace stack: every packet is copied through the NIC Pod's
  sockets, and throughput is far below a kernel bridge or tap.
- Only TCP, UDP and ICMP/ICMPv6 echo cross passt. Other IP protocols (GRE,
  SCTP, ESP, multicast routing, raw L2) do not.
- Inbound ports exclude 7700 and the ephemeral range. Ports below 1024 are
  forwarded only where the node lets an unprivileged Pod bind them.
- At most 4 NICs per VM. Each NIC Pod costs about 320 MiB of memory (see
  [Resource cost](#resource-cost)).
