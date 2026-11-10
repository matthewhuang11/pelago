import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from "react";

type Kind = "info" | "success" | "error";
interface Toast {
  id: number;
  kind: Kind;
  text: string;
  action?: { label: string; run: () => void };
}

interface ToastApi {
  show: (text: string, kind?: Kind, action?: Toast["action"]) => void;
  error: (e: unknown) => void;
}

const Ctx = createContext<ToastApi>({ show: () => {}, error: () => {} });

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const next = useRef(1);

  const show = useCallback((text: string, kind: Kind = "info", action?: Toast["action"]) => {
    const id = next.current++;
    setToasts((t) => [...t.slice(-3), { id, kind, text, action }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), action ? 9000 : kind === "error" ? 7000 : 3500);
  }, []);

  const error = useCallback(
    (e: unknown) => show(typeof e === "string" ? e : e instanceof Error ? e.message : String(e), "error"),
    [show],
  );

  const value = useMemo(() => ({ show, error }), [show, error]);

  return (
    <Ctx.Provider value={value}>
      {children}
      <div className="toasts">
        {toasts.map((t) => (
          <div key={t.id} className={`toast ${t.kind}`}>
            <span>{t.kind === "success" ? "✅" : t.kind === "error" ? "⚠️" : "💬"}</span>
            <span>{t.text}</span>
            {t.action && (
              <button
                onClick={() => {
                  t.action!.run();
                  setToasts((all) => all.filter((x) => x.id !== t.id));
                }}
              >
                {t.action.label}
              </button>
            )}
          </div>
        ))}
      </div>
    </Ctx.Provider>
  );
}

export const useToast = () => useContext(Ctx);
