import RFB from './novnc/core/rfb.js';
import { createAudioContext } from './novnc/core/audio.js';

// Every management read and action goes to the in-pod Redfish service
// through the BMC's /bmc/redfish browser bridge; the BMC itself serves only the session
// and the console.
const SYSTEM = '/redfish/v1/Systems/1';
const RESET_INFO = `${SYSTEM}/ResetActionInfo`;
const EVENT_LOG = '/redfish/v1/Managers/1/LogServices/Events';
const EVENT_STREAM = '/redfish/v1/EventService/SSE';
const RESOURCES = '/redfish/v1/Managers/1/Oem/Qrab/Resources';
const VM_REPORT = '/redfish/v1/TelemetryService/MetricReports/VM';
// ResetType, label and whether it is disruptive (two-step confirm); only
// those Redfish allows are shown.
const RESETS = [
    ['Pause', 'Pause'],
    ['Resume', 'Resume'],
    ['ForceRestart', 'Reset', true],
    ['GracefulRestart', 'Restart guest', true],
    ['GracefulShutdown', 'Shut down', true],
    ['PowerCycle', 'Power cycle', true],
    ['Nmi', 'NMI', true],
];
const TABS = ['overview', 'metrics', 'events', 'agent', 'config'];
const element = id => document.getElementById(id);
let session, rfb, polling, metricsTimer, droppedTimer, events, system, refreshTimer;
let agentResult = [];
let scaleToFit = true;
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
// Redfish advertises native /redfish URIs. Browser requests use a separate
// cookie/CSRF bridge; the public /redfish endpoint stays directly on Redfish.
const browserPath = path => {
    if (typeof path !== 'string' || !path.startsWith('/') || path.startsWith('//')) throw new Error('Invalid management URL');
    return path === '/redfish' || path.startsWith('/redfish/') ? `/bmc${path}` : path;
};
async function request(path, options = {}) {
    const response = await fetch(browserPath(path), {credentials: 'same-origin', ...options});
    if (response.status === 401) { signedOut(); throw new Error('Your session ended. Sign in to reconnect.'); }
    if (!response.ok) throw new Error(await failure(response));
    return response.status === 204 ? null : response.json();
}
const post = (path, body) => request(path, {method: 'POST', headers: {'Content-Type': 'application/json', 'X-CSRF-Token': session.user.csrf}, body: JSON.stringify(body)});
function signedOut() {
    clearInterval(polling);
    clearInterval(metricsTimer);
    clearInterval(droppedTimer);
    clearTimeout(refreshTimer);
    if (events) { events.close(); events = null; }
    if (rfb) rfb.disconnect();
    session = null;
    element('dashboard').hidden = true;
    element('login').hidden = false;
    element('logout').hidden = true;
    element('identity').textContent = '';
    element('role').hidden = true;
    element('vm-status').hidden = true;
}
const node = (tag, text, className) => {
    const item = document.createElement(tag);
    if (text !== undefined) item.textContent = text;
    if (className) item.className = className;
    return item;
};
const pairs = rows => rows.flatMap(([key, value]) => [node('dt', key), node('dd', String(value))]);
// Replace a list only when its content changed, so polling keeps selections.
function render(id, rows) {
    const key = JSON.stringify(rows);
    if (rendered[id] === key) return;
    rendered[id] = key;
    element(id).replaceChildren(...pairs(rows));
}

// ── Two-step confirmation: a disruptive button first turns into
// "Confirm <action>" for five seconds; only a second click sends it.
let armed = null, armTimer = null;
function disarm() {
    clearTimeout(armTimer);
    if (armed) { armed.textContent = armed.dataset.label; armed.classList.remove('armed'); }
    armed = null;
}
function confirmed(button) {
    if (armed === button) { disarm(); return true; }
    disarm();
    armed = button;
    button.textContent = `Confirm ${button.dataset.label.toLowerCase()}`;
    button.classList.add('armed');
    armTimer = setTimeout(disarm, 5000);
    return false;
}

