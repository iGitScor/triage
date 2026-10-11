//! What a review will take, computed locally from file paths and line counts (never code). Port of the macOS
//! `ReviewPrep.swift`, with `ReviewQueue`, `ReviewTiming` and `ReviewPace`.
//!
//! Text: the core can't translate, so the functions that build a sentence take a translator
//! (`&dyn Fn(&str) -> String`, English key → the language's format string, e.g. `|k| translator.t(k)`) and fill in
//! the `%d`/`%@` placeholders themselves. The keys are the Swift ones, so the shared French dictionary applies.
use crate::{ChangedFile, InboxItem, Prioritizer};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReviewSize {
    Tiny,
    Small,
    Medium,
    Large,
}

/// Areas that deserve extra attention, read from the paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReviewFlag {
    Migrations,
    Auth,
    PersonalData,
    Infra,
    Dependencies,
    LockfileOnly,
}

impl ReviewFlag {
    /// In display order, as Swift's `allCases`.
    pub const ALL: [ReviewFlag; 6] = [
        ReviewFlag::Migrations,
        ReviewFlag::Auth,
        ReviewFlag::PersonalData,
        ReviewFlag::Infra,
        ReviewFlag::Dependencies,
        ReviewFlag::LockfileOnly,
    ];

    /// English key, translated by the UI.
    pub fn title(self) -> &'static str {
        match self {
            ReviewFlag::Migrations => "Migrations",
            ReviewFlag::Auth => "Auth",
            ReviewFlag::PersonalData => "Personal data",
            ReviewFlag::Infra => "Infra/CI",
            ReviewFlag::Dependencies => "Dependencies",
            ReviewFlag::LockfileOnly => "Lockfile only",
        }
    }

    /// The SF Symbol the Mac shows, for the Windows UI to map to its own icons.
    pub fn symbol(self) -> &'static str {
        match self {
            ReviewFlag::Migrations => "cylinder.split.1x2",
            ReviewFlag::Auth => "key",
            ReviewFlag::PersonalData => "person.text.rectangle",
            ReviewFlag::Infra => "server.rack",
            ReviewFlag::Dependencies => "shippingbox",
            ReviewFlag::LockfileOnly => "lock.doc",
        }
    }

    /// Words in a path that flag an area: whole words of the path, so `auth` is in `src/auth/` and
    /// `OAuthClient`, not in "author".
    fn words(self) -> &'static [&'static str] {
        match self {
            ReviewFlag::Migrations => &["migration", "migrations", "migrate", "db", "schema"],
            ReviewFlag::Auth => &[
                "auth",
                "oauth",
                "authn",
                "authz",
                "authentication",
                "authorization",
                "login",
                "logout",
                "session",
                "sessions",
                "permission",
                "permissions",
                "rbac",
                "sso",
                "saml",
                "jwt",
            ],
            ReviewFlag::PersonalData => &["privacy", "gdpr", "consent", "pii", "personal"],
            ReviewFlag::Infra => {
                &["docker", "dockerfile", "terraform", "helm", "k8s", "kubernetes", "workflows", "env"]
            }
            ReviewFlag::Dependencies | ReviewFlag::LockfileOnly => &[],
        }
    }
}

/// Manifests, by file name.
const MANIFESTS: &[&str] = &[
    "package.json",
    "go.mod",
    "gemfile",
    "requirements.txt",
    "podfile",
    "cargo.toml",
    "pyproject.toml",
    "package.swift",
    "build.gradle",
    "pom.xml",
];
const TEST_WORDS: &[&str] = &["test", "tests", "spec", "specs", "testing"];
const LOCKFILES: &[&str] = &[
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "go.sum",
    "gemfile.lock",
    "podfile.lock",
    "cargo.lock",
    "poetry.lock",
    "package.resolved",
    "composer.lock",
    "flake.lock",
];
/// Folders and suffixes of files nobody reviews line by line: they don't count in the estimate.
const GENERATED_FOLDERS: &[&str] =
    &["dist", "build", "vendor", "node_modules", "pods", "__generated__", "generated", "__snapshots__"];
const GENERATED_SUFFIXES: &[&str] =
    &[".min.js", ".min.css", ".map", ".snap", ".pb.go", ".g.dart", ".generated.ts", ".pbxproj", "_pb2.py"];

