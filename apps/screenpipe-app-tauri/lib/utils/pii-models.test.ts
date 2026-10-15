import { describe, expect, it } from "vitest";

import {
  anySmartPiiModelReady,
  imagePiiModelReady,
  textPiiModelReady,
  type PiiModelStatus,
} from "./pii-models";

const status = (textReady: boolean, imageReady: boolean): PiiModelStatus => ({
  textReady,
  imageReady,
  textDirectory: "models/text",
  imageDirectory: "models",
  textFiles: [],
  imageFiles: [],
});

describe("PII model readiness", () => {
  it("gates each Smart feature on its own verified pack", () => {
    expect(textPiiModelReady(status(true, false))).toBe(true);
    expect(textPiiModelReady(status(false, true))).toBe(false);
    expect(imagePiiModelReady(status(false, true))).toBe(true);
    expect(imagePiiModelReady(status(true, false))).toBe(false);
  });

  it("allows Smart when at least one enhancement is provisioned", () => {
    expect(anySmartPiiModelReady(status(true, true))).toBe(true);
    expect(anySmartPiiModelReady(status(true, false))).toBe(true);
    expect(anySmartPiiModelReady(status(false, true))).toBe(true);
    expect(anySmartPiiModelReady(status(false, false))).toBe(false);
    expect(anySmartPiiModelReady(null)).toBe(false);
  });
});
