import RFB from './novnc/core/rfb.js';

const element = id => document.getElementById(id);
let session, rfb, polling;
const showError = message => { element('error').textContent = message; element('error').hidden = !message; };
async function request(path, options = {}) {
    const response = await fetch(path, {credentials: 'same-origin', ...options});
    if (response.status === 401) { signedOut(); throw new Error('Your session ended. Sign in to reconnect.'); }
    if (!response.ok) throw new Error(await response.text());
    return response.status === 204 ? null : response.json();
}
function signedOut() {
    clearInterval(polling);
    if (rfb) rfb.disconnect();
    session = null;
    element('dashboard').hidden = true;
    element('login').hidden = false;
    element('logout').hidden = true;
    element('identity').textContent = '';
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
    element('vm-name').textContent = session.vm_name;
    element('dashboard').hidden = false;
    element('logout').hidden = false;
    await refresh();
    polling = setInterval(refresh, 2000);
} catch (error) { signedOut(); if (!error.message.startsWith('Your session ended')) showError(error.message); }
