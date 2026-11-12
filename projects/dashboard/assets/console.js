// The only client-side script: noVNC against the dashboard's console proxy.
import RFB from './novnc/core/rfb.js';

const element = id => document.getElementById(id);
const screen = element('screen');
const status = element('console-status');
const connect = element('connect');
let rfb = null;

function showError(message) {
    element('console-error').textContent = message;
    element('console-error').hidden = !message;
}
function disconnected(message) {
    rfb = null;
    connect.textContent = 'Connect console';
    status.textContent = message;
    element('cad').disabled = true;
    element('fullscreen').disabled = true;
}
connect.addEventListener('click', () => {
    if (rfb) { rfb.disconnect(); return; }
    showError('');
    screen.replaceChildren();
    try {
        const scheme = location.protocol === 'https:' ? 'wss:' : 'ws:';
        rfb = new RFB(screen, `${scheme}//${location.host}${screen.dataset.socket}`);
        rfb.scaleViewport = true;
        rfb.resizeSession = false;
        rfb.focusOnClick = true;
        rfb.showDotCursor = true;
        rfb.addEventListener('connect', () => {
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
});
element('cad').addEventListener('click', () => rfb?.sendCtrlAltDel());
element('fullscreen').addEventListener('click', () => screen.requestFullscreen().catch(error => showError(error.message)));
