export type ToastType = "info" | "success" | "warning" | "error";

export function showToast(
  message: string,
  type: ToastType = "info",
  durationMs = 3500,
): void {
  let container = document.getElementById("admin-toast-container");
  if (!container) {
    container = document.createElement("div");
    container.id = "admin-toast-container";
    container.className = "toast-container";
    document.body.appendChild(container);
  }

  const toast = document.createElement("div");
  toast.className = `toast toast-${type}`;

  const icon = document.createElement("span");
  icon.className = "toast-icon";
  icon.textContent =
    type === "success"
      ? "✓"
      : type === "warning"
        ? "⚠"
        : type === "error"
          ? "✕"
          : "ℹ";

  const text = document.createElement("span");
  text.className = "toast-message";
  text.textContent = message;

  toast.appendChild(icon);
  toast.appendChild(text);
  container.appendChild(toast);

  // Trigger entrance animation
  requestAnimationFrame(() => {
    toast.classList.add("toast-visible");
  });

  setTimeout(() => {
    toast.classList.remove("toast-visible");
    toast.classList.add("toast-leaving");
    setTimeout(() => toast.remove(), 250);
  }, durationMs);
}
