import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { LibraryProvider } from "./lib/library";
import { DownloadsProvider } from "./lib/downloads";
import { applyTheme, savedTheme } from "./lib/theme";

// Before the first paint, so the app doesn't flash in the other theme.
applyTheme(savedTheme());

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <LibraryProvider>
      <DownloadsProvider>
        <App />
      </DownloadsProvider>
    </LibraryProvider>
  </React.StrictMode>,
);
