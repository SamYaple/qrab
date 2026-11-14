#!/usr/bin/env bash
# Experiment A-USB: usb-redir on a plain unix chardev (no SPICE), as candidate A
# would add it, on the experiment image (Containerfile.qemu-spice; the pinned qrab
# image has no usb-redir because it is built without libusbredirparser).
#   run-usbredir.sh <workdir> [image]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=$(mkdir -p "$1" && cd "$1" && pwd)
image=${2:-localhost/qrab-experiment/qemu-spice:11.1.3}
rm -f "$work/usbredir0.sock"
podman run -d --name qrab-exp-usbredir --network=none --user 0 -v "$work:/run/x:z" "$image" \
  -machine q35 -m 128 -accel tcg -nodefaults -display none \
  -device qemu-xhci,id=usb -device usb-tablet,bus=usb.0 \
  -chardev socket,id=usbredir0,path=/run/x/usbredir0.sock,server=on,wait=off \
  -device usb-redir,chardev=usbredir0,bus=usb.0 >/dev/null
finish() {
  podman inspect qrab-exp-usbredir --format '## qemu container: {{.State.Status}} exit={{.State.ExitCode}}'
  podman logs qrab-exp-usbredir 2>&1 | tail -4 || true
  podman rm -f qrab-exp-usbredir >/dev/null
}
trap finish EXIT
for _ in $(seq 300); do [[ -S $work/usbredir0.sock ]] && break; sleep 0.1; done || true
python3 -I "$here/usbredir_hello_probe.py" "$work/usbredir0.sock"
