#!/usr/bin/python3
"""Test the gateway-on-bridge MKA layout and all directed staging probes."""
import importlib.util
import json
import os
from pathlib import Path
import secrets
import subprocess
import sys
import tempfile
import time


def load_module(path):
    spec=importlib.util.spec_from_file_location("deploy",path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    original=module.secure_state
    # These private namespaces have no Cilium daemon; live TCX attachment is
    # covered separately. Keep all kernel MACsec checks.
    module.secure_state=lambda state,**_: original(state,require_cilium=False)
    return module


def main():
    if len(sys.argv)==5 and sys.argv[1]=="--worker":
        module=load_module(sys.argv[2]);state=Path(sys.argv[3]);action=sys.argv[4]
        if action=="init":module.init_upper(state)
        elif action=="server":module.probe_server(state)
        elif action=="peers":print(json.dumps(module.peer_checks(state)))
        elif action=="capture":
            module.CAPTURE_SECONDS=1
            print(json.dumps(module.capture_clear(state)))
        return
    if os.geteuid()!=0 or len(sys.argv)!=3:
        raise SystemExit("usage: sudo python3 check-topology.py DEPLOY_ROOT_PY EXPORTED_TOOLS")
    program=Path(sys.argv[1]).resolve();tools=Path(sys.argv[2]).resolve()
    module=load_module(program)
    prefix="qrabtop"+str(os.getpid())
    namespaces={role:prefix+str(index) for index,role in enumerate(module.HOSTS)}
    owned=[];processes=[];outputs=[]
    def run(*argv,check=True):
        return subprocess.run(list(map(str,argv)),capture_output=True,text=True,check=check,timeout=25)
    def ip(role,*argv):
        return run("ip","-n",namespaces[role],*argv)
    def command(role,*argv,check=True):
        return run("ip","netns","exec",namespaces[role],*argv,check=check)
    with tempfile.TemporaryDirectory(prefix="qrab-topology-",dir="/run") as work:
        root=Path(work)
        try:
            for role,namespace in namespaces.items():
                run("ip","netns","add",namespace);owned.append(namespace)
                ip(role,"link","set","lo","up")
            ip("gateway","link","add","br-kube","type","bridge")
            ip("gateway","link","set","br-kube","address","02:aa:00:00:00:01")
            ip("gateway","link","set","br-kube","type","bridge","group_fwd_mask","8")
            ip("gateway","link","set","br-kube","up")
            for index,role in enumerate(("kube01","kube02","kube03"),1):
                ip("gateway","link","add",f"p{index}","type","veth","peer","name",f"e{index}")
                ip("gateway","link","set",f"e{index}","netns",namespaces[role])
                ip(role,"link","set",f"e{index}","name","eth0")
                ip(role,"link","set","eth0","address",f"52:54:00:00:00:0{index}")
                ip("gateway","link","set",f"p{index}","master","br-kube")
                ip("gateway","link","set",f"p{index}","mtu","1532","up")
                ip(role,"link","set","eth0","up")
            cak,ckn=secrets.token_hex(16),secrets.token_hex(16)
            for role,host in module.HOSTS.items():
                state=root/role;state.mkdir()
                (state/"tools").symlink_to(tools)
                profile=dict(host,role=role,cak=cak,ckn=ckn,
                             lower="br-kube" if role=="gateway" else "eth0")
                module.save(state/"profile.json",profile);module.phase(state,"staging")
                conf=module.mka_config(profile).replace("/run/qrab-macsec",str(state/"ctrl"))
                module.atomic(state/"mka.conf",conf.encode())
                ip(role,"address","add",host["ip"]+"/24","dev",profile["lower"])
                command(role,"sysctl","-q","-w",f"net.ipv6.conf.{profile['lower']}.disable_ipv6=1")
            def launch(role,name,*argv):
                output=(root/(role+"-"+name+".log")).open("wb");outputs.append(output)
                process=subprocess.Popen(["ip","netns","exec",namespaces[role],*map(str,argv)],
                                         stdout=output,stderr=subprocess.STDOUT)
                processes.append(process);return process
            for role in ("kube01","kube02","kube03","gateway"):
                state=root/role;profile=module.load(state/"profile.json")
                launch(role,"wpa",tools/"run-wpa","-Dmacsec_linux","-i"+profile["lower"],
                       "-c",state/"mka.conf")
            for role in module.HOSTS:
                state=root/role
                command(role,sys.executable,Path(__file__).resolve(),"--worker",program,state,"init")
                launch(role,"server",sys.executable,Path(__file__).resolve(),"--worker",program,state,"server")
            time.sleep(2)
            baseline={}
            start=time.monotonic()
            for attempt in range(12):
                for role in module.HOSTS:
                    result=command(role,sys.executable,Path(__file__).resolve(),"--worker",program,
                                   root/role,"peers",check=False)
                    baseline[role]={"exit":result.returncode,"stderr":result.stderr[-3000:]}
                if all(v["exit"]==0 for v in baseline.values()):
                    break
                time.sleep(1)
            print(json.dumps({"bridgeMasterBaseline":baseline}),flush=True)
            print(json.dumps({"convergenceSeconds":time.monotonic()-start}),flush=True)
            assert all(v["exit"]==0 for v in baseline.values()),"gateway-on-bridge baseline failed"
            # The actual gateway captures on the bridge master, not a veth
            # endpoint. Exercise that placement while all encrypted paths run.
            capture=launch("gateway","capture",sys.executable,Path(__file__).resolve(),
                           "--worker",program,root/"gateway","capture")
            time.sleep(.2)
            for role in module.HOSTS:
                command(role,sys.executable,Path(__file__).resolve(),"--worker",program,
                        root/role,"peers")
            assert capture.wait(timeout=5)==0,(root/"gateway-capture.log").read_text()
            evidence=module.load(root/"gateway"/"capture.json")
            assert evidence["dropped"]==0 and not any(evidence["clear"].values())
            print(json.dumps({"bridgeMasterCapture":evidence}),flush=True)
            command("gateway","iptables-legacy","-P","INPUT","DROP")
            command("gateway","iptables-legacy","-A","INPUT","-m","conntrack","--ctstate",
                    "ESTABLISHED,RELATED","-j","ACCEPT")
            outward=command("gateway",sys.executable,Path(__file__).resolve(),"--worker",program,
                            root/"gateway","peers",check=False)
            inward=command("kube01",sys.executable,Path(__file__).resolve(),"--worker",program,
                           root/"kube01","peers",check=False)
            assert outward.returncode==0,"gateway-initiated probes should pass established replies"
            assert inward.returncode!=0,"default-drop gateway must reject new inbound probes"
            error=module.load(root/"kube01"/"peer-error.json")
            assert error["target"]=="gateway" and error["step"]=="connect"
            assert "-P INPUT DROP" in command("gateway","iptables-legacy","-S","INPUT").stdout
            print(json.dumps({"defaultDropDiagnosedWithoutPolicyChange":True,
                              "peerError":error,"all12PathsPassedAfterMkaConvergence":True}),flush=True)
        finally:
            for process in reversed(processes):
                if process.poll() is None:process.terminate()
            for process in reversed(processes):
                try:process.wait(timeout=3)
                except subprocess.TimeoutExpired:process.kill();process.wait()
            for output in outputs:output.close()
            for namespace in reversed(owned):run("ip","netns","del",namespace,check=False)


if __name__=="__main__":
    main()
