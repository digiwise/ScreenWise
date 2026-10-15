import { describe, expect, it } from "vitest";
import { recordingReason } from "../recording-status";
describe("precise privacy reasons", () => {
  it("does not conflate missing foreground enumeration with unknown application identity", () => {
    expect(recordingReason("foreground not enumerated")).toContain("capture window list");
    expect(recordingReason("foreground unavailable")).toContain("Native foreground");
  });
  it("identifies failed input-check stages without passing arbitrary provider text through", () => {
    expect(recordingReason("input password property unsupported")).toContain("Boolean IsPassword");
    expect(recordingReason("input password property failed")).toContain("failed to read");
    expect(recordingReason("input visual privacy closed")).toContain("visual privacy gate");
    expect(recordingReason("input focused element failed")).toContain("focused element");
    expect(recordingReason("private provider error secret")).toBe("Reason unavailable");
  });
});
