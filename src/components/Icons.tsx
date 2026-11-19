type Props = { size?: number };

const svg = (path: string) =>
  function Icon({ size = 20 }: Props) {
    return (
      <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d={path} />
      </svg>
    );
  };

export const PlayIcon = svg("M8 5.14v13.72a1 1 0 0 0 1.5.86l11-6.86a1 1 0 0 0 0-1.72l-11-6.86A1 1 0 0 0 8 5.14Z");
export const PauseIcon = svg("M7 4h3.5v16H7zM13.5 4H17v16h-3.5z");
export const NextIcon = svg("M5 5.5v13a1 1 0 0 0 1.53.85L16 13.2V19h2.5V5H16v5.8L6.53 4.65A1 1 0 0 0 5 5.5Z");
export const PrevIcon = svg("M19 5.5v13a1 1 0 0 1-1.53.85L8 13.2V19H5.5V5H8v5.8l9.47-6.15A1 1 0 0 1 19 5.5Z");
export const VolumeIcon = svg("M4 9v6h4l5 4V5L8 9H4Zm12.5 3a4.5 4.5 0 0 0-2.5-4.03v8.06A4.5 4.5 0 0 0 16.5 12Z");
export const SearchIcon = svg("M10.5 3a7.5 7.5 0 0 1 5.96 12.05l4.25 4.24-1.42 1.42-4.24-4.25A7.5 7.5 0 1 1 10.5 3Zm0 2a5.5 5.5 0 1 0 0 11 5.5 5.5 0 0 0 0-11Z");
export const PuzzleIcon = svg("M10 3a2 2 0 0 1 2 2v1h4a1 1 0 0 1 1 1v4h1a2 2 0 1 1 0 4h-1v4a1 1 0 0 1-1 1h-4v-1a2 2 0 1 0-4 0v1H4a1 1 0 0 1-1-1v-4h1a2 2 0 1 0 0-4H3V7a1 1 0 0 1 1-1h4V5a2 2 0 0 1 2-2Z");
export const CloseIcon = svg("M6.4 5 12 10.6 17.6 5 19 6.4 13.4 12l5.6 5.6-1.4 1.4-5.6-5.6L6.4 19 5 17.6l5.6-5.6L5 6.4 6.4 5Z");
