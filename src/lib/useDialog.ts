import { useEffect, useRef, useState } from "react";

const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Keyboard behaviour of a modal dialog: put the ref on its root element.
 *
 * - Focus moves into it when it opens (to an element marked `data-autofocus`, else the
 *   first control that isn't the close button) and goes back to what had it on close.
 * - Tab and Shift+Tab stay inside it.
 * - Escape closes it (unless something inside, like an open menu, handled it first).
 */
export function useDialog<T extends HTMLElement>(onClose: () => void) {
  const ref = useRef<T>(null);
  // What had the focus when the dialog was asked for. Read while rendering: by the time
  // effects run, an autoFocus field inside the dialog has already taken it.
  const [opener] = useState(() => (document.activeElement instanceof HTMLElement ? document.activeElement : null));
  const close = useRef(onClose);
  useEffect(() => {
    close.current = onClose;
  });

  useEffect(() => {
    const root = ref.current;
    if (!root) return;
    const focusables = () =>
      [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => el.offsetParent !== null || el === document.activeElement);

    const first =
      root.querySelector<HTMLElement>("[data-autofocus]") ??
      focusables().find((el) => !el.matches("[aria-label^='Cerrar']")) ??
      root;
    if (!root.contains(document.activeElement)) first.focus();

    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return;
      if (e.key === "Escape") {
        e.preventDefault();
        close.current();
        return;
      }
      if (e.key !== "Tab") return;
      const list = focusables();
      if (list.length === 0) {
        e.preventDefault();
        root.focus();
        return;
      }
      const [head, tail] = [list[0]!, list[list.length - 1]!];
      const inside = root.contains(document.activeElement);
      if (e.shiftKey && (document.activeElement === head || !inside)) {
        e.preventDefault();
        tail.focus();
      } else if (!e.shiftKey && (document.activeElement === tail || !inside)) {
        e.preventDefault();
        head.focus();
      }
    };
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("keydown", onKey);
      // Back to where the user was (if it is still on the page).
      if (opener?.isConnected) opener.focus();
    };
  }, [opener]);

  return ref;
}
