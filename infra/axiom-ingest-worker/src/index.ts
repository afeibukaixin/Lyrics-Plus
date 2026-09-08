interface Env {
  AXIOM_INGEST_URL: string;
  AXIOM_INGEST_TOKEN: string;
  ALLOWED_ORIGIN?: string;
}

const MAX_BODY_BYTES = 64 * 1024;
const MAX_EVENTS = 50;
const RATE_LIMIT_WINDOW_MS = 60_000;
const RATE_LIMIT_REQUESTS = 120;
const rateBuckets = new Map<string, { startedAt: number; count: number }>();
const EVENT_NAMES = new Set([
  "app_session_started",
  "app_daily_active",
  "feature_usage_rollup",
  "lyrics_search_completed",
  "lyrics_provider_completed",
]);

const PROPERTY_KEYS: Record<string, Set<string>> = {
  app_session_started: new Set(),
  app_daily_active: new Set(),
  feature_usage_rollup: new Set(["searches", "manualSearches", "automaticSearches"]),
  lyrics_search_completed: new Set([
    "intent",
    "resultCount",
    "providerCount",
    "autoApplied",
    "succeeded",
    "durationMs",
  ]),
  lyrics_provider_completed: new Set(["providerId", "health", "resultCount", "durationMs"]),
};

type IncomingEvent = {
  event: unknown;
  timestampMs: unknown;
  installId: unknown;
  appVersion: unknown;
  platform: unknown;
  [key: string]: unknown;
};

function json(data: unknown, status = 200, origin?: string) {
  return new Response(JSON.stringify(data), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      ...(origin ? { "access-control-allow-origin": origin } : {}),
    },
  });
}

function validString(value: unknown, maxLength: number) {
  return typeof value === "string" && value.length > 0 && value.length <= maxLength;
}

function rateLimited(request: Request) {
  const key = request.headers.get("cf-connecting-ip") ?? "unknown";
  const now = Date.now();
  const bucket = rateBuckets.get(key);
  if (!bucket || now - bucket.startedAt >= RATE_LIMIT_WINDOW_MS) {
    rateBuckets.set(key, { startedAt: now, count: 1 });
    return false;
  }
  bucket.count += 1;
  return bucket.count > RATE_LIMIT_REQUESTS;
}

function validateEvent(value: unknown): value is IncomingEvent {
  if (typeof value !== "object" || value === null || Array.isArray(value)) return false;
  const event = value as IncomingEvent;
  const eventName = typeof event.event === "string" ? event.event : null;
  if (!eventName || !validString(eventName, 64) || !EVENT_NAMES.has(eventName)) return false;
  if (typeof event.timestampMs !== "number" || !Number.isSafeInteger(event.timestampMs) || event.timestampMs <= 0) return false;
  if (typeof event.installId !== "string" || !validString(event.installId, 128) || !/^[a-f0-9-]+$/i.test(event.installId)) return false;
  if (typeof event.appVersion !== "string" || !validString(event.appVersion, 64)) return false;
  if (typeof event.platform !== "string" || !validString(event.platform, 32)) return false;
  const allowed = PROPERTY_KEYS[eventName];
  if (!Object.keys(event).every((key) =>
    ["event", "timestampMs", "installId", "appVersion", "platform"].includes(key) || allowed.has(key),
  )) return false;
  if (eventName === "lyrics_search_completed") {
    if (typeof event.intent !== "string" || !["automatic", "refresh", "manual"].includes(event.intent)) return false;
    if (typeof event.resultCount !== "number" || typeof event.providerCount !== "number") return false;
    if (typeof event.autoApplied !== "boolean" || typeof event.succeeded !== "boolean") return false;
    if (typeof event.durationMs !== "number") return false;
  }
  if (eventName === "lyrics_provider_completed") {
    if (typeof event.providerId !== "string" || !/^[a-z0-9_-]{1,64}$/i.test(event.providerId)) return false;
    if (typeof event.health !== "string" || !["unknown", "available", "degraded", "unavailable"].includes(event.health)) return false;
    if (typeof event.resultCount !== "number") return false;
    if (event.durationMs !== undefined && typeof event.durationMs !== "number") return false;
  }
  return true;
}

function toAxiomEvent(event: IncomingEvent) {
  const properties: Record<string, unknown> = {};
  for (const key of PROPERTY_KEYS[event.event as string] ?? []) {
    if (key in event) properties[key] = event[key];
  }
  return {
    time: new Date(Number(event.timestampMs)).toISOString(),
    event: event.event,
    installId: event.installId,
    appVersion: event.appVersion,
    platform: event.platform,
    ...properties,
  };
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const origin = env.ALLOWED_ORIGIN ?? "*";
    if (request.method === "OPTIONS") {
      return new Response(null, {
        status: 204,
        headers: {
          "access-control-allow-origin": origin,
          "access-control-allow-methods": "POST, OPTIONS",
          "access-control-allow-headers": "content-type",
        },
      });
    }
    const url = new URL(request.url);
    if (url.pathname !== "/v1/events" || request.method !== "POST") {
      return json({ error: "not_found" }, 404, origin);
    }
    if (rateLimited(request)) return json({ error: "rate_limited" }, 429, origin);
    if (!env.AXIOM_INGEST_URL || !env.AXIOM_INGEST_TOKEN) {
      return json({ error: "worker_not_configured" }, 503, origin);
    }

    const contentLength = Number(request.headers.get("content-length") ?? 0);
    if (contentLength > MAX_BODY_BYTES) return json({ error: "body_too_large" }, 413, origin);
    const raw = await request.arrayBuffer();
    if (raw.byteLength > MAX_BODY_BYTES) return json({ error: "body_too_large" }, 413, origin);

    let parsed: unknown;
    try {
      parsed = JSON.parse(new TextDecoder().decode(raw));
    } catch {
      return json({ error: "invalid_json" }, 400, origin);
    }
    if (!Array.isArray(parsed) || parsed.length === 0 || parsed.length > MAX_EVENTS) {
      return json({ error: "invalid_batch" }, 400, origin);
    }
    if (!parsed.every(validateEvent)) return json({ error: "invalid_event" }, 400, origin);

    const axiomResponse = await fetch(env.AXIOM_INGEST_URL, {
      method: "POST",
      headers: {
        authorization: `Bearer ${env.AXIOM_INGEST_TOKEN}`,
        "content-type": "application/json",
      },
      body: JSON.stringify(parsed.map(toAxiomEvent)),
    });
    if (!axiomResponse.ok) {
      return json({ error: "upstream_rejected" }, 502, origin);
    }
    return json({ accepted: parsed.length }, 202, origin);
  },
};
