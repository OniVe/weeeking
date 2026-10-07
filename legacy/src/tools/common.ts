import { WeeekApiError, WeeekNetworkError, WeeekTokenError } from "../http.js";

export type ToolResult = {
  content: { type: "text"; text: string }[];
  isError?: boolean;
};

export function jsonResult(data: unknown, maxChars = 60000): ToolResult {
  const text = typeof data === "string" ? data : JSON.stringify(data, null, 1);
  const clipped =
    text.length > maxChars
      ? `${text.slice(0, maxChars)}\n…[ответ обрезан: ${maxChars} из ${text.length} символов]`
      : text;
  return { content: [{ type: "text", text: clipped }] };
}

export function errorResult(message: string): ToolResult {
  return { content: [{ type: "text", text: message }], isError: true };
}

export function describeError(e: unknown): string {
  if (e instanceof WeeekTokenError) return e.message;
  if (e instanceof WeeekApiError) {
    return `Ошибка Weeek API (HTTP ${e.status}): ${e.body.slice(0, 800)}`;
  }
  if (e instanceof WeeekNetworkError) return e.message;
  return `Ошибка: ${e instanceof Error ? e.message : String(e)}`;
}
