import { useEffect } from "react";
import { SHORTCUTS } from "../hooks/useShortcuts";
import { CloseIcon } from "./Icons";
import { useDialog } from "../lib/useDialog";

export function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  const dialog = useDialog<HTMLElement>(onClose);
  // «?» opens it and closes it again (Escape is handled by useDialog).
  useEffect(() => {
    const help = (e: KeyboardEvent) => e.key === "?" && onClose();
    window.addEventListener("keydown", help);
    return () => window.removeEventListener("keydown", help);
  }, [onClose]);

  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section ref={dialog} tabIndex={-1} className="dialog" role="dialog" aria-modal="true" aria-labelledby="shortcuts-title">
        <header className="dialog-header">
          <h2 id="shortcuts-title">Atajos de teclado</h2>
          <button className="icon-btn" onClick={onClose} aria-label="Cerrar">
            <CloseIcon />
          </button>
        </header>
        <dl className="shortcuts">
          {SHORTCUTS.map(({ combos, sep = "/", action }) => (
            <div key={action} className="shortcut">
              <dt>
                {combos.map((keys, i) => (
                  <span key={i} className="shortcut-combo">
                    {i > 0 && <span className="shortcut-sep">{sep}</span>}
                    {keys.map((k, j) => (
                      <span key={j}>
                        {j > 0 && <span className="shortcut-plus">+</span>}
                        <kbd>{k}</kbd>
                      </span>
                    ))}
                  </span>
                ))}
              </dt>
              <dd>{action}</dd>
            </div>
          ))}
        </dl>
      </section>
    </div>
  );
}
