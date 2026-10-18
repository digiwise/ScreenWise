// Mirrors crates/screenpipe-engine/src/schedule_monitor.rs: an invalid window
// blocks its whole day and a non-"all" window blocks recording during its hours.

export type ScheduleWindow = {
  dayOfWeek: number;
  startTime: string;
  endTime: string;
  recordMode: string;
};

/** Canonical HH:MM in minutes; 24:00 is only valid as an end bound. */
export function scheduleMinutes(value: string, end: boolean): number | null {
  const match = /^(\d{2}):(\d{2})$/.exec(value);
  if (!match) return null;
  const hour = Number(match[1]);
  const minute = Number(match[2]);
  if (minute < 60 && (hour < 24 || (end && hour === 24 && minute === 0))) return hour * 60 + minute;
  return null;
}

export function scheduleWindowValid(start: string, end: string): boolean {
  const from = scheduleMinutes(start, false);
  const to = scheduleMinutes(end, true);
  return from !== null && to !== null && from < to;
}

/** Saved windows that stop recording: invalid ones (whole day) and legacy modes. */
export function blockingScheduleWindows(rules: ScheduleWindow[]): ScheduleWindow[] {
  return rules.filter((rule) => !scheduleWindowValid(rule.startTime, rule.endTime) || rule.recordMode !== "all");
}
