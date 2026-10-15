import { beforeAll, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/path", () => ({ homeDir: vi.fn() }));
vi.mock("@tauri-apps/plugin-os", () => ({ platform: () => "windows" }));
vi.mock("@tauri-apps/plugin-store", () => ({ Store: class {} }));
vi.mock("@/lib/utils/tauri", () => ({ commands: {} }));

let createDefaultSettingsObject: typeof import("../use-settings").createDefaultSettingsObject;

beforeAll(async () => {
	({ createDefaultSettingsObject } = await import("../use-settings"));
});

describe("privacy-safe renderer defaults", () => {
	it("matches the native first-run basic PII default", () => {
		const settings = createDefaultSettingsObject();

		expect(settings.usePiiRemoval).toBe(true);
		expect(settings.disableKeyboardCapture).toBe(true);
		expect(settings.disableClipboardCapture).toBe(true);
	});
});
