/** Whether a key belongs to the focused control instead of the world. */
export function isTypingTarget(target: EventTarget | null): boolean {
  if (
    target instanceof HTMLInputElement ||
    target instanceof HTMLSelectElement ||
    target instanceof HTMLTextAreaElement
  )
    return true;
  return target instanceof HTMLElement && target.isContentEditable;
}

/** Preserve Space for a control reached by keyboard, but not one clicked by pointer. */
export function isKeyboardFocusedControl(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (!isPointerActivatedControl(target)) return false;
  try {
    return target.matches(":focus-visible");
  } catch {
    return false;
  }
}

export function isPointerActivatedControl(target: EventTarget | null): boolean {
  if (
    target instanceof HTMLButtonElement ||
    target instanceof HTMLAnchorElement ||
    (target instanceof HTMLElement && target.tagName === "SUMMARY")
  )
    return true;
  return (
    target instanceof HTMLInputElement &&
    (target.type === "checkbox" || target.type === "radio")
  );
}
