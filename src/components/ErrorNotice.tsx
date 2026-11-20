import type { ReactNode } from "react";
import { friendlyError } from "../lib/errors";

/** An error in plain words, with "Reintentar" when it may help and the original text as details. */
export function ErrorNotice({
  error,
  onRetry,
  children,
}: {
  error: unknown;
  onRetry?: () => void;
  /** Extra actions (e.g. "Instalar lo que falta"). */
  children?: ReactNode;
}) {
  const e = friendlyError(error);
  return (
    <div className="error-notice" role="alert">
      <p>{e.message}</p>
      {(onRetry || children || e.details) && (
        <div className="error-actions">
          {onRetry && e.retryable && (
            <button className="btn ghost small" onClick={onRetry}>
              Reintentar
            </button>
          )}
          {children}
          {e.details && (
            <details>
              <summary>Detalles</summary>
              <code>{e.details}</code>
            </details>
          )}
        </div>
      )}
    </div>
  );
}
