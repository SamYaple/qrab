import RFB from './novnc/core/rfb.js';
import { createAudioContext } from './novnc/core/audio.js';

// Every management read and action goes to the in-pod Redfish service
// through the BMC's /bmc/redfish browser bridge; the BMC itself serves only the session
// and the console.
// The dashboard mounts this same interface under one VM; standalone BMCs
// keep their root routes. This prefix comes from server-rendered HTML.
const vmBase = document.body.dataset.vmBase || '';
const consolePath = vmBase ? `${vmBase}/console/ws` : '/api/console';
if (vmBase) {
    document.getElementById('vm-navigation').hidden = false;
    document.getElementById('vm-detail-link').href = vmBase;
    document.getElementById('sign-in').href = `/auth/login?next=${encodeURIComponent(`${vmBase}/bmc`)}`;
}
const SYSTEM = '/redfish/v1/Systems/1';
const RESET_INFO = `${SYSTEM}/ResetActionInfo`;
const EVENT_LOG = '/redfish/v1/Managers/1/LogServices/Events';
const EVENT_STREAM = '/redfish/v1/EventService/SSE';
const RESOURCES = '/redfish/v1/Managers/1/Oem/Qrab/Resources';
const VM_REPORT = '/redfish/v1/TelemetryService/MetricReports/VM';
// ResetType, label and whether it is disruptive (two-step confirm); only
// those Redfish allows are shown.
// A powered-off VM keeps its pod: On starts QEMU again in place.
const RESETS = [
    ['On', 'Power on'],
    ['Pause', 'Pause'],
    ['Resume', 'Resume'],
    ['ForceRestart', 'Reset', true],
    ['GracefulRestart', 'Restart guest', true],
    ['GracefulShutdown', 'Shut down', true],
    ['ForceOff', 'Force off', true],
    ['PowerCycle', 'Power cycle', true],
    ['Nmi', 'NMI', true],
];
// Which reset types make sense in each PowerState; unknown states allow all.
const POWER_ALLOWS = {
    On: new Set(['Pause', 'ForceRestart', 'GracefulRestart', 'GracefulShutdown', 'ForceOff', 'PowerCycle', 'Nmi']),
    Paused: new Set(['Resume', 'ForceRestart', 'GracefulShutdown', 'ForceOff', 'PowerCycle']),
    Off: new Set(['On']),
    PoweringOn: new Set(['ForceOff']),
    PoweringOff: new Set(['ForceOff']),
};
const TABS = ['overview', 'metrics', 'events', 'agent', 'config'];
const element = id => document.getElementById(id);
// Header wrapping changes with zoom, long names and narrow screens. Keep the
// rail and anchor offsets tied to its actual height rather than a fixed guess.
if (typeof ResizeObserver === 'function') {
    const header = document.querySelector('.topbar');
    new ResizeObserver(() => {
        document.documentElement.style.setProperty('--topbar-height', `${Math.ceil(header.getBoundingClientRect().height)}px`);
    }).observe(header);
}
let session, rfb, polling, metricsTimer, events, system, refreshTimer;
let pendingAction = null, refreshTask = null, metricsTask = null, lastRead = null;
let agentResult = [];
let scaleToFit = true;
const rendered = {};
const MAX_EVENT_ROWS = 200;
const showError = message => { element('error').textContent = message; element('error').hidden = !message; };
const showAction = message => { element('action-status').textContent = message; element('action-status').hidden = !message; };
async function failure(response) {
    const text = await response.text();
    let message;
    try {
        const error = JSON.parse(text).error;
        message = typeof error === 'string' ? error : error?.['@Message.ExtendedInfo']?.[0]?.Message || error?.message;
    } catch { /* A failed proxy or ingress may return plain text or HTML. */ }
    if (typeof message !== 'string') message = null;
    if (!message && (response.headers.get('content-type')?.includes('text/html') || /^\s*</.test(text))) {
        return `VM access is unavailable (${response.status}). Check the VM's status or try again shortly.`;
    }
    return (message || text).trim().slice(0, 600) || `Request failed (${response.status}).`;
}
// Redfish advertises native /redfish URIs. Browser requests use a separate
// cookie/CSRF bridge; the public /redfish endpoint stays directly on Redfish.
const browserPath = path => {
    if (typeof path !== 'string' || !path.startsWith('/') || path.startsWith('//')) throw new Error('Invalid management URL');
    if (path === '/api/session') return vmBase ? `${vmBase}/bmc/session` : path;
    if (path === '/auth/logout') return vmBase ? `${vmBase}/bmc/logout` : path;
    let target;
    try { target = new URL(path, location.origin); } catch { throw new Error('Invalid management URL'); }
    if (target.origin !== location.origin || target.hash || !(target.pathname === '/redfish' || target.pathname.startsWith('/redfish/'))) {
        throw new Error('Invalid management URL');
    }
    // Normalize before adding this VM's prefix, so an upstream action URL can
    // never traverse out of its bridge or post to another dashboard VM.
    return `${vmBase}/bmc${target.pathname}${target.search}`;
};
async function request(path, options = {}) {
    if (path !== '/api/session' && !session) throw new Error('Your session ended. Sign in to reconnect.');
    const url = browserPath(path);
    let response;
    try {
        response = await fetch(url, {credentials: 'same-origin', ...options});
    } catch {
        throw new Error(options.method === 'POST'
            ? 'Connection lost. The action may have reached the VM; check its status before retrying.'
            : 'Unable to reach the VM. Check your connection or try again shortly.');
    }
    if (response.status === 401) { signedOut(); throw new Error('Your session ended. Sign in to reconnect.'); }
    if (path !== '/api/session' && !session) throw new Error('Your session ended. Sign in to reconnect.');
    if (!response.ok) throw new Error(await failure(response));
    return response.status === 204 ? null : response.json();
}
const post = (path, body) => request(path, {method: 'POST', headers: {'Content-Type': 'application/json', 'X-CSRF-Token': session.user.csrf}, body: JSON.stringify(body)});
function signedOut() {
    clearInterval(polling);
    clearInterval(metricsTimer);
    clearTimeout(refreshTimer);
    if (events) { events.close(); events = null; }
    if (rfb) rfb.disconnect();
    if (audioContext) { audioContext.close(); audioContext = null; }
    session = null;
    system = null;
    disarm();
    showError('');
    element('loading').hidden = true;
    element('read-status').hidden = true;
    showAction('');
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
    const index = TABS.indexOf(event.target.closest('[role=tab]')?.id.replace('tab-', ''));
    if (index < 0) return;
    const next = {ArrowRight: index + 1, ArrowDown: index + 1, ArrowLeft: index - 1, ArrowUp: index - 1, Home: 0, End: TABS.length - 1}[event.key];
    if (next === undefined) return;
    event.preventDefault();
    selectTab(TABS[(next + TABS.length) % TABS.length], {focus: true});
});
keepConsoleFocus(element('rail-tabs'));
keepConsoleFocus(element('console-toolbar'));

