import RFB from './novnc/core/rfb.js';
import { createAudioContext } from './novnc/core/audio.js';

// Every management read and action goes to the in-pod Redfish service
// through the BMC's /redfish proxy; the BMC itself serves only the session
// and the console.
const SYSTEM = '/redfish/v1/Systems/1';
const RESET_INFO = `${SYSTEM}/ResetActionInfo`;
const EVENT_LOG = '/redfish/v1/Managers/1/LogServices/Events';
const EVENT_STREAM = '/redfish/v1/EventService/SSE';
const RESOURCES = '/redfish/v1/Managers/1/Oem/Qrab/Resources';
// ResetType, label and confirmation; only those Redfish allows are shown.
const RESETS = [
    ['Pause', 'Pause'],
    ['Resume', 'Resume'],
    ['ForceRestart', 'Reset', 'Reset this VM?'],
    ['GracefulRestart', 'Restart guest', 'Ask the guest agent to restart the guest?'],
    ['GracefulShutdown', 'Shut down', 'Shut down this VM?'],
    ['PowerCycle', 'Power cycle', 'Power-cycle this VM? QEMU exits and a fresh instance starts.'],
    ['Nmi', 'NMI', 'Send a non-maskable interrupt to every vCPU?'],
];
const element = id => document.getElementById(id);
let session, rfb, polling, background, events, system;
let agentResult = [];
const rendered = {};
const MAX_EVENT_ROWS = 200;
const showError = message => { element('error').textContent = message; element('error').hidden = !message; };
async function failure(response) {
    const text = await response.text();
    try {
        const error = JSON.parse(text).error;
        return error?.['@Message.ExtendedInfo']?.[0]?.Message || error?.message || text;
    } catch { return text; }
}
async function request(path, options = {}) {
    const response = await fetch(path, {credentials: 'same-origin', ...options});
    if (response.status === 401) { signedOut(); throw new Error('Your session ended. Sign in to reconnect.'); }
    if (!response.ok) throw new Error(await failure(response));
    return response.status === 204 ? null : response.json();
}
const post = (path, body) => request(path, {method: 'POST', headers: {'Content-Type': 'application/json', 'X-CSRF-Token': session.user.csrf}, body: JSON.stringify(body)});
function signedOut() {
    clearInterval(polling);
    clearInterval(background);
    if (events) { events.close(); events = null; }
    if (rfb) rfb.disconnect();
    session = null;
    element('dashboard').hidden = true;
    element('login').hidden = false;
    element('logout').hidden = true;
    element('identity').textContent = '';
    element('role').hidden = true;
}
const node = (tag, text, className) => {
    const item = document.createElement(tag);
    if (text !== undefined) item.textContent = text;
    if (className) item.className = className;
    return item;
};
const pairs = rows => rows.flatMap(([key, value]) => [node('dt', key), node('dd', String(value))]);
// Replace a panel only when its content changed, so polling keeps selections.
function render(id, rows) {
    const key = JSON.stringify(rows);
    if (rendered[id] === key) return;
    rendered[id] = key;
    element(id).replaceChildren(...pairs(rows));
}
// LogEntry (history) and Event (SSE) records carry the QMP event under Oem.Qrab.
const fromEntry = entry => ({seq: entry.Oem?.Qrab?.Sequence ?? Number(entry.Id), time: entry.Created, ...qmp(entry)});
const fromEvent = record => ({seq: record.Oem?.Qrab?.Sequence ?? Number(record.EventId), time: record.EventTimestamp, ...qmp(record)});
const qmp = record => ({event: record.Oem?.Qrab?.Event ?? record.Message, data: record.Oem?.Qrab?.Data, truncated: record.Oem?.Qrab?.Truncated});
function eventRow(event) {
    const at = event.time ? new Date(event.time) : new Date();
    const time = node('time', at.toLocaleTimeString());
    time.dateTime = at.toISOString();
    const data = event.truncated ? '(payload too large; omitted)' : JSON.stringify(event.data ?? {});
    const row = node('li');
    row.append(time, node('b', event.event), node('code', data === '{}' ? '' : data));
    row.title = `#${event.seq} ${data}`;
    return row;
}
function addEvents(rows) {
    const list = element('events');
    for (const row of rows) list.prepend(row);
    while (list.children.length > MAX_EVENT_ROWS) list.lastElementChild.remove();
}
async function loadDropped() {
    const log = await request(EVENT_LOG);
    const count = log.Oem?.Qrab?.QmpDropped ?? 0;
    element('events-dropped').textContent = count > 0 ? `${count} QMP event(s) were dropped before the manager recorded them` : '';
    element('events-dropped').hidden = !(count > 0);
}
// The stream opens first and buffers until the history is shown, so no event
// falls between them; sequence numbers drop the overlap.
async function startEvents() {
    const status = element('events-status');
    let last = -1, pending = [];
    const show = record => { if (record.seq > last) { last = record.seq; addEvents([eventRow(record)]); } };
    events = new EventSource(EVENT_STREAM);
    events.addEventListener('open', () => { status.textContent = 'Live'; });
    events.addEventListener('message', message => {
        let records;
        try { records = (JSON.parse(message.data).Events || []).map(fromEvent); } catch { return; }
        for (const record of records) pending ? pending.push(record) : show(record);
    });
    events.addEventListener('error', () => {
        status.textContent = events?.readyState === EventSource.CLOSED ? 'Disconnected' : 'Reconnecting…';
        refresh();
    });
    const page = await request(`${EVENT_LOG}/Entries`);
    const history = (page.Members || []).map(fromEntry).sort((a, b) => a.seq - b.seq).slice(-100);
    for (const record of history) show(record);
    const buffered = pending;
    pending = null;
    for (const record of buffered) show(record);
    await loadDropped();
}
function describeDisk(disk) {
    const parts = [disk.kind === 'iscsi' ? `iSCSI ${disk.id} (${disk.bus})` : `file (${disk.format})`, disk.read_only ? 'read-only' : 'read-write'];
    if (disk.boot_index !== undefined) parts.push(`boot ${disk.boot_index}`);
    if (disk.path) parts.push(disk.path);
    if (disk.target) parts.push(`${disk.target.portal} ${disk.target.target} LUN ${disk.target.lun}`, `user ${disk.target.username}`, `secret ${disk.target.password_file}`, `initiator ${disk.target.initiator_name}`);
    return parts.join(' · ');
}
const stamp = millis => millis ? new Date(millis).toLocaleString() : '—';
function updateSummary(update) {
    const exit = update['exit-code'] !== null && update['exit-code'] !== undefined ? ` · exit ${update['exit-code']}` : '';
    return `${update.state}${update['run-id'] ? ` · run ${update['run-id']}` : ''}${exit}${update['hook-installed'] ? '' : ' · no hook installed'}`;
}
function showAgent(status) {
    const rows = [
        ['Channel', status?.enabled ? 'enabled' : 'disabled'],
        ['Connected', status?.connected ? `yes, since ${stamp(status.connected_since_unix_ms)}` : 'no'],
        ['Version', status?.version || '—'],
        ['Last seen', stamp(status?.last_seen_unix_ms)],
    ];
    if (status?.last_error) rows.push(['Last error', status.last_error]);
    render('agent', [...rows, ...agentResult]);
    element('agent-note').textContent = status?.connected ? 'Connected' : 'Not connected';
    for (const button of document.querySelectorAll('[data-agent]')) button.disabled = !status?.connected;
}
const agentTarget = name => system?.Actions?.Oem?.[`#QrabComputerSystem.${name}`]?.target || `${SYSTEM}/Actions/Oem/QrabComputerSystem.${name}`;
for (const button of document.querySelectorAll('[data-agent]')) {
    button.addEventListener('click', async () => {
        const operation = button.dataset.agent;
        let body = {};
        if (operation === 'freeze' && !confirm('Freeze guest filesystems? Writes block until thawed or the agent watchdog thaws them.')) return;
        if (operation === 'update') {
            const args = prompt('Update hook arguments (space separated tokens, may be empty):', '');
            if (args === null) return;
            body = {Args: args.split(/\s+/).filter(Boolean)};
        }
        button.disabled = true;
        try {
            const name = operation[0].toUpperCase() + operation.slice(1);
            const result = await post(agentTarget(name), body);
            if (operation === 'freeze') agentResult = [['Filesystems', `frozen (${result.FrozenFilesystems})`]];
            if (operation === 'thaw') agentResult = [['Filesystems', `thawed (${result.ThawedFilesystems})`]];
            if (operation === 'update') {
                agentResult = [['Update', updateSummary(result.Update)]];
                if (result.Update.output) agentResult.push(['Update output', result.Update.output]);
            }
            showError('');
        } catch (error) { showError(error.message); }
        finally { await refresh(); }
    });
}
function showConfig(view) {
    if (!view) return;
    const rows = [
        ['Name', view.name], ['Memory', `${view.memory_mib} MiB`], ['vCPUs', view.cpus], ['Accelerator', view.accelerator],
        ['Boot', view.boot], ['TPM', view.tpm ? 'yes' : 'no'], ['Console', view.console ? 'yes' : 'no'],
        ['Guest network', view.guest_network ? 'enabled' : 'disabled'],
        ['Startup timeout', `${view.startup_timeout_secs}s`], ['Shutdown grace', `${view.shutdown_grace_secs}s`],
        ...view.disks.map((disk, index) => [`Disk ${index}`, describeDisk(disk)]),
        ...(view.nics || []).map(nic => [`NIC ${nic.id}`, `${nic.mac}${nic.endpoint ? ` · relay ${nic.endpoint}` : ''}`]),
    ];
    if (view.hotplug) {
        const h = view.hotplug;
        rows.push(['Hot-plug maximum', `${h.max_cpus} vCPUs · ${h.max_memory_mib} MiB`]);
        if (h.phase) {
            const size = h.current ? ` · ${h.current.cpus} vCPUs, ${h.current.memory_mib} MiB` : '';
            rows.push(['Resize', `${h.phase.replaceAll('_', ' ')}${size}${h.last_error ? ` · ${h.last_error}` : ''}`]);
        }
    }
    if (view.seed) {
        const s = view.seed;
        rows.push(['cloud-init seed', [s.hostname ? `hostname ${s.hostname}` : 'attached', `user-data ${s.user_data_bytes} bytes`, s.network_config ? 'network-config' : ''].filter(Boolean).join(' · ')]);
    }
    if (view.state_id) rows.push(['State ID', view.state_id]);
    if (view.kernel_args) rows.push(['Kernel args', view.kernel_args]);
    for (const [key, value] of Object.entries(view.deployment || {})) rows.push([key[0].toUpperCase() + key.slice(1), value]);
    for (const [key, value] of Object.entries(view.paths || {})) rows.push([key.replaceAll('_', ' '), value]);
    render('config', rows);
    element('config-note').textContent = view.redacted ? 'Sensitive fields hidden for viewers' : 'Full view';
}
async function loadResources() {
    const usage = await request(RESOURCES);
    const rows = [];
    const mib = value => value == null ? 'unavailable' : `${(value / 1048576).toFixed(1)} MiB`;
    for (const name of ['Redfish', 'Manager']) {
        const sample = usage[name];
        if (!sample) { rows.push([name, 'unavailable']); continue; }
        const process = sample.process || {};
        rows.push([`${name} process RSS`, mib(process.resident_memory_bytes)]);
        rows.push([`${name} process CPU total`, process.cpu_time_seconds == null ? 'unavailable' : `${process.cpu_time_seconds.toFixed(2)} s`]);
        if (sample.container) {
            rows.push([`${name} container memory`, mib(sample.container.memory_current_bytes)]);
            rows.push([`${name} memory limit`, mib(sample.container.memory_limit_bytes)]);
            rows.push([`${name} container CPU total`, sample.container.cpu_usage_seconds == null ? 'unavailable' : `${sample.container.cpu_usage_seconds.toFixed(2)} s`]);
        }
    }
    render('resources', rows);
}
// One ComputerSystem read carries power state, health, the guest agent and
// the (role-redacted) configuration.
async function refresh() {
    try {
        system = await request(SYSTEM);
        const health = system.Status?.Health;
        element('vm-status').textContent = `${system.PowerState ?? 'Unavailable'}${health ? ` · ${health}` : ''}`;
        element('vm-status').className = system.PowerState === 'On' ? 'running' : '';
        showAgent(system.Oem?.Qrab?.GuestAgent);
        showConfig(system.Oem?.Qrab?.Configuration);
    } catch (error) { showError(error.message); }
}
async function loadPower() {
    const info = await request(RESET_INFO);
    const allowed = new Set(info.Parameters?.find(parameter => parameter.Name === 'ResetType')?.AllowableValues || []);
    const target = () => system?.Actions?.['#ComputerSystem.Reset']?.target || `${SYSTEM}/Actions/ComputerSystem.Reset`;
    element('power').replaceChildren(...RESETS.filter(([type]) => allowed.has(type)).map(([type, label, question]) => {
        const button = node('button', label);
        button.addEventListener('click', async () => {
            if (question && !confirm(question)) return;
            button.disabled = true;
            try {
                await post(target(), {ResetType: type});
                showError('');
                await refresh();
            } catch (error) { showError(error.message); }
            finally { button.disabled = false; }
        });
        return button;
    }));
}
function disconnected(message) {
    rfb = null;
    element('connect').textContent = 'Connect console';
    element('console-status').textContent = message;
    element('cad').disabled = true;
    element('fullscreen').disabled = true;
}
element('connect').addEventListener('click', () => {
    if (rfb) { rfb.disconnect(); return; }
    showError('');
    const screen = element('screen');
    screen.replaceChildren();
    try {
        const scheme = location.protocol === 'https:' ? 'wss:' : 'ws:';
        const audio = audioContext ? {audioContext} : {};
        rfb = new RFB(screen, `${scheme}//${location.host}/api/console${audioContext ? '?features=audio' : ''}`, audio);
        rfb.scaleViewport = true;
        rfb.resizeSession = false;
        rfb.focusOnClick = true;
        rfb.showDotCursor = true;
        rfb.addEventListener('connect', () => {
            element('connect').textContent = 'Disconnect';
            element('console-status').textContent = 'Connected · Keyboard and mouse enabled';
            element('cad').disabled = false;
            element('fullscreen').disabled = false;
        });
        rfb.addEventListener('disconnect', event => { disconnected(event.detail.clean ? 'Console disconnected' : 'Connection closed. Check your session and reconnect.'); });
        rfb.addEventListener('securityfailure', () => { showError('Console connection failed.'); });
        element('console-status').textContent = 'Connecting…';
    } catch (error) { disconnected('Console disconnected'); showError(error.message); }
});
// Admin-bucket guest audio (docs/bmc.md#admin-bucket): off until clicked.
// The BMC admits audio per connection, so a change reconnects an open console.
let audioContext = null;
function audioToggle() {
    const button = node('button', 'Audio off');
    button.id = 'audio';
    button.title = 'Guest audio playback, about 1.4 Mbit/s';
    button.setAttribute('aria-pressed', 'false');
    button.addEventListener('click', () => {
        if (audioContext) { audioContext.close(); audioContext = null; } else { audioContext = createAudioContext(); }
        button.textContent = audioContext ? 'Audio on' : 'Audio off';
        button.setAttribute('aria-pressed', String(Boolean(audioContext)));
        if (rfb) {
            rfb.addEventListener('disconnect', () => element('connect').click(), {once: true});
            rfb.disconnect();
        }
    });
    element('fullscreen').after(button);
}
element('cad').addEventListener('click', () => rfb?.sendCtrlAltDel());
element('fullscreen').addEventListener('click', () => element('screen').requestFullscreen().catch(error => showError(error.message)));
element('logout').addEventListener('click', async () => {
    try {
        await request('/auth/logout', {method: 'POST', headers: {'X-CSRF-Token': session.user.csrf}});
        signedOut();
    } catch (error) { showError(error.message); }
});
try {
    session = await request('/api/session');
    element('identity').textContent = session.user.name;
    element('role').textContent = session.role === 'admin' ? 'Admin' : 'Viewer';
    element('role').hidden = false;
    element('vm-name').textContent = session.vm_name;
    const admin = session.role === 'admin';
    if (admin && session.admin_bucket?.includes('audio')) audioToggle();
    if (!admin) {
        for (const item of document.querySelectorAll('.admin-only')) item.hidden = true;
        element('screen').replaceChildren(node('p', 'Console and power actions require the admin role.'));
        element('console-status').textContent = 'View-only access';
        element('hint').hidden = true;
    }
    element('dashboard').hidden = false;
    element('logout').hidden = false;
    await refresh();
    polling = setInterval(refresh, 3000);
    await Promise.all([admin ? loadPower() : null, loadResources(), startEvents()]).catch(error => showError(error.message));
    background = setInterval(() => { loadResources().catch(() => {}); loadDropped().catch(() => {}); }, 5000);
} catch (error) { signedOut(); if (!error.message.startsWith('Your session ended')) showError(error.message); }
