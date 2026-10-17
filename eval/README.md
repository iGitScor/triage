# Evaluation set

Labelled chat messages, in English and French, that both apps' tests sort and score (ML-06). They are made up, close
to what arrives in Slack or Teams, and many say what they want without a keyword ("Le client attend toujours le
devis", "ping").

- `intents.json`: one message per line, `{"text", "lang": "en" | "fr", "intent": "request" | "info" | "closing"}`.
- `baseline.json`: per classifier, the floor of each metric. A test fails when a number falls below its floor.

## Labels

- **request**: the sender expects something from you (an answer, an action, a decision, your presence), said or not.
- **info**: nothing is expected; reading is enough.
- **closing**: the exchange is over (thanks, OK, 👍, "bon week-end").

There is no "unsure" label: unsure is what a classifier answers, and the app then puts the message in *To reply*.

## Metrics

All higher is better. `coverage`: messages answered; `accuracy`: right among those answered; `precision.<intent>` and
`recall.<intent>`; with the app's fallback (unsure → *To reply*), `verbAccuracy`, `requestsKept` (requests left in
*To reply*: a missed request is the worst mistake) and `othersKept` (the rest kept out of it).

`keywords` is checked on both apps (Swift `IntentEvaluationTests`, Rust `intent_eval.rs`); `macos.embeddings` and
`macos.chain` (keywords, then Apple's sentence embeddings, as the app runs them) on a Mac that has the embeddings.

## Use

`make eval` prints each report and its wrong answers. When a change improves a number, raise its floor (measured,
rounded down to two decimals). Add a message when real mail is sorted wrong, labelled by the rules above, never to
make a classifier pass.
