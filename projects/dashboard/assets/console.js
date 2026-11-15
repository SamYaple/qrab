// noVNC and power controls for the dashboard's dedicated console page.
import RFB from './novnc/core/rfb.js';
import { createAudioContext } from './novnc/core/audio.js';

const element = id => document.getElementById(id);
const screen = element('screen');
const status = element('console-status');
const connect = element('connect');
const power = element('power');
const morePower = element('console-power-more');
let rfb = null;
let expired = false, pollTask = null;
function updateText(node, text) {
    if (node.textContent !== text) node.textContent = text;
}
function focusInView(node) {
    node.focus({preventScroll: true});
    const box = node.getBoundingClientRect();
    const header = document.querySelector('header');
    const top = header && getComputedStyle(header).position === 'sticky' ? header.getBoundingClientRect().bottom : 0;
    if (box.top < top || box.bottom > innerHeight || box.left < 0 || box.right > innerWidth) {
        node.scrollIntoView({block: 'center', inline: 'nearest'});
    }
}

function showError(message) {
    updateText(element('console-error'), message);
    element('console-error').hidden = !message;
}
function showPowerStatus(message) {
    const note = element('power-action-status');
    if (!note) return;
    updateText(note, message);
    note.hidden = !message;
}
function showPowerError(message) {
    const note = element('power-action-error');
    if (!note) { if (message) showError(message); return; }
    updateText(note, message);
    note.hidden = !message;
}

function endedScreen() {
    const note = document.createElement('p');
    note.textContent = 'Your session has ended. Sign in again to reconnect.';
    screen.replaceChildren(note);
}

function sessionEnded() {
    if (expired) return;
    const hadConsoleFocus = [screen, power, element('console-controls') || connect.parentElement].some(node => node.contains(document.activeElement));
    expired = true;
    pendingAction = null;
    busy = false;
    showPowerStatus('');
    showPowerError('');
    stopReconnecting();
    clearInterval(polling);
    state = null;
    resume = false;
    disarm();
    render();
    connect.disabled = true;
    element('cad').disabled = true;
    element('fullscreen').disabled = true;
    status.textContent = 'Session ended · Sign in to reconnect';
    element('console-recovery').hidden = false;
    const leaveFullscreen = document.fullscreenElement === screen ? document.exitFullscreen().catch(() => {}) : Promise.resolve();
    if (rfb) rfb.disconnect();
    else endedScreen();
    if (audioContext) { audioContext.close(); audioContext = null; }
    if (audioButton) {
        audioButton.disabled = true;
        audioButton.textContent = 'Audio off';
        audioButton.setAttribute('aria-pressed', 'false');
    }
    leaveFullscreen.then(() => {
        if (hadConsoleFocus) {
            const link = element('console-recovery').querySelector('a');
            focusInView(link);
        }
    });
}

async function failure(response) {
    if (response.status === 401) { sessionEnded(); return 'Your session ended. Sign in again to reconnect.'; }
    const text = await response.text();
    try {
        const message = JSON.parse(text).error?.message;
        if (typeof message === 'string') return message.slice(0, 600);
    } catch { /* Proxies may return a plain text or HTML error. */ }
    if (response.headers.get('content-type')?.includes('text/html') || /^\s*</.test(text)) return `VM access is unavailable (${response.status}). Try again shortly.`;
    return text.trim().slice(0, 600) || `Request failed (${response.status}).`;
}

async function powerRequest(options = {}) {
    let response;
    try { response = await fetch(power.dataset.endpoint, {credentials: 'same-origin', ...options}); }
    catch {
        throw new Error(options.method === 'POST'
            ? 'Connection lost. The action may have reached the VM; check its status before retrying.'
            : 'Unable to reach the VM. Check your connection or try again shortly.');
    }
    if (!response.ok) throw new Error(await failure(response));
    return response;
}

