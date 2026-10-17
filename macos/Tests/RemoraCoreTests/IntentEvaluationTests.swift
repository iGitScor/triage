import Foundation
import NaturalLanguage
import Testing

@testable import RemoraCore

/// ML-06: the classifiers measured on the labelled messages shared with Windows (`eval/intents.json`), against the
/// floors of `eval/baseline.json`. Same metrics as `intent_eval.rs`. `make eval` prints the reports.
struct IntentEvaluationTests {
    struct Case: Decodable {
        let text: String
        let intent: String
    }

    static let intents: [(TextIntent, String)] = [(.request, "request"), (.info, "info"), (.closing, "closing")]
    static let folder = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent().appendingPathComponent("eval")

    func cases() throws -> [(text: String, intent: TextIntent)] {
        let data = try Data(contentsOf: Self.folder.appendingPathComponent("intents.json"))
        return try JSONDecoder().decode([Case].self, from: data).map { item in
            (item.text, try #require(Self.intents.first { $0.1 == item.intent }?.0))
        }
    }

    /// Every metric, higher is better. Unsure answers count as wrong for accuracy by intent, and as *To reply* for
    /// the verb, as in the app.
    static func report(_ results: [(want: TextIntent, got: TextIntent?)]) -> [String: Double] {
        func ratio(_ n: Int, _ d: Int) -> Double { d == 0 ? 0 : Double(n) / Double(d) }
        func verb(_ intent: TextIntent?) -> TextIntent { intent ?? .request }
        let answered = results.filter { $0.got != nil }.count
        let correct = results.filter { $0.got == $0.want }.count
        let requests = results.filter { $0.want == .request }
        let others = results.filter { $0.want != .request }
        var metrics: [String: Double] = [
            "coverage": ratio(answered, results.count),
            "accuracy": ratio(correct, answered),
            "verbAccuracy": ratio(results.filter { verb($0.got) == $0.want }.count, results.count),
            "requestsKept": ratio(requests.filter { verb($0.got) == .request }.count, requests.count),
            "othersKept": ratio(others.filter { verb($0.got) != .request }.count, others.count),
        ]
        for (intent, name) in intents {
            let hits = results.filter { $0.want == intent && $0.got == intent }.count
            metrics["precision.\(name)"] = ratio(hits, results.filter { $0.got == intent }.count)
            metrics["recall.\(name)"] = ratio(hits, results.filter { $0.want == intent }.count)
        }
        return metrics
    }

    func measure(_ name: String, _ classify: (String) -> TextIntent?) throws {
        let cases = try cases()
        let results = cases.map { (want: $0.intent, got: classify($0.text)) }
        let metrics = Self.report(results)
        print("\(name) on \(cases.count) messages:")
        for (metric, value) in metrics.sorted(by: { $0.key < $1.key }) {
            print("  \(metric.padding(toLength: 18, withPad: " ", startingAt: 0)) \(String(format: "%.3f", value))")
        }
        for (item, result) in zip(cases, results) {
            if let got = result.got, got != result.want { print("  wrong: \(got) for \(result.want): \(item.text)") }
        }
        let data = try Data(contentsOf: Self.folder.appendingPathComponent("baseline.json"))
        let floors = try JSONDecoder().decode([String: [String: Double]].self, from: data)[name] ?? [:]
        for (metric, floor) in floors.sorted(by: { $0.key < $1.key }) {
            let value = metrics[metric] ?? .nan
            #expect(value + 1e-9 >= floor, "\(name) \(metric): \(String(format: "%.3f", value)) < \(floor)")
        }
    }

    @Test func keywordsMeetTheirFloors() throws {
        try measure("keywords") { KeywordIntentClassifier().intent(of: $0) }
    }

    /// macOS downloads the embeddings on demand, so a fresh machine (CI) may not have them.
    @Test(
        .enabled(
            if: NLEmbedding.sentenceEmbedding(for: .english) != nil
                && NLEmbedding.sentenceEmbedding(for: .french) != nil))
    func embeddingsMeetTheirFloors() throws {
        let embeddings = EmbeddingIntentClassifier()
        try measure("macos.embeddings") { embeddings.intent(of: $0) }
        let chain = VerbClassifier([KeywordIntentClassifier(), embeddings])
        try measure("macos.chain") { chain.intent(of: $0) }
    }
}
