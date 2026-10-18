"use client";

import { AlertTriangle, X } from "lucide-react";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useSettings } from "@/lib/hooks/use-settings";
import { dismissMainNotice, mainNotices } from "@/lib/main-notices";

/** Banners at the top of the main window; each stays until dismissed. */
export function MainNotices({ onOpenSettings }: { onOpenSettings: (section: string) => void }) {
  const { settings, updateSettings, isSettingsLoaded } = useSettings();
  if (!isSettingsLoaded) return null;
  const notices = mainNotices(settings);
  if (!notices.length) return null;
  return (
    // Top padding keeps the banners below the window drag region.
    <div className="shrink-0 space-y-2 px-4 pt-9">
      {notices.map((notice) => (
        <Alert key={notice.key} className="border-amber-500/50 py-3">
          {/* Before the icon: the Alert indents every element after its svg. */}
          <button
            className="absolute right-2 top-2 rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
            aria-label={`Dismiss: ${notice.title}`}
            onClick={() => void updateSettings(dismissMainNotice(settings, notice))}
          >
            <X className="h-3.5 w-3.5" />
          </button>
          <AlertTriangle className="h-4 w-4 !text-amber-600" aria-hidden="true" />
          <AlertTitle className="pr-6">{notice.title}</AlertTitle>
          <AlertDescription className="text-xs text-muted-foreground">
            <p>{notice.body}</p>
            <Button variant="outline" size="sm" className="mt-2 h-7 text-xs" onClick={() => onOpenSettings(notice.settingsSection)}>
              {notice.actionLabel}
            </Button>
          </AlertDescription>
        </Alert>
      ))}
    </div>
  );
}