// After a reset, restart, shutdown, power cycle or power-on the console comes back by itself:
// retries back off from 1 s to 30 s and stop two minutes after the action.
const RECONNECT_WINDOW_MS = 120000;
let reconnectUntil = 0;
let reconnectDelay = 1000;
let reconnectTimer = null;
function stopReconnecting() {
    reconnectUntil = 0;
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
}
function startReconnecting() {
    reconnectUntil = Date.now() + RECONNECT_WINDOW_MS;
    reconnectDelay = 1000;
    if (!rfb && !reconnectTimer) scheduleReconnect();
}
function scheduleReconnect() {
    if (Date.now() >= reconnectUntil) { stopReconnecting(); return false; }
    if (poweredOff) {
        // Nothing serves the console until power on, which reconnects it.
        stopReconnecting();
        resume = true;
        showPoweredOff();
        return true;
    }
    clearTimeout(reconnectTimer);
    status.textContent = `Reconnecting in ${Math.round(reconnectDelay / 1000)} s…`;
    reconnectTimer = setTimeout(openConsole, reconnectDelay);
    reconnectDelay = Math.min(reconnectDelay * 2, 30000);
    return true;
}

// While Redfish reports the VM Off (or the VM is stopped) the console rests:
// no retries, Connect disabled, power controls unchanged. `resume` records
// that the console was in use, so it reconnects once the polled power state
// is On or Paused again.
const placeholder = [...screen.childNodes].map(child => child.cloneNode(true));
let poweredOff = false;
let resume = false;
function isOff(state) {
    return (!state.running && !state.pod) || (state.pod && !state.redfish_error && state.power_state === 'Off');
}
function consoleServed(state) {
    return state.running && state.pod && !state.redfish_error && ['On', 'Paused'].includes(state.power_state);
}
function showPoweredOff() {
    status.textContent = 'Powered off · Power on to use the console';
    const note = document.createElement('p');
    note.textContent = 'The VM is powered off. The console reconnects when it is powered on again.';
    screen.replaceChildren(note);
}
function followPower() {
    if (!state) return;
    const off = isOff(state);
    if (off && !poweredOff) {
        if (rfb || reconnectTimer || reconnectUntil > Date.now()) resume = true;
        stopReconnecting();
        poweredOff = true;
        if (!rfb) showPoweredOff();
    } else if (!off && poweredOff) {
        poweredOff = false;
        if (!rfb && resume) {
            status.textContent = 'Powering on · The console reconnects when the VM is running';
        } else if (!rfb) {
            status.textContent = 'Console disconnected';
            screen.replaceChildren(...placeholder.map(child => child.cloneNode(true)));
        }
    }
    if (!off && resume && consoleServed(state)) {
        resume = false;
        startReconnecting();
    }
    connect.disabled = poweredOff && !rfb;
}

function disconnected(message) {
    rfb = null;
    connect.textContent = 'Connect console';
    connect.disabled = expired || poweredOff;
    element('cad').disabled = true;
    element('fullscreen').disabled = true;
    if (scheduleReconnect()) return;
    if (expired) {
        status.textContent = 'Session ended · Sign in to reconnect';
        endedScreen();
    } else if (poweredOff) showPoweredOff(); else status.textContent = message;
}
function openConsole() {
    if (expired) return;
    reconnectTimer = null;
    showError('');
    screen.replaceChildren();
    try {
        const scheme = location.protocol === 'https:' ? 'wss:' : 'ws:';
        const audio = audioContext ? {audioContext} : {};
        rfb = new RFB(screen, `${scheme}//${location.host}${screen.dataset.socket}${audioContext ? '?features=audio' : ''}`, audio);
        rfb.scaleViewport = true;
        rfb.resizeSession = false;
        rfb.focusOnClick = true;
        rfb.showDotCursor = true;
        rfb.addEventListener('connect', () => {
            reconnectDelay = 1000;
            connect.textContent = 'Disconnect';
            status.textContent = 'Connected · Keyboard and mouse enabled';
            element('cad').disabled = false;
            element('fullscreen').disabled = false;
        });
        rfb.addEventListener('disconnect', event => {
            disconnected(event.detail.clean ? 'Console disconnected' : 'Connection closed. Your session may have ended: reload the page or sign in again.');
        });
        rfb.addEventListener('securityfailure', () => showError('Console connection failed.'));
        status.textContent = 'Connecting…';
    } catch (error) {
        disconnected('Console disconnected');
        showError(error.message);
    }
}
connect.addEventListener('click', () => {
    if (expired) return;
    stopReconnecting();
    resume = false;
    if (rfb) { rfb.disconnect(); return; }
    openConsole();
});
// Admin-bucket guest audio, rendered only when the VM enables it: off until
// clicked. The BMC admits audio per connection, so a change reconnects an
// open console with or without `?features=audio`.
let audioContext = null;
const audioButton = element('audio');
audioButton?.addEventListener('click', () => {
    if (expired) return;
    if (audioContext) { audioContext.close(); audioContext = null; } else { audioContext = createAudioContext(); }
    audioButton.textContent = audioContext ? 'Audio on' : 'Audio off';
    audioButton.setAttribute('aria-pressed', String(Boolean(audioContext)));
    if (rfb) {
        stopReconnecting();
        rfb.addEventListener('disconnect', openConsole, {once: true});
        rfb.disconnect();
    }
});
element('cad').addEventListener('click', () => { if (!expired) rfb?.sendCtrlAltDel(); });
element('fullscreen').addEventListener('click', () => { if (!expired) screen.requestFullscreen().catch(error => showError(error.message)); });

