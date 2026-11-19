import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { LibraryProvider } from "./lib/library";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <LibraryProvider>
      <App />
    </LibraryProvider>
  </React.StrictMode>,
);
