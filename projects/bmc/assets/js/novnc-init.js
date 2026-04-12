// noVNC initializer. Loaded as an ES module from /static/js/novnc-init.js
// and invoked by the Rust Console component once the backend reports the
// console is available.
//
// Exposes window._novncInit() which performs the actual RFB connection.
// The detailed console.log statements mirror the inline debug logging that
// used to live in components/console.rs; they help diagnose connection
// issues in the browser dev console.

import RFB from '/novnc/core/rfb.js';

window._novncInit = function _novncInit() {
    try {
        if (window._rfb) {
            console.log('noVNC: already connected');
            return;
        }
        const container = document.getElementById('vnc-container');
        if (!container) {
            console.error('noVNC: #vnc-container not in DOM');
            return;
        }
        const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const url = `${proto}//${window.location.host}/api/console`;
        console.log('noVNC: connecting to', url);
        const rfb = new RFB(container, url, { scaleViewport: true });
        window._rfb = rfb;
        rfb.addEventListener('connect', () => {
            console.log('noVNC: connected, resolution', rfb._fbWidth + 'x' + rfb._fbHeight);
        });
        rfb.addEventListener('disconnect', (e) => {
            console.log('noVNC: disconnect', e.detail);
            window._rfb = null;
        });
        rfb.addEventListener('securityfailure', (e) => {
            console.error('noVNC: security failure', e.detail);
        });
        console.log('noVNC: RFB created, waiting for connect event...');
    } catch (e) {
        console.error('noVNC: EXCEPTION:', e.message, e.stack);
    }
};