// Power. The server enforces role, CSRF, Origin and confirmation, and Redfish
// enforces the role again; the buttons only reflect what currently makes
// sense. In-pod buttons are Redfish ResetTypes, shown while ResetActionInfo
// allows them. Power on asks the VM's Pod (Redfish On) while it exists, since
// a powered-off VM keeps its Pod, and sets spec.running otherwise; Power off
// sets spec.running false, which ends the Pod and keeps the VM off.
const buttons = [...power.querySelectorAll('[data-power]')];
const RECONNECTS = ['on', 'ForceRestart', 'GracefulRestart', 'GracefulShutdown', 'PowerCycle'];
let state = null;
let busy = false;
let pendingAction = null;
let armed = null;
let armTimer = null;

function offered(button) {
    // Unknown (Redfish unreachable or not yet asked): keep the button, disabled.
    return button.dataset.inPod !== 'true' || !state?.reset_types || state.reset_types.includes(button.dataset.power);
}
function allowed(action) {
    if (!state) return false;
    const power = state.power_state || '';
    const live = state.pod && !state.redfish_error;
    switch (action) {
        case 'on': return !state.running || (live && power === 'Off');
        case 'off': return state.running || state.pod;
        case 'Resume': return live && power === 'Paused';
        case 'Pause':
        case 'GracefulShutdown':
        case 'Nmi': return live && power === 'On';
        case 'GracefulRestart': return live && power === 'On' && !state.guest_restart_unavailable;
        case 'ForceRestart': return live && (power === 'On' || power === 'Paused');
        case 'PowerCycle': return live && (power === 'On' || power === 'Paused' || power === 'Off');
        case 'ForceOff': return live && power !== 'Off';
        default: return false;
    }
}
const POWER_STATES = {On: 'Running', Paused: 'Paused', PoweringOn: 'Starting', PoweringOff: 'Stopping', Off: 'Off'};
function describe() {
    if (!state) return ['Unknown', ''];
    const phase = state.phase ? `Operator: ${state.phase}` : '';
    if (!state.running) return [state.pod ? 'Stopping' : 'Off', phase];
    if (!state.pod) return ['Starting', phase];
    if (state.redfish_error) return ['Unreachable', state.redfish_error];
    const health = state.health && state.health !== 'OK' ? `Health: ${state.health}` : '';
    return [POWER_STATES[state.power_state] || state.power_state || 'Unknown', [health, phase].filter(Boolean).join(' · ')];
}
function render() {
    const focused = document.activeElement;
    const [chip, detail] = describe();
    updateText(element('power-chip'), chip);
    element('power-chip').dataset.state = chip.toLowerCase();
    updateText(element('power-detail'), detail);
    const restartNote = element('restart-requirement');
    if (restartNote) {
        updateText(restartNote, state?.guest_restart_unavailable || '');
        restartNote.hidden = !state?.guest_restart_unavailable || state.power_state !== 'On' || !buttons.some(button => button.dataset.power === 'GracefulRestart' && offered(button));
    }
    const menuFocused = morePower?.contains(document.activeElement);
    if (armed && (!offered(armed) || !allowed(armed.dataset.power))) disarm();
    for (const button of buttons) {
        const pending = pendingAction?.button === button;
        if (pending) updateText(button, pendingAction.accepted ? 'Updating state…' : `Sending ${button.dataset.label.toLowerCase()}…`);
        else if (button !== armed) updateText(button, button.dataset.label);
        if (button.getAttribute('aria-busy') !== String(pending)) button.setAttribute('aria-busy', String(pending));
        button.hidden = !offered(button);
        if (button.dataset.power === 'GracefulRestart') {
            button.title = state?.guest_restart_unavailable || '';
            if (state?.guest_restart_unavailable && restartNote) button.setAttribute('aria-describedby', 'restart-requirement');
            else button.removeAttribute('aria-describedby');
        }
        const blocked = expired || busy || !offered(button) || !allowed(button.dataset.power);
        if (button.getAttribute('aria-disabled') !== String(blocked)) button.setAttribute('aria-disabled', String(blocked));
        // Keep the active request focusable while busy; the handler still
        // rejects every further send. Other unavailable controls stay native-disabled.
        button.disabled = blocked && !pending;
    }
    if (morePower) {
        morePower.hidden = expired || [...morePower.querySelectorAll('[data-power]')].every(button => !offered(button));
        if (morePower.hidden) morePower.open = false;
    }
    const unavailableFocus = buttons.includes(focused) && (focused.disabled || focused.hidden);
    if (!expired && (unavailableFocus || (morePower?.hidden && menuFocused))) focusInView(element('power-chip'));
}
function disarm() {
    clearTimeout(armTimer);
    if (armed) { armed.textContent = armed.dataset.label; armed.classList.remove('armed'); }
    armed = null;
}
function poll() {
    if (expired) return Promise.resolve();
    if (pollTask) return pollTask;
    pollTask = (async () => {
        try {
            const response = await powerRequest();
            const current = await response.json();
            if (expired) return;
            state = current;
            if (element('console-error').textContent.startsWith('Power state unavailable')) showError('');
        } catch (error) {
            state = null;
            showError(`Power state unavailable: ${error.message}`);
        }
        followPower();
        render();
    })().finally(() => { pollTask = null; });
    return pollTask;
}
async function send(button) {
    if (expired || busy || button.disabled || !offered(button) || !allowed(button.dataset.power)) return;
    const action = button.dataset.power;
    const disruptive = button.classList.contains('danger');
    if (disruptive && armed !== button) {
        disarm();
        armed = button;
        button.textContent = `Confirm ${button.dataset.label.toLowerCase()}`;
        button.classList.add('armed');
        armTimer = setTimeout(() => { disarm(); render(); }, 5000);
        return;
    }
    disarm();
    busy = true;
    showPowerError('');
    pendingAction = {button, accepted: false};
    showPowerStatus(`Sending ${button.dataset.label.toLowerCase()}…`);
    render();
    try {
        await powerRequest({
            method: 'POST',
            headers: {'Content-Type': 'application/json', 'X-CSRF-Token': power.dataset.csrf},
            body: JSON.stringify({action, confirm: disruptive}),
        });
        if (expired) return;
        pendingAction.accepted = true;
        showPowerStatus(`${button.dataset.label} request accepted. Updating state…`);
        render();
        if (RECONNECTS.includes(action)) {
            startReconnecting();
        } else if (action === 'off') {
            stopReconnecting();
            resume = false;
        }
    } catch (error) {
        if (!expired) {
            showPowerStatus('');
            showPowerError(`${button.dataset.label}: ${error.message}`);
        }
    } finally {
        if (pollTask) await pollTask;
        await poll();
        if (!expired && pendingAction?.accepted) showPowerStatus(`${button.dataset.label} request accepted.`);
        pendingAction = null;
        busy = false;
        render();
    }
}
for (const button of buttons) {
    const suppressedKeys = new Set();
    button.addEventListener('keydown', event => {
        if (event.key === 'Enter' && event.repeat) { event.preventDefault(); return; }
        if ((event.key === 'Enter' || event.key === ' ') &&
            (button.getAttribute('aria-disabled') === 'true' || suppressedKeys.has(event.key))) {
            suppressedKeys.add(event.key);
            event.preventDefault();
        }
    });
    button.addEventListener('keyup', event => { if (suppressedKeys.delete(event.key)) event.preventDefault(); });
    button.addEventListener('blur', () => suppressedKeys.clear());
    button.addEventListener('click', () => send(button));
}
morePower?.addEventListener('toggle', () => {
    if (!morePower.open && armed && morePower.contains(armed)) disarm();
});

let polling = null;
function watch() {
    clearInterval(polling);
    polling = null;
    if (!expired && document.visibilityState === 'visible') {
        poll();
        polling = setInterval(poll, 3000);
    }
}
document.addEventListener('visibilitychange', watch);
watch();
