use remora_core::badge;
use remora_core::{Badge, Tone};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Checks {
    Passing,
    Failing,
    Running,
    None,
}

/// Provider-neutral view of a pull/merge request, turned into badges the same way everywhere.
pub struct CodeReview {
    pub is_draft: bool,
    pub is_approved: bool,
    pub changes_requested: bool,
    pub has_conflicts: bool,
    pub checks: Checks,
    pub comments: u32,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
}

impl CodeReview {
    /// Review requests always need you; your own MR only when it's blocked or ready to merge.
    pub fn needs_action(&self, authored: bool) -> bool {
        if self.is_draft {
            return false;
        }
        !authored || self.is_approved || self.changes_requested || self.has_conflicts || self.checks == Checks::Failing
    }

    pub fn badges(&self, authored: bool) -> Vec<Badge> {
        let notify = |badge: Badge, title: &str| if authored { badge.notifying(title) } else { badge };
        let mut badges = vec![];
        if self.is_draft {
            badges.push(Badge::new(badge::DRAFT, "Draft", Tone::Neutral));
        }
        if self.is_approved {
            badges.push(notify(Badge::new(badge::APPROVED, "Approved", Tone::Accent), "Approved"));
        }
        if self.changes_requested {
            badges.push(notify(Badge::new(badge::CHANGES, "Changes requested", Tone::Negative), "Changes requested"));
        }
        if self.has_conflicts {
            badges.push(Badge::new(badge::CONFLICTS, "Conflicts", Tone::Warning));
        }
        match self.checks {
            Checks::Passing => badges.push(Badge::new(badge::CHECKS_PASSING, "Checks", Tone::Positive)),
            Checks::Failing => {
                badges.push(notify(Badge::new(badge::CHECKS_FAILING, "Checks failed", Tone::Negative), "Checks failed"))
            }
            Checks::Running => badges.push(Badge::new(badge::CHECKS_RUNNING, "Running", Tone::Warning)),
            Checks::None => {}
        }
        if self.comments > 0 {
            badges.push(Badge::new(badge::COMMENTS, &self.comments.to_string(), Tone::Neutral));
        }
        if let (Some(additions), Some(deletions)) = (self.additions, self.deletions) {
            badges.push(Badge::new(badge::DIFF, &format!("+{additions} −{deletions}"), Tone::Neutral));
        }
        badges
    }
}
