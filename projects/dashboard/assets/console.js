// The only client-side script: noVNC against the dashboard's console proxy,
// and the console page's power controls.
import RFB from './novnc/core/rfb.js';
import { createAudioContext } from './novnc/core/audio.js';

const element = id => document.getElementById(id);
const screen = element('screen');
const status = element('console-status');
const connect = element('connect');
const power = element('power');
let rfb = null;

function showError(message) {
    element('console-error').textContent = message;
    element('console-error').hidden = !message;
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
    connect.disabled = poweredOff;
    element('cad').disabled = true;
    element('fullscreen').disabled = true;
    if (scheduleReconnect()) return;
    if (poweredOff) showPoweredOff(); else status.textContent = message;
}
function openConsole() {
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
    if (audioContext) { audioContext.close(); audioContext = null; } else { audioContext = createAudioContext(); }
    audioButton.textContent = audioContext ? 'Audio on' : 'Audio off';
    audioButton.setAttribute('aria-pressed', String(Boolean(audioContext)));
    if (rfb) {
        stopReconnecting();
        rfb.addEventListener('disconnect', openConsole, {once: true});
        rfb.disconnect();
    }
});
element('cad').addEventListener('click', () => rfb?.sendCtrlAltDel());
element('fullscreen').addEventListener('click', () => screen.requestFullscreen().catch(error => showError(error.message)));

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
        case 'GracefulRestart':
        case 'GracefulShutdown':
        case 'Nmi': return live && power === 'On';
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
    const [chip, detail] = describe();
    element('power-chip').textContent = chip;
    element('power-chip').dataset.state = chip.toLowerCase();
    element('power-detail').textContent = detail;
    for (const button of buttons) {
        if (button !== armed) button.textContent = button.dataset.label;
        // The stylesheet sets buttons' display, so `hidden` alone would not hide them.
        button.style.display = offered(button) ? '' : 'none';
        button.disabled = busy || !allowed(button.dataset.power);
    }
}
function disarm() {
    clearTimeout(armTimer);
    if (armed) { armed.textContent = armed.dataset.label; armed.classList.remove('armed'); }
    armed = null;
}
async function poll() {
    try {
        const response = await fetch(power.dataset.endpoint, {credentials: 'same-origin'});
        if (!response.ok) throw new Error(await response.text());
        state = await response.json();
        if (element('console-error').textContent.startsWith('Power state unavailable')) showError('');
    } catch (error) {
        state = null;
        showError(`Power state unavailable: ${error.message}`);
    }
    followPower();
    render();
}
async function send(button) {
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
    render();
    try {
        const response = await fetch(power.dataset.endpoint, {
            method: 'POST',
            credentials: 'same-origin',
            headers: {'Content-Type': 'application/json', 'X-CSRF-Token': power.dataset.csrf},
            body: JSON.stringify({action, confirm: disruptive}),
        });
        if (!response.ok) throw new Error(await response.text());
        showError('');
        status.textContent = `${button.dataset.label} sent.`;
        if (RECONNECTS.includes(action)) {
            startReconnecting();
        } else if (action === 'off') {
            stopReconnecting();
            resume = false;
        }
    } catch (error) {
        showError(error.message);
    } finally {
        busy = false;
        await poll();
    }
}
for (const button of buttons) button.addEventListener('click', () => send(button));

let polling = null;
function watch() {
    clearInterval(polling);
    polling = null;
    if (document.visibilityState === 'visible') {
        poll();
        polling = setInterval(poll, 3000);
    }
}
document.addEventListener('visibilitychange', watch);
watch();