// ── Events. LogEntry (history) and Event (SSE) records carry the QMP event under Oem.Qrab.
// The stream also sends two notices without an SSE id: a history gap and the
// QMP-dropped total (at open when nonzero, then on every change).
const MISSED = 'Qrab.1.0.EventsMissed';
const DROPPED = 'Qrab.1.0.QmpEventsDropped';
const fromEntry = entry => ({seq: entry.Oem?.Qrab?.Sequence ?? Number(entry.Id), epoch: entry.Oem?.Qrab?.Epoch, time: entry.Created, ...qmp(entry)});
const fromEvent = record => {
    const oem = record.Oem?.Qrab ?? {};
    if (record.MessageId === MISSED) return {kind: 'gap', time: record.EventTimestamp, restarted: oem.Reason === 'SequenceRestarted', missed: oem.MissedEvents, after: oem.After, resumes: oem.OldestAvailableSequence};
    if (record.MessageId === DROPPED) return {kind: 'dropped', total: oem.QmpDropped ?? 0};
    return {kind: 'qmp', seq: oem.Sequence ?? Number(record.EventId), time: record.EventTimestamp, ...qmp(record)};
};
const qmp = record => ({event: record.Oem?.Qrab?.Event ?? record.Message, data: record.Oem?.Qrab?.Data, truncated: record.Oem?.Qrab?.Truncated});
const eventFilter = () => element('events-filter').value.trim().toLowerCase();
let eventHistoryLoaded = false;
function showEventEmpty() {
    const list = element('events');
    const empty = element('events-empty');
    const visible = [...list.children].some(row => !row.hidden);
    empty.hidden = visible;
    if (!visible) empty.textContent = list.children.length
        ? `No events match “${element('events-filter').value.trim()}”.`
        : eventHistoryLoaded ? 'No events recorded yet.' : 'Event history is unavailable. New events will appear when connected.';
}
// Gap markers stay visible under any filter: the gap may hide matching events.
const matches = (row, filter) => !filter || row.classList.contains('gap') || row.dataset.event.toLowerCase().includes(filter);
function eventTime(value) {
    const at = value ? new Date(value) : null;
    const known = at && Number.isFinite(at.getTime());
    const time = node(known ? 'time' : 'span', known ? at.toLocaleTimeString() : 'Time unavailable', 'event-time');
    if (known) { time.dateTime = at.toISOString(); time.title = at.toLocaleString(); }
    return time;
}
function eventRow(event) {
    const time = eventTime(event.time);
    const data = event.truncated ? 'Payload exceeded the event size limit and was omitted.' : JSON.stringify(event.data ?? {}, null, 2);
    const row = node('li');
    const details = node('details', undefined, 'event-details');
    const summary = node('summary');
    summary.append(time, ' ', node('b', event.event || 'Unknown event'));
    const sequence = Number.isFinite(event.seq) ? `#${event.seq}` : 'Sequence unavailable';
    details.append(summary, node('p', `${sequence} · ${time.dateTime || 'Timestamp unavailable'}`, 'event-context'));
    if (data === '{}') details.append(node('p', 'No event payload.', 'empty-note'));
    else {
        const payload = node('pre', data, 'event-payload');
        payload.tabIndex = 0;
        payload.setAttribute('aria-label', `Payload for ${event.event || 'unknown event'}`);
        details.append(payload);
    }
    row.append(details);
    row.dataset.event = String(event.event ?? '');
    row.hidden = !matches(row, eventFilter());
    return row;
}
function addEvents(rows) {
    const list = element('events');
    for (const row of rows) list.prepend(row);
    while (list.children.length > MAX_EVENT_ROWS) list.lastElementChild.remove();
    showEventEmpty();
}
element('events-filter').addEventListener('input', () => {
    const filter = eventFilter();
    for (const row of element('events').children) row.hidden = !matches(row, filter);
    showEventEmpty();
});
function gapRow(gap) {
    const time = eventTime(gap.time);
    const after = gap.resumes ? `; history resumes at #${gap.resumes}` : '';
    const text = gap.restarted
        ? `The VM manager restarted after #${gap.after} and numbers events from #1 again; events in between may be missing${after}`
        : `${gap.missed} event(s) after #${gap.after} were evicted before they could be shown${after}`;
    const row = node('li', undefined, 'gap');
    const heading = node('div', undefined, 'event-heading');
    heading.append(time, ' ', node('b', 'Gap'));
    row.append(heading, node('p', text, 'gap-message'));
    row.dataset.event = '';
    return row;
}
function showDropped(count) {
    element('events-dropped').textContent = count > 0 ? `${count} QMP event(s) were dropped before the manager recorded them` : '';
    element('events-dropped').hidden = !(count > 0);
}
// A QMP event usually means the power state or health changed: re-read
// Systems/1 soon, coalescing bursts.
function refreshSoon() {
    clearTimeout(refreshTimer);
    if (document.hidden) return;
    refreshTimer = setTimeout(refresh, 250);
}
// The history is read first and the stream opens after its newest event
// (?after=<epoch>.<sequence>, the form of an SSE id), so no event falls between
// them, a manager restart in between arrives as a gap, and a reconnect before
// any SSE id arrives still resumes from that cursor rather than the newest
// event. Once an id has arrived, the browser's Last-Event-ID takes precedence
// on reconnect. Rows are deduplicated by Oem.Qrab.Sequence, not by SSE id.
async function startEvents() {
    const status = element('events-status');
    let last = 0, cursor = '';
    const show = record => {
        if (record.kind === 'gap') {
            // The replay after a restart is numbered from #1 again.
            if (record.restarted) last = 0;
            addEvents([gapRow(record)]);
        } else if (record.seq > last) { last = record.seq; addEvents([eventRow(record)]); }
    };
    try {
        const page = await request(`${EVENT_LOG}/Entries`);
        const history = (page.Members || []).map(fromEntry).sort((a, b) => a.seq - b.seq);
        eventHistoryLoaded = true;
        for (const record of history.slice(-100)) show(record);
        last = history.at(-1)?.seq ?? 0;
        const epoch = history.at(-1)?.epoch;
        cursor = `?after=${epoch ? `${encodeURIComponent(epoch)}.` : ''}${last}`;
    } catch (error) {
        // Without a history cursor the stream starts at the newest event.
        if (!session) return;
        showError(error.message);
    }
    showEventEmpty();
    if (!session || events) return;
    events = new EventSource(browserPath(`${EVENT_STREAM}${cursor}`));
    events.addEventListener('open', () => { status.textContent = 'Live'; });
    events.addEventListener('message', message => {
        let records;
        try { records = (JSON.parse(message.data).Events || []).map(fromEvent); } catch { return; }
        for (const record of records) {
            if (record.kind === 'dropped') showDropped(record.total);
            else show(record);
        }
        if (records.some(record => record.kind === 'qmp')) refreshSoon();
    });
    events.addEventListener('error', () => {
        status.textContent = events?.readyState === EventSource.CLOSED ? 'Disconnected' : 'Reconnecting…';
        if (!document.hidden) refresh();
    });
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
    updateActionControls();
}
function updateActionControls() {
    const allows = POWER_ALLOWS[system?.PowerState];
    for (const button of element('power')?.querySelectorAll('button[data-type]') || []) {
        button.disabled = !session || Boolean(pendingAction) || !allows?.has(button.dataset.type);
    }
    const connected = system?.Oem?.Qrab?.GuestAgent?.connected;
    for (const button of document.querySelectorAll('[data-agent]')) button.disabled = !session || Boolean(pendingAction) || !connected;
    const connect = element('connect');
    if (connect && !rfb) connect.disabled = !session || !system || system.PowerState === 'Off';
    if (armed?.disabled) disarm();
}
async function runAction(button, operation) {
    if (pendingAction || !session) return;
    disarm();
    pendingAction = button;
    const label = button.dataset.label.replace(/…$/, '');
    button.textContent = `${label}…`;
    button.setAttribute('aria-busy', 'true');
    showError('');
    showAction(`${label} in progress…`);
    updateActionControls();
    try {
        await operation();
        if (session) showAction(`${label}: request accepted.`);
    } catch (error) {
        showAction('');
        showError(`${label}: ${error.message}`);
    } finally {
        // A poll already in flight may predate the action. Read again after it
        // settles, keeping controls locked until that fresh read completes.
        if (refreshTask) await refreshTask;
        await refresh();
        pendingAction = null;
        button.textContent = button.dataset.label;
        button.removeAttribute('aria-busy');
        updateActionControls();
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
// Oem.Qrab.GuestAgent.LastUpdate; Output and Error are present for admins only.
function updateSummary(update) {
    const exit = update.ExitCode != null ? ` · exit ${update.ExitCode}` : update.Signal != null ? ` · signal ${update.Signal}` : '';
    const ended = update.EndTime ? ` · ${new Date(update.EndTime).toLocaleString()}` : '';
    return `${update.State.toLowerCase()}${update.RunId ? ` · run ${update.RunId}` : ''}${exit}${ended}${update.HookInstalled ? '' : ' · no hook installed'}`;
}
function showAgent(status) {
    const rows = [
        ['Channel', status?.enabled ? 'enabled' : 'disabled'],
        ['Connected', status?.connected ? `yes, since ${stamp(status.connected_since_unix_ms)}` : 'no'],
        ['Version', status?.version || '—'],
        ['Last seen', stamp(status?.last_seen_unix_ms)],
    ];
    if (status?.last_error) rows.push(['Last error', status.last_error]);
    // Read from Systems/1 on every refresh, so they survive a page reload.
    if (status?.connected) rows.push(['Filesystems', status.FreezeState ? status.FreezeState.toLowerCase() : 'unknown']);
    const update = status?.connected ? status.LastUpdate : null;
    if (update) {
        rows.push(['Last update', updateSummary(update)]);
        if (update.Error) rows.push(['Update error', update.Error]);
        if (update.Output) rows.push([update.OutputTruncated ? 'Update output (tail)' : 'Update output', update.Output]);
    }
    render('agent', [...rows, ...agentResult]);
    element('agent-note').textContent = status?.connected ? 'Connected' : 'Not connected';
    updateActionControls();
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
            if (pendingAction || !session || button.disabled) return;
            const operation = button.dataset.agent;
            let body = {};
            if (operation === 'freeze' && !confirmed(button)) return;
            if (operation === 'update') {
                const args = prompt('Update hook arguments (space separated tokens, may be empty):', '');
                if (args === null) return;
                body = {Args: args.split(/\s+/).filter(Boolean)};
            }
            await runAction(button, async () => {
                const name = operation[0].toUpperCase() + operation.slice(1);
                const result = await post(agentTarget(name), body);
                // The freeze state and update run come back with the refresh below.
                if (operation === 'freeze') agentResult = [['Last action', `froze ${result.FrozenFilesystems} filesystems`]];
                if (operation === 'thaw') agentResult = [['Last action', `thawed ${result.ThawedFilesystems} filesystems`]];
                if (operation === 'update') agentResult = [['Last action', `started update run ${result.Update['run-id']}`]];
            });
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
        ...(view.nics || []).map(nic => [`NIC ${nic.id}`, nic.mac]),
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
function refresh() {
    if (!session) return Promise.resolve();
    if (refreshTask) return refreshTask;
    refreshTask = (async () => {
        try {
            const current = await request(SYSTEM);
            if (!session) return;
            system = current;
            lastRead = new Date();
            element('read-status').hidden = true;
            showStatus();
            followPower();
            showOverview();
            showAgent(system.Oem?.Qrab?.GuestAgent);
            showConfig(system.Oem?.Qrab?.Configuration);
        } catch (error) {
            if (!session) return;
            system = null;
            showStatus();
            const note = element('read-status');
            note.textContent = `Live data unavailable: ${error.message} ${lastRead ? `Values below were last updated at ${lastRead.toLocaleTimeString()}.` : 'No live data has been loaded yet.'} Retrying automatically.`;
            note.hidden = false;
        }
    })().finally(() => { refreshTask = null; });
    return refreshTask;
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
    const rows = (report.MetricValues || []).map(({MetricId, MetricValue}) => {
        const [label, format] = REPORT_FORMATS[MetricId] || [MetricId, value => value];
        const number = Number(MetricValue);
        return [label, Number.isFinite(number) ? format(number) : MetricValue];
    });
    render('telemetry', rows.length ? rows : [['Samples', 'No metrics have been reported yet.']]);
    return report.Timestamp;
}
// ── Metric history: Redfish MetricHistory, answered from the cluster
// Prometheus with fixed server-side PromQL. Hand-written SVG line charts;
// colours come from CSS custom properties. loadMetrics() calls
// renderMetricHistory every 5 s while the tab shows; history is re-read at
// most every 30 s (Redfish caches it for 15 s).
const HISTORY = '/redfish/v1/TelemetryService/Oem/Qrab/MetricHistory';
const HISTORY_METRICS = [
    ['container-cpu', 'Container CPU'],
    ['container-memory', 'Container memory'],
    ['block-read', 'Disk read'],
    ['block-write', 'Disk write'],
    ['nic-rx', 'Network received'],
    ['nic-tx', 'Network sent'],
];
const HISTORY_RANGES = ['15m', '1h', '6h', '24h', '7d'];
const HISTORY_REFRESH_MS = 30000;
const SERIES_COLOURS = 6;
const SVG_NS = 'http://www.w3.org/2000/svg';
const metricHistory = {range: '1h', fetchedAt: 0, loading: false, answers: {}, built: false, resize: null};
const rangeKey = () => `qrab-bmc:history-range:${session?.user?.subject || 'anonymous'}`;
function svgNode(tag, attributes = {}, text) {
    const item = document.createElementNS(SVG_NS, tag);
    for (const [key, value] of Object.entries(attributes)) item.setAttribute(key, String(value));
    if (text !== undefined) item.textContent = text;
    return item;
}
function scaled(value, suffix) {
    const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
    let index = 0;
    while (Math.abs(value) >= 1024 && index < units.length - 1) { value /= 1024; index += 1; }
    const digits = index === 0 || Math.abs(value) >= 10 ? 0 : 1;
    return `${value.toFixed(digits)} ${units[index]}${suffix}`;
}
function formatValue(value, unit) {
    if (unit === 'bytes') return scaled(value, '');
    if (unit === 'bytes/s') return scaled(value, '/s');
    if (unit === 'cores') return Math.abs(value) < 1 ? `${Math.round(value * 1000)} mcores` : `${value.toFixed(2)} cores`;
    return `${+value.toPrecision(3)} ${unit || ''}`.trim();
}
// A round axis maximum: 1, 2 or 5 times a power of 1024 (bytes) or 10.
function niceMax(max, unit) {
    if (!(max > 0)) return 1;
    const base = unit === 'bytes' || unit === 'bytes/s' ? 1024 : 10;
    const power = base ** Math.floor(Math.log(max) / Math.log(base));
    const step = [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000].find(n => n * power >= max);
    return step * power;
}
function axisTime(unix, range) {
    const at = new Date(unix * 1000);
    return range === '7d' ? at.toLocaleDateString([], {weekday: 'short', day: 'numeric'}) : at.toLocaleTimeString([], {hour: '2-digit', minute: '2-digit'});
}
const seriesLabel = series => Object.values(series.labels || {})[0] || '—';
function historySummary(answer, title, max) {
    const latest = answer.series.map(series => {
        const last = series.points.at(-1);
        return `${seriesLabel(series)} ${last ? formatValue(last[1], answer.unit) : 'no data'}`;
    });
    if (!latest.length) return `${title}: no data over the last ${answer.range}.`;
    return `${title} over the last ${answer.range}: peak ${formatValue(max, answer.unit)}; latest ${latest.join(', ')}.`;
}
function drawChart(figure, answer, title) {
    const body = figure.querySelector('.chart-body');
    const readout = figure.querySelector('.readout');
    const width = Math.max(240, Math.round(body.clientWidth || 320));
    const height = 132;
    const pad = {left: 74, right: 8, top: 8, bottom: 20};
    const plotWidth = width - pad.left - pad.right;
    const plotHeight = height - pad.top - pad.bottom;
    const values = answer.series.flatMap(series => series.points.map(point => point[1]));
    const max = values.length ? Math.max(...values) : 0;
    const top = niceMax(max, answer.unit);
    const x = t => pad.left + (t - answer.start) / Math.max(1, answer.end - answer.start) * plotWidth;
    const y = v => pad.top + plotHeight - Math.min(1, Math.max(0, v / top)) * plotHeight;
    const summary = historySummary(answer, title, max);
    figure.querySelector('.peak').textContent = values.length ? `peak ${formatValue(max, answer.unit)}` : '';
    if (!values.length) {
        body.replaceChildren(node('p', `No samples over the last ${answer.range}.`, 'chart-empty'));
        return;
    }
    const svg = svgNode('svg', {viewBox: `0 0 ${width} ${height}`, width, height, role: 'img', 'aria-label': summary, tabindex: 0, class: 'chart'});
    svg.append(svgNode('title', {}, summary));
    for (const fraction of [0, 0.5, 1]) {
        const level = pad.top + plotHeight * (1 - fraction);
        svg.append(svgNode('line', {x1: pad.left, x2: width - pad.right, y1: level, y2: level, class: fraction ? 'grid' : 'axis'}));
        svg.append(svgNode('text', {x: pad.left - 5, y: level + 4, 'text-anchor': 'end', class: 'tick'}, formatValue(top * fraction, answer.unit)));
    }
    for (const [fraction, anchor] of [[0, 'start'], [0.5, 'middle'], [1, 'end']]) {
        const t = answer.start + (answer.end - answer.start) * fraction;
        svg.append(svgNode('text', {x: pad.left + plotWidth * fraction, y: height - 5, 'text-anchor': anchor, class: 'tick'}, axisTime(t, answer.range)));
    }
    answer.series.forEach((series, index) => {
        // A gap longer than two steps (no samples, or NaN) breaks the line.
        let d = '', previous = null;
        for (const [t, v] of series.points) {
            d += `${previous !== null && t - previous <= 2 * answer.step ? 'L' : 'M'}${x(t).toFixed(1)},${y(v).toFixed(1)}`;
            previous = t;
        }
        if (series.points.length === 1) d += `h1`;
        svg.append(svgNode('path', {d, class: `line series-${index % SERIES_COLOURS}`}));
    });
    const cursor = svgNode('line', {y1: pad.top, y2: pad.top + plotHeight, class: 'cursor', visibility: 'hidden'});
    const hit = svgNode('rect', {x: pad.left, y: pad.top, width: plotWidth, height: plotHeight, class: 'hit'});
    svg.append(cursor, hit);
    const times = [...new Set(answer.series.flatMap(series => series.points.map(point => point[0])))].sort((a, b) => a - b);
    let selected = -1;
    const show = index => {
        if (!times.length) return;
        selected = Math.min(times.length - 1, Math.max(0, index));
        const t = times[selected];
        cursor.setAttribute('x1', x(t));
        cursor.setAttribute('x2', x(t));
        cursor.setAttribute('visibility', 'visible');
        const parts = answer.series.map(series => {
            const point = series.points.find(p => p[0] === t);
            return `${seriesLabel(series)} ${point ? formatValue(point[1], answer.unit) : '—'}`;
        });
        readout.textContent = `${new Date(t * 1000).toLocaleString()}: ${parts.join(' · ')}`;
    };
    const hide = () => { cursor.setAttribute('visibility', 'hidden'); selected = -1; readout.textContent = ''; };
    const nearest = t => times.reduce((best, value, index) => Math.abs(value - t) < Math.abs(times[best] - t) ? index : best, 0);
    svg.addEventListener('pointermove', event => {
        const box = svg.getBoundingClientRect();
        const px = (event.clientX - box.left) * width / box.width;
        show(nearest(answer.start + (px - pad.left) / plotWidth * (answer.end - answer.start)));
    });
    svg.addEventListener('pointerleave', () => { if (document.activeElement !== svg) hide(); });
    svg.addEventListener('focus', () => show(times.length - 1));
    svg.addEventListener('blur', hide);
    svg.addEventListener('keydown', event => {
        const next = {ArrowLeft: selected - 1, ArrowRight: selected + 1, Home: 0, End: times.length - 1}[event.key];
        if (next === undefined) return;
        event.preventDefault();
        show(next);
    });
    const legend = node('ul', undefined, 'legend');
    answer.series.forEach((series, index) => {
        const item = node('li');
        const last = series.points.at(-1);
        item.append(node('span', undefined, `swatch series-${index % SERIES_COLOURS}`), node('code', seriesLabel(series)), document.createTextNode(` ${last ? formatValue(last[1], answer.unit) : 'no data'}`));
        legend.append(item);
    });
    body.replaceChildren(svg, legend);
}
function drawHistory(slot) {
    for (const [metric, title] of HISTORY_METRICS) {
        const figure = slot.querySelector(`[data-metric="${metric}"]`);
        const answer = metricHistory.answers[metric];
        if (!answer) continue;
        if (answer.error) {
            figure.querySelector('.peak').textContent = '';
            figure.querySelector('.readout').textContent = '';
            figure.querySelector('.chart-body').replaceChildren(node('p', answer.error, 'chart-error'));
        } else drawChart(figure, answer.value, title);
    }
}
function buildHistory(slot) {
    try { const stored = localStorage.getItem(rangeKey()); if (HISTORY_RANGES.includes(stored)) metricHistory.range = stored; } catch { /* storage unavailable */ }
    const head = node('div', undefined, 'history-head');
    const ranges = node('div', undefined, 'ranges');
    ranges.setAttribute('role', 'group');
    ranges.setAttribute('aria-label', 'History range');
    for (const range of HISTORY_RANGES) {
        const button = node('button', range);
        button.type = 'button';
        button.dataset.range = range;
        button.setAttribute('aria-pressed', String(range === metricHistory.range));
        button.addEventListener('click', () => {
            if (metricHistory.range === range) return;
            metricHistory.range = range;
            try { localStorage.setItem(rangeKey(), range); } catch { /* storage unavailable */ }
            for (const other of ranges.children) other.setAttribute('aria-pressed', String(other.dataset.range === range));
            metricHistory.fetchedAt = 0;
            renderMetricHistory(slot);
        });
        ranges.append(button);
    }
    const status = node('span', 'Loading…', 'history-status');
    status.setAttribute('role', 'status');
    head.append(node('h3', 'History'), ranges, status);
    const charts = HISTORY_METRICS.map(([metric, title]) => {
        const figure = node('figure', undefined, 'history-chart');
        figure.dataset.metric = metric;
        const caption = node('figcaption');
        caption.append(node('b', title), node('span', '', 'peak'));
        const readout = node('p', '', 'readout');
        readout.setAttribute('aria-live', 'polite');
        figure.append(caption, node('div', undefined, 'chart-body'), readout);
        return figure;
    });
    slot.classList.remove('slot');
    slot.replaceChildren(head, ...charts);
    metricHistory.resize = new ResizeObserver(() => drawHistory(slot));
    metricHistory.resize.observe(slot);
    metricHistory.built = true;
}
async function renderMetricHistory(slot) {
    if (!session || !slot) return;
    if (!metricHistory.built) buildHistory(slot);
    if (metricHistory.loading || Date.now() - metricHistory.fetchedAt < HISTORY_REFRESH_MS) return;
    metricHistory.loading = true;
    const range = metricHistory.range;
    const status = slot.querySelector('.history-status');
    status.textContent = 'Loading…';
    try {
        const results = await Promise.allSettled(HISTORY_METRICS.map(([metric]) => request(`${HISTORY}?metric=${metric}&range=${range}`)));
        if (!session) return;
        if (range !== metricHistory.range) return;
        HISTORY_METRICS.forEach(([metric], index) => {
            const result = results[index];
            metricHistory.answers[metric] = result.status === 'fulfilled' ? {value: result.value} : {error: result.reason.message};
        });
        metricHistory.fetchedAt = Date.now();
        drawHistory(slot);
        const failed = results.filter(result => result.status === 'rejected').length;
        status.textContent = failed === results.length ? 'Unavailable' : `Updated ${new Date().toLocaleTimeString()}`;
    } finally {
        metricHistory.loading = false;
        if (range !== metricHistory.range) renderMetricHistory(slot);
    }
}
function loadMetrics() {
    if (!session || currentTab !== 'metrics' || document.hidden) return Promise.resolve();
    if (metricsTask) return metricsTask;
    metricsTask = (async () => {
        const note = element('metrics-note');
        const [report, resources] = await Promise.allSettled([loadReport(), loadResources()]);
        if (!session) return;
        if (report.status === 'rejected') render('telemetry', [['VM report', report.reason.message]]);
        if (resources.status === 'rejected') render('resources', [['Resources', resources.reason.message]]);
        const failures = [report, resources].filter(result => result.status === 'rejected').length;
        note.textContent = failures === 2 ? 'Unavailable' : failures ? 'Some data unavailable'
            : report.value ? `Sampled ${new Date(report.value).toLocaleTimeString()}` : 'Live';
        renderMetricHistory(element('metric-history'));
    })().finally(() => { metricsTask = null; });
    return metricsTask;
}

// ── Power group: the ResetType values Redfish's ResetActionInfo allows.
async function loadPower() {
    const info = await request(RESET_INFO);
    const allowed = new Set(info.Parameters?.find(parameter => parameter.Name === 'ResetType')?.AllowableValues || []);
    const target = () => system?.Actions?.['#ComputerSystem.Reset']?.target || `${SYSTEM}/Actions/ComputerSystem.Reset`;
    const buttons = RESETS.filter(([type]) => allowed.has(type)).map(([type, label, disruptive]) => {
        const button = node('button', label, disruptive ? 'danger' : undefined);
        button.dataset.type = type;
        button.dataset.label = label;
        button.addEventListener('click', async () => {
            if (pendingAction || !session || button.disabled) return;
            if (disruptive && !confirmed(button)) return;
            await runAction(button, async () => {
                await post(target(), {ResetType: type});
                // Power on brings the console up once the VM runs.
                if (type === 'On') { consoleWanted = true; resume = true; }
            });
        });
        return button;
    });
    const advanced = new Set(['ForceRestart', 'ForceOff', 'PowerCycle', 'Nmi']);
    const primary = buttons.filter(button => !advanced.has(button.dataset.type));
    const secondary = buttons.filter(button => advanced.has(button.dataset.type));
    if (secondary.length) {
        const more = node('details', undefined, 'power-advanced');
        more.id = 'advanced-power';
        const controls = node('div', undefined, 'group advanced-actions');
        controls.setAttribute('role', 'group');
        controls.setAttribute('aria-label', 'More power actions');
        controls.append(...secondary);
        more.append(node('summary', 'More power actions'), controls);
        primary.push(more);
    }
    element('power').replaceChildren(...primary);
    showStatus();
}

// ── Console.
// While Redfish reports PowerState Off nothing serves the console: Connect
// is disabled and the console says so, with the power group unchanged. A
// console that was in use when the VM went off reconnects once the polled
// power state is On or Paused again; there are no retries in between.
const POWERED_OFF = 'Powered off · Power on to use the console';
let consoleWanted = false;
let poweredOff = false;
let resume = false;
const placeholder = [...element('screen').childNodes].map(child => child.cloneNode(true));
function showPoweredOff() {
    element('console-status').textContent = POWERED_OFF;
    element('screen').dataset.state = 'off';
    element('screen').replaceChildren(node('p', resume
        ? 'The VM is powered off. The console will reconnect when it powers on.'
        : 'The VM is powered off. Power it on to make the console available.'));
}
function followPower() {
    if (session?.role !== 'admin' || !system) return;
    const off = system.PowerState === 'Off';
    if (off && !poweredOff) {
        poweredOff = true;
        if (consoleWanted) resume = true;
        if (!rfb) showPoweredOff();
    } else if (!off && poweredOff) {
        poweredOff = false;
        element('screen').removeAttribute('data-state');
        if (!rfb && resume) {
            element('console-status').textContent = 'Powering on · The console reconnects when the VM is running';
        } else if (!rfb) {
            element('console-status').textContent = 'Console disconnected';
            element('screen').replaceChildren(...placeholder.map(child => child.cloneNode(true)));
        }
    }
    if (!off && resume && !rfb && ['On', 'Paused'].includes(system.PowerState)) {
        resume = false;
        connect();
    }
    element('connect').disabled = poweredOff && !rfb;
}
function disconnected(message) {
    rfb = null;
    element('connect').textContent = 'Connect console';
    updateActionControls();
    element('cad').disabled = true;
    element('fullscreen').disabled = true;
    if (poweredOff) showPoweredOff(); else element('console-status').textContent = message;
}
function connect() {
    if (rfb) { rfb.disconnect(); return; }
    if (!session || !system || system.PowerState === 'Off') return;
    showError('');
    const screen = element('screen');
    screen.replaceChildren();
    try {
        const scheme = location.protocol === 'https:' ? 'wss:' : 'ws:';
        const audio = audioContext ? {audioContext} : {};
        rfb = new RFB(screen, `${scheme}//${location.host}${consolePath}${audioContext ? '?features=audio' : ''}`, audio);
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
    element('connect').addEventListener('click', () => {
        // A console the user closed stays closed through power changes.
        consoleWanted = !rfb;
        resume = false;
        connect();
    });
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
    element('loading').hidden = true;
    element('logout').hidden = false;
    selectTab(remembered() || 'overview', {store: false});
    await refresh();
    if (!session) throw new Error('Your session ended. Sign in to reconnect.');
    polling = setInterval(() => { if (!document.hidden) refresh(); }, 3000);
    await Promise.all([admin ? loadPower() : null, startEvents()]).catch(error => showError(error.message));
    metricsTimer = setInterval(() => { if (!document.hidden) loadMetrics(); }, 5000);
} catch (error) { signedOut(); if (!error.message.startsWith('Your session ended')) showError(error.message); }

document.addEventListener('visibilitychange', () => {
    if (!document.hidden && session) { refresh(); loadMetrics(); }
});
