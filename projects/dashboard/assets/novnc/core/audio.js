/*
 * noVNC: HTML5 VNC client
 * Copyright (C) 2025 Dietmar Maurer, 2026 the qrab authors
 * Licensed under MPL 2.0 (see LICENSE.txt)
 *
 * QEMU VNC audio playback (pseudo-encoding -259). Adapted for qrab from
 * novnc/noVNC#1952 "Qemu audio support v2" by Dietmar Maurer (MPL-2.0):
 * qrab requests S16 instead of U16 and plays through an AudioWorklet with a
 * bounded jitter buffer instead of scheduling AudioBufferSourceNodes.
 *
 * This file is both the page-side player and, loaded with
 * audioWorklet.addModule, the worklet processor, so a script-src 'self'
 * policy covers it.
 */

const PROCESSOR = 'novnc-qemu-audio';
export const SAMPLE_RATE = 44100;
export const CHANNELS = 2;
// Playback starts, and restarts after an underrun, once this much is buffered.
const PRIME_SECONDS = 0.06;
// Frames that arrive while the buffer holds this much are dropped.
const CAPACITY_SECONDS = 0.1;

if (typeof registerProcessor === 'function') {
    // eslint-disable-next-line no-undef
    registerProcessor(PROCESSOR, class extends AudioWorkletProcessor {
        constructor(options) {
            super();
            const { channels, capacity, prime } = options.processorOptions;
            this._channels = channels;
            this._capacity = capacity;
            this._prime = prime;
            this._ring = new Float32Array(capacity * channels);
            this._read = 0;
            this._length = 0;
            this._playing = false;
            this.port.onmessage = ({ data }) => (data === null ? this._reset() : this._push(data));
        }

        _reset() {
            this._read = 0;
            this._length = 0;
            this._playing = false;
        }

        // Interleaved S16 little-endian frames: [L0, R0, L1, R1, ...].
        _push(buffer) {
            const samples = new DataView(buffer);
            const frames = Math.floor(buffer.byteLength / (2 * this._channels));
            const take = Math.min(frames, this._capacity - this._length);
            let write = (this._read + this._length) % this._capacity;
            for (let f = 0; f < take; f++) {
                for (let c = 0; c < this._channels; c++) {
                    const sample = samples.getInt16((f * this._channels + c) * 2, true);
                    this._ring[write * this._channels + c] = sample / 32768;
                }
                write = (write + 1) % this._capacity;
            }
            this._length += take;
            if (this._length >= this._prime) {
                this._playing = true;
            }
        }

        process(inputs, outputs) {
            const output = outputs[0];
            if (!this._playing || output.length === 0) {
                return true;
            }
            for (let f = 0; f < output[0].length; f++) {
                if (this._length === 0) {
                    this._playing = false;
                    break;
                }
                for (let c = 0; c < output.length; c++) {
                    const channel = Math.min(c, this._channels - 1);
                    output[c][f] = this._ring[this._read * this._channels + channel];
                }
                this._read = (this._read + 1) % this._capacity;
                this._length--;
            }
            return true;
        }
    });
}

// Browsers only let an AudioContext play when it was created or resumed
// during a user gesture, so the page creates it in its click handler.
export function createAudioContext() {
    return new AudioContext({ latencyHint: 'interactive', sampleRate: SAMPLE_RATE });
}

export default class Audio {
    constructor(context) {
        this._context = context;
        this._node = null;
        this._loading = null;
        this._closed = false;
    }

    get sampleRate() { return SAMPLE_RATE; }
    get channels() { return CHANNELS; }

    // The server's audio-begin. Data before the worklet is ready is dropped.
    start() {
        if (!this._loading) {
            this._loading = this._context.audioWorklet.addModule(import.meta.url)
                .then(() => {
                    if (this._closed) {
                        return;
                    }
                    this._node = new AudioWorkletNode(this._context, PROCESSOR, {
                        numberOfInputs: 0,
                        outputChannelCount: [CHANNELS],
                        processorOptions: {
                            channels: CHANNELS,
                            capacity: Math.round(SAMPLE_RATE * CAPACITY_SECONDS),
                            prime: Math.round(SAMPLE_RATE * PRIME_SECONDS),
                        },
                    });
                    this._node.connect(this._context.destination);
                });
        }
        if (this._context.state === 'suspended') {
            this._context.resume().catch(() => {});
        }
        return this._loading;
    }

    // The server's audio-end: drop what is buffered.
    stop() {
        if (this._node) {
            this._node.port.postMessage(null);
        }
    }

    // One audio-data payload, which the player takes over.
    play(bytes) {
        if (!this._node || bytes.length === 0) {
            return;
        }
        const buffer = bytes.byteOffset === 0 && bytes.byteLength === bytes.buffer.byteLength
            ? bytes.buffer : bytes.slice().buffer;
        this._node.port.postMessage(buffer, [buffer]);
    }

    // The connection ended; the page owns and closes the AudioContext.
    close() {
        this._closed = true;
        if (this._node) {
            this._node.disconnect();
            this._node.port.close();
            this._node = null;
        }
    }
}