static CAMEL_HUMP: LazyLock<Regex> = LazyLock::new(|| Regex::new("([a-z0-9])([A-Z])").expect("valid regex"));

/// What a review will take. Built only for items with a file list.
#[derive(Clone, Debug, PartialEq)]
pub struct ReviewPrep {
    /// None when the source only gives file paths (GitLab): the estimate then uses the file count.
    pub lines: Option<u32>,
    pub file_count: usize,
    pub size: ReviewSize,
    pub estimated_minutes: u32,
    pub tests_touched: bool,
    pub flags: Vec<ReviewFlag>,
    pub top_files: Vec<ChangedFile>,
}

impl ReviewPrep {
    /// An estimate for a review Remora knows nothing about (no file list).
    pub const UNKNOWN_MINUTES: u32 = 10;

    /// `pace`: how your reviews compare with the estimate, learned from timed ones (`ReviewPace`); 1 until then.
    pub fn new(item: &InboxItem, pace: f64) -> Option<ReviewPrep> {
        let changes = item.changes.as_ref().filter(|c| !c.files.is_empty())?;
        let paths: Vec<String> = changes.files.iter().map(|f| f.path.to_lowercase()).collect();
        // Lockfiles and generated files are listed but not reviewed: they don't make a review longer.
        let reviewable: Vec<&ChangedFile> = changes.files.iter().filter(|f| !is_generated(&f.path)).collect();
        let generated = changes.files.len() - reviewable.len();
        let counted = changes.files.iter().any(|f| f.additions.is_some() || f.deletions.is_some());
        // The total from the tool, when there are no per-file counts, can't leave generated lines out.
        let lines = if counted { Some(reviewable.iter().map(|f| f.lines()).sum()) } else { diff_size(item) };
        let file_count = changes.file_count;
        let reviewable_count = changes.file_count.saturating_sub(generated) as u32;
        let (size, estimate) = if reviewable_count == 0 {
            (ReviewSize::Tiny, 2)
        } else if let Some(lines) = lines {
            let size = match lines {
                0..20 => ReviewSize::Tiny,
                20..150 => ReviewSize::Small,
                150..500 => ReviewSize::Medium,
                _ => ReviewSize::Large,
            };
            (size, 60.min((2 + lines / 40).max(reviewable_count.div_ceil(3))))
        } else {
            let size = match reviewable_count {
                ..=2 => ReviewSize::Small,
                3..=8 => ReviewSize::Medium,
                _ => ReviewSize::Large,
            };
            (size, 60.min(2 + reviewable_count * 2))
        };
        let estimated_minutes = paced(estimate, pace);
        let tests_touched = changes.files.iter().any(|f| has_any(&words_of(&f.path), TEST_WORDS));

        let only_lockfiles = paths.iter().all(|p| LOCKFILES.iter().any(|l| p.ends_with(l)));
        let flags = if only_lockfiles {
            vec![ReviewFlag::LockfileOnly]
        } else {
            let path_words: Vec<HashSet<String>> = changes.files.iter().map(|f| words_of(&f.path)).collect();
            let names: Vec<&str> = paths.iter().map(|p| p.rsplit('/').next().unwrap_or(p)).collect();
            ReviewFlag::ALL
                .into_iter()
                .filter(|flag| match flag {
                    ReviewFlag::Dependencies => names.iter().any(|n| MANIFESTS.contains(n)),
                    ReviewFlag::LockfileOnly => false,
                    _ => path_words.iter().any(|w| has_any(w, flag.words())),
                })
                .collect()
        };
        let mut top: Vec<&ChangedFile> = reviewable;
        top.sort_by_key(|f| std::cmp::Reverse(f.lines()));
        let top_files = top.into_iter().take(3).cloned().collect();
        Some(ReviewPrep { lines, file_count, size, estimated_minutes, tests_touched, flags, top_files })
    }

    /// "~6 min · 4 files", the rest is shown as chips. `tr` maps an English key to the UI language's format.
    pub fn summary(&self, tr: &dyn Fn(&str) -> String) -> String {
        let files = plural("%d file", "%d files", self.file_count as i64, tr);
        fill(&tr("~%d min · %@"), &[self.estimated_minutes.to_string(), files])
    }
}

