#!/usr/bin/python3
"""Privileged packet admission/loss regressions in a private network namespace."""
import importlib.util
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time


def load_module(path):
    spec=importlib.util.spec_from_file_location("deploy",path)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    return module


PRODUCER = r"""
import pathlib,socket,sys,time
interface,kind,ready,delay,seconds,interval=sys.argv[1:]
s=socket.socket(socket.AF_PACKET,socket.SOCK_RAW,socket.htons(3));s.bind((interface,0))
frame=b'\xff'*6+s.getsockname()[4]+int(kind,16).to_bytes(2,'big')+b'\x00'*1200
pathlib.Path(ready).touch();time.sleep(float(delay));end=time.monotonic()+float(seconds)
while time.monotonic()<end:
 try:s.send(frame)
 except OSError:pass
 if float(interval):time.sleep(float(interval))
"""


def inside(program,before):
    module=load_module(program);old=load_module(before)
    for name in ("target","noise"):
        module.run(["ip","link","add",name+"0","type","veth","peer","name",name+"1"])
        for suffix in ("0","1"):
            module.run(["sysctl","-q","-w",f"net.ipv6.conf.{name+suffix}.disable_ipv6=1"])
            module.run(["ip","link","set",name+suffix,"up"])
    with tempfile.TemporaryDirectory(prefix="qrab-cap-state-",dir="/run") as work:
        root=Path(work)
        real_socket=socket.socket
        def scenario(candidate,label,*,interface="noise0",kind="0800",slow_setup=False,
                     delayed=False,starve=False,expect=None):
            state=root/label;state.mkdir()
            candidate.save(state/"profile.json",{"lower":"target0"})
            candidate.CAPTURE_SECONDS=.6
            ready=state/"ready"
            processes=[]
            def start():
                if processes:return
                p=subprocess.Popen([sys.executable,"-c",PRODUCER,interface,kind,str(ready),
                                    ".25" if delayed else "0","1.0",
                                    ".005" if delayed and not starve else "0"],
                                   stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
                processes.append(p)
                for _ in range(100):
                    if ready.exists():break
                    time.sleep(.005)
                assert ready.exists(),"producer did not start"
            class Proxy:
                def __init__(self,raw):self.raw=raw;self.slow=True
                def __getattr__(self,name):return getattr(self.raw,name)
                def setsockopt(self,level,option,value):
                    if starve and level==socket.SOL_SOCKET and option==socket.SO_RCVBUF:value=4096
                    return self.raw.setsockopt(level,option,value)
                def recvmsg(self,*args):
                    if starve and self.slow:self.slow=False;time.sleep(.45)
                    return self.raw.recvmsg(*args)
            def factory(*args,**kwargs):
                raw=real_socket(*args,**kwargs)
                if args[0]==socket.AF_PACKET:
                    start()
                    if slow_setup:time.sleep(.2)
                return Proxy(raw)
            candidate.socket.socket=factory
            error=None
            try:
                candidate.capture_clear(state)
            except candidate.DeployError as caught:error=str(caught)
            finally:
                candidate.socket.socket=real_socket
                for process in processes:
                    if process.poll() is None:process.terminate()
                    process.wait(timeout=3)
            evidence=candidate.load(state/"capture.json") if (state/"capture.json").exists() else {}
            if expect is None:
                assert error is None,(label,error,evidence)
                assert evidence["dropped"]==0 and not any(evidence["clear"].values()),evidence
            else:
                assert error and expect in error,(label,error,evidence)
            print(json.dumps({"scenario":label,"error":error,"counters":evidence}),flush=True)
            return evidence,error
        scenario(old,"old-unrelated-startup",slow_setup=True,expect="capture dropped packets")
        scenario(module,"fixed-unrelated-startup",slow_setup=True)
        scenario(module,"fixed-busy-cipher-target",interface="target1",kind="88e5",slow_setup=True)
        scenario(module,"target-ipv4",interface="target1",kind="0800",delayed=True,
                 expect="plaintext IP or VLAN")
        scenario(module,"target-ipv6",interface="target1",kind="86dd",delayed=True,
                 expect="plaintext IP or VLAN")
        scenario(module,"target-vlan",interface="target1",kind="8100",delayed=True,
                 expect="plaintext IP or VLAN")
        evidence,_=scenario(module,"target-real-loss",interface="target1",kind="0800",
                            delayed=True,starve=True,expect="capture on target0 dropped")
        assert evidence["dropped"]>0
        print(json.dumps({"startupRaceReproduced":True,"unrelatedTrafficIsolated":True,
                          "busyCipherFiltered":True,"plaintextStillRefused":True,
                          "actualLossStillRefused":True}),flush=True)


def main():
    if len(sys.argv)==4 and sys.argv[1]=="--inside":
        inside(Path(sys.argv[2]),Path(sys.argv[3]));return
    if os.geteuid()!=0 or len(sys.argv)!=3:
        raise SystemExit("usage: sudo python3 check-capture.py DEPLOY_ROOT_PY PREVIOUS_DEPLOY_ROOT_PY")
    namespace="qrabcap"+str(os.getpid())
    subprocess.run(["ip","netns","add",namespace],check=True)
    try:
        subprocess.run(["ip","-n",namespace,"link","set","lo","up"],check=True)
        subprocess.run(["ip","netns","exec",namespace,sys.executable,str(Path(__file__).resolve()),
                        "--inside",str(Path(sys.argv[1]).resolve()),str(Path(sys.argv[2]).resolve())],check=True)
    finally:subprocess.run(["ip","netns","del",namespace],check=True)


if __name__=="__main__":main()
