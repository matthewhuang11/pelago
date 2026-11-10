import type { ReactNode } from "react";
import { classNames } from "../util";

export function Toggle({
  on,
  onChange,
  title,
  desc,
  disabled,
}: {
  on: boolean;
  onChange: (v: boolean) => void;
  title: ReactNode;
  desc?: ReactNode;
  disabled?: boolean;
}) {
  return (
    <div
      className={classNames("toggle", disabled && "disabled")}
      onClick={() => !disabled && onChange(!on)}
      role="switch"
      aria-checked={on}
    >
      <div className="text">
        <div className="t">{title}</div>
        {desc && <div className="d">{desc}</div>}
      </div>
      <div className={classNames("switch", on && "on")} />
    </div>
  );
}

export function Checkbox({ on }: { on: boolean }) {
  return <div className={classNames("checkbox", on && "on")}>{on ? "✓" : ""}</div>;
}

export function Modal({
  children,
  onClose,
  wide,
}: {
  children: ReactNode;
  onClose: () => void;
  wide?: boolean;
}) {
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={classNames("modal", wide && "wide")} role="dialog" aria-modal="true">
        {children}
      </div>
    </div>
  );
}

export function Spinner({ dark }: { dark?: boolean }) {
  return <span className={classNames("spinner", dark && "dark")} />;
}

export const GROUP_EMOJIS = [
  "🚀", "⚡", "🧪", "🎓", "🌐", "📦", "💼", "🎮", "🤖", "🧠",
  "🎨", "📱", "🛠️", "🔬", "📊", "🎵", "🏝️", "🌊", "🔥", "💡",
  "🧩", "📚", "🏆", "❤️", "⭐", "🐍", "🦀", "☕", "🍎", "📁",
];

export const GROUP_COLORS = [
  "#1c9af0", "#58cc02", "#ff9600", "#ffc800", "#ce82ff", "#ff4b4b", "#2b70c9", "#00cd9c", "#ff86d0", "#afafaf",
];
