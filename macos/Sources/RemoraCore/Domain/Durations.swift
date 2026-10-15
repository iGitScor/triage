import Foundation

extension TimeInterval {
    /// 24 hours, for a duration ("a week ago", "in 3 d"). A calendar day, which a daylight-saving change makes 23 or
    /// 25 hours long, is `Calendar.date(byAdding: .day…)`.
    public static let day: TimeInterval = 86_400
}
