// The time slider's scale, as SnoozeClock has it in remora_core (clock.rs): computed here, so dragging asks Rust
// nothing (PERF-14). A test keeps the table equal to Rust's.

/** Each step of the slider, in minutes: small moves give minutes, long ones days. */
export const STEPS_MINUTES = [
  5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 360, 480, 720, 1440, 2880, 4320, 7200, 10080,
]

/** Drag position (0…1) → the date it means, counted from `from` (a preset), else from now. */
export function sliderDate(progress: number, from: string | null, now = new Date()): string {
  const clamped = Math.min(1, Math.max(0, progress))
  const minutes = STEPS_MINUTES[Math.round(clamped * (STEPS_MINUTES.length - 1))]
  const start = from ? new Date(from) : now
  return new Date(start.getTime() + minutes * 60_000).toISOString()
}