// ── Keyboard focus. noVNC captures keys while its canvas has focus
// (focusOnClick); pressing a toolbar or rail-tab button must not take that
// focus away, so only a click outside those releases the keyboard.
const consoleFocused = () => Boolean(rfb) && element('screen').contains(document.activeElement);
function keepConsoleFocus(container) {
    container.addEventListener('mousedown', event => {
        if (event.target.closest('button') && consoleFocused()) event.preventDefault();
    });
}

// ── Tabs (WAI-ARIA tabs, automatic activation, roving tabindex).
const tabKey = () => `qrab-bmc:tab:${session?.user?.subject || 'anonymous'}`;
function remembered() {
    try { return localStorage.getItem(tabKey()); } catch { return null; }
}
function remember(name) {
    try { localStorage.setItem(tabKey(), name); } catch { /* storage unavailable */ }
}
let currentTab = 'overview';
function selectTab(name, {focus = false, store = true} = {}) {
    if (!TABS.includes(name)) name = 'overview';
    currentTab = name;
    for (const tab of TABS) {
        const selected = tab === name;
        const button = element(`tab-${tab}`);
        button.setAttribute('aria-selected', String(selected));
        button.tabIndex = selected ? 0 : -1;
        element(`panel-${tab}`).hidden = !selected;
        if (selected && focus) button.focus();
    }
    if (store) remember(name);
    if (name === 'metrics') loadMetrics();
}
for (const tab of TABS) element(`tab-${tab}`).addEventListener('click', () => selectTab(tab));
// Arrow keys act only on a focused tab; keys sent to the console never reach here.
element('rail-tabs').addEventListener('keydown', event => {
    const index = TABS.indexOf(currentTab);
    const next = {ArrowRight: index + 1, ArrowDown: index + 1, ArrowLeft: index - 1, ArrowUp: index - 1, Home: 0, End: TABS.length - 1}[event.key];
    if (next === undefined) return;
    event.preventDefault();
    selectTab(TABS[(next + TABS.length) % TABS.length], {focus: true});
});
keepConsoleFocus(element('rail-tabs'));
keepConsoleFocus(element('console-toolbar'));

