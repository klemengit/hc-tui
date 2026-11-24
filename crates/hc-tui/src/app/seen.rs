// Derived from herdr-app src/window.rs `share_seen` (Apache-2.0).
//! The `herdr.seen` turn rule: while a pane with a status word is visible on
//! the active window, the client writes its current turn to the
//! `herdr.seen` label — once per turn, optimistic locally, never lowering a
//! turn the core already holds. Writes go over a dedicated RPC worker so a
//! failure only re-arms the pane, never raises an action notice.
use super::*;
use hc_proto::client::Client;
use hc_proto::watch::OpenRpc;
use serde_json::json;

/// A `terminal.label` request: `{"terminal": t, "labels": {"herdr.seen": turn}}`.
pub(super) struct SeenWrite {
    pub terminal: String,
    pub turn: u64,
}

pub(super) struct SeenResult {
    pub terminal: String,
    pub turn: u64,
    pub result: Result<(), String>,
}

/// The write worker: one cached client (reopened after any failure or
/// wire death), a 15s cap per write like the layout store. The returned
/// handle is the app's to abort — dropping it must never leak the
/// worker's `Client` clone.
fn spawn_seen_worker(
    open: OpenRpc,
    mut requests: mpsc::Receiver<SeenWrite>,
    results: mpsc::Sender<SeenResult>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut client = None;
        while let Some(request) = requests.recv().await {
            if client.as_ref().is_none_or(Client::is_closed) {
                client = open().await.ok();
            }
            let outcome = match &client {
                None => Err("core unavailable".to_owned()),
                Some(client) => tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    client.call(
                        "terminal.label",
                        Some(json!({
                            "terminal": request.terminal,
                            "labels": {"herdr.seen": request.turn.to_string()},
                        })),
                    ),
                )
                .await
                .map_err(|_| "label write timed out".to_owned())
                .and_then(|result| result.map(|_| ()).map_err(|e| e.to_string())),
            };
            if outcome.is_err() {
                client = None;
            }
            if results
                .send(SeenResult {
                    terminal: request.terminal,
                    turn: request.turn,
                    result: outcome,
                })
                .await
                .is_err()
            {
                break;
            }
        }
    })
}

/// The dwell clock of `[ui] seen_after_ms`: one focused, visible pane and
/// since when it has been continuously so.
pub(super) struct Dwell {
    pane: String,
    since: Instant,
    reached: bool,
}

impl App {
    /// Advance the dwell clock. With `seen_after_ms = 0` there is none.
    /// Otherwise it runs for the focused pane while that pane is visible and
    /// the outer terminal is not known unfocused; any other state, or a
    /// different pane, resets it. Returns true the moment the dwell is
    /// reached, so the caller redraws (the acknowledgement happens in the
    /// draw) and shares `herdr.seen` at that same moment.
    pub(super) fn tick_dwell(&mut self, now: Instant) -> bool {
        let dwell = std::time::Duration::from_millis(self.shell.seen_after_ms);
        if dwell.is_zero() {
            self.dwell = None;
            return false;
        }
        let candidate = self
            .focused
            .clone()
            .filter(|id| self.outer_focused != Some(false) && self.visible_panes().contains(id));
        let Some(pane) = candidate else {
            self.dwell = None;
            return false;
        };
        let current = self.dwell.get_or_insert_with(|| Dwell {
            pane: pane.clone(),
            since: now,
            reached: false,
        });
        if current.pane != pane {
            *current = Dwell {
                pane,
                since: now,
                reached: false,
            };
        }
        if !current.reached && now.saturating_duration_since(current.since) >= dwell {
            current.reached = true;
            return true;
        }
        false
    }

    /// When the running dwell expires; the loop wakes then.
    pub(super) fn dwell_deadline(&self) -> Option<Instant> {
        self.dwell
            .as_ref()
            .filter(|dwell| !dwell.reached)
            .map(|dwell| dwell.since + std::time::Duration::from_millis(self.shell.seen_after_ms))
    }

    /// Whether `pane` may be acknowledged as seen: always with no dwell,
    /// otherwise only the focused pane once its dwell is reached.
    pub(super) fn seen_dwell_met(&self, pane: &str) -> bool {
        self.shell.seen_after_ms == 0
            || self
                .dwell
                .as_ref()
                .is_some_and(|dwell| dwell.reached && dwell.pane == pane)
    }

    /// `set_operations` also starts the seen writer, once; a restart
    /// aborts the previous worker first.
    pub(super) fn start_seen_worker(&mut self, open: OpenRpc) {
        if let Some(requests) = self.seen_req_rx.take() {
            if let Some(task) = self.seen_task.take() {
                task.abort();
            }
            self.seen_task = Some(spawn_seen_worker(open, requests, self.seen_res_tx.clone()));
        }
    }

    /// `share_seen`: on the active window, each visible pane with a word
    /// and a turn the core hasn't already recorded gets its `herdr.seen`
    /// label written — at most once per turn. Markers for panes that left
    /// the screen are dropped so a revisit sees the pane fresh.
    pub(super) fn share_seen(&mut self) {
        if self.outer_focused == Some(false) {
            return;
        }
        let visible: std::collections::HashSet<String> = self
            .visible_panes()
            .into_iter()
            .filter(|id| self.seen_dwell_met(id))
            .collect();
        self.seen_looked.retain(|id, _| visible.contains(id));
        for id in visible {
            let Some(term) = self.view.inventory.get(&id) else {
                continue;
            };
            // No word, not live, or already looked at this turn — skip.
            if crate::project::effective_status(term).is_none() || term.state != "live" {
                continue;
            }
            // The core's own record is authoritative: never lower it.
            let core_seen: u64 = term
                .labels
                .get("herdr.seen")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0)
                .max(*self.seen_looked.get(&id).unwrap_or(&0));
            if core_seen >= term.turn {
                self.seen_looked
                    .insert(id.clone(), core_seen.max(term.turn));
                continue;
            }
            // Optimistic: record the turn before the write so a republish
            // cannot re-arm the same one.
            self.seen_looked.insert(id.clone(), term.turn);
            if !self.seen_inflight.insert((id.clone(), term.turn)) {
                continue;
            }
            if self
                .seen_tx
                .try_send(SeenWrite {
                    terminal: id.clone(),
                    turn: term.turn,
                })
                .is_err()
            {
                self.seen_inflight.remove(&(id.clone(), term.turn));
            }
        }
    }

    /// A finished write clears `inflight`; a failed one also clears the
    /// optimistic marker so the next publish retries — silently, like the
    /// reference's `warn!`.
    pub(super) fn on_seen_result(&mut self, result: SeenResult) {
        self.seen_inflight
            .remove(&(result.terminal.clone(), result.turn));
        if result.result.is_err() && self.seen_looked.get(&result.terminal) == Some(&result.turn) {
            self.seen_looked.remove(&result.terminal);
        }
    }
}

impl Drop for App {
    /// Dropping the app aborts the seen writer, the probe worker and
    /// the reaper — a detached worker would keep its `Client` clone
    /// (and any ssh child) alive past the app's lifetime.
    fn drop(&mut self) {
        if let Some(task) = self.seen_task.take() {
            task.abort();
        }
        if let Some(task) = self.waiting_task.take() {
            task.abort();
        }
        if let Some(task) = self.reap_task.take() {
            task.abort();
        }
        if let Some(task) = self.image_read_task.take() {
            task.abort();
        }
    }
}
