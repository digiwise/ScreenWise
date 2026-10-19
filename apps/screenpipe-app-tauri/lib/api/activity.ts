/**
 * Typed client for the dedicated activity/search read endpoints that replace
 * the app's own hand-written `/raw_sql` queries (`POST /raw_sql` stays
 * available, but only the local AI agent writes SQL against it now).
 *
 * This is a thin layer: build the query string, fetch through `localFetch`,
 * check `response.ok`, parse JSON, return typed data. Caching and
 * error-handling policy stay with each caller, same as before.
 *
 * Shapes mirror `.local/plans/dedicated-endpoints-contract-20261019.md`
 * exactly, with `| null` wherever that document marks a field nullable.
 */

import { localFetch } from "@/lib/api";

/** Build a query string from params, skipping undefined/null values. */
function buildQuery(params: Record<string, string | number | undefined>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null) {
      search.set(key, String(value));
    }
  }
  const qs = search.toString();
  return qs ? `?${qs}` : "";
}

async function getJson<T>(
  path: string,
  options?: { timeoutMs?: number; signal?: AbortSignal },
): Promise<T> {
  let signal = options?.signal;
  if (options?.timeoutMs !== undefined) {
    const timeoutSignal = AbortSignal.timeout(options.timeoutMs);
    signal = signal ? AbortSignal.any([signal, timeoutSignal]) : timeoutSignal;
  }
  const response = await localFetch(path, signal ? { signal } : undefined);
  if (!response.ok) {
    const text = await response.text().catch(() => "");
    throw new Error(`${path} failed: HTTP ${response.status} ${text}`);
  }
  return response.json() as Promise<T>;
}

// ---------------------------------------------------------------------------
// GET /activity/coverage
// ---------------------------------------------------------------------------

export interface ActivityCoverage {
  first_timestamp: string | null;
  last_timestamp: string | null;
  days: string[];
}

export async function getActivityCoverage(): Promise<ActivityCoverage> {
  return getJson<ActivityCoverage>("/activity/coverage");
}

// ---------------------------------------------------------------------------
// GET /activity/apps
// ---------------------------------------------------------------------------

export interface AppFrameCount {
  app_name: string;
  frame_count: number;
}

export async function getActivityApps(params?: {
  since_minutes?: number;
  limit?: number;
}): Promise<{ apps: AppFrameCount[] }> {
  const qs = buildQuery({
    since_minutes: params?.since_minutes,
    limit: params?.limit,
  });
  return getJson<{ apps: AppFrameCount[] }>(`/activity/apps${qs}`);
}

// ---------------------------------------------------------------------------
// GET /activity/app-windows
// ---------------------------------------------------------------------------

export interface AppWindowCount {
  window_name: string | null;
  frame_count: number;
}

export interface AppWindowUsage {
  app_name: string;
  frame_count: number;
  window_count: number;
  windows: AppWindowCount[];
}

export async function getActivityAppWindows(params?: {
  since_minutes?: number;
  limit?: number;
  windows_per_app?: number;
}): Promise<{ apps: AppWindowUsage[] }> {
  const qs = buildQuery({
    since_minutes: params?.since_minutes,
    limit: params?.limit,
    windows_per_app: params?.windows_per_app,
  });
  return getJson<{ apps: AppWindowUsage[] }>(`/activity/app-windows${qs}`);
}

// ---------------------------------------------------------------------------
// GET /activity/domains
// ---------------------------------------------------------------------------

export interface DomainCount {
  domain: string;
  frame_count: number;
}

export async function getActivityDomains(params?: {
  since_minutes?: number;
  limit?: number;
}): Promise<{ domains: DomainCount[] }> {
  const qs = buildQuery({
    since_minutes: params?.since_minutes,
    limit: params?.limit,
  });
  return getJson<{ domains: DomainCount[] }>(`/activity/domains${qs}`);
}

// ---------------------------------------------------------------------------
// GET /activity/text-snippets
// ---------------------------------------------------------------------------

export type TextSnippetSource = "accessibility" | "ocr";

export interface TextSnippet {
  app_name: string | null;
  window_name: string | null;
  text: string;
}

export interface TextSnippetPage {
  total: number;
  snippets: TextSnippet[];
}

