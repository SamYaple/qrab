import RFB from './novnc/core/rfb.js';

const element = id => document.getElementById(id);
let session, rfb, polling, events;
const MAX_EVENT_ROWS = 200;
const showError = message => { element('error').textContent = message; element('error').hidden = !message; };
async function request(path, options = {}) {
    const response = await fetch(path, {credentials: 'same-origin', ...options});
    if (response.status === 401) { signedOut(); throw new Error('Your session ended. Sign in to reconnect.'); }
    if (!response.ok) throw new Error(await response.text());
    return response.status === 204 ? null : response.json();
}
function signedOut() {
    clearInterval(polling);
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
function eventRow(event) {
    const millis = event.qemu_time ? event.qemu_time.seconds * 1000 + Math.floor(event.qemu_time.microseconds / 1000) : event.received_unix_ms;
    const time = node('time', new Date(millis).toLocaleTimeString());
    time.dateTime = new Date(millis).toISOString();
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
function missedRow(count) {
    const row = node('li', undefined, 'gap');
    row.append(node('time', new Date().toLocaleTimeString()), node('b', 'MISSED'), node('code', `${count} older event(s) were evicted`));
    return row;
}
async function startEvents() {
    const page = await request('/api/events?limit=100');
    addEvents(page.events.map(eventRow));
    const status = element('events-status');
    const dropped = count => {
        element('events-dropped').textContent = count > 0 ? `${count} QMP event(s) were dropped before the manager recorded them` : '';
        element('events-dropped').hidden = !(count > 0);
    };
    dropped(page.qmp_dropped);
    events = new EventSource(`/api/events/stream?after=${page.next}`);
    events.addEventListener('open', () => { status.textContent = 'Live'; });
    events.addEventListener('qmp', message => addEvents([eventRow(JSON.parse(message.data))]));
    events.addEventListener('missed', message => addEvents([missedRow(message.data)]));
    events.addEventListener('qmp_dropped', message => dropped(Number(message.data)));
    events.addEventListener('error', () => {
        status.textContent = events?.readyState === EventSource.CLOSED ? 'Disconnected' : 'Reconnecting…';
        refresh();
    });
}
function describeDisk(disk) {
    const parts = [disk.kind === 'iscsi' ? `iSCSI ${disk.id} (${disk.bus})` : `file (${disk.format})`, disk.read_only ? 'read-only' : 'read-write'];
    if (disk.boot_index !== undefined) parts.push(`boot ${disk.boot_index}`);
    if (disk.path) parts.push(disk.path);
    if (disk.target) parts.push(`${disk.target.portal} ${disk.target.target} LUN ${disk.target.lun}`, `user ${disk.target.username}`, `secret ${disk.target.password_file}`, `initiator ${disk.target.initiator_name}`);
    return parts.join(' · ');
}
const stamp = millis => millis ? new Date(millis).toLocaleString() : '—';
async function loadAgent() {
    const view = await request('/api/agent');
    const status = view.status;
    const rows = [
        ['Channel', status.enabled ? 'enabled' : 'disabled'],
        ['Connected', status.connected ? `yes, since ${stamp(status.connected_since_unix_ms)}` : 'no'],
        ['Version', status.version || '—'],
        ['Last seen', stamp(status.last_seen_unix_ms)],
    ];
    if (status.last_error) rows.push(['Last error', status.last_error]);
    if (view.freeze) rows.push(['Filesystems', view.freeze]);
    if (view.update) {
        const update = view.update;
        rows.push(['Update', `${update.state}${update['run-id'] ? ` · run ${update['run-id']}` : ''}${update['exit-code'] !== null && update['exit-code'] !== undefined ? ` · exit ${update['exit-code']}` : ''}${update['hook-installed'] ? '' : ' · no hook installed'}`]);
        if (update.output) rows.push(['Update output', update.output]);
    }
    element('agent').replaceChildren(...rows.flatMap(([key, value]) => [node('dt', key), node('dd', String(value))]));
    element('agent-note').textContent = status.connected ? 'Connected' : 'Not connected';
    for (const button of document.querySelectorAll('[data-agent]')) button.disabled = !status.connected;
}
for (const button of document.querySelectorAll('[data-agent]')) {
    button.addEventListener('click', async () => {
        const operation = button.dataset.agent;
        let body;
        if (operation === 'freeze' && !confirm('Freeze guest filesystems? Writes block until thawed or the agent watchdog thaws them.')) return;
        if (operation === 'update') {
            const args = prompt('Update hook arguments (space separated tokens, may be empty):', '');
            if (args === null) return;
            body = JSON.stringify({args: args.split(/\s+/).filter(Boolean)});
        }
        button.disabled = true;
        try {
            await request(`/api/agent/${operation}`, {method: 'POST', headers: {'Content-Type': 'application/json', 'X-CSRF-Token': session.user.csrf}, body});
            showError('');
        } catch (error) { showError(error.message); }
        finally { await loadAgent().catch(error => showError(error.message)); }
    });
}
async function loadConfig() {
    const view = await request('/api/config');
    const rows = [
        ['Name', view.name], ['Memory', `${view.memory_mib} MiB`], ['vCPUs', view.cpus], ['Accelerator', view.accelerator],
        ['Boot', view.boot], ['TPM', view.tpm ? 'yes' : 'no'], ['Console', view.console ? 'yes' : 'no'],
        ['Guest network', view.guest_network ? 'enabled' : 'disabled'],
        ['Startup timeout', `${view.startup_timeout_secs}s`], ['Shutdown grace', `${view.shutdown_grace_secs}s`],
        ...view.disks.map((disk, index) => [`Disk ${index}`, describeDisk(disk)]),
    ];
    if (view.hotplug) {
        const h = view.hotplug;
        rows.push(['Hot-plug maximum', `${h.max_cpus} vCPUs · ${h.max_memory_mib} MiB`]);
        if (h.phase) {
            const size = h.current ? ` · ${h.current.cpus} vCPUs, ${h.current.memory_mib} MiB` : '';
            rows.push(['Resize', `${h.phase.replaceAll('_', ' ')}${size}${h.last_error ? ` · ${h.last_error}` : ''}`]);
        }
    }
    if (view.state_id) rows.push(['State ID', view.state_id]);
    if (view.kernel_args) rows.push(['Kernel args', view.kernel_args]);
    for (const [key, value] of Object.entries(view.deployment || {})) rows.push([key[0].toUpperCase() + key.slice(1), value]);
    for (const [key, value] of Object.entries(view.paths || {})) rows.push([key.replaceAll('_', ' '), value]);
    const list = element('config');
    list.replaceChildren(...rows.flatMap(([key, value]) => [node('dt', key), node('dd', String(value))]));
    element('config-note').textContent = view.redacted ? 'Sensitive fields hidden for viewers' : 'Full view';
}
async function refresh() {
    try {
        const status = await request('/api/status');
        element('vm-status').textContent = `${status.run_state || status.phase} · QEMU ${(status.qemu_version || []).join('.')}`;
        element('vm-status').className = status.ready ? 'running' : '';
    } catch (error) { showError(error.message); }
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
        rfb = new RFB(screen, `${scheme}//${location.host}/api/console`);
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
element('cad').addEventListener('click', () => rfb?.sendCtrlAltDel());
element('fullscreen').addEventListener('click', () => element('screen').requestFullscreen().catch(error => showError(error.message)));
for (const button of document.querySelectorAll('[data-action]')) {
    button.addEventListener('click', async () => {
        const action = button.dataset.action;
        if (['reset', 'powerdown'].includes(action) && !confirm(`${action === 'reset' ? 'Reset' : 'Shut down'} this VM?`)) return;
        button.disabled = true;
        try {
            await request('/api/action', {method: 'POST', headers: {'Content-Type': 'application/json', 'X-CSRF-Token': session.user.csrf}, body: JSON.stringify(action)});
            showError('');
            await refresh();
        } catch (error) { showError(error.message); }
        finally { button.disabled = false; }
    });
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
    if (session.role !== 'admin') {
        for (const item of document.querySelectorAll('.admin-only')) item.hidden = true;
        element('screen').replaceChildren(node('p', 'Console and power actions require the admin role.'));
        element('console-status').textContent = 'View-only access';
        element('hint').hidden = true;
    }
    element('dashboard').hidden = false;
    element('logout').hidden = false;
    await refresh();
    polling = setInterval(refresh, 2000);
    await Promise.all([loadConfig(), loadAgent(), startEvents()]).catch(error => showError(error.message));
    setInterval(() => loadAgent().catch(() => {}), 5000);
} catch (error) { signedOut(); if (!error.message.startsWith('Your session ended')) showError(error.message); }
