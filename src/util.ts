export function timeAgo(secs: number | null | undefined): string {
  if (!secs) return "never";
  const diff = Date.now() / 1000 - secs;
  if (diff < 60) return "just now";
  const units: [number, string][] = [
    [60 * 60 * 24 * 365, "y"],
    [60 * 60 * 24 * 30, "mo"],
    [60 * 60 * 24 * 7, "w"],
    [60 * 60 * 24, "d"],
    [60 * 60, "h"],
    [60, "m"],
  ];
  for (const [size, label] of units) {
    if (diff >= size) return `${Math.floor(diff / size)}${label} ago`;
  }
  return "just now";
}

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null) return "";
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v >= 10 ? v.toFixed(0) : v.toFixed(1)} ${units[i]}`;
}

const STACK_COLORS: Record<string, string> = {
  React: "#1cb0f6",
  "Next.js": "#4b4b4b",
  Vue: "#58cc02",
  Svelte: "#ff4b4b",
  TypeScript: "#2b70c9",
  JavaScript: "#e5b800",
  Python: "#3776ab",
  Jupyter: "#f37626",
  Rust: "#c2410c",
  Go: "#00acd7",
  Swift: "#ff7a00",
  Java: "#b07219",
  Kotlin: "#a97bff",
  "C++": "#f34b7d",
  C: "#555",
  Ruby: "#cc342d",
  PHP: "#777bb4",
  HTML: "#e34c26",
  Flutter: "#02569b",
  Tauri: "#ffc131",
  Electron: "#47848f",
  TouchDesigner: "#9b59b6",
};

export function stackColor(s: string): string {
  return STACK_COLORS[s] ?? "#777";
}

const STACK_EMOJI: Record<string, string> = {
  React: "⚛️",
  "Next.js": "▲",
  Vue: "💚",
  Svelte: "🔥",
  TypeScript: "📘",
  JavaScript: "📒",
  Python: "🐍",
  Jupyter: "📓",
  Rust: "🦀",
  Go: "🐹",
  Swift: "🕊️",
  Java: "☕",
  Kotlin: "🟣",
  "C++": "⚙️",
  C: "⚙️",
  Ruby: "💎",
  PHP: "🐘",
  HTML: "🌐",
  Flutter: "🦋",
  TouchDesigner: "🎛️",
};

export function projectEmoji(stack: string[]): string {
  for (const s of stack) if (STACK_EMOJI[s]) return STACK_EMOJI[s];
  return "📁";
}

export function githubUrl(slug: string | null | undefined): string | null {
  return slug ? `https://github.com/${slug}` : null;
}

export function classNames(...xs: (string | false | null | undefined)[]): string {
  return xs.filter(Boolean).join(" ");
}