export async function getActivityTextSnippets(params: {
  source: TextSnippetSource;
  since_minutes?: number;
  limit?: number;
  max_chars?: number;
  min_chars?: number;
  order?: "recent" | "random";
}): Promise<TextSnippetPage> {
  const qs = buildQuery({
    source: params.source,
    since_minutes: params.since_minutes,
    limit: params.limit,
    max_chars: params.max_chars,
    min_chars: params.min_chars,
    order: params.order,
  });
  return getJson<TextSnippetPage>(`/activity/text-snippets${qs}`);
}

// ---------------------------------------------------------------------------
// GET /activity/ui-events
// ---------------------------------------------------------------------------

export interface UiEventSummary {
  event_type: string;
  text_content: string | null;
  app_name: string | null;
  window_title: string | null;
  timestamp: string;
}

export async function getActivityUiEvents(params: {
  /** RFC 3339 UTC string. */
  start: string;
  /** RFC 3339 UTC string. */
  end: string;
  limit?: number;
  order?: "asc" | "desc";
}): Promise<{ events: UiEventSummary[] }> {
  const qs = buildQuery({
    start: params.start,
    end: params.end,
    limit: params.limit,
    order: params.order,
  });
  return getJson<{ events: UiEventSummary[] }>(`/activity/ui-events${qs}`);
}

// ---------------------------------------------------------------------------
// GET /tags/vision and /tags/vision/frames
// ---------------------------------------------------------------------------

export interface VisionTagCount {
  name: string;
  frame_count: number;
}

export async function getVisionTags(params?: {
  limit?: number;
  timeoutMs?: number;
  signal?: AbortSignal;
}): Promise<{ tags: VisionTagCount[] }> {
  const qs = buildQuery({ limit: params?.limit });
  return getJson<{ tags: VisionTagCount[] }>(`/tags/vision${qs}`, {
    timeoutMs: params?.timeoutMs,
    signal: params?.signal,
  });
}

export interface TaggedFrame {
  frame_id: number;
  timestamp: string;
  app_name: string | null;
  tag_names: string[];
}

export async function getFramesWithVisionTags(params: {
  tags: string[];
  limit?: number;
  timeoutMs?: number;
  signal?: AbortSignal;
}): Promise<{ frames: TaggedFrame[] }> {
  const qs = buildQuery({
    tags: params.tags.join(","),
    limit: params.limit,
  });
  return getJson<{ frames: TaggedFrame[] }>(`/tags/vision/frames${qs}`, {
    timeoutMs: params.timeoutMs,
    signal: params.signal,
  });
}

// ---------------------------------------------------------------------------
// GET /search/facets
// ---------------------------------------------------------------------------

export interface SearchFacetApp {
  app_name: string;
  match_count: number;
}

export interface SearchFacetUrl {
  browser_url: string;
  match_count: number;
}

export interface SearchFacetDay {
  day: string;
  first_timestamp: string;
  match_count: number;
}

export interface SearchFacets {
  apps: SearchFacetApp[];
  urls: SearchFacetUrl[];
  days: SearchFacetDay[];
}

export async function getSearchFacets(params: {
  q: string;
  app_limit?: number;
  url_limit?: number;
  day_limit?: number;
  timeoutMs?: number;
  signal?: AbortSignal;
}): Promise<SearchFacets> {
  const qs = buildQuery({
    q: params.q,
    app_limit: params.app_limit,
    url_limit: params.url_limit,
    day_limit: params.day_limit,
  });
  return getJson<SearchFacets>(`/search/facets${qs}`, {
    timeoutMs: params.timeoutMs,
    signal: params.signal,
  });
}

// ---------------------------------------------------------------------------
// GET /frames/nearest
// ---------------------------------------------------------------------------

export interface NearbyFrame {
  frame_id: number;
  timestamp: string;
  app_name: string | null;
}

export async function getNearestFrame(params: {
  /** RFC 3339 UTC string. */
  timestamp: string;
  window_seconds?: number;
  timeoutMs?: number;
  signal?: AbortSignal;
}): Promise<{ frame: NearbyFrame | null }> {
  const qs = buildQuery({
    timestamp: params.timestamp,
    window_seconds: params.window_seconds,
  });
  return getJson<{ frame: NearbyFrame | null }>(`/frames/nearest${qs}`, {
    timeoutMs: params.timeoutMs,
    signal: params.signal,
  });
}
