import { useState } from "react";
import { api, errorText } from "../api";
import type { Config, GroupInfo } from "../types";
import { classNames } from "../util";
import { GROUP_COLORS, GROUP_EMOJIS, Modal, Spinner } from "./ui";

export default function GroupDialog({
  group,
  onClose,
  onSaved,
  onDeleted,
}: {
  group: GroupInfo | null; // null = create
  onClose: () => void;
  onSaved: (c: Config, name: string) => void;
  onDeleted?: (c: Config) => void;
}) {
  const [name, setName] = useState(group?.name ?? "");
  const [emoji, setEmoji] = useState(group?.emoji ?? "📁");
  const [color, setColor] = useState(group?.color ?? GROUP_COLORS[0]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const meta = { name: name.trim(), emoji, color };
      const cfg = group ? await api.updateGroup(group.name, meta) : await api.createGroup(meta);
      onSaved(cfg, meta.name);
      onClose();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  const del = async () => {
    if (!group) return;
    setBusy(true);
    try {
      const cfg = await api.deleteGroup(group.name);
      onDeleted?.(cfg);
      onClose();
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
      setConfirmDelete(false);
    }
  };

  return (
    <Modal onClose={onClose}>
      <div className="modal-head">
        <span style={{ fontSize: 30 }}>{emoji}</span>
        <h2 className="grow">{group ? "Edit group" : "New group"}</h2>
        <button className="icon-btn" onClick={onClose}>
          ✕
        </button>
      </div>
      <div className="col" style={{ gap: 16 }}>
        <div className="field">
          <label>Name</label>
          <input
            className="input"
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && name.trim() && save()}
          />
          {group && name.trim() && name.trim() !== group.name && (
            <div className="hint">The folder on disk will be renamed too.</div>
          )}
        </div>
        <div className="field">
          <label>Icon</label>
          <div className="emoji-pick">
            {GROUP_EMOJIS.map((e) => (
              <button key={e} className={classNames(emoji === e && "on")} onClick={() => setEmoji(e)}>
                {e}
              </button>
            ))}
          </div>
        </div>
        <div className="field">
          <label>Color</label>
          <div className="color-pick">
            {GROUP_COLORS.map((c) => (
              <button key={c} className={classNames(color === c && "on")} style={{ background: c }} onClick={() => setColor(c)} />
            ))}
          </div>
        </div>
        {error && (
          <div className="notice error">
            <span className="ico">⚠️</span>
            <span>{error}</span>
          </div>
        )}
      </div>
      <div className="modal-foot">
        {group && (
          <button
            className={classNames("btn", confirmDelete ? "red" : "ghost gray")}
            onClick={() => (confirmDelete ? del() : setConfirmDelete(true))}
            disabled={busy}
            style={{ marginRight: "auto" }}
          >
            {confirmDelete ? "Really delete?" : "Delete"}
          </button>
        )}
        <button className="btn ghost gray" onClick={onClose}>
          Cancel
        </button>
        <button className="btn green" onClick={save} disabled={!name.trim() || busy}>
          {busy ? <Spinner /> : "Save"}
        </button>
      </div>
    </Modal>
  );
}
