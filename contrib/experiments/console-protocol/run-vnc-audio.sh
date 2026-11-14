#!/usr/bin/env bash
# Experiment A: VNC audio on the PINNED qrab QEMU image, qrab-style arguments.
#   run-vnc-audio.sh <workdir> [image]
# Three runs: (1) today's qrab console args (no audiodev), (2) audio added the way
# work/admin-audio-playback proposes, (3) same VM, client that did NOT negotiate -259
# sends audio enable. Needs rootless podman; no network, no KVM (TCG).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=$(mkdir -p "$1" && cd "$1" && pwd)
image=${2:-zot.yapplesauce.com/qrab/qemu@sha256:771ce49b1185dc38986a67e7cafbd3de4fa5100f80661e681f4f3add012c66b5}
python3 -I "$here/beep-disk.py" "$work/beep.img"

run() {  # name, extra machine opts, extra args...
  local name=$1 machine=$2; shift 2
  rm -f "$work/vnc.sock"
  podman run -d --rm --name "qrab-exp-$name" --network=none --user 0 -v "$work:/run/x:z" "$image" \
    -machine "q35$machine" -m 128 -accel tcg -nodefaults -display none \
    -device VGA,vgamem_mb=16 -device qemu-xhci,id=usb -device usb-tablet,bus=usb.0 \
    -drive file=/run/x/beep.img,format=raw,if=ide "$@" >/dev/null
  for _ in $(seq 300); do [[ -S $work/vnc.sock ]] && break; sleep 0.1; done || true
  sleep 2  # let SeaBIOS boot the MBR and start the tone
}
stop() { podman rm -f "qrab-exp-$1" >/dev/null; }

echo '## 1. current qrab args (no -audiodev), client advertises -259'
run today '' -vnc unix:/run/x/vnc.sock,share=force-shared
python3 -I "$here/vnc_audio_probe.py" "$work/vnc.sock" --seconds 2 || true
stop today

echo '## 2. proposed admin-bucket audio args, client advertises -259'
run audio ',pcspk-audiodev=snd0' -audiodev none,id=snd0 -device intel-hda,id=hda -device hda-output,audiodev=snd0 \
  -vnc unix:/run/x/vnc.sock,share=force-shared,audiodev=snd0
python3 -I "$here/vnc_audio_probe.py" "$work/vnc.sock" --seconds 3
echo '## 3. same VM, client sends audio enable without negotiating -259'
python3 -I "$here/vnc_audio_probe.py" "$work/vnc.sock" --seconds 2 --no-audio-encoding || true
echo '## 4. same VM, a fresh negotiating client after 3 (per-connection isolation)'
python3 -I "$here/vnc_audio_probe.py" "$work/vnc.sock" --seconds 1
stop audio
