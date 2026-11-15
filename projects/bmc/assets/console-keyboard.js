// Host focus handling around the SDK's canvas keyboard capture. Use DOM key-up
// events so noVNC releases its own tracked state without private SDK access.
export function wireConsoleKeyboard(canvas, {enabled, returnFocus, describedBy}) {
    const pressed = new Map();
    let ready = false;
    canvas.tabIndex = 0;
    canvas.setAttribute('aria-label', 'VM console display');
    canvas.setAttribute('aria-describedby', describedBy);

    const release = () => {
        const keys = [...pressed.values()].reverse();
        pressed.clear();
        for (const key of keys) canvas.dispatchEvent(new KeyboardEvent('keyup', {...key, bubbles: true, cancelable: true}));
    };
    const down = event => {
        if (!ready || !enabled()) return;
        // Keep right Alt/AltGr combinations available to the guest.
        const altGr = event.code === 'AltRight' || event.key === 'AltGraph'
            || [...pressed.values()].some(key => key.code === 'AltRight' || key.key === 'AltGraph');
        const exit = event.ctrlKey && event.altKey && event.shiftKey && !event.metaKey
            && ['Control', 'Alt', 'Shift'].includes(event.key)
            && !altGr;
        if (exit) {
            event.preventDefault();
            event.stopImmediatePropagation();
            release();
            canvas.blur();
            const leave = document.fullscreenElement?.contains(canvas)
                ? document.exitFullscreen().catch(() => {}) : Promise.resolve();
            leave.then(() => { if (enabled()) returnFocus(); });
        } else if (event.code) pressed.set(event.code, {key: event.key, code: event.code, location: event.location});
    };
    const up = event => pressed.delete(event.code);
    canvas.addEventListener('keydown', down, true);
    canvas.addEventListener('keyup', up, true);
    canvas.addEventListener('blur', release);
    window.addEventListener('blur', release);
    return {
        connected() { ready = true; },
        dispose() {
            release();
            ready = false;
            canvas.tabIndex = -1;
            canvas.removeEventListener('keydown', down, true);
            canvas.removeEventListener('keyup', up, true);
            canvas.removeEventListener('blur', release);
            window.removeEventListener('blur', release);
        },
    };
}
