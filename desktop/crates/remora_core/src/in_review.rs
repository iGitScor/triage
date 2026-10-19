//! A ticket up for review: its status says so, or a pull request for it is still open (Swift's `InReview`). It then
//! waits on the reviewers, not on you.
use crate::{InboxBundle, InboxItem};

/// A review status, in English or French: "In Review", "Code review", "En revue", "Relecture".
pub fn status(name: Option<&str>) -> bool {
    let Some(name) = name.map(str::to_lowercase) else { return false };
    ["review", "revue", "relecture"].iter().any(|word| name.contains(word))
}

/// A pull or merge request's page: GitHub's `/pull/9`, GitLab's `/-/merge_requests/3`, Bitbucket's
/// `/pull-requests/4`, Azure DevOps's `/pullrequest/12`. Not `/pull/new` nor an issue.
pub fn is_change(url: &str) -> bool {
    let path = host_and_path(url).1.to_lowercase();
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    parts.windows(2).any(|pair| {
        ["pull", "merge_requests", "pull-requests", "pullrequest"].contains(&pair[0])
            && pair[1].chars().all(|c| c.is_ascii_digit())
    })
}

/// A link's host and path, without its scheme, port, query or anchor: "https://GitHub.com:443/a/b?x#y" →
/// ("GitHub.com", "/a/b").
pub(crate) fn host_and_path(url: &str) -> (&str, &str) {
    let url = url.split(['?', '#']).next().unwrap_or("");
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let (authority, path) = rest.find('/').map_or((rest, ""), |slash| rest.split_at(slash));
    let host = authority.rsplit('@').next().unwrap_or("");
    (host.split(':').next().unwrap_or(""), path)
}

impl InboxItem {
    /// Waiting on another ticket that blocks it: in Waiting until that one is done, when it comes back by itself.
    pub fn blocked_by_another(mut self) -> InboxItem {
        self.bundle = InboxBundle::blocked();
        self.needs_action = false;
        self
    }

    /// Up for review: in Awaiting, nothing to do until the reviewers answer, tied to its pull requests when known.
    pub fn waiting_on_review(mut self, changes: Vec<String>) -> InboxItem {
        self.bundle = InboxBundle::awaiting();
        self.needs_action = false;
        if !changes.is_empty() {
            self.related.get_or_insert_with(Vec::new).extend(changes);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;

    /// Review statuses in English and French, not work in progress. As macOS.
    #[test]
    fn review_statuses() {
        for name in ["In Review", "Code review", "En revue", "Relecture"] {
            assert!(status(Some(name)), "{name}");
        }
        assert!(!status(Some("In Progress")));
        assert!(!status(None));
    }

    /// Pull and merge requests of every forge, not issues nor the page that opens a new one. As macOS.
    #[test]
    fn changes_of_every_forge() {
        for url in [
            "https://github.com/acme/app/pull/9",
            "https://gitlab.com/acme/app/-/merge_requests/3",
            "https://bitbucket.org/acme/app/pull-requests/4",
            "https://dev.azure.com/acme/web/_git/app/pullrequest/12",
        ] {
            assert!(is_change(url), "{url}");
        }
        assert!(!is_change("https://github.com/acme/app/issues/9"));
        assert!(!is_change("https://github.com/acme/app/pull/new"));
    }

    /// Awaiting, nothing to do, its pull requests added to what it was already tied to. As macOS.
    #[test]
    fn waiting_on_review_keeps_what_it_was_tied_to() {
        let mut task = item("1", InboxBundle::tasks());
        task.needs_action = true;
        task.related = Some(vec!["https://a".into()]);
        let waiting = task.clone().waiting_on_review(vec!["https://b".into()]);
        assert_eq!(waiting.bundle, InboxBundle::awaiting());
        assert!(!waiting.needs_action);
        assert_eq!(waiting.related, Some(vec!["https://a".to_string(), "https://b".to_string()]));
        task.related = None;
        assert_eq!(task.waiting_on_review(vec![]).related, None, "nothing to tie it to");
    }
}
