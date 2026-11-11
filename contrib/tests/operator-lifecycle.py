#!/usr/bin/env python3
"""Disruptive acceptance for the disposable qrab demo; never deletes its PVC."""
import argparse
import copy
import json
import re
import subprocess
import time
import uuid

p=argparse.ArgumentParser(description=__doc__)
p.add_argument('--namespace',default='qrab')
p.add_argument('--vm',default='qrab-demo')
a=p.parse_args()

def command(*args,body=None,check=True):
    result=subprocess.run(['kubectl','-n',a.namespace,*args],input=json.dumps(body) if body is not None else None,text=True,capture_output=True)
    if check and result.returncode:raise RuntimeError(result.stderr.strip())
    return result

def get(kind,name):
    raw=command('get',kind,name,'--ignore-not-found','-o','json').stdout
    return json.loads(raw) if raw.strip() else None

def until(function,timeout=150):
    deadline=time.monotonic()+timeout
    while time.monotonic()<deadline:
        result=function()
        if result:return result
        time.sleep(.5)
    raise TimeoutError('Timed out waiting for VM lifecycle convergence')

def patch(data):
    return json.loads(command('patch','qvm',a.vm,'--type','merge','-p',json.dumps({'spec':data}),'-o','json').stdout)

def healthy(previous=None,generation=None):
    current=get('qvm',a.vm)
    if not current:return None
    status=current.get('status',{})
    if not status.get('ready') or status.get('observedGeneration')!=current['metadata']['generation']:return None
    if generation is not None and current['metadata']['generation']!=generation:return None
    pod=get('pod',a.vm)
    if not pod or pod['metadata'].get('deletionTimestamp') or pod['metadata']['uid']==previous or status.get('podUid')!=pod['metadata']['uid']:return None
    logs=command('logs',a.vm).stdout
    match=re.search(r'QRAB_TPM_EK_SHA256=([a-f0-9]{64})',logs)
    if not match or 'QRAB_TPM_EVENTLOG_PRESENT' not in logs:return None
    return {'crUid':current['metadata']['uid'],'podUid':pod['metadata']['uid'],'ek':match.group(1),'claimUid':status['stateVolumeUid']}

original=get('qvm',a.vm)
assert original and original['spec'].get('running',True), 'Start the disposable demo before this test'
assert not original['spec'].get('iscsiDisks'), 'This legacy lifecycle test deliberately fails a probe; native disks require the drain/fence-specific checks instead'
assert original['spec'].get('bootDisk')=='/opt/qrab/demo/boot.img', 'This test is only for the RAM-only demo fixture'
baseline=until(healthy)
claim_name=original['spec']['stateVolumeClaim']
claim=get('pvc',claim_name)
assert claim['metadata']['uid']==baseline['claimUid'] and not claim['metadata'].get('ownerReferences')
canary=f'{a.vm}-probe-{uuid.uuid4().hex[:6]}'
created_canary=False
stopped=False

def report(message):
    print(message,flush=True)

def same_identity(snapshot):
    assert snapshot['ek']==baseline['ek'], 'TPM endorsement identity changed'
    assert snapshot['crUid']==baseline['crUid'] and snapshot['claimUid']==baseline['claimUid'], 'CR/PVC identity changed'
    return snapshot

try:
    # The generated schema must reject accidental identity or invalid CPU edits.
    for data in [{'stateVolumeClaim':'different-identity'},{'cpus':0}]:
        result=command('patch','qvm',a.vm,'--type','merge','--dry-run=server','-p',json.dumps({'spec':data}),check=False)
        assert result.returncode!=0, 'Invalid/identity-changing spec edit was accepted'
    report('PASS server-side bounds and immutable state claim')

    updated=patch({'restartNonce':'conformance-'+uuid.uuid4().hex[:8]})
    current=same_identity(until(lambda:healthy(baseline['podUid'],updated['metadata']['generation'])))
    report('PASS CR-driven replacement retained the TPM endorsement key')

    old=current['podUid']
    command('delete','pod',a.vm,'--wait=true','--timeout=40s')
    current=same_identity(until(lambda:healthy(old)))
    report('PASS deleted Pod was recreated by its CR owner')

    config=get('configmap',a.vm+'-config')
    altered=json.loads(config['data']['vm.json']);altered['memory_mib']+=128
    old=current['podUid']
    command('patch','configmap',a.vm+'-config','--type','merge','-p',json.dumps({'data':{'vm.json':json.dumps(altered)}}))
    current=same_identity(until(lambda:healthy(old)))
    repaired=json.loads(get('configmap',a.vm+'-config')['data']['vm.json'])
    assert repaired['memory_mib']==original['spec']['memoryMiB']
    report('PASS configuration drift repaired and effective VM config restarted')

    stopped=True
    updated=patch({'running':False})
    def off():
        vm=get('qvm',a.vm);status=vm.get('status',{})
        return status.get('observedGeneration')==updated['metadata']['generation'] and status.get('phase')=='Stopped' and not get('pod',a.vm) and not any(status.get(k) for k in ['podName','podUid','nodeName'])
    until(off)
    assert get('pvc',claim_name)['metadata']['uid']==baseline['claimUid']
    report('PASS stopped CR removed its Pod and cleared links while retaining state')

    probe={'apiVersion':original['apiVersion'],'kind':'VirtualMachine','metadata':{'name':canary,'namespace':a.namespace},'spec':copy.deepcopy(original['spec'])}
    probe['spec']['running']=True
    command('create','-f','-',body=probe)
    created_canary=True
    def refused():
        pod=get('pod',canary)
        if not pod or pod.get('status',{}).get('phase')!='Failed':return False
        logs=command('logs',canary).stdout
        assert 'different state_id/name' in logs, 'Probe failed for an unexpected reason: '+logs[-1200:]
        return True
    until(refused)
    report('PASS another CR UID was refused the retained TPM identity')

    command('delete','qvm',canary,'--wait=true','--timeout=30s')
    until(lambda:not get('pod',canary) and not get('configmap',canary+'-config'))
    created_canary=False
    assert get('pvc',claim_name)['metadata']['uid']==baseline['claimUid']
    report('PASS CR deletion garbage-collected owned resources and retained the PVC')

    updated=patch({'running':True})
    current=same_identity(until(lambda:healthy(None,updated['metadata']['generation'])))
    stopped=False
    report('PASS CR start restored the same UEFI/TPM identity; EK='+current['ek'])
finally:
    if created_canary:
        command('delete','qvm',canary,'--ignore-not-found','--wait=true','--timeout=30s',check=False)
        until(lambda:not get('pod',canary),timeout=60)
    if stopped:
        patch({'running':True})
        same_identity(until(healthy))