// ── Events. LogEntry (history) and Event (SSE) records carry the QMP event under Oem.Qrab.
const fromEntry = entry => ({seq: entry.Oem?.Qrab?.Sequence ?? Number(entry.Id), time: entry.Created, ...qmp(entry)});
const fromEvent = record => ({seq: record.Oem?.Qrab?.Sequence ?? Number(record.EventId), time: record.EventTimestamp, ...qmp(record)});
const qmp = record => ({event: record.Oem?.Qrab?.Event ?? record.Message, data: record.Oem?.Qrab?.Data, truncated: record.Oem?.Qrab?.Truncated});
const eventFilter = () => element('events-filter').value.trim().toLowerCase();
const matches = (row, filter) => !filter || row.dataset.event.toLowerCase().includes(filter);
function eventRow(event) {
    const at = event.time ? new Date(event.time) : new Date();
    const time = node('time', at.toLocaleTimeString());
    time.dateTime = at.toISOString();
    const data = event.truncated ? '(payload too large; omitted)' : JSON.stringify(event.data ?? {});
    const row = node('li');
    row.append(time, node('b', event.event), node('code', data === '{}' ? '' : data));
    row.title = `#${event.seq} ${data}`;
    row.dataset.event = String(event.event ?? '');
    row.hidden = !matches(row, eventFilter());
    return row;
}
function addEvents(rows) {
    const list = element('events');
    for (const row of rows) list.prepend(row);
    while (list.children.length > MAX_EVENT_ROWS) list.lastElementChild.remove();
}
element('events-filter').addEventListener('input', () => {
    const filter = eventFilter();
    for (const row of element('events').children) row.hidden = !matches(row, filter);
});
async function loadDropped() {
    const log = await request(EVENT_LOG);
    const count = log.Oem?.Qrab?.QmpDropped ?? 0;
    element('events-dropped').textContent = count > 0 ? `${count} QMP event(s) were dropped before the manager recorded them` : '';
    element('events-dropped').hidden = !(count > 0);
}
// A QMP event usually means the power state or health changed: re-read
// Systems/1 soon, coalescing bursts.
function refreshSoon() {
    clearTimeout(refreshTimer);
    refreshTimer = setTimeout(refresh, 250);
}
// The stream opens first and buffers until the history is shown, so no event
// falls between them; sequence numbers drop the overlap.
async function startEvents() {
    const status = element('events-status');
    let last = -1, pending = [];
    const show = record => { if (record.seq > last) { last = record.seq; addEvents([eventRow(record)]); } };
    events = new EventSource(browserPath(EVENT_STREAM));
    events.addEventListener('open', () => { status.textContent = 'Live'; });
    events.addEventListener('message', message => {
        let records;
        try { records = (JSON.parse(message.data).Events || []).map(fromEvent); } catch { return; }
        for (const record of records) pending ? pending.push(record) : show(record);
        if (records.length) refreshSoon();
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

// ── Status header and overview, from one Systems/1 read.
const stamp = millis => millis ? new Date(millis).toLocaleString() : '—';
function showStatus() {
    const chip = element('vm-status');
    const power = system?.PowerState ?? 'Unavailable';
    const health = system?.Status?.Health;
    const text = `${power}${health ? ` · ${health}` : ''}`;
    if (chip.textContent !== text) chip.textContent = text;
    chip.dataset.state = system?.PowerState ? system.PowerState.toLowerCase() : 'unknown';
    chip.dataset.health = health || '';
    chip.title = system?.Status?.State ? `State: ${system.Status.State}` : '';
    chip.hidden = false;
    for (const button of element('power')?.children || []) {
        if (button.dataset.type === 'Pause') button.disabled = system?.PowerState === 'Paused';
        if (button.dataset.type === 'Resume') button.disabled = system?.PowerState === 'On';
    }
}
function showOverview() {
    const view = system?.Oem?.Qrab?.Configuration;
    const agent = system?.Oem?.Qrab?.GuestAgent;
    const disks = view?.disks || [];
    const nics = view?.nics || [];
    const kinds = disks.map(disk => disk.kind === 'iscsi' ? 'iSCSI' : 'file');
    const memory = system?.MemorySummary?.TotalSystemMemoryGiB;
    render('overview', [
        ['Power', system?.PowerState ?? 'Unavailable'],
        ['Health', system?.Status?.Health ?? '—'],
        ['State', system?.Status?.State ?? '—'],
        ['vCPUs', system?.ProcessorSummary?.LogicalProcessorCount ?? '—'],
        ['Memory', memory == null ? '—' : memory >= 1 ? `${+memory.toFixed(2)} GiB` : `${Math.round(memory * 1024)} MiB`],
        ['Disks', disks.length ? `${disks.length} (${kinds.join(', ')})` : 'none'],
        ['NICs', nics.length ? `${nics.length} (${nics.map(nic => nic.mac).join(', ')})` : 'none'],
        ['Guest network', view ? (view.guest_network ? 'enabled' : 'disabled') : '—'],
        ['Guest agent', !agent?.enabled ? 'disabled' : agent.connected ? `connected${agent.version ? ` · ${agent.version}` : ''}` : 'not connected'],
    ]);
}
function describeDisk(disk) {
    const parts = [disk.kind === 'iscsi' ? `iSCSI ${disk.id} (${disk.bus})` : `file (${disk.format})`, disk.read_only ? 'read-only' : 'read-write'];
    if (disk.boot_index !== undefined) parts.push(`boot ${disk.boot_index}`);
    if (disk.path) parts.push(disk.path);
    if (disk.target) parts.push(`${disk.target.portal} ${disk.target.target} LUN ${disk.target.lun}`, `user ${disk.target.username}`, `secret ${disk.target.password_file}`, `initiator ${disk.target.initiator_name}`);
    return parts.join(' · ');
}
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
    for (const button of document.querySelectorAll('[data-agent]')) if (button !== armed) button.disabled = !status?.connected;
}
const agentTarget = name => system?.Actions?.Oem?.[`#QrabComputerSystem.${name}`]?.target || `${SYSTEM}/Actions/Oem/QrabComputerSystem.${name}`;
function wireAgentActions() {
    for (const button of document.querySelectorAll('[data-agent]')) {
        button.dataset.label = button.textContent;
        if (button.dataset.agent === 'freeze') {
            button.classList.add('danger');
            button.title = 'Writes block until thawed or the agent watchdog thaws them';
        }
        button.addEventListener('click', async () => {
            const operation = button.dataset.agent;
            let body = {};
            if (operation === 'freeze' && !confirmed(button)) return;
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
// One ComputerSystem read carries power state, health, the guest agent and
// the (role-redacted) configuration.
async function refresh() {
    try {
        system = await request(SYSTEM);
        showStatus();
        showOverview();
        showAgent(system.Oem?.Qrab?.GuestAgent);
        showConfig(system.Oem?.Qrab?.Configuration);
    } catch (error) {
        showError(error.message);
        if (session) { system = null; showStatus(); }
    }
}

// ── Metrics: current values only. Each VM report read takes a fresh QMP
// sample, so the Metrics tab polls only while it is shown.
const mib = value => value == null ? 'unavailable' : `${(value / 1048576).toFixed(1)} MiB`;
const seconds = value => value == null ? 'unavailable' : `${value.toFixed(2)} s`;
const REPORT_FORMATS = {
    LogicalProcessors: ['vCPUs', value => value],
    MemoryBytes: ['Memory', mib],
    BlockReadBytes: ['Disk read', mib],
    BlockWriteBytes: ['Disk written', mib],
    BlockReadOperations: ['Read operations', value => value.toLocaleString()],
    BlockWriteOperations: ['Write operations', value => value.toLocaleString()],
    BlockFlushOperations: ['Flush operations', value => value.toLocaleString()],
    BlockReadTimeNanoseconds: ['Read time', value => seconds(value / 1e9)],
    BlockWriteTimeNanoseconds: ['Write time', value => seconds(value / 1e9)],
};
async function loadResources() {
    const usage = await request(RESOURCES);
    const rows = [];
    for (const name of ['Redfish', 'Manager']) {
        const sample = usage[name];
        if (!sample) { rows.push([name, 'unavailable']); continue; }
        const process = sample.process || {};
        rows.push([`${name} process RSS`, mib(process.resident_memory_bytes)]);
        rows.push([`${name} process CPU total`, seconds(process.cpu_time_seconds)]);
        if (sample.container) {
            rows.push([`${name} container memory`, mib(sample.container.memory_current_bytes)]);
            rows.push([`${name} memory limit`, mib(sample.container.memory_limit_bytes)]);
            rows.push([`${name} container CPU total`, seconds(sample.container.cpu_usage_seconds)]);
        }
    }
    render('resources', rows);
}
async function loadReport() {
    const report = await request(VM_REPORT);
    render('telemetry', (report.MetricValues || []).map(({MetricId, MetricValue}) => {
        const [label, format] = REPORT_FORMATS[MetricId] || [MetricId, value => value];
        const number = Number(MetricValue);
        return [label, Number.isFinite(number) ? format(number) : MetricValue];
    }));
    return report.Timestamp;
}
// Hook for work/bmc-metric-history-graphs: called with the #metric-history
// slot each time the Metrics tab is shown or polled. Until history exists the
// slot keeps its static placeholder.
function renderMetricHistory(/* slot */) {}
async function loadMetrics() {
    if (!session || currentTab !== 'metrics') return;
    const note = element('metrics-note');
    const [report, resources] = await Promise.allSettled([loadReport(), loadResources()]);
    if (report.status === 'rejected') render('telemetry', [['VM report', report.reason.message]]);
    if (resources.status === 'rejected') render('resources', [['Resources', resources.reason.message]]);
    note.textContent = report.status === 'fulfilled' && report.value ? `Sampled ${new Date(report.value).toLocaleTimeString()}` : 'Live';
    renderMetricHistory(element('metric-history'));
}

// ── Power group: the ResetType values Redfish's ResetActionInfo allows.
async function loadPower() {
    const info = await request(RESET_INFO);
    const allowed = new Set(info.Parameters?.find(parameter => parameter.Name === 'ResetType')?.AllowableValues || []);
    const target = () => system?.Actions?.['#ComputerSystem.Reset']?.target || `${SYSTEM}/Actions/ComputerSystem.Reset`;
    element('power').replaceChildren(...RESETS.filter(([type]) => allowed.has(type)).map(([type, label, disruptive]) => {
        const button = node('button', label, disruptive ? 'danger' : undefined);
        button.dataset.type = type;
        button.dataset.label = label;
        button.addEventListener('click', async () => {
            if (disruptive && !confirmed(button)) return;
            disarm();
            button.disabled = true;
            try {
                await post(target(), {ResetType: type});
                showError('');
            } catch (error) { showError(error.message); }
            finally { button.disabled = false; await refresh(); }
        });
        return button;
    }));
    showStatus();
}

// ── Console.
function disconnected(message) {
    rfb = null;
    element('connect').textContent = 'Connect console';
    element('console-status').textContent = message;
    element('cad').disabled = true;
    element('fullscreen').disabled = true;
}
function connect() {
    if (rfb) { rfb.disconnect(); return; }
    showError('');
    const screen = element('screen');
    screen.replaceChildren();
    try {
        const scheme = location.protocol === 'https:' ? 'wss:' : 'ws:';
        const audio = audioContext ? {audioContext} : {};
        rfb = new RFB(screen, `${scheme}//${location.host}/api/console${audioContext ? '?features=audio' : ''}`, audio);
        rfb.scaleViewport = scaleToFit;
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
}
function toggleScale() {
    scaleToFit = !scaleToFit;
    const button = element('scale');
    button.textContent = scaleToFit ? 'Fit' : '1:1';
    button.setAttribute('aria-pressed', String(scaleToFit));
    if (rfb) rfb.scaleViewport = scaleToFit;
}
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
            rfb.addEventListener('disconnect', () => connect(), {once: true});
            rfb.disconnect();
        }
    });
    element('fullscreen').after(button);
}
function wireConsole() {
    element('connect').addEventListener('click', connect);
    element('scale').addEventListener('click', toggleScale);
    element('cad').addEventListener('click', () => { rfb?.sendCtrlAltDel(); rfb?.focus(); });
    element('fullscreen').addEventListener('click', () => element('screen').requestFullscreen().catch(error => showError(error.message)));
}

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
    document.title = `${session.vm_name} · qrab BMC`;
    const admin = session.role === 'admin';
    if (admin) {
        wireConsole();
        wireAgentActions();
        if (session.admin_bucket?.includes('audio')) audioToggle();
    } else {
        // Viewers get no admin controls at all, not disabled or hidden ones.
        for (const item of document.querySelectorAll('[data-admin]')) item.remove();
        element('dashboard').classList.add('view-only');
        element('screen').replaceChildren(node('p', 'Console and power actions require the admin role.'));
        element('console-status').textContent = 'View-only access';
    }
    element('dashboard').hidden = false;
    element('logout').hidden = false;
    selectTab(remembered() || 'overview', {store: false});
    await refresh();
    polling = setInterval(refresh, 3000);
    await Promise.all([admin ? loadPower() : null, startEvents()]).catch(error => showError(error.message));
    metricsTimer = setInterval(() => { if (!document.hidden) loadMetrics(); }, 5000);
    droppedTimer = setInterval(() => { loadDropped().catch(() => {}); }, 5000);
} catch (error) { signedOut(); if (!error.message.startsWith('Your session ended')) showError(error.message); }
