import { describe, expect, it } from "vitest";
import type { Settings } from "@/lib/hooks/use-settings";
import { SCREEN_SCOPE_OFF_NOTICE, dismissMainNotice, mainNotices } from "@/lib/main-notices";
import { scheduleMinutes, scheduleWindowValid } from "@/lib/schedule-windows";

const window = (startTime: string, endTime: string, recordMode = "all", dayOfWeek = 0) => ({ dayOfWeek, startTime, endTime, recordMode });
const settings = (value: Partial<Settings>) => value as Settings;

describe("schedule window validity mirrors the recorder", () => {
  it("accepts canonical HH:MM with 24:00 only as an end bound", () => {
    expect(scheduleMinutes("09:30", false)).toBe(570);
    expect(scheduleMinutes("24:00", true)).toBe(1440);
    for (const value of ["9:00", "+9:00", "24:00", "25:00", "10:60", "1000", ""]) expect(scheduleMinutes(value, false)).toBeNull();
    expect(scheduleMinutes("24:01", true)).toBeNull();
    expect(scheduleWindowValid("00:00", "24:00")).toBe(true);
    for (const [start, end] of [["17:00", "09:00"], ["10:00", "10:00"], ["09:00", "00:00"], ["22:00", "06:00"]]) {
      expect(scheduleWindowValid(start, end)).toBe(false);
    }
  });
});

describe("main notices", () => {
  it("shows known pending codes once and dismisses only that code", () => {
    const current = settings({ pendingNotices: [SCREEN_SCOPE_OFF_NOTICE, "unknown_code", SCREEN_SCOPE_OFF_NOTICE] });
    const notices = mainNotices(current);
    expect(notices.map((notice) => notice.key)).toEqual([SCREEN_SCOPE_OFF_NOTICE]);
    expect(notices[0].settingsSection).toBe("recording");
    expect(dismissMainNotice(current, notices[0])).toEqual({ pendingNotices: ["unknown_code"] });
  });

  it("warns about blocking schedule windows only while scheduling is enabled", () => {
    const rules = [window("09:00", "17:00"), window("09:00", "00:00", "all", 1), window("10:00", "11:00", "audio_only", 2)];
    expect(mainNotices(settings({ scheduleEnabled: false, scheduleRules: rules }))).toEqual([]);
    expect(mainNotices(settings({ scheduleEnabled: true, scheduleRules: [window("00:00", "24:00")] }))).toEqual([]);
    const [notice] = mainNotices(settings({ scheduleEnabled: true, scheduleRules: rules }));
    expect(notice.body).toMatch(/^2 schedule windows cannot be applied/);
    expect(notice.settingsSection).toBe("privacy");
    expect(notice.key).toMatch(/^schedule_blocking:[0-9a-f]{8}$/);
  });

  it("keeps a dismissed schedule warning hidden until the blocking windows change", () => {
    const rules = [window("09:00", "00:00")];
    const current = settings({ scheduleEnabled: true, scheduleRules: rules });
    const [notice] = mainNotices(current);
    const dismissed = settings({ ...current, ...dismissMainNotice(current, notice) });
    expect(mainNotices(dismissed)).toEqual([]);
    // Reordering or adding valid windows does not change the warning.
    expect(mainNotices(settings({ ...dismissed, scheduleRules: [window("01:00", "02:00"), ...rules] }))).toEqual([]);
    expect(mainNotices(settings({ ...dismissed, scheduleRules: [window("18:00", "08:00")] }))).toHaveLength(1);
  });

  it("bounds dismissed keys", () => {
    let current = settings({ scheduleEnabled: true, dismissedNotices: [] });
    for (let day = 0; day < 40; day++) {
      current = settings({ ...current, scheduleRules: [window("09:00", "00:00", "all", day)] });
      const [notice] = mainNotices(current);
      current = settings({ ...current, ...dismissMainNotice(current, notice) });
    }
    expect(current.dismissedNotices).toHaveLength(32);
    expect(mainNotices(current)).toEqual([]);
  });
});
