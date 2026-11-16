#!/usr/bin/python3
"""Exercise the root -> operator -> SSH boundary using disposable keys/sshd."""
import importlib.util
import json
import os
from pathlib import Path
import pwd
import shutil
import socket
import subprocess
import sys
import tempfile
import time


def inside(program, operator):
    spec=importlib.util.spec_from_file_location("deploy",program)
    module=importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    account=pwd.getpwnam(operator)
    with tempfile.TemporaryDirectory(prefix="qrab-ssh-fixture-",dir="/run") as work:
        root=Path(work)
        root.chmod(0o755)
        for name in ("host","client"):
            subprocess.run(["ssh-keygen","-q","-t","ed25519","-N","","-f",str(root/name)],check=True)
        os.chown(root/"client",account.pw_uid,account.pw_gid)
        (root/"client").chmod(0o600)
        shutil.copyfile(root/"client.pub",root/"authorized_keys")
        (root/"authorized_keys").chmod(0o644)
        with socket.socket() as free:
            free.bind(("127.0.0.1",0))
            port=free.getsockname()[1]
        public=(root/"host.pub").read_text().strip()
        (root/"known_hosts").write_text(f"127.0.0.1 {public}\n[127.0.0.1]:{port} {public}\n")
        (root/"known_hosts").chmod(0o644)
        config=f"""ListenAddress 127.0.0.1
Port {port}
HostKey {root}/host
PidFile {root}/sshd.pid
AuthorizedKeysFile {root}/authorized_keys
AllowUsers {operator}
PasswordAuthentication no
KbdInteractiveAuthentication no
PubkeyAuthentication yes
UsePAM yes
StrictModes yes
PrintMotd no
"""
        (root/"sshd.conf").write_text(config)
        # This wrapper supplies only the fixture's public configuration; no
        # actual operator SSH configuration, private key or host key is copied.
        real_ssh=shutil.which("ssh",path=module.SEARCH_PATH)
        wrapper=root/"fixture-ssh"
        wrapper.write_text(f'#!/bin/sh\nexec {real_ssh} -F /dev/null -p {port} -i {root}/client '
                           f'-o IdentitiesOnly=yes -o IdentityAgent=none '
                           f'-o UserKnownHostsFile={root}/known_hosts "$@"\n')
        wrapper.chmod(0o755)
        real_which=module.shutil.which
        module.shutil.which=lambda name,path=None: str(wrapper) if name=="ssh" else real_which(name,path=path)
        # ssh_command's remote user is the existing node automation account.
        assert operator=="automation"
        host={"ip":"127.0.0.1","aux":"127.0.0.1"}
        log=(root/"sshd.log").open("wb")
        daemon=subprocess.Popen([real_which("sshd",path=module.SEARCH_PATH),"-D","-e","-f",
                                 str(root/"sshd.conf")],stdout=log,stderr=subprocess.STDOUT)
        try:
            for _ in range(50):
                if daemon.poll() is not None:
                    raise RuntimeError("fixture sshd exited")
                try:
                    with socket.create_connection(("127.0.0.1",port),timeout=.1):
                        break
                except OSError:
                    time.sleep(.1)
            else:raise RuntimeError("fixture sshd did not listen")
            old_cwd=os.getcwd()
            inaccessible=root/"root-only-cwd"
            inaccessible.mkdir(mode=0o700)
            os.chdir(inaccessible)
            try:
                payload=b"synthetic-stdin-not-argv-" + os.urandom(8192)
                result=module.ssh_run(operator,host,"cat",data=payload,timeout=15)
                assert result.stdout==payload
                command="sudo -n /usr/bin/python3 -I -c 'import os;print(os.geteuid())'"
                result=module.ssh_run(operator,host,command,timeout=15)
                assert result.stdout.strip()==b"0"
                os.environ["SUDO_USER"]="unrelated-root-caller"
                assert module.arguments(["deploy"]).operator_user=="bob"
            finally:os.chdir(old_cwd)
            print(json.dumps({"runuserSsh":True,"operatorHomeFromRootOnlyCwd":True,
                              "stdinPreserved":True,"remoteSudoVerified":True,
                              "sudoCallerCannotSelectIdentity":True}))
        finally:
            daemon.terminate()
            try:daemon.wait(timeout=3)
            except subprocess.TimeoutExpired:daemon.kill();daemon.wait()
            log.close()


def main():
    if len(sys.argv)==4 and sys.argv[1]=="--inside":
        inside(Path(sys.argv[2]),sys.argv[3])
        return
    if os.geteuid()!=0 or len(sys.argv)!=3:
        raise SystemExit("usage: sudo python3 check-ssh.py DEPLOY_ROOT_PY automation")
    namespace="qrabssh"+str(os.getpid())
    subprocess.run(["ip","netns","add",namespace],check=True)
    try:
        subprocess.run(["ip","-n",namespace,"link","set","lo","up"],check=True)
        subprocess.run(["ip","netns","exec",namespace,sys.executable,str(Path(__file__).resolve()),
                        "--inside",str(Path(sys.argv[1]).resolve()),sys.argv[2]],check=True)
    finally:subprocess.run(["ip","netns","del",namespace],check=True)


if __name__=="__main__":
    main()
