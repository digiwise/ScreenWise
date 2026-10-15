const FRAME_CAPTURE_NOTICES: Readonly<Record<string, string>> = {
	"BACKGROUND REDACTED — ACTIVE-WINDOW-ONLY":
		"Background pixels are redacted; capture is limited to the active window.",
	"CAPTURE REDACTED — ACTIVE WINDOW EXCLUDED":
		"Frame withheld because the active window is excluded by your privacy settings.",
	"CAPTURE REDACTED — NO SAFE ACTIVE WINDOW":
		"Frame withheld because no safe active window was available.",
	"CAPTURE REDACTED — INCONSISTENT FOCUS":
		"Frame withheld because window focus could not be confirmed consistently.",
	"CAPTURE FAILED — INITIAL PRIVACY EVALUATION":
		"Frame unavailable because the initial privacy check failed; no screen image was captured.",
	"CAPTURE FAILED — MONITOR ACQUISITION":
		"Frame unavailable because monitor image acquisition failed.",
	"CAPTURE FAILED — ACTIVE-WINDOW ACQUISITION":
		"Frame unavailable because active-window image acquisition failed.",
	"CAPTURE FAILED — ACCESSIBILITY INSPECTION":
		"Screenshot storage and OCR withheld after accessibility retrieval failed. Recording status identifies the failure cause and unresolved privacy checks.",
	"CAPTURE FAILED — POST-CAPTURE PRIVACY EVALUATION":
		"Frame withheld because the follow-up privacy check failed.",
};

/** Resolve only known, content-free capture markers to fixed user-facing copy. */
export function getFrameCaptureNotice(marker: string | null | undefined): string | null {
	if (!marker) return null;
	return Object.prototype.hasOwnProperty.call(FRAME_CAPTURE_NOTICES, marker)
		? FRAME_CAPTURE_NOTICES[marker]
		: null;
}
