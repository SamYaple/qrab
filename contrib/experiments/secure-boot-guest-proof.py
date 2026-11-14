#!/usr/bin/env python3
"""Disposable signed Linux/guest-agent Secure Boot proof using explicit artifacts.

Never opens host efivarfs or any existing VM identity. Copies all boot disks
and VARS into a new output directory, creates throwaway signing keys, and boots
three separate VMs: signed/enabled, unsigned/enabled, unsigned/disabled.
Use extracted artifacts from an immutable published qrab image, not host QEMU.
"""
import argparse
import datetime
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import stat
import struct
import subprocess
import time
import uuid


def command(argv, env=None, cwd=None, data=None):
    result = subprocess.run([str(v) for v in argv], env=env, cwd=cwd, input=data,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True)
    return result.stdout


def sha(path):
    with Path(path).open("rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()


class QgaError(RuntimeError):
    pass


class Qga:
    def __init__(self, path):
        self.sock = socket.socket(socket.AF_UNIX)
        self.sock.settimeout(3)
        self.sock.connect(str(path))
        self.buffer = b""
        nonce = time.time_ns() & ((1 << 62) - 1)
        reply = self.request("guest-sync-delimited", {"id": nonce}, prefix=b"\xff")
        if reply != nonce:
            raise QgaError("guest sync nonce mismatch")

    def request(self, name, args=None, prefix=b""):
        frame = {"execute": name}
        if args is not None:
            frame["arguments"] = args
        self.sock.sendall(prefix + json.dumps(frame).encode() + b"\n")
        while True:
            if prefix and b"\xff" in self.buffer:
                self.buffer = self.buffer.split(b"\xff", 1)[1]
                prefix = b""
            if not prefix and b"\n" in self.buffer:
                line, self.buffer = self.buffer.split(b"\n", 1)
                reply = json.loads(line)
                if "error" in reply:
                    raise QgaError(reply["error"].get("class", "Error") + ": " + reply["error"].get("desc", ""))
                return reply["return"]
            chunk = self.sock.recv(65536)
            if not chunk:
                raise QgaError("guest agent disconnected")
            self.buffer += chunk
            if len(self.buffer) > 1024 * 1024:
                raise QgaError("guest reply exceeds 1MiB")

    def close(self):
        self.sock.close()


def unpack_newc(archive, root):
    """Extract our bounded image archive without host mknod or privileges."""
    offset = entries = 0
    while offset < len(archive):
        header = archive[offset:offset + 110]
        if len(header) != 110 or header[:6] != b"070701":
            raise RuntimeError("unsupported initramfs archive")
        fields = [int(header[6 + i * 8:14 + i * 8], 16) for i in range(13)]
        mode, size, namesize = fields[1], fields[6], fields[11]
        offset += 110
        if namesize == 0 or namesize > 4096 or size > 256 * 1024 * 1024:
            raise RuntimeError("initramfs entry exceeds bound")
        name_bytes = archive[offset:offset + namesize]
        if len(name_bytes) != namesize or name_bytes[-1:] != b"\0":
            raise RuntimeError("truncated initramfs name")
        name = name_bytes[:-1].decode()
        offset = (offset + namesize + 3) & ~3
        data = archive[offset:offset + size]
        if len(data) != size:
            raise RuntimeError("truncated initramfs data")
        offset = (offset + size + 3) & ~3
        if name == "TRAILER!!!":
            return
        entries += 1
        if entries > 65536:
            raise RuntimeError("too many initramfs entries")
        relative = Path(name)
        if relative.is_absolute() or ".." in relative.parts:
            raise RuntimeError("unsafe initramfs path")
        path = root / relative
        if name == ".":
            continue
        if not path.parent.resolve().is_relative_to(root.resolve()):
            raise RuntimeError("initramfs path traverses a symlink")
        path.parent.mkdir(parents=True, exist_ok=True)
        if stat.S_ISDIR(mode):
            path.mkdir(exist_ok=True)
            path.chmod(mode & 0o777)
        elif stat.S_ISREG(mode):
            if path.is_symlink():
                path.unlink()
            path.write_bytes(data)
            path.chmod(mode & 0o777)
        elif stat.S_ISLNK(mode):
            path.symlink_to(data.decode())
        # Device nodes are intentionally omitted. make-initramfs.py encodes
        # virtual console/null node metadata without creating host devices.
    raise RuntimeError("initramfs trailer is missing")


def prepare(args):
    out = args.output
    command(["openssl", "req", "-new", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
             "-subj", "/CN=qrab disposable signed Linux proof/", "-keyout", out / "key.pem", "-out", out / "cert.pem"])
    (out / "key.pem").chmod(0o600)
    unsigned = args.guest / "BOOTX64.EFI"
    if args.rebuild_initrd:
        root = out / "guest-root"
        root.mkdir()
        with gzip.open(args.guest / "initramfs.gz", "rb") as f:
            archive = f.read(256 * 1024 * 1024 + 1)
        if len(archive) > 256 * 1024 * 1024:
            raise RuntimeError("initrd expansion exceeds bound")
        unpack_newc(archive, root)
        probe_source = out / "efi-metadata-probe.rs"
        probe_source.write_text(r'''
use std::{fs::File,os::fd::AsRawFd};
unsafe extern "C" { fn ioctl(fd:i32,request:u64,arg:*mut u64)->i32; }
fn main(){
 let path="/sys/firmware/efi/efivars/db-d719b2cb-3d3a-4596-a3bc-dad00e67656f";
 match File::open(path){
  Ok(file)=>{let mut flags=0u64;
   // FS_IOC_GETFLAGS is read-only; this diagnostic never calls SETFLAGS.
   let result=unsafe{ioctl(file.as_raw_fd(),0x80086601,&mut flags)};
   println!("QRAB_DB_FILE_FLAGS result={result} flags={flags:#x} immutable={}",flags&0x10!=0);
  },Err(error)=>println!("QRAB_DB_FILE_FLAGS error={error}"),
 }
 if let Ok(status)=std::fs::read_to_string("/proc/self/status"){
  for line in status.lines().filter(|l|l.starts_with("CapEff:")){println!("QRAB_SB_{line}");}
 }
}
''')
        command(["rustc", "--edition=2021", "--target=x86_64-unknown-linux-musl", "-O", probe_source,
                 "-o", root / "usr/bin/qrab-efi-metadata-probe"])
        init = (args.source_root / "contrib/images/guest-init").read_text()
        # Diagnostic reads occur only inside the disposable guest. They expose
        # metadata, never trust payloads or private keys, and never clear flags.
        init = init.replace("# The guest agent finds", """echo QRAB_SB_DIAGNOSTICS
/usr/bin/qrab-efi-metadata-probe
mount -t securityfs securityfs /sys/kernel/security 2>/dev/null || true
cat /sys/kernel/security/lockdown 2>/dev/null || true
# The guest agent finds""")
        (root / "init").write_text(init)
        (root / "init").chmod(0o755)
        if args.agent is None:
            raise RuntimeError("--rebuild-initrd requires --agent")
        shutil.copyfile(args.agent, root / "usr/sbin/qrab-guest-agent")
        (root / "usr/sbin/qrab-guest-agent").chmod(0o755)
        if args.efivarfs_module:
            target = root / "lib/qrab-modules/efivarfs.ko"
            shutil.copyfile(args.efivarfs_module, target)
            with (root / "etc/qrab-modules").open("a") as f:
                f.write("/lib/qrab-modules/efivarfs.ko\n")
        command(["python3", args.source_root / "contrib/images/make-initramfs.py", root, out / "initramfs.gz"])
        unsigned = out / "unsigned.efi"
        command(["ukify", "build", "--linux=" + str(args.guest / "vmlinuz"),
                 "--initrd=" + str(out / "initramfs.gz"), "--cmdline=@" + str(args.guest / "cmdline"),
                 "--os-release=@" + str(args.guest / "os-release"), "--output=" + str(unsigned)])
    command(["sbsign", "--key", out / "key.pem", "--cert", out / "cert.pem", "--output", out / "signed.efi", unsigned])
    command(["sbverify", "--cert", out / "cert.pem", out / "signed.efi"])
    owner = "17b5b3ca-1170-4b35-bf84-91427e1fb028"
    command([args.virt_fw_vars, "-i", args.vars, "-o", out / "enabled.fd", "--set-pk", owner, out / "cert.pem",
             "--add-kek", owner, out / "cert.pem", "--add-db", owner, out / "cert.pem", "--secure-boot"])
    command([args.virt_fw_vars, "-i", out / "enabled.fd", "-o", out / "disabled.fd", "--set-false", "SecureBootEnable"])
    return unsigned


def auth_append(args, guest):
    """A signed append is authorized by firmware, never by local signature checks."""
    timestamp = datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0)
    stamp = struct.pack("<HBBBBBxIhBB", timestamp.year, timestamp.month, timestamp.day,
                        timestamp.hour, timestamp.minute, timestamp.second, 0, 0, 0, 0)
    owner = uuid.UUID("17b5b3ca-1170-4b35-bf84-91427e1fb028").bytes_le
    marker = hashlib.sha256(b"qrab signed Linux authenticated db append proof").digest()
    data = uuid.UUID("c1c41626-504c-4092-aca9-41f936934328").bytes_le + struct.pack("<III", 76, 0, 48) + owner + marker
    signed = "db".encode("utf-16-le") + uuid.UUID("d719b2cb-3d3a-4596-a3bc-dad00e67656f").bytes_le + struct.pack("<I", 0x67) + stamp + data
    (args.output / "append.sign-input").write_bytes(signed)
    command(["openssl", "cms", "-sign", "-binary", "-noattr", "-md", "sha256", "-signer", args.output / "cert.pem",
             "-inkey", args.output / "key.pem", "-in", args.output / "append.sign-input", "-outform", "DER", "-out", args.output / "append.pkcs7"])
    cms = (args.output / "append.pkcs7").read_bytes()
    auth = stamp + struct.pack("<IHH", 24 + len(cms), 0x0200, 0x0EF1) + uuid.UUID("4aafd29d-68df-49ee-8aa9-347d375665a7").bytes_le + cms + data
    try:
        returned = guest.request("qrab-secure-boot-update", {"database-id": "db", "payload-hex": auth.hex(), "append": True})
        db = next(v for v in returned["databases"] if v["id"] == "db")
        present = any(v["data-hex"] == marker.hex() for v in db["signatures"])
        return {"authorized": present, "database_present": db["present"], "database_error": db["error"]}
    except QgaError as error:
        return {"authorized": False, "denial": str(error)}