/// A lockfile or a generated file: shown in the count, left out of the estimate.
pub fn is_generated(path: &str) -> bool {
    let path = path.to_lowercase();
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    LOCKFILES.iter().any(|l| path.ends_with(l))
        || GENERATED_SUFFIXES.iter().any(|s| path.ends_with(s))
        || segments.split_last().is_some_and(|(_, folders)| folders.iter().any(|f| GENERATED_FOLDERS.contains(f)))
}

/// The words of a path: split at anything but letters and digits, and at camelCase humps.
fn words_of(path: &str) -> HashSet<String> {
    let spaced = CAMEL_HUMP.replace_all(path, "$1 $2").to_lowercase();
    spaced.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(String::from).collect()
}

fn has_any(words: &HashSet<String>, wanted: &[&str]) -> bool {
    wanted.iter().any(|w| words.contains(*w))
}

fn paced(minutes: u32, pace: f64) -> u32 {
    ((minutes as f64 * pace).round() as u32).max(1)
}

/// Lines changed, read from the "+a −d" badge (Swift's `InboxItem.diffSize`).
pub(crate) fn diff_size(item: &InboxItem) -> Option<u32> {
    let label = &item.badges.iter().find(|b| b.id == "diff")?.label;
    let numbers: Vec<u32> =
        label.split(|c: char| !c.is_numeric()).filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect();
    if numbers.is_empty() {
        None
    } else {
        Some(numbers.iter().sum())
    }
}

/// A count with the right form: `singular` for one, `plural` otherwise. English rule; French also says "0 élément",
/// which only differs at 0, and every count passed here (files of a non-empty list, days waited) is at least 1.
pub(crate) fn plural(singular: &str, plural: &str, count: i64, tr: &dyn Fn(&str) -> String) -> String {
    fill(&tr(if count == 1 { singular } else { plural }), &[count.to_string()])
}

/// Fills a format string's `%d`/`%@` (or positional `%1$@`) in order, as `String(format:)` does; `%%` is a percent.
pub(crate) fn fill(format: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(format.len());
    let mut chars = format.chars().peekable();
    let mut next = 0;
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut digits = String::new();
        while let Some(d) = chars.peek().filter(|d| d.is_ascii_digit()) {
            digits.push(*d);
            chars.next();
        }
        let index = if !digits.is_empty() && chars.peek() == Some(&'$') {
            chars.next();
            digits.parse::<usize>().ok().and_then(|n| n.checked_sub(1))
        } else {
            None
        };
        match chars.peek() {
            Some('d') | Some('@') if index.is_some() || digits.is_empty() => {
                chars.next();
                let i = index.unwrap_or_else(|| {
                    next += 1;
                    next - 1
                });
                out.push_str(args.get(i).map(String::as_str).unwrap_or(""));
            }
            Some('%') if digits.is_empty() => {
                chars.next();
                out.push('%');
            }
            _ => {
                out.push('%');
                out.push_str(&digits);
            }
        }
    }
    out
}

/// The order of a review session: pressing first, then quick wins, then whoever waited longest.
pub struct ReviewQueue;

impl ReviewQueue {
    pub fn order(items: &[InboxItem], now: DateTime<Utc>) -> Vec<InboxItem> {
        let prioritizer = Prioritizer::new(now);
        let mut sorted = items.to_vec();
        sorted.sort_by(|a, b| {
            prioritizer
                .is_pressing(b)
                .cmp(&prioritizer.is_pressing(a))
                .then(Self::minutes(a, 1.0).cmp(&Self::minutes(b, 1.0)))
                .then(a.date.cmp(&b.date))
        });
        sorted
    }

    pub fn remaining_minutes(items: &[InboxItem], pace: f64) -> u32 {
        items.iter().map(|i| Self::minutes(i, pace)).sum()
    }

    /// One default for a review without a file list, wherever it is counted.
    pub fn minutes(item: &InboxItem, pace: f64) -> u32 {
        ReviewPrep::new(item, pace).map_or_else(|| paced(ReviewPrep::UNKNOWN_MINUTES, pace), |p| p.estimated_minutes)
    }
}

