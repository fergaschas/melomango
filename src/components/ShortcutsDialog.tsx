import { useEffect } from "react";
import { SHORTCUTS } from "../hooks/useShortcuts";
import { CloseIcon } from "./Icons";

export function ShortcutsDialog({ onClose }: { onClose: () => void }) {
  useEffect(() => {
    const esc = (e: KeyboardEvent) => (e.key === "Escape" || e.key === "?") && onClose();
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onClose]);

  return (
    <div className="dialog-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section className="dialog" role="dialog" aria-modal="true" aria-labelledby="shortcuts-title">
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
