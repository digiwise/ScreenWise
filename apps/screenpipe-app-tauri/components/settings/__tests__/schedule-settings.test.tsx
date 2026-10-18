import React, { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ScheduleSettings, type ScheduleRule } from "../schedule-settings";

vi.mock("@/components/ui/help-tooltip", () => ({ HelpTooltip: () => null }));

function fixture(initial: ScheduleRule[] = [], enabled = true) {
  const changed = vi.fn();
  function Harness() {
    const [rules, setRules] = useState(initial);
    const [active, setActive] = useState(enabled);
    return <ScheduleSettings enabled={active} rules={rules} onChange={(nextActive, nextRules) => {
      changed(nextActive, nextRules);
      setActive(nextActive);
      setRules(nextRules);
    }} />;
  }
  render(<Harness />);
  return changed;
}

const legacy: ScheduleRule = { dayOfWeek: 0, startTime: "09:00", endTime: "17:00", recordMode: "audio_only" };

describe("simple recording schedule", () => {
  it("adds all-enabled-source windows without a source-mode selector", () => {
    const changed = fixture();
    fireEvent.click(screen.getByTitle("Add time range for Monday"));
    expect(changed).toHaveBeenLastCalledWith(true, [{ ...legacy, recordMode: "all" }]);
    fireEvent.click(screen.getByText("Mon"));
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });

  it.each(["audio_only", "screen_only", "future_mode"])("keeps %s paused until explicit conversion", (recordMode) => {
    const changed = fixture([{ ...legacy, recordMode }]);
    expect(changed).not.toHaveBeenCalled();
    expect(screen.getByRole("status")).toHaveTextContent("paused until you convert");
    fireEvent.click(screen.getByText("Mon"));
    fireEvent.change(screen.getByLabelText("Start time"), { target: { value: "08:00" } });
    expect(changed).toHaveBeenLastCalledWith(true, [{ ...legacy, startTime: "08:00", recordMode }]);
    fireEvent.click(screen.getByRole("button", { name: "Use all enabled sources" }));
    expect(changed).toHaveBeenLastCalledWith(true, [{ ...legacy, startTime: "08:00", recordMode: "all" }]);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("does not convert saved rules when enabling the schedule", () => {
    const changed = fixture([legacy], false);
    fireEvent.click(screen.getByRole("switch", { name: "Enable recording schedule" }));
    expect(changed).toHaveBeenLastCalledWith(true, [legacy]);
    expect(screen.getByRole("status")).toHaveTextContent("paused until you convert");
  });

  it("does not save a window that ends at or before its start", () => {
    const valid: ScheduleRule = { ...legacy, recordMode: "all" };
    const changed = fixture([valid]);
    fireEvent.click(screen.getByText("Mon"));
    fireEvent.change(screen.getByLabelText("Start time"), { target: { value: "18:00" } });
    expect(changed).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toHaveTextContent("Not saved: the end must be after the start");
    expect(screen.getByLabelText("Start time")).toHaveValue("18:00");
    fireEvent.change(screen.getByLabelText("End time"), { target: { value: "20:00" } });
    expect(changed).toHaveBeenLastCalledWith(true, [{ ...valid, startTime: "18:00", endTime: "20:00" }]);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("flags a saved window that pauses its whole day", () => {
    const changed = fixture([{ ...legacy, endTime: "00:00", recordMode: "all" }]);
    fireEvent.click(screen.getByText("Mon"));
    expect(screen.getByRole("alert")).toHaveTextContent("pauses recording for this whole day");
    expect(changed).not.toHaveBeenCalled();
  });

  it("makes Always On cover every complete day and round-trips end-of-day midnight", () => {
    const changed = fixture();
    fireEvent.click(screen.getByRole("button", { name: "Always On" }));
    expect(changed).toHaveBeenLastCalledWith(true, Array.from({ length: 7 }, (_, dayOfWeek) => ({
      dayOfWeek, startTime: "00:00", endTime: "24:00", recordMode: "all",
    })));
    fireEvent.click(screen.getByText("Mon"));
    expect(screen.getByLabelText("End time")).toHaveValue("00:00");
    fireEvent.change(screen.getByLabelText("End time"), { target: { value: "23:00" } });
    fireEvent.change(screen.getByLabelText("End time"), { target: { value: "00:00" } });
    const rules = changed.mock.calls.at(-1)![1] as ScheduleRule[];
    expect(rules[0].endTime).toBe("24:00");
  });
});
