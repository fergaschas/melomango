import { HeartFilledIcon, NoteIcon } from "./Icons";

/** 2×2 mosaic of the first covers, or a mango gradient with an icon. */
export function PlaylistCover({
  artworkUrls = [],
  kind = "playlist",
  size,
}: {
  artworkUrls?: string[];
  kind?: "playlist" | "likes";
  size: number;
}) {
  const style = { width: size, height: size };
  if (kind === "playlist" && artworkUrls.length >= 4) {
    return (
      <span className="cover mosaic" style={style}>
        {artworkUrls.slice(0, 4).map((src) => (
          <img key={src} src={src} alt="" loading="lazy" />
        ))}
      </span>
    );
  }
  if (kind === "playlist" && artworkUrls.length > 0) {
    return (
      <span className="cover" style={style}>
        <img src={artworkUrls[0]} alt="" loading="lazy" />
      </span>
    );
  }
  return (
    <span className={`cover placeholder ${kind}`} style={style}>
      {kind === "likes" ? <HeartFilledIcon size={size * 0.45} /> : <NoteIcon size={size * 0.45} />}
    </span>
  );
}