/// How long a review you timed really took, against its estimate. Kept on this machine only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewTiming {
    pub estimated: i64,
    pub actual: i64,
}

/// Your pace from timed reviews: the median of actual over estimated, from 5 of them, between half and three times.
/// A review left running for hours, or done in under a minute, says nothing and is left out.
pub struct ReviewPace;

impl ReviewPace {
    pub const MINIMUM_TIMINGS: usize = 5;

    pub fn factor(timings: &[ReviewTiming]) -> f64 {
        let recent = &timings[timings.len().saturating_sub(20)..];
        let mut ratios: Vec<f64> = recent
            .iter()
            .filter(|t| t.estimated > 0 && (1..=240).contains(&t.actual))
            .map(|t| t.actual as f64 / t.estimated as f64)
            .collect();
        if ratios.len() < Self::MINIMUM_TIMINGS {
            return 1.0;
        }
        ratios.sort_by(f64::total_cmp);
        ratios[ratios.len() / 2].clamp(0.5, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::*;
    use crate::{ChangeSet, InboxBundle};
    use chrono::Duration;

    fn english(key: &str) -> String {
        key.to_string()
    }

    fn review(id: &str, files: &[(&str, u32)], hours_ago: i64) -> InboxItem {
        let mut item = item(id, InboxBundle::reviews());
        item.date = now() - Duration::hours(hours_ago);
        let files = files
            .iter()
            .map(|(path, lines)| ChangedFile { path: path.to_string(), additions: Some(*lines), deletions: Some(0) })
            .collect();
        item.changes = Some(ChangeSet::new(files, None));
        item
    }

    fn prep(files: &[(&str, u32)]) -> ReviewPrep {
        ReviewPrep::new(&review("1", files, 1), 1.0).unwrap()
    }

    fn ids(items: &[InboxItem]) -> Vec<&str> {
        items.iter().map(|i| i.id.as_str()).collect()
    }

    #[test]
    fn sizes_estimates_and_tests() {
        let p = prep(&[("src/api/retry.ts", 40), ("src/api/retry.test.ts", 30)]);
        assert!(p.size == ReviewSize::Small && p.lines == Some(70) && p.file_count == 2);
        assert_eq!(p.estimated_minutes, 3);
        assert!(p.tests_touched);
        assert_eq!(p.summary(&english), "~3 min · 2 files");
        assert_eq!(prep(&[("a.ts", 900)]).size, ReviewSize::Large);
        assert!(ReviewPrep::new(&item("3", InboxBundle::reviews()), 1.0).is_none(), "no files, no prep");
    }

    #[test]
    fn flags_come_from_paths() {
        let p = prep(&[
            ("db/migrations/2026_add_avatars.sql", 20),
            ("src/privacy/consent.ts", 50),
            ("src/auth/session.ts", 5),
            (".github/workflows/ci.yml", 3),
            ("package.json", 1),
        ]);
        use ReviewFlag::*;
        assert_eq!(p.flags, [Migrations, Auth, PersonalData, Infra, Dependencies]);
        assert!(!p.tests_touched);
        assert_eq!(p.top_files[0].path, "src/privacy/consent.ts");
    }

    #[test]
    fn lockfile_only_is_its_own_flag() {
        assert_eq!(prep(&[("package-lock.json", 800), ("yarn.lock", 40)]).flags, [ReviewFlag::LockfileOnly]);
    }

    #[test]
    fn session_does_quick_wins_first_then_oldest() {
        let big = review("big", &[("a.ts", 600)], 48);
        let quick_old = review("quickOld", &[("b.ts", 10)], 30);
        let quick_new = review("quickNew", &[("c.ts", 10)], 2);
        let mut overdue = review("overdue", &[("d.ts", 400)], 1);
        overdue.due = Some(now() - Duration::days(1));
        let ordered = ReviewQueue::order(&[big, quick_new, overdue, quick_old], now());
        assert_eq!(ids(&ordered), ["overdue", "quickOld", "quickNew", "big"]);
    }

    #[test]
    fn paths_only_estimate_from_file_count() {
        let mut item = item("1", InboxBundle::reviews());
        let files = (1..=5).map(|i| ChangedFile { path: format!("src/f{i}.ts"), ..Default::default() }).collect();
        item.changes = Some(ChangeSet::new(files, None));
        let p = ReviewPrep::new(&item, 1.0).unwrap();
        assert!(p.lines.is_none() && p.size == ReviewSize::Medium && p.estimated_minutes == 12);
    }

    /// Without per-file counts, the "+a −d" badge gives the lines.
    #[test]
    fn paths_only_use_the_diff_badge() {
        let mut item = item("1", InboxBundle::reviews());
        item.badges = vec![crate::Badge::new("diff", "+12 −3", crate::Tone::Neutral)];
        item.changes = Some(ChangeSet::new(vec![ChangedFile { path: "a.ts".into(), ..Default::default() }], None));
        assert_eq!(diff_size(&item), Some(15));
        assert_eq!(ReviewPrep::new(&item, 1.0).unwrap().lines, Some(15));
    }

    /// Lockfiles and generated files don't make a review longer.
    #[test]
    fn generated_files_dont_count() {
        let lockfile_only = prep(&[("package-lock.json", 2_400)]);
        assert!(lockfile_only.estimated_minutes == 2 && lockfile_only.size == ReviewSize::Tiny, "not ~60 min");
        let mixed = prep(&[
            ("src/api.ts", 40),
            ("yarn.lock", 900),
            ("dist/app.min.js", 3_000),
            ("src/__snapshots__/api.test.ts.snap", 200),
        ]);
        assert!(
            mixed.lines == Some(40) && mixed.file_count == 4,
            "every file listed, only the reviewable ones counted"
        );
        assert_eq!(mixed.estimated_minutes, 3);
        assert_eq!(mixed.top_files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["src/api.ts"]);
    }

    /// Flags and tests match words of the path, not fragments.
    #[test]
    fn flags_match_words_not_fragments() {
        let lookalikes = prep(&[("src/author/latest.ts", 10), ("src/personality.ts", 10), ("docs/environment.md", 5)]);
        assert!(lookalikes.flags.is_empty() && !lookalikes.tests_touched);
        let real = prep(&[
            ("Sources/OAuthClient.swift", 10),
            ("Tests/RetryTests.swift", 10),
            (".env.example", 1),
            ("Package.swift", 1),
        ]);
        assert_eq!(real.flags, [ReviewFlag::Auth, ReviewFlag::Infra, ReviewFlag::Dependencies]);
        assert!(real.tests_touched);
    }

    /// One default for a review without a file list.
    #[test]
    fn unknown_reviews_share_one_default() {
        let unknown = item("u", InboxBundle::reviews());
        assert_eq!(ReviewQueue::remaining_minutes(std::slice::from_ref(&unknown), 1.0), ReviewPrep::UNKNOWN_MINUTES);
        let quick = review("q", &[("a.ts", 10)], 1);
        assert_eq!(ids(&ReviewQueue::order(&[unknown, quick], now())), ["q", "u"], "ordered with the same default");
    }

    /// From five timed reviews on, estimates follow your pace.
    #[test]
    fn estimates_learn_your_pace() {
        let timing = |actual| ReviewTiming { estimated: 10, actual };
        assert_eq!(ReviewPace::factor(&[timing(20); 4]), 1.0, "not before five");
        let slower = vec![timing(20); 5];
        assert_eq!(ReviewPace::factor(&slower), 2.0);
        let forgotten = [slower.clone(), vec![timing(600); 10]].concat();
        assert_eq!(ReviewPace::factor(&forgotten), 2.0, "a timer left running for hours says nothing");
        assert_eq!(ReviewPace::factor(&[timing(100); 5]), 3.0, "at most three times");
        assert_eq!(ReviewPrep::new(&review("1", &[("a.ts", 400)], 1), 2.0).unwrap().estimated_minutes, 24);
    }

    #[test]
    fn timings_are_camel_case_json() {
        let json = serde_json::to_string(&ReviewTiming { estimated: 10, actual: 12 }).unwrap();
        assert_eq!(json, r#"{"estimated":10,"actual":12}"#);
    }

    #[test]
    fn fill_handles_positional_and_percent() {
        assert_eq!(fill("%2$@ then %1$@, 100%%", &["a".into(), "b".into()]), "b then a, 100%");
        assert_eq!(plural("%d file", "%d files", 1, &english), "1 file");
    }
}
