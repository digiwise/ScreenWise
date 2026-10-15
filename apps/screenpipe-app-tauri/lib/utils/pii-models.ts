import type { PiiModelStatus } from "./tauri";

export type { PiiModelStatus } from "./tauri";

export function textPiiModelReady(status: PiiModelStatus | null): boolean {
  return status?.textReady === true;
}

export function imagePiiModelReady(status: PiiModelStatus | null): boolean {
  return status?.imageReady === true;
}

export function anySmartPiiModelReady(status: PiiModelStatus | null): boolean {
  return textPiiModelReady(status) || imagePiiModelReady(status);
}
