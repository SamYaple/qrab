//! Central client-side store.
//!
//! Replaces per-component polling. One `EventSource` is opened once, at
//! `App` mount, and every server-pushed frame (`BmcPush`) is dispatched
//! into signals that every panel reads via `use_bmc_store()`.
//!
//! Before this refactor the UI polled five different `#[server]` endpoints
//! at 500 ms–3 s intervals (dashboard, qmp queue x2, config, event history
//! seed). That traffic is gone: the server pushes snapshot + deltas over
//! SSE and the browser re-renders when the signal values change.

use std::collections::VecDeque;

use dioxus::prelude::*;

use bmc::types::{BmcConfig, BmcEvent, BmcPush, BmcSnapshot, DashboardState, QueueStatus};

/// Max events retained client-side. Mirrors `EVENT_HISTORY_CAP` in
/// `state.rs` so the UI never holds more than the server would have
/// emitted.
const EVENT_HISTORY_CAP: usize = 1000;

/// Ambient store. Signals are `Copy` so this whole struct is `Copy` and
/// can be passed by value from the context without cloning.
#[derive(Clone, Copy)]
pub struct BmcStore {
    pub dashboard: Signal<DashboardState>,
    pub queue: Signal<QueueStatus>,
    pub config: Signal<BmcConfig>,
    pub events: Signal<VecDeque<BmcEvent>>,
}

impl BmcStore {
    fn new() -> Self {
        Self {
            dashboard: Signal::new(DashboardState::default()),
            queue: Signal::new(QueueStatus::default()),
            config: Signal::new(BmcConfig::default()),
            events: Signal::new(VecDeque::new()),
        }
    }
}

/// Install the store + SSE subscriber. Call exactly once from `App`.
///
/// Returns the store so `App` can also read from it if it ever needs to
/// (usually components grab it via `use_bmc_store`).
pub fn provide_bmc_store() -> BmcStore {
    let store = use_context_provider(BmcStore::new);
    use_future(move || async move { sse_loop(store).await });
    store
}

/// Hook: grab the ambient store. Panics if the provider was not
/// installed at a parent scope (always safe under `App` root).
pub fn use_bmc_store() -> BmcStore {
    use_context::<BmcStore>()
}

/// SSE receiver. Installs an `EventSource` on `/api/events` via
/// `document::eval`, pipes parsed JSON back via `dioxus.send`, and
/// dispatches each `BmcPush` frame into the shared signals.
///
/// Browser auto-reconnect covers transient drops; on reconnect the
/// server sends a fresh `Snapshot` frame first so the store fully
/// resyncs without a manual refetch.
async fn sse_loop(store: BmcStore) {
    let mut eval_handle = document::eval(
        r#"
        (function() {
            if (window._bmcEventSource) {
                try { window._bmcEventSource.close(); } catch (e) {}
                window._bmcEventSource = null;
            }
            const es = new EventSource('/api/events');
            window._bmcEventSource = es;
            es.onmessage = (e) => {
                try {
                    const obj = JSON.parse(e.data);
                    dioxus.send(obj);
                } catch (err) {
                    console.error('SSE parse error:', err);
                }
            };
            es.onerror = (err) => {
                console.warn('SSE error (browser will auto-reconnect):', err);
            };
        })();
        "#,
    );

    loop {
        match eval_handle.recv::<BmcPush>().await {
            Ok(push) => dispatch(store, push),
            Err(e) => {
                tracing::warn!("SSE recv error: {e}");
                break;
            }
        }
    }
}

/// Apply a single push frame to the store. Scoped write guards so we
/// never hold a RefCell write across an await (no awaits here, but the
/// pattern keeps future extensions safe).
fn dispatch(mut store: BmcStore, push: BmcPush) {
    match push {
        BmcPush::Snapshot(BmcSnapshot {
            dashboard,
            queue,
            config,
            events,
        }) => {
            store.dashboard.set(dashboard);
            store.queue.set(queue);
            store.config.set(config);
            let mut buf = VecDeque::with_capacity(events.len().min(EVENT_HISTORY_CAP));
            for ev in events.into_iter().rev().take(EVENT_HISTORY_CAP).rev() {
                buf.push_back(ev);
            }
            store.events.set(buf);
        }
        BmcPush::Dashboard(d) => store.dashboard.set(d),
        BmcPush::Queue(q) => store.queue.set(q),
        BmcPush::Config(c) => store.config.set(c),
        BmcPush::QmpEvent(ev) => {
            let mut buf = store.events.write();
            buf.push_back(ev);
            while buf.len() > EVENT_HISTORY_CAP {
                buf.pop_front();
            }
        }
        BmcPush::Heartbeat => {
            // No state change — only liveness signal. Browsers reset
            // their reconnect timer on any frame, so receiving this
            // keeps the connection alive without any store mutation.
        }
    }
}
