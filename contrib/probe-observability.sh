#!/usr/bin/env bash
# Read-only workload TLS oracle for the current x86_64 Linux cluster.
# The helper runs inside the requested container's attestation boundary.
# No credentials leave that container; no runtime volume is used for staging.
set -euo pipefail
cd "$(dirname "$0")/.."
[[ $# == 7 ]] || { echo 'usage: probe-observability.sh POD-NAMESPACE POD CONTAINER metrics|history VM-NAMESPACE VM-UID POD-IP|POD-NAME' >&2; exit 2; }
pod_namespace=$1
probe_pod=$2
probe_container=$3
probe_kind=$4
vm_namespace=$5
vm_uid=$6
probe_destination=$7
[[ $probe_kind == metrics || $probe_kind == history ]] || exit 2
architecture=$(kubectl -n "$pod_namespace" exec "$probe_pod" -c "$probe_container" -- uname -m)
[[ $architecture == x86_64 ]] || { echo 'The in-memory wrapper requires x86_64 Linux.' >&2; exit 1; }
cargo build --locked -p qrab-observability --example workload_probe
probe_target=${CARGO_TARGET_DIR:-"$PWD/target"}
probe_binary="$probe_target/debug/examples/workload_probe"
[[ -x $probe_binary ]] || { echo 'built probe executable missing' >&2; exit 1; }
staged_probe=$(mktemp)
trap 'rm -f "$staged_probe"' EXIT
strip --strip-debug -o "$staged_probe" "$probe_binary"
[[ $(wc -c < "$staged_probe") -le 33554432 ]] || { echo 'probe exceeds 32MiB' >&2; exit 1; }
# Perl is present in the pinned Debian runtime. Linux memfd_create (319 on
# x86_64) gives this one diagnostic process an anonymous executable file.
# It vanishes on exit. Normal seccomp/kernel restrictions remain in force;
# any refusal fails the diagnostic, with no disk or plaintext fallback.
kubectl -n "$pod_namespace" exec -i "$probe_pod" -c "$probe_container" -- perl -e '
    $name="qrab-observability-probe";
    $fd=syscall(319,$name,0);
    die "memfd unavailable: $!\n" if $fd<0;
    $^F=$fd;
    open($out, ">&=$fd") or die "memfd open: $!\n";
    binmode(STDIN); binmode($out);
    $total=0;
    while (($count=read(STDIN,$buffer,65536))>0) {
        $total+=$count;
        die "probe executable exceeds 32MiB\n" if $total>32*1024*1024;
        $offset=0;
        while ($offset<$count) {
            $written=syswrite($out,$buffer,$count-$offset,$offset);
            die "memfd write: $!\n" unless defined($written) && $written>0;
            $offset+=$written;
        }
    }
    die "probe read: $!\n" unless defined($count);
    exec {"/proc/self/fd/$fd"} "workload_probe", @ARGV;
    die "probe exec: $!\n";
' -- "$probe_kind" "$vm_namespace" "$vm_uid" "$probe_destination" < "$staged_probe"