def boot(args, label, firmware, loader, expect_boot, secure):
    case = args.output / label
    case.mkdir()
    shutil.copyfile(firmware, case / "vars.fd")
    shutil.copyfile(args.guest / "boot.img", case / "boot.img")
    command(["mcopy", "-o", "-i", str(case / "boot.img") + "@@1048576", loader, "::/EFI/BOOT/BOOTX64.EFI"])
    argv = [args.qemu, "-L", args.qemu_data, "-machine", "q35,smm=on", "-accel", args.accelerator,
            "-m", "512M", "-smp", "2", "-display", "none", "-nodefaults", "-no-reboot", "-nic", "none",
            "-global", "driver=cfi.pflash01,property=secure,value=on",
            "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={args.code}",
            "-drive", f"if=pflash,unit=1,format=raw,file={case / 'vars.fd'}",
            "-drive", f"if=none,id=boot,format=raw,readonly=on,file={case / 'boot.img'}",
            "-device", "virtio-blk-pci,drive=boot,bootindex=1", "-device", "virtio-serial-pci,id=qga-serial",
            "-chardev", f"socket,id=qga,path={case / 'qga.sock'},server=on,wait=off",
            "-device", "virtserialport,bus=qga-serial.0,chardev=qga,name=org.qemu.guest_agent.0",
            "-serial", f"file:{case / 'serial.log'}"]
    env = os.environ.copy()
    if args.library_path:
        env["LD_LIBRARY_PATH"] = args.library_path
    started = time.monotonic()
    high_water = ticks = 0
    result = {"case": label, "expect_boot": expect_boot, "expected_secure_boot": secure, "booted": False}
    with (case / "stderr.log").open("wb") as errors:
        process = subprocess.Popen([str(v) for v in argv], stdout=subprocess.DEVNULL, stderr=errors, env=env)
        try:
            while process.poll() is None and time.monotonic() - started < args.timeout:
                try:
                    stat = Path(f"/proc/{process.pid}/stat").read_text().rsplit(") ", 1)[1].split()
                    ticks = int(stat[11]) + int(stat[12])
                    for line in Path(f"/proc/{process.pid}/status").read_text().splitlines():
                        if line.startswith("VmHWM:"):
                            high_water = max(high_water, int(line.split()[1]))
                except FileNotFoundError:
                    pass
                serial = (case / "serial.log").read_text(errors="replace") if (case / "serial.log").exists() else ""
                result["booted"] = "QRAB_GUEST_BOOTED" in serial
                if "Security Violation" in serial or "Access Denied" in serial:
                    result["firmware_rejected"] = True
                    if not expect_boot:
                        break
                if result["booted"] and expect_boot and (case / "qga.sock").exists():
                    try:
                        guest = Qga(case / "qga.sock")
                        try:
                            result["agent_info"] = guest.request("guest-info")
                            result["firmware"] = guest.request("qrab-firmware-state")
                            result["databases"] = guest.request("qrab-secure-boot-databases")
                            result["boot_options"] = guest.request("qrab-boot-state")
                            if args.authenticated_updates and secure:
                                result["authenticated_update"] = auth_append(args, guest)
                        finally:
                            guest.close()
                        break
                    except (QgaError, OSError, json.JSONDecodeError) as error:
                        result["last_agent_error"] = str(error)
                time.sleep(0.25)
        finally:
            if process.poll() is None:
                process.terminate()
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=5)
    result.update(elapsed_seconds=time.monotonic() - started, cpu_seconds_sampled=ticks / os.sysconf("SC_CLK_TCK"),
                  max_rss_kib_sampled=high_water, stderr_tail=(case / "stderr.log").read_text(errors="replace")[-2000:],
                  serial_tail=(case / "serial.log").read_text(errors="replace")[-5000:])
    result["passed"] = (result["booted"] and result.get("firmware", {}).get("secure-boot") is secure) if expect_boot else (not result["booted"] and result.get("firmware_rejected", False))
    if expect_boot:
        result["passed"] = result["passed"] and result.get("databases", {}).get("efi-runtime-available", False)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["qemu", "qemu-data", "code", "vars", "guest", "virt-fw-vars", "output"]:
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--image-ref", required=True)
    parser.add_argument("--source-root", type=Path)
    parser.add_argument("--agent", type=Path)
    parser.add_argument("--efivarfs-module", type=Path)
    parser.add_argument("--rebuild-initrd", action="store_true")
    parser.add_argument("--library-path")
    parser.add_argument("--accelerator", choices=["tcg", "kvm"], default="tcg")
    parser.add_argument("--timeout", type=float, default=120)
    parser.add_argument("--authenticated-updates", action="store_true")
    parser.add_argument("--case", action="append", choices=["signed-enabled", "unsigned-enabled", "unsigned-disabled"], help="run selected disposable cases; default all")
    args = parser.parse_args()
    if "@sha256:" not in args.image_ref:
        parser.error("--image-ref must record an immutable published source digest")
    if args.rebuild_initrd and not args.source_root:
        parser.error("--rebuild-initrd requires --source-root")
    for key in ["qemu", "qemu_data", "code", "vars", "guest", "virt_fw_vars", "output", "source_root", "agent", "efivarfs_module"]:
        value = getattr(args, key)
        if value is not None:
            setattr(args, key, value.resolve())
    args.output.mkdir(parents=True, exist_ok=False)
    env = os.environ.copy()
    if args.library_path:
        env["LD_LIBRARY_PATH"] = args.library_path
    version = command([args.qemu, "--version"], env=env).decode().splitlines()[0]
    if "11.1." not in version:
        parser.error("this proof requires the pinned QEMU 11.1 release")
    unsigned = prepare(args)
    configurations = [("signed-enabled", args.output / "enabled.fd", args.output / "signed.efi", True, True),
                      ("unsigned-enabled", args.output / "enabled.fd", unsigned, False, True),
                      ("unsigned-disabled", args.output / "disabled.fd", unsigned, True, False)]
    cases = [boot(args, *case) for case in configurations if not args.case or case[0] in args.case]
    boot_passed = all(case["passed"] for case in cases)
    auth_passed = all(case.get("authenticated_update", {}).get("authorized", False)
                      for case in cases if case["expect_boot"] and case["expected_secure_boot"]) if args.authenticated_updates else None
    report = {"image_ref": args.image_ref, "qemu_version": version, "qemu_sha256": sha(args.qemu),
              "code_sha256": sha(args.code), "vars_template_sha256": sha(args.vars), "source_boot_sha256": sha(args.guest / "boot.img"),
              "source_agent_sha256": sha(args.agent) if args.agent else None,
              "source_efivarfs_module_sha256": sha(args.efivarfs_module) if args.efivarfs_module else None, "rebuilt_disposable_initrd": args.rebuild_initrd,
              "cases": cases, "boot_and_query_passed": boot_passed,
              "authenticated_updates_requested": args.authenticated_updates, "authenticated_updates_passed": auth_passed,
              "passed": boot_passed and (auth_passed is not False)}
    (args.output / "result.json").write_text(json.dumps(report, indent=2))
    print(json.dumps(report, indent=2))
    if not report["passed"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
