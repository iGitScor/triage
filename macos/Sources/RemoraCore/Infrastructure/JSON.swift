import Foundation

extension JSONDecoder {
    /// Decoder tolerant to the ISO-8601 flavours used by GitHub, GitLab and Notion.
    public static func api(snakeCase: Bool = false) -> JSONDecoder {
        let decoder = JSONDecoder()
        if snakeCase { decoder.keyDecodingStrategy = .convertFromSnakeCase }
        decoder.dateDecodingStrategy = .custom { decoder in
            let container = try decoder.singleValueContainer()
            let raw = try container.decode(String.self)
            guard let date = Date(iso8601: raw) else {
                throw DecodingError.dataCorruptedError(in: container, debugDescription: "Invalid date \(raw)")
            }
            return date
        }
        return decoder
    }
}

extension Date {
    public init?(iso8601 raw: String) {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if let date = formatter.date(from: raw) {
            self = date
            return
        }
        formatter.formatOptions = [.withInternetDateTime]
        guard let date = formatter.date(from: raw) else { return nil }
        self = date
    }
}

extension URL {
    /// Accepts absolute URLs, host-relative paths and empty strings.
    public init?(lenient raw: String?, relativeTo base: URL? = nil) {
        guard let raw, !raw.isEmpty, let url = URL(string: raw, relativeTo: base) else { return nil }
        self = url.absoluteURL
    }
}
