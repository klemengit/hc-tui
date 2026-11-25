//! `[keys] recent_agent`: the agents in the order they were last focused,
//! stepped through like a browser's recent-tab switch. One press focuses the
//! agent focused before this one; presses within `[ui]
//! recent_agent_window_ms` of each other step further back without
//! reordering the list, and the agent the run ends on becomes the newest.
//! In memory only.

use std::sync::Arc;
use std::time::{Duration, Instant};

/// Where `recent_agent` reads the time; tests inject their own.
pub(super) type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

/// One run of quick presses.
struct Step {
    /// The agents to step through, most recent first, the agent focused
    /// when the run started last (so a full lap returns to it).
    ring: Vec<String>,
    /// The ring entry focused by the last press.
    pos: usize,
    last_press: Instant,
}

pub(super) struct RecentAgents {
    /// Agent pane ids, most recently focused first.
    order: Vec<String>,
    step: Option<Step>,
    /// Counts presses, so a key handler can tell whether its key was one.
    pub(super) presses: u64,
    pub(super) clock: Clock,
}

impl Default for RecentAgents {
    fn default() -> Self {
        Self {
            order: Vec::new(),
            step: None,
            presses: 0,
            clock: Arc::new(Instant::now),
        }
    }
}

impl RecentAgents {
    /// The list, most recent first.
    #[cfg(test)]
    pub(super) fn order(&self) -> &[String] {
        &self.order
    }

    fn touch(&mut self, id: &str) {
        self.order.retain(|known| known != id);
        self.order.insert(0, id.to_owned());
    }

    fn landed(&self) -> Option<&str> {
        self.step.as_ref().map(|step| step.ring[step.pos].as_str())
    }

    /// Focus is on agent `id`. The agent a run of presses landed on keeps
    /// the run going; any other agent ends it first.
    pub(super) fn focused(&mut self, id: &str) {
        if self.landed() == Some(id) {
            return;
        }
        self.end_step();
        self.touch(id);
    }

    /// End a run of presses: the agent it landed on becomes the newest.
    pub(super) fn end_step(&mut self) {
        if let Some(id) = self.landed().map(str::to_owned) {
            self.touch(&id);
        }
        self.step = None;
    }

    /// One press at `now`. `agents` is the current agent list (the set
    /// next/previous agent use) and `focused` the focused pane. Returns the
    /// agent to focus, or `None` when there is no other agent.
    pub(super) fn press(
        &mut self,
        agents: &[String],
        focused: Option<&str>,
        now: Instant,
        window: Duration,
    ) -> Option<String> {
        self.presses += 1;
        let chained = self.step.as_ref().is_some_and(|step| {
            now.saturating_duration_since(step.last_press) < window
                && Some(step.ring[step.pos].as_str()) == focused
        });
        if !chained {
            self.end_step();
            self.order.retain(|id| agents.contains(id));
            let others = |id: &&String| Some(id.as_str()) != focused;
            let mut ring: Vec<String> = self.order.iter().filter(others).cloned().collect();
            // Agents never focused follow, in list order.
            ring.extend(
                agents
                    .iter()
                    .filter(others)
                    .filter(|id| !self.order.contains(id))
                    .cloned(),
            );
            if ring.is_empty() {
                return None;
            }
            if let Some(current) = focused.filter(|id| agents.iter().any(|a| a == id)) {
                ring.push(current.to_owned());
            }
            self.step = Some(Step {
                pos: ring.len() - 1,
                ring,
                last_press: now,
            });
        }
        let step = self.step.as_mut()?;
        step.last_press = now;
        // The next ring entry that still exists; vanished agents are skipped.
        let len = step.ring.len();
        let next = (1..=len)
            .map(|offset| (step.pos + offset) % len)
            .find(|&i| agents.contains(&step.ring[i]))?;
        if Some(step.ring[next].as_str()) == focused {
            return None;
        }
        step.pos = next;
        Some(step.ring[next].clone())
    }
}
