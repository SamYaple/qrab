#!/usr/bin/python3
"""Check a real Cilium host-to-auxiliary path against an isolated MKA peer."""
import importlib.util
import json
import os
from pathlib import Path
import secrets
import shutil
import socket
import subprocess
import sys
import tempfile
import time


def main():
    if os.geteuid()!=0 or len(sys.argv)!=4:
        raise SystemExit("usage: sudo python3 check-cilium-peer.py DEPLOY_ROOT_PY TOOLS RELEASE_JSON")
    program,tools,release=map(lambda s:Path(s).resolve(),sys.argv[1:])
    spec=importlib.util.spec_from_file_location("deploy",program)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    assert module.link("macsec0") is None and module.link("qrabpeer0") is None
    assert not any(v.get("dst")=="192.0.2.0/24" for v in module.jrun(["ip","-j","-4","route"]))
    namespace="qrabcp"+str(os.getpid())
    processes=[];outputs=[];owned=False;lower=False
    with tempfile.TemporaryDirectory(prefix="qrab-cilium-peer-",dir="/run") as work:
        root=Path(work);cak=secrets.token_hex(16);ckn=secrets.token_hex(16)
        try:
            module.run(["ip","netns","add",namespace]);owned=True
            module.run(["ip","link","add","qrabpeer0","type","veth","peer","name","qrabpeer1"]);lower=True
            module.run(["ip","link","set","qrabpeer1","netns",namespace])
            module.run(["ip","-n",namespace,"link","set","qrabpeer1","name","eth0"])
            module.run(["ip","link","set","qrabpeer0","up"])
            module.run(["ip","-n",namespace,"link","set","eth0","up"])
            module.run(["ip","-n",namespace,"link","set","lo","up"])
            for role,dev in (("kube01","qrabpeer0"),("gateway","eth0")):
                state=root/role;state.mkdir()
                profile=dict(module.HOSTS[role],role=role,lower=dev,cak=cak,ckn=ckn)
                module.save(state/"profile.json",profile);module.phase(state,"staging")
                shutil.copyfile(release,state/"release.json")
                config=module.mka_config(profile).replace("/run/qrab-macsec",str(state/"ctrl"))
                module.atomic(state/"mka.conf",config.encode())
                output=(state/"wpa.log").open("wb");outputs.append(output)
                cmd=[str(tools/"run-wpa"),"-Dmacsec_linux","-i"+dev,"-c",str(state/"mka.conf")]
                if role=="gateway":cmd=["ip","netns","exec",namespace,*cmd]
                processes.append(subprocess.Popen(cmd,stdout=output,stderr=subprocess.STDOUT))
            module.init_upper(root/"kube01")
            gateway_init="""import importlib.util,pathlib,sys
s=importlib.util.spec_from_file_location('deploy',sys.argv[1]);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
m.init_upper(pathlib.Path(sys.argv[2]))
"""
            module.run(["ip","netns","exec",namespace,sys.executable,"-c",gateway_init,
                        str(program),str(root/"gateway")])
            module.run(["ip","-n",namespace,"address","add","10.203.0.1/24","dev","macsec0"])
            server="""import socket,json,hashlib
s=socket.socket();s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
s.setsockopt(socket.SOL_SOCKET,socket.SO_BINDTODEVICE,b'macsec0\\0')
s.bind(('192.0.2.1',41377));s.listen()
while True:
 c,p=s.accept();c.settimeout(5);d=b''
 while len(d)<32768:
  x=c.recv(32768-len(d))
  if not x:break
  d+=x
 c.sendall(json.dumps({'peer':p[0],'bytes':len(d),'sha256':hashlib.sha256(d).hexdigest()}).encode());c.close()
"""
            output=(root/"server.log").open("wb");outputs.append(output)
            processes.append(subprocess.Popen(["ip","netns","exec",namespace,sys.executable,"-c",server],
                                               stdout=output,stderr=subprocess.STDOUT))
            for _ in range(45):
                try:
                    module.secure_state(root/"kube01")
                    with socket.socket() as client:
                        client.settimeout(2)
                        client.setsockopt(socket.SOL_SOCKET,socket.SO_BINDTODEVICE,b"macsec0\0")
                        client.bind(("192.0.2.11",0));client.connect(("192.0.2.1",41377))
                        data=secrets.token_bytes(32768);client.sendall(data);client.shutdown(socket.SHUT_WR)
                        answer=b""
                        while True:
                            part=client.recv(4096)
                            if not part:break
                            answer+=part
                    value=json.loads(answer)
                    assert value=={"peer":"192.0.2.11","bytes":len(data),"sha256":module.digest(data)},value
                    print(json.dumps({"realCiliumAuxSourcePreserved":True,"tcpBytes":len(data),
                                      "upperKernelHooksVerified":True}),flush=True)
                    break
                except (OSError,module.DeployError):
                    time.sleep(.5)
            else:raise RuntimeError("real Cilium auxiliary path did not become ready")
        finally:
            for process in reversed(processes):
                if process.poll() is None:process.terminate()
            for process in reversed(processes):
                try:process.wait(timeout=3)
                except subprocess.TimeoutExpired:process.kill();process.wait()
            for output in outputs:output.close()
            if lower:
                if module.link("macsec0"):module.run(["ip","link","del","macsec0"])
                if module.link("qrabpeer0"):module.run(["ip","link","del","qrabpeer0"])
            if owned:module.run(["ip","netns","del",namespace])


if __name__=="__main__":
    main()
