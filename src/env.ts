export type Config = {
  token?: string;
  baseUrl: string;
  timeoutMs: number;
  readOnly: boolean;
  maxChars: number;
};

export function loadConfig(env: NodeJS.ProcessEnv = process.env): Config {
  const token = (env.WEEEK_API_TOKEN ?? env.WEEEK_TOKEN ?? "").trim() || undefined;
  const timeout = Number(env.WEEEK_TIMEOUT_MS ?? 30000);
  const maxChars = Number(env.WEEEK_MAX_RESPONSE_CHARS ?? 60000);
  return {
    token,
    baseUrl: (env.WEEEK_BASE_URL ?? "https://api.weeek.net/public/v1").replace(/\/+$/, ""),
    timeoutMs: Number.isFinite(timeout) && timeout > 0 ? timeout : 30000,
    readOnly: !/^(0|false)$/i.test((env.READ_ONLY ?? "true").trim()),
    maxChars: Number.isFinite(maxChars) && maxChars > 1000 ? maxChars : 60000,
  };
}
