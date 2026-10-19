//! Where a ticket stands in its tracker (Swift's `TaskStage`): within one priority, To do orders by it, and a badge
//! says why when it's worth saying.
use crate::{badge, Badge, Tone};
use chrono::Duration;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// First to last: a comment for you, started, planned now, to do, triage, blocked, backlog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum TaskStage {
    Discussed,
    Started,
    Planned,
    Todo,
    Triage,
    Blocked,
    Backlog,
}

impl TaskStage {
    /// A status that puts started work aside: "Paused", "On hold", "En pause". It ranks as not started.
    pub fn paused(name: Option<&str>) -> bool {
        let Some(name) = name.map(str::to_lowercase) else { return false };
        name.contains("pause") || name.contains("hold")
    }

    /// Open for weeks: an urgent or high ticket unstarted this long says so.
    pub const LONG_OPEN: Duration = Duration::days(14);

    /// The badges that say where it stands. `sprint`: the tracker plans in sprints (Jira), not cycles (Linear).
    pub fn badges(discussed: bool, blocked: bool, planned: bool, long_open: bool, sprint: bool) -> Vec<Badge> {
        let mut badges = Vec::new();
        if discussed {
            badges.push(Badge::new(badge::DISCUSSED, "New comment", Tone::Accent));
        }
        if blocked {
            badges.push(Badge::new(badge::BLOCKED, "Blocked", Tone::Warning));
        }
        if planned {
            let label = if sprint { "Current sprint" } else { "Current cycle" };
            badges.push(Badge::new(badge::CYCLE, label, Tone::Neutral));
        }
        if long_open {
            badges.push(Badge::new(badge::AGE, "Open for weeks", Tone::Warning));
        }
        badges
    }
}
