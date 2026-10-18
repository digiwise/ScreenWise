import React from "react";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({ settings: {} as Record<string, unknown>, save: vi.fn(), loaded: true }));
vi.mock("@/lib/hooks/use-settings", () => ({
  useSettings: () => ({ settings: mocks.settings, updateSettings: mocks.save, isSettingsLoaded: mocks.loaded }),
}));
import { MainNotices } from "../main-notices";

beforeEach(() => { vi.clearAllMocks(); mocks.loaded = true; mocks.settings = {}; });
afterEach(cleanup);

describe("main window notices", () => {
  it("renders nothing without notices or before settings load", () => {
    const { container, rerender } = render(<MainNotices onOpenSettings={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
    mocks.settings = { pendingNotices: ["screen_scope_migrated_off"] };
    mocks.loaded = false;
    rerender(<MainNotices onOpenSettings={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("keeps the screen-off notice until dismissed and links to recording settings", () => {
    mocks.settings = { pendingNotices: ["screen_scope_migrated_off"] };
    const open = vi.fn();
    render(<MainNotices onOpenSettings={open} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Screen recording was turned off");
    fireEvent.click(screen.getByRole("button", { name: "Open recording settings" }));
    expect(open).toHaveBeenCalledWith("recording");
    expect(mocks.save).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Dismiss: Screen recording was turned off" }));
    expect(mocks.save).toHaveBeenCalledWith({ pendingNotices: [] });
  });
});
