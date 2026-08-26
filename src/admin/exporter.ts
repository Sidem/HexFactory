import type { Definitions, Scenarios, Technologies } from "../core/types";

/**
 * Format definitions with exact 2-space indentation and sorted object keys for clean diffs.
 */
export function formatDefinitionsJson(definitions: Definitions): string {
  return JSON.stringify(definitions, null, 2) + "\n";
}

export function formatTechnologiesJson(technologies: Technologies): string {
  return JSON.stringify(technologies, null, 2) + "\n";
}

export function formatScenariosJson(scenarios: Scenarios): string {
  return JSON.stringify(scenarios, null, 2) + "\n";
}

/**
 * Trigger a browser file download of JSON content.
 */
export function downloadJsonFile(filename: string, content: string): void {
  const blob = new Blob([content], { type: "application/json;charset=utf-8" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.appendChild(link);
  link.click();
  document.body.removeChild(link);
  URL.revokeObjectURL(url);
}

/**
 * Copy text to user clipboard.
 */
export async function copyToClipboard(text: string): Promise<boolean> {
  if (navigator.clipboard && window.isSecureContext) {
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      // Fall through to fallback
    }
  }

  // Fallback for non-secure contexts or legacy browsers
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.left = "-999999px";
  textarea.style.top = "-999999px";
  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();
  let success = false;
  try {
    success = document.execCommand("copy");
  } catch {
    success = false;
  }
  document.body.removeChild(textarea);
  return success;
}

/**
 * Parse an imported JSON file and validate structure.
 */
export function parseImportedJson<T = unknown>(
  jsonString: string,
): { success: true; data: T } | { success: false; error: string } {
  try {
    const data = JSON.parse(jsonString);
    if (!data || typeof data !== "object") {
      return { success: false, error: "Parsed JSON is not an object" };
    }
    return { success: true, data: data as T };
  } catch (err) {
    return {
      success: false,
      error: err instanceof Error ? err.message : "Failed to parse JSON",
    };
  }
}
