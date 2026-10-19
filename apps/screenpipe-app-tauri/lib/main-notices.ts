// Main-window notices that stay visible until dismissed. Messages are fixed text;
// keys are fixed codes or content-free hashes, never captured content.
import type { Settings } from "@/lib/hooks/use-settings";
import { blockingScheduleWindows, type ScheduleWindow } from "@/lib/schedule-windows";

/** Recorded by settings normalization (Rust and renderer) when it turns screen recording off. */
export const SCREEN_SCOPE_OFF_NOTICE = "screen_scope_migrated_off";
const MAX_DISMISSED = 32;

export type MainNotice = {
  /** Pending code, or the dismissal key of a notice derived from current settings. */
  key: string;
  pending: boolean;
  /** Defaults to true. False for a standing condition that must stay visible
   * while it holds, so dismissing it would misrepresent the current state. */
  dismissible?: boolean;
  title: string;
  body: string;
  settingsSection: string;
  actionLabel: string;
};

const PENDING: Record<string, Omit<MainNotice, "key" | "pending">> = {
  [SCREEN_SCOPE_OFF_NOTICE]: {
    title: "Screen recording was turned off",
    body: "Recording selected monitors is no longer supported. Your earlier monitor selection was replaced with screen recording off rather than every display. Turn screen recording on in Recording settings to capture all connected monitors.",
    settingsSection: "recording",
    actionLabel: "Open recording settings",
  },
};

function fnv1a(text: string): string {
  let hash = 0x811c9dc5;
  for (let index = 0; index < text.length; index++) {
    hash ^= text.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

export function mainNotices(settings: Settings): MainNotice[] {
  const notices: MainNotice[] = [];
  // `accessibilityWalkDisabled` has no settings control on purpose, so this
  // banner is the only place the mode is visible. It is a standing privacy
  // reduction rather than a one-off event, so it is not dismissible: it shows
  // for as long as the setting is on, and therefore on every activation.
  if (settings.accessibilityWalkDisabled) {
    notices.push({
      key: "accessibility_walk_disabled",
      pending: false,
      dismissible: false,
      title: "Accessibility checks are turned off",
      body:
        "Screen text is being recorded from OCR alone. ScreenWise is not reading any application's accessibility tree, so it cannot verify what a window is showing before a frame is stored, and in-memory content screening is the only check left — that is heuristic and can miss things. Exclusions, URL rules, private windows, protected content, screen lock and monitor checks all still apply. Remove accessibilityWalkDisabled from settings to restore the full checks.",
      settingsSection: "privacy",
      actionLabel: "Open privacy settings",
    });
  }
  for (const code of settings.pendingNotices ?? []) {
    const notice = PENDING[code];
    if (notice && !notices.some((item) => item.key === code)) notices.push({ key: code, pending: true, ...notice });
  }
  if (settings.scheduleEnabled) {
    const blocking = blockingScheduleWindows((settings.scheduleRules ?? []) as ScheduleWindow[]);
    if (blocking.length) {
      // A changed blocking configuration produces a new key, so it is shown again.
      const fingerprint = blocking.map((rule) => [rule.dayOfWeek, rule.startTime, rule.endTime, rule.recordMode].join("|")).sort().join(";");
      const key = `schedule_blocking:${fnv1a(fingerprint)}`;
      if (!(settings.dismissedNotices ?? []).includes(key)) {
        notices.push({
          key,
          pending: false,
          title: "Recording schedule is blocking recording",
          body: `${blocking.length} schedule ${blocking.length === 1 ? "window" : "windows"} cannot be applied. A window whose end is not after its start (such as one ending at 00:00, or overnight) stops recording for that whole day; a window from a retired audio-only or screen-only mode stops recording during its hours. Edit or remove these windows in Privacy settings.`,
          settingsSection: "privacy",
          actionLabel: "Open schedule settings",
        });
      }
    }
  }
  return notices;
}

/** The settings update that dismisses `notice`. */
export function dismissMainNotice(settings: Settings, notice: MainNotice): Partial<Settings> {
  if (notice.pending) return { pendingNotices: (settings.pendingNotices ?? []).filter((code) => code !== notice.key) };
  const dismissed = (settings.dismissedNotices ?? []).filter((key) => key !== notice.key);
  return { dismissedNotices: [...dismissed, notice.key].slice(-MAX_DISMISSED) };
}
