import RemoraCore
import SwiftUI

/// "Repeat" and "Until" for a new reminder, as on Windows. Date fields, not popovers: the quick reminder panel
/// closes when it stops being the key window.
struct RepeatPicker: View {
    @Binding var rule: Recurrence.Rule?
    @Binding var until: Date?
    /// The first reminder: the end can't be before its day.
    let start: Date

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Picker(L("Repeat"), selection: $rule) {
                Text(L("Never")).tag(Recurrence.Rule?.none)
                ForEach(Recurrence.Rule.allCases, id: \.self) { rule in
                    Text(rule.title).tag(Recurrence.Rule?.some(rule))
                }
            }
            .pickerStyle(.menu)
            .fixedSize()
            if rule != nil {
                HStack(spacing: 8) {
                    Toggle(L("Until"), isOn: hasEnd).toggleStyle(.checkbox)
                    if let until {
                        DatePicker(
                            L("Until"),
                            selection: Binding {
                                until
                            } set: {
                                self.until = $0
                            },
                            displayedComponents: .date
                        )
                        .datePickerStyle(.field)
                        .labelsHidden()
                    } else {
                        Text(L("No end")).foregroundStyle(RemoraDesign.muted)
                    }
                }
                if endsBeforeStart {
                    Text(L("The end is before the first reminder.")).foregroundStyle(RemoraDesign.danger)
                }
            }
        }
        .font(RemoraDesign.font(12))
    }

    /// Whether the choice can't be saved.
    var endsBeforeStart: Bool { Self.endsBeforeStart(rule: rule, until: until, start: start) }

    static func endsBeforeStart(rule: Recurrence.Rule?, until: Date?, start: Date) -> Bool {
        guard let rule else { return false }
        return Recurrence(rule: rule, anchor: start, until: until).endsBeforeStart()
    }

    private var hasEnd: Binding<Bool> {
        Binding {
            until != nil
        } set: { on in
            until = on ? Calendar.current.date(byAdding: .month, value: 1, to: start) : nil
        }
    }
}
