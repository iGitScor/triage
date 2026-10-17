//! The keyword classifier measured on the labelled messages shared with the Mac (`eval/intents.json`), against
//! the floors of `eval/baseline.json`. Same metrics as `IntentEvaluationTests.swift`. `make eval` prints the report.
use remora_core::{KeywordIntentClassifier, TextIntent};
use serde::Deserialize;
use std::collections::BTreeMap;

const CASES: &str = include_str!("../../../../eval/intents.json");
const BASELINE: &str = include_str!("../../../../eval/baseline.json");
const INTENTS: [(TextIntent, &str); 3] =
    [(TextIntent::Request, "request"), (TextIntent::Info, "info"), (TextIntent::Closing, "closing")];

#[derive(Deserialize)]
struct Case {
    text: String,
    intent: String,
}

fn intent(name: &str) -> TextIntent {
    INTENTS.iter().find(|(_, n)| *n == name).map(|(i, _)| *i).unwrap_or_else(|| panic!("unknown intent {name}"))
}

/// Every metric, higher is better. Unsure answers count as wrong for accuracy by intent, and as *To reply* for the
/// verb, as in the app.
fn report(results: &[(TextIntent, Option<TextIntent>)]) -> BTreeMap<String, f64> {
    let ratio = |n: usize, d: usize| if d == 0 { 0.0 } else { n as f64 / d as f64 };
    let answered = results.iter().filter(|(_, got)| got.is_some()).count();
    let correct = results.iter().filter(|(want, got)| *got == Some(*want)).count();
    let verb = |i: Option<TextIntent>| i.unwrap_or(TextIntent::Request);
    let requests = results.iter().filter(|(want, _)| *want == TextIntent::Request);
    let others = results.iter().filter(|(want, _)| *want != TextIntent::Request);
    let mut metrics = BTreeMap::from([
        ("coverage".to_string(), ratio(answered, results.len())),
        ("accuracy".to_string(), ratio(correct, answered)),
        (
            "verbAccuracy".to_string(),
            ratio(results.iter().filter(|(want, got)| verb(*got) == *want).count(), results.len()),
        ),
        (
            "requestsKept".to_string(),
            ratio(requests.clone().filter(|(_, got)| verb(*got) == TextIntent::Request).count(), requests.count()),
        ),
        (
            "othersKept".to_string(),
            ratio(others.clone().filter(|(_, got)| verb(*got) != TextIntent::Request).count(), others.count()),
        ),
    ]);
    for (i, name) in INTENTS {
        let predicted = results.iter().filter(|(_, got)| *got == Some(i)).count();
        let actual = results.iter().filter(|(want, _)| *want == i).count();
        let hits = results.iter().filter(|(want, got)| *want == i && *got == Some(i)).count();
        metrics.insert(format!("precision.{name}"), ratio(hits, predicted));
        metrics.insert(format!("recall.{name}"), ratio(hits, actual));
    }
    metrics
}

#[test]
fn keywords_meet_their_floors() {
    let cases: Vec<Case> = serde_json::from_str(CASES).expect("eval/intents.json");
    let baseline: BTreeMap<String, BTreeMap<String, f64>> = serde_json::from_str(BASELINE).expect("baseline.json");
    let results: Vec<_> = cases.iter().map(|c| (intent(&c.intent), KeywordIntentClassifier::intent(&c.text))).collect();
    let metrics = report(&results);
    println!("keywords on {} messages:", cases.len());
    for (name, value) in &metrics {
        println!("  {name:<18} {value:.3}");
    }
    for (case, (want, got)) in cases.iter().zip(&results) {
        if let Some(got) = got.filter(|got| got != want) {
            println!("  wrong: {:?} for {want:?}: {}", got, case.text);
        }
    }
    let below: Vec<String> = baseline["keywords"]
        .iter()
        .filter(|(name, floor)| metrics.get(*name).is_none_or(|v| *v + 1e-9 < **floor))
        .map(|(name, floor)| format!("{name}: {:.3} < {floor}", metrics.get(name).copied().unwrap_or(f64::NAN)))
        .collect();
    assert!(below.is_empty(), "below the floors of eval/baseline.json: {}", below.join(", "));
}
