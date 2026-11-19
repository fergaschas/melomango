import { useState } from "react";

/** Cover image that falls back to the mango placeholder when missing or broken. */
export function Artwork({ src }: { src: string | null | undefined }) {
  const [failed, setFailed] = useState<string | null>(null);
  if (!src || failed === src) return <span className="art-placeholder" />;
  return <img src={src} alt="" loading="lazy" onError={() => setFailed(src)} />;
}
