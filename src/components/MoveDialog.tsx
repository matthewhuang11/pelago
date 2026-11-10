import { useEffect, useState } from "react";
import { api, errorText, type MovePreview, type MoveRecord } from "../api";
import type { GroupInfo, Project } from "../types";
import { classNames } from "../util";
import { Modal, Spinner } from "./ui";

export default function MoveDialog({
  project,
  groups,
  onClose,
  onMoved,
}: {
  project: Project;
  groups: GroupInfo[];
  onClose: () => void;
  onMoved: (rec: MoveRecord) => void;
}) {
  const [group, setGroup] = useState<string | null>(
    project.suggestedGroup ?? (project.group && project.location === "root" ? null : groups[0]?.name ?? null),
  );
  const [rename, setRename] = useState("");
  const [previewResult, setPreviewResult] = useState<{ key: string; value: MovePreview } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const previewKey = JSON.stringify([project.path, group, rename.trim()]);
  const preview = previewResult?.key === previewKey ? previewResult.value : null;

  useEffect(() => {
    let live = true;
    setPreviewResult(null);
    setError(null);
    const t = setTimeout(() => {
      api
        .previewMove(project.path, group, rename.trim() || undefined)
        .then((p) => live && setPreviewResult({ key: previewKey, value: p }))
        .catch((e) => live && setError(errorText(e)));
    }, 120);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [project.path, group, rename, previewKey]);

  const go = async () => {
    if (busy || !preview?.ok) return;
    setBusy(true);
    setError(null);
    try {
      const rec = await api.moveProject(project.path, group, rename.trim() || undefined);
      onMoved(rec);
      onClose();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  return (
    <Modal onClose={busy ? () => {} : onClose}>
      <div className="modal-head">
        <span style={{ fontSize: 28 }}>🧭</span>
        <div className="grow">
          <h2>Move {project.name}</h2>
          <div className="muted mono" style={{ fontSize: 12 }}>
            {project.displayPath}
          </div>
        </div>
        <button className="icon-btn" onClick={onClose} disabled={busy}>
          ✕
        </button>
      </div>

      <div className="field">
        <label>Into group</label>
        <div className="choices" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(130px, 1fr))" }}>
          {groups.map((g) => (
            <button
              key={g.name}
              className={classNames("choice", group === g.name && "on")}
              disabled={busy}
              onClick={() => setGroup(g.name)}
            >
              <span className="e">{g.emoji}</span>
              <span className="t ellipsis">{g.name}</span>
              {project.suggestedGroup === g.name && <span className="d">Suggested</span>}
            </button>
          ))}
          <button className={classNames("choice", group === null && "on")} disabled={busy} onClick={() => setGroup(null)}>
            <span className="e">🏝️</span>
            <span className="t">No group</span>
          </button>
        </div>
      </div>

      <div className="field" style={{ marginTop: 14 }}>
        <label>Rename (optional)</label>
        <input className="input" placeholder={project.name} value={rename} disabled={busy} onChange={(e) => setRename(e.target.value)} />
      </div>

      <div style={{ marginTop: 16 }} className="col">
        {!preview ? (
          !error && <div className="row" role="status"><Spinner dark /> Checking destination…</div>
        ) : (
          <>
            <div className="notice info">
              <span className="ico">➡️</span>
              <span className="mono" style={{ fontSize: 13, wordBreak: "break-all" }}>
                {preview.toDisplay}
              </span>
            </div>
            {preview.blockers.map((b) => (
              <div key={b} className="notice error">
                <span className="ico">⛔</span>
                <span>{b}</span>
              </div>
            ))}
            {preview.warnings.map((w) => (
              <div key={w} className="notice warn">
                <span className="ico">⚠️</span>
                <span style={{ fontSize: 14 }}>{w}</span>
              </div>
            ))}
          </>
        )}
        {error && (
          <div className="notice error">
            <span className="ico">⚠️</span>
            <span>{error}</span>
          </div>
        )}
      </div>

      <div className="modal-foot">
        <span className="muted grow" style={{ fontSize: 13, alignSelf: "center" }}>
          You can undo this afterwards.
        </span>
        <button className="btn ghost gray" onClick={onClose} disabled={busy}>
          Cancel
        </button>
        <button className="btn green" onClick={go} disabled={!preview?.ok || busy}>
          {busy ? <Spinner /> : "Move"}
        </button>
      </div>
    </Modal>
  );
}
