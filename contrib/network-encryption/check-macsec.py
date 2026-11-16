#!/usr/bin/env python3
"""Privileged, isolated MACsec/MKA oracle; never touches a production interface."""
import argparse
import importlib.util
import json
import os
import pathlib
import secrets
import shutil
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tools', type=pathlib.Path, required=True)
    parser.add_argument('--deployment-agent', type=pathlib.Path,
                        help='also check retained lower IPs, MTU 1500 and the deployer nft guards')
    args = parser.parse_args()
    if os.geteuid() != 0:
        raise SystemExit('root is required for isolated network namespaces')
    tools = args.tools.resolve()
    assert (tools / 'run-wpa').is_file()
    os.umask(0o077)
    work = pathlib.Path(tempfile.mkdtemp(prefix='qrab-macsec-', dir='/run'))
    prefix = 'qm' + str(os.getpid())
    switch = prefix + 's'
    peers = {name: prefix + name for name in (['a', 'b', 'c', 'd', 'e']
                                             if args.deployment_agent else ['a', 'b', 'c'])}
    owned = []
    processes = []
    outputs = []
    supplicants = {}
    configs = {}
    deploy = None
    deployment_results = {}
    if args.deployment_agent:
        spec = importlib.util.spec_from_file_location('qrab_deploy', args.deployment_agent)
        deploy = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(deploy)

    def command(*argv, check=True):
        return subprocess.run(list(argv), capture_output=True, text=True, check=check, timeout=12)

    def ip(ns, *argv, check=True):
        return command('ip', '-n', ns, *argv, check=check)

    def inside(ns, *argv, check=True):
        return command('ip', 'netns', 'exec', ns, *argv, check=check)

    def launch(ns, name, argv):
        output = (work / (name + '.log')).open('wb')
        outputs.append(output)
        process = subprocess.Popen(['ip', 'netns', 'exec', ns, *argv], stdout=output, stderr=subprocess.STDOUT)
        processes.append(process)
        return process

    def macsec(ns):
        devices = json.loads(ip(ns, '-d', '-j', 'link', 'show').stdout)
        return next((d['ifname'] for d in devices if d.get('linkinfo', {}).get('info_kind') == 'macsec'), None)

    try:
        for ns in [switch, *peers.values()]:
            command('ip', 'netns', 'add', ns)
            owned.append(ns)
            ip(ns, 'link', 'set', 'lo', 'up')
        ip(switch, 'link', 'add', 'br0', 'type', 'bridge')
        ip(switch, 'link', 'set', 'br0', 'type', 'bridge', 'group_fwd_mask', '8')
        inside(switch, 'sysctl', '-q', '-w', 'net.ipv6.conf.br0.disable_ipv6=1')
        ip(switch, 'link', 'set', 'br0', 'up')
        for name, ns in peers.items():
            ip(switch, 'link', 'add', 'p' + name, 'type', 'veth', 'peer', 'name', 'e' + name)
            ip(switch, 'link', 'set', 'e' + name, 'netns', ns)
            ip(ns, 'link', 'set', 'e' + name, 'name', 'eth0')
            inside(ns, 'sysctl', '-q', '-w', 'net.ipv6.conf.eth0.disable_ipv6=1')
            ip(ns, 'link', 'set', 'eth0', 'up')
            inside(switch, 'sysctl', '-q', '-w', 'net.ipv6.conf.p' + name + '.disable_ipv6=1')
            ip(switch, 'link', 'set', 'p' + name, 'master', 'br0')
            ip(switch, 'link', 'set', 'p' + name, 'up')
        cak = secrets.token_hex(16)
        ckn = secrets.token_hex(16)
        for number, (name, ns) in enumerate(peers.items()):
            key = cak if name != 'c' else secrets.token_hex(16)
            config = work / (name + '.conf')
            config.write_text('ctrl_interface=' + str(work / ('ctrl-' + name)) + '\n'
                              'eapol_version=3\nap_scan=0\nnetwork={\n'
                              ' key_mgmt=NONE\n eapol_flags=0\n macsec_policy=1\n'
                              ' macsec_integ_only=0\n macsec_replay_protect=1\n macsec_replay_window=0\n'
                              ' mka_cak=' + key + '\n mka_ckn=' + ckn + '\n'
                              ' mka_priority=' + str(number * 16) + '\n}\n')
            configs[name] = config
            supplicants[name] = launch(ns, 'wpa-' + name, [str(tools / 'run-wpa'), '-Dmacsec_linux', '-ieth0', '-c', str(config)])
        links = {}
        for _ in range(100):
            links = {name: macsec(ns) for name, ns in peers.items()}
            if all(links.values()):
                break
            if any(p.poll() is not None for p in processes):
                raise RuntimeError('supplicant exited; private logs retained on failure')
            time.sleep(.1)
        else:
            raise RuntimeError('MACsec interfaces were not created')
        for number, (name, ns) in enumerate(peers.items(), 1):
            if deploy:
                inside(ns, 'sysctl', '-q', '-w', 'net.ipv4.conf.all.rp_filter=0',
                       'net.ipv4.conf.eth0.rp_filter=0')
                ip(ns, 'addr', 'add', f'198.51.100.{number}/24', 'dev', 'eth0')
                ip(switch, 'link', 'set', 'p' + name, 'mtu', '1532')
                ip(ns, 'link', 'set', 'eth0', 'mtu', '1532')
                ip(ns, 'link', 'set', links[name], 'mtu', '1500')
            ip(ns, 'addr', 'add', f'192.0.2.{number}/24', 'dev', links[name])
            # The deployment fixture assigns known unique IPv6 test addresses;
            # it exercises encrypted payloads, not autoconfiguration during MKA
            # membership changes. Production native links are IPv4-only.
            ip(ns, '-6', 'addr', 'add', f'2001:db8::{number}/64', 'dev', links[name],
               *(['nodad'] if deploy else []))
            ip(ns, 'link', 'set', links[name], 'up')
        # Do not probe a tentative IPv6 destination during MKA enrollment.
        # An early solicitation with no preferred source can look like DAD.
        for _ in range(100):
            preferred=[]
            for number,name in [(1,'a'),(2,'b')]:
                rows=json.loads(ip(peers[name],'-6','-j','addr','show','dev',links[name]).stdout)
                preferred.append(any(v['local']==f'2001:db8::{number}'
                                     and not v.get('tentative') and not v.get('dadfailed')
                                     for row in rows for v in row.get('addr_info',[])))
            if all(preferred):break
            time.sleep(.1)
        else:raise RuntimeError('isolated IPv6 addresses did not complete DAD')
        echo = """import socket,threading,json,hashlib,pathlib,sys
payload=b'qrab-macsec-probe'
def udp(family,addr):
 s=socket.socket(family,socket.SOCK_DGRAM)
 if family==socket.AF_INET6:s.setsockopt(socket.IPPROTO_IPV6,socket.IPV6_V6ONLY,1)
 s.bind((addr,3434))
 while True:
  data,peer=s.recvfrom(4096)
  if data==b'qrab-forged-clear':pathlib.Path(sys.argv[1]).touch()
  s.sendto(json.dumps({'peer':peer[0],'bytes':len(data)}).encode(),peer)
def tcp(port):
 s=socket.socket();s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);s.bind(('0.0.0.0',port));s.listen()
 while True:
  c,peer=s.accept();data=b''
  while len(data)<16384:
   part=c.recv(16384-len(data))
   if not part:break
   data+=part
  c.sendall(json.dumps({'peer':peer[0],'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}).encode());c.close()
for family,address in [(socket.AF_INET,'192.0.2.2'),(socket.AF_INET6,'::')]:threading.Thread(target=udp,args=(family,address),daemon=True).start()
for port in [179,3260]:threading.Thread(target=tcp,args=(port,),daemon=True).start()
threading.Event().wait()
"""
        plaintext_marker = work / 'plaintext-reached-application'
        launch(peers['b'], 'echo', ['python3', '-u', '-c', echo, str(plaintext_marker)])
        udp_probe = """import socket,json,sys
family=socket.AF_INET6 if ':' in sys.argv[1] else socket.AF_INET
s=socket.socket(family,socket.SOCK_DGRAM);s.settimeout(1);s.sendto(b'qrab-macsec-probe',(sys.argv[1],3434));data,_=s.recvfrom(4096);print(data.decode())
"""
        for _ in range(20):
            result=inside(peers['a'], 'python3', '-c', udp_probe, '192.0.2.2', check=False)
            if result.returncode==0:break
            time.sleep(.2)
        else:raise RuntimeError('trusted MKA peers did not exchange IPv4 UDP data')
        assert json.loads(result.stdout)['peer']=='192.0.2.1'
        for _ in range(10):
            result=inside(peers['a'], 'python3', '-c', udp_probe, '2001:db8::2', check=False)
            if result.returncode==0:break
            time.sleep(.2)
        else:raise RuntimeError('trusted MKA peers did not exchange IPv6 UDP data')
        assert json.loads(result.stdout)['peer']=='2001:db8::1'
        tcp_probe="""import socket,json,hashlib,sys
payload=b'Q'*8192
s=socket.create_connection((sys.argv[2] if len(sys.argv)>2 else '192.0.2.2',int(sys.argv[1])),2);s.sendall(payload);s.shutdown(socket.SHUT_WR);data=b''
while True:
 part=s.recv(4096)
 if not part:break
 data+=part
v=json.loads(data);assert v['peer']==(sys.argv[3] if len(sys.argv)>3 else '192.0.2.1') and v['bytes']==len(payload) and v['sha256']==hashlib.sha256(payload).hexdigest();print(json.dumps({'port':int(sys.argv[1]),'bytes':len(payload),'addressPreserved':True}))
"""
        if deploy:
            ip(switch, 'link', 'set', 'br0', 'mtu', '1532')
            inside(peers['a'], 'python3', '-c', tcp_probe, '179', '198.51.100.2', '198.51.100.1')
            deployment_results['clearPrimaryWorksDuringStage'] = True
            def guards(name):
                body = json.dumps(deploy.guard_commands({'lower': 'eth0'}, 'isolated-test'))
                result = subprocess.run(['ip', 'netns', 'exec', peers[name],
                                         str(tools / 'run-nft'), '-j', '-f', '-'],
                                        input=body, text=True, capture_output=True, timeout=5)
                if result.returncode:
                    raise RuntimeError('isolated deployer guard rejected: ' + result.stderr)
            for name in peers:
                guards(name)
            blocked = inside(peers['a'], 'python3', '-c', tcp_probe, '179',
                             '198.51.100.2', '198.51.100.1', check=False)
            assert blocked.returncode != 0, 'the guarded raw lower still carries clear TCP'
            deployment_results.update({'clearPrimaryBlockedByGuard': True, 'upperMTU':1500,
                                       'lowerMTU':1532, 'rpFilterDisabled':True,
                                       'staticIpv6TestAddressesWithoutDAD':True})
            time.sleep(.3)
        # Capture only this isolated namespace's lower Ethernet link.
        capture_file = work / 'capture.json'
        replay_file = work / 'replay.bin'
        capture_ready = work / 'capture.ready'
        mac_b = json.loads(ip(peers['b'], '-j', 'link', 'show', 'eth0').stdout)[0]['address']
        capture = """import socket,time,pathlib,json,sys
output,replay,ready,peer=sys.argv[1:]
peer=bytes.fromhex(peer.replace(':',''))
s=socket.socket(socket.AF_PACKET,socket.SOCK_RAW,socket.htons(3));s.bind(('eth0',0));s.settimeout(.2)
pathlib.Path(ready).touch();deadline=time.monotonic()+3;counts={'macsec':0,'clearIP':0,'mka':0};frame=None
while time.monotonic()<deadline:
 try:data,address=s.recvfrom(65535)
 except socket.timeout:continue
 kind=int.from_bytes(data[12:14],'big')
 if kind==0x88e5:
  counts['macsec']+=1
  if address[2]==4 and data[:6]==peer:frame=data
 elif kind in [0x800,0x86dd]:counts['clearIP']+=1
 elif kind==0x888e:counts['mka']+=1
s.close()
if frame:pathlib.Path(replay).write_bytes(frame)
pathlib.Path(output).write_text(json.dumps(counts))
"""
        sniff = launch(peers['a'], 'capture', ['python3', '-u', '-c', capture,
                       str(capture_file), str(replay_file), str(capture_ready), mac_b])
        for _ in range(50):
            if capture_ready.exists():break
            time.sleep(.02)
        assert capture_ready.exists()
        tcp_results=[json.loads(inside(peers['a'], 'python3', '-c', tcp_probe, str(port)).stdout) for port in [179,3260]]
        inside(peers['a'], 'python3', '-c', udp_probe, '192.0.2.2')
        sniff.wait(timeout=5)
        captured = json.loads(capture_file.read_text())
        assert captured['macsec'] > 0 and captured['clearIP'] == 0, captured
        assert replay_file.exists(), 'no valid unicast frame available for replay'
        def state(name):
            return json.loads(ip(peers[name], '-j', '-s', 'macsec', 'show').stdout)[0]
        def late():
            return sum(v.get('InPktsLate',0) for v in state('b').get('rx_sc',[]))
        before_late = late()
        inject_replay="""import socket,pathlib,sys
s=socket.socket(socket.AF_PACKET,socket.SOCK_RAW,socket.htons(3));s.bind(('eth0',0));s.send(pathlib.Path(sys.argv[1]).read_bytes())
"""
        inside(peers['a'], 'python3', '-c', inject_replay, str(replay_file))
        time.sleep(.1)
        assert late() > before_late, 'replayed frame was not rejected by MACsec'
        # A valid IPv4/UDP packet injected below MACsec must not reach the
        # still-live UDP application on the receiver.
        plain=r"""import socket,struct,sys,json,errno
peer=bytes.fromhex(sys.argv[1].replace(':',''));s=socket.socket(socket.AF_PACKET,socket.SOCK_RAW,socket.htons(3));s.bind(('eth0',0));own=s.getsockname()[4]
payload=b'qrab-forged-clear';udp=struct.pack('!HHHH',40000,3434,8+len(payload),0)+payload
header=struct.pack('!BBHHHBBH4s4s',0x45,0,20+len(udp),1,0,64,17,0,socket.inet_aton('192.0.2.1'),socket.inet_aton('192.0.2.2'))
words=struct.unpack('!10H',header);total=sum(words)
while total>>16:total=(total&65535)+(total>>16)
header=header[:10]+struct.pack('!H',(~total)&65535)+header[12:]
u=socket.socket(socket.AF_INET,socket.SOCK_DGRAM);u.bind(('192.0.2.1',40000));u.settimeout(1)
try:s.send(peer+own+b'\x08\x00'+header+udp)
except OSError as error:
 if len(sys.argv)>2 and sys.argv[2]=='guarded' and error.errno in [errno.EPERM,errno.ENOBUFS]:
  print(json.dumps({'plaintextRefused':True}));raise SystemExit(0)
 raise
try:u.recvfrom(4096);raise SystemExit('plaintext injection reached the application')
except socket.timeout:print(json.dumps({'plaintextRefused':True}))
"""
        def guard_drops(name, chain):
            value=json.loads(inside(peers[name],str(tools/'run-nft'),'-j','list','table',
                                    'netdev',deploy.GUARD_TABLE).stdout)
            return sum(expr.get('counter',{}).get('packets',0)
                       for item in value['nftables'] for rule in [item.get('rule',{})]
                       if rule.get('chain')==chain and any('drop' in v for v in rule.get('expr',[]))
                       for expr in rule['expr'])
        egress_before=guard_drops('a','egress') if deploy else 0
        plain_response=inside(peers['a'], 'python3', '-c', plain, mac_b,
                              'guarded' if deploy else 'unguarded', check=False)
        if plain_response.returncode:
            raise RuntimeError('isolated plaintext check failed: ' + plain_response.stderr.strip())
        plain_result=json.loads(plain_response.stdout)
        if deploy:
            assert guard_drops('a','egress')>egress_before, 'sender guard did not reject the injected frame'
            # Inject below the receiver from its switch port, bypassing the
            # sender guard. The kernel reverse-path filter is disabled above.
            injector=plain.split('u=socket.socket',1)[0] + "s.send(peer+own+b'\\x08\\x00'+header+udp)\n"
            def attack():
                inside(switch, 'python3', '-c', injector.replace("('eth0',0)", "('pb',0)"), mac_b, check=False)
                time.sleep(.15)
            ingress_before=guard_drops('b','ingress')
            attack()
            assert not plaintext_marker.exists(), 'receiver guard admitted unencrypted IP'
            assert guard_drops('b','ingress')>ingress_before, 'receiver guard did not see the attack'
            deployment_results['receiverGuardRefusedInjectedPlaintext'] = True
            inside(peers['b'], str(tools / 'run-nft'), 'delete', 'table', 'netdev', deploy.GUARD_TABLE)
            attack()
            deployment_results['unguardedPlaintextReachedApplication'] = plaintext_marker.exists()
            plaintext_marker.unlink(missing_ok=True)
            guards('b')
            attack()
            assert not plaintext_marker.exists(), 'restored receiver guard admitted unencrypted IP'
            assert guard_drops('b','ingress')>0
            assert len(state('a').get('rx_sc',[]))==3, 'four trusted MKA peers did not enroll'
            deployment_results['trustedMkaMembers']=4
        inside(peers['a'], 'python3', '-c', udp_probe, '192.0.2.2')
        wrong=inside(peers['c'], 'python3', '-c', udp_probe, '192.0.2.2', check=False)
        assert wrong.returncode!=0, 'wrong CAK was admitted'
        # Advance (never reset) this disposable transmit SA near exhaustion.
        # Standard MKA must negotiate a new key before packet-number reuse.
        before_rekey = state('a')['sa_list']
        assert len(before_rekey) == 1, 'expected one initial transmit SA'
        an = before_rekey[0]['an']
        ip(peers['a'], 'macsec', 'set', links['a'], 'tx', 'sa', str(an), 'pn', '4294967280')
        rekeyed = False
        for _ in range(40):
            result=inside(peers['a'], 'python3', '-c', udp_probe, '192.0.2.2', check=False)
            current_ids=[v.get('key') for v in state('a').get('sa_list',[])]
            if result.returncode == 0 and any(v and v != before_rekey[0].get('key') for v in current_ids):
                rekeyed = True
                break
            time.sleep(.2)
        assert rekeyed, 'MKA did not rekey near packet-number exhaustion'
        # Restart the key server with the same long-term CAK. MKA must
        # establish fresh session state rather than reuse a static transmit SA.
        before_id={v.get('key') for v in state('a').get('sa_list',[]) if v.get('key')}
        supplicants['a'].terminate();supplicants['a'].wait(timeout=3)
        supplicants['a']=launch(peers['a'],'wpa-a-restarted',[str(tools/'run-wpa'),'-Dmacsec_linux','-ieth0','-c',str(configs['a'])])
        for _ in range(100):
            restored=macsec(peers['a'])
            if restored:break
            time.sleep(.1)
        else:raise RuntimeError('restarted supplicant created no MACsec interface')
        ip(peers['a'],'addr','add','192.0.2.1/24','dev',restored)
        ip(peers['a'],'-6','addr','add','2001:db8::1/64','dev',restored,
           *(['nodad'] if deploy else []))
        ip(peers['a'],'link','set',restored,'up')
        for _ in range(20):
            result=inside(peers['a'],'python3','-c',udp_probe,'192.0.2.2',check=False)
            if result.returncode==0:break
            time.sleep(.2)
        else:raise RuntimeError('MKA did not recover after key-server restart')
        after_id={v.get('key') for v in state('a').get('sa_list',[]) if v.get('key')}
        assert before_id and after_id and before_id.isdisjoint(after_id), 'session key identity was reused after restart'
        # Only structural metadata leaves the test, not key/config/log contents.
        print(json.dumps({'trustedIPv4': True, 'trustedIPv6': True, 'wrongCAKRefused': True, 'tcp': tcp_results,
                          'interfaces': links, 'lowerCapture':captured, 'replayRefused':True, **plain_result,
                          'automaticRekey':rekeyed, 'restartRecovered':True, 'freshSessionKey':True}))
        if deploy:
            print(json.dumps({'deployment':deployment_results}), flush=True)
    except BaseException:
        def public(value):
            if isinstance(value, dict):
                return {k: public(v) for k, v in value.items() if 'key' not in k.lower() and 'cak' not in k.lower()}
            if isinstance(value, list):
                return [public(v) for v in value]
            return value
        diagnostics = {}
        for name, ns in peers.items():
            if ns in owned:
                result = ip(ns, '-j', '-s', 'macsec', 'show', check=False)
                try:
                    diagnostics[name] = public(json.loads(result.stdout))
                    if deploy:
                        diagnostics[name] = {'macsec':diagnostics[name],
                                             'ipv6':json.loads(ip(ns, '-6', '-j', 'addr', 'show').stdout)}
                except json.JSONDecodeError:
                    diagnostics[name] = {'statusExit': result.returncode}
        print(json.dumps({'publicState': diagnostics}), flush=True)
        for config in work.glob('*.conf'):
            config.unlink()
        print('Prototype failed; credential-free logs remain at ' + str(work), flush=True)
        raise
    else:
        shutil.rmtree(work)
    finally:
        for p in reversed(processes):
            if p.poll() is None:
                p.terminate()
        for p in reversed(processes):
            try:
                p.wait(timeout=2)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait(timeout=2)
        for output in outputs:
            output.close()
        for ns in reversed(owned):
            command('ip', 'netns', 'del', ns, check=False)


if __name__ == '__main__':
    main()
