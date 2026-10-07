/**
 * Reads the Weeek API token from the OS keychain.
 *
 * Uses the same credential entry the `weeek-mcp` wizard writes
 * (service "weeek-mcp", account "api-token"): Windows Credential Manager,
 * macOS Keychain, or Linux Secret Service. The native module is optional —
 * when it is missing or the store is unreadable the server simply falls back
 * to env-only mode. WEEEK_DISABLE_KEYCHAIN=1 turns this off explicitly.
 */
export async function readKeychainToken(): Promise<string | undefined> {
  if (/^(1|true|yes)$/i.test((process.env.WEEEK_DISABLE_KEYCHAIN ?? "").trim())) return undefined;
  try {
    const { AsyncEntry } = await import("@napi-rs/keyring");
    const stored = (await new AsyncEntry("weeek-mcp", "api-token").getPassword())?.trim();
    return stored || undefined;
  } catch {
    return undefined;
  }
}
