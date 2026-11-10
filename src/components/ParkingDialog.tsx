import { useEffect, useState } from "react";
import { api, errorText, type ParkPreview } from "../api";
import type { Project } from "../types";
import { formatBytes } from "../util";
import { Modal, Spinner } from "./ui";
import { useToast } from "./Toast";

export function strongPassphrase(value: string) {
  const classes = [/\p{Ll}/u, /\p{Lu}/u, /\p{N}/u, /[^\p{L}\p{N}]/u].filter((re) => re.test(value)).length;
  return Array.from(value).length >= 16 && (classes >= 3 || value.trim().split(/\s+/).length >= 6);
}

export default function ParkingDialog({ project, restore, vaultRepo, onClose, onComplete }: {
  project: Project; restore: boolean; vaultRepo: string | null;
  onClose: () => void; onComplete: () => void | Promise<void>;
}) {
  const toast = useToast();
  const [preview, setPreview] = useState<ParkPreview | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [passphrase, setPassphrase] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [saved, setSaved] = useState(false);
  const [closed, setClosed] = useState(false);
  const [unrecoverable, setUnrecoverable] = useState(false);
  const [deletion, setDeletion] = useState("");
  const [busy, setBusy] = useState(false);
  const [restored, setRestored] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (restore) return;
    let active = true;
    setPreviewError(null);
    api.previewPark(project.path).then((p) => { if (active) setPreview(p); }).catch((e) => { if (active) setPreviewError(errorText(e)); });
    return () => { active = false; };
  }, [project.path, restore, attempt]);

  const ready = restore ? !!passphrase : !!vaultRepo && !!preview && saved && closed && unrecoverable && strongPassphrase(passphrase) && passphrase === confirmation && deletion === `DELETE ${project.name}`;
  const submit = async () => {
    if (!ready || busy) return;
    setBusy(true);
    setError(null);
    try {
      if (restore) {
        await api.restoreProject(project.path, passphrase);
        setRestored(true);
      } else await api.parkProject(project.path, passphrase);
      setPassphrase("");
      setConfirmation("");
      await onComplete();
      if (restore) {
        try { await api.openProject(project.path, project.editor ?? undefined); }
        catch (e) { setError(`Files restored successfully, but the editor could not open: ${errorText(e)}`); return; }
      }
      toast.show(restore ? `Restored ${project.name}` : `Parked ${project.name}`, "success");
      onClose();
    } catch (e) {
      // Never echo a backend error containing a secret back into the interface.
      const message = errorText(e);
      setError(passphrase ? message.split(passphrase).join("[redacted]") : message);
      setPassphrase("");
      setConfirmation("");
    } finally { setBusy(false); }
  };

  return <Modal onClose={busy ? () => {} : onClose} wide>
    <div className="modal-head"><span style={{ fontSize: 30 }}>{restore ? "🌊" : "☁️"}</span><h2 className="grow">{restore ? "Restore" : "Park"} {project.name}</h2><button className="icon-btn" disabled={busy} onClick={onClose} aria-label="Close">✕</button></div>
    <div className="col" style={{ gap: 14 }}>
      <div className="notice info"><div><b>{restore ? "Bring your saved files back" : "An encrypted snapshot, not just a git push"}</b><div>{restore ? "The snapshot is decrypted to its original path. An existing destination will not be overwritten." : "Includes saved files, uncommitted work and .git history in your private GitHub vault. Local files are removed only after upload verification."}</div></div></div>
      <div className="mono muted" style={{ overflowWrap: "anywhere" }}>{project.displayPath}</div>
      <p className="muted" style={{ margin: 0 }}>Unsaved editor buffers are not included in snapshots. Editor tabs are not guaranteed to restore. Processes and running sessions are not restored. Save your work before parking.</p>
      {!restore && <div className="hint">Encrypted snapshots use chunked uploads and need substantial temporary disk space for verification. Unsupported links, special files, linked worktrees and macOS extended attributes/ACLs are blocked rather than silently omitted. Ownership and creation times are not preserved.</div>}
      {!restore && <>
        {!vaultRepo && <div className="notice error" role="alert">Configure a private vault repository in Settings before parking.</div>}
        {!preview && !previewError && <div className="row" role="status"><Spinner dark /> Measuring full snapshot…</div>}
        {previewError && <div className="notice error" role="alert"><span className="grow">{previewError}</span><button className="btn small" onClick={() => setAttempt((n) => n + 1)}>Retry</button></div>}
        {preview && <><div className="parking-summary"><b>{formatBytes(preview.sizeBytes)}</b><span>{preview.fileCount.toLocaleString()} files · encrypted private snapshot</span></div>{preview.warnings.map((warning, i) => <div className="notice warn" key={i}>{warning}</div>)}</>}
        <label className="parking-check"><input type="checkbox" checked={saved} disabled={busy} onChange={(e) => setSaved(e.target.checked)} /><span>I have saved <b>all editor buffers</b> to disk.</span></label>
        <label className="parking-check"><input type="checkbox" checked={closed} disabled={busy} onChange={(e) => setClosed(e.target.checked)} /><span>I have closed editors and stopped terminals, dev servers and other processes using this folder.</span></label>
      </>}
      {!restored && <>
        <div className="field"><label htmlFor="parking-passphrase">{restore ? "Snapshot passphrase" : "Choose a strong passphrase"}</label><input id="parking-passphrase" className="input" type="password" autoComplete="off" spellCheck={false} disabled={busy} value={passphrase} onChange={(e) => setPassphrase(e.target.value)} autoFocus={restore} /><div className="hint">{restore ? "Use the passphrase chosen when this snapshot was parked." : "At least 16 characters, using 3 character types or at least 6 words. Store it somewhere safe yourself."}</div></div>
        {!restore && <>
          <div className="field"><label htmlFor="parking-confirm">Confirm passphrase</label><input id="parking-confirm" className="input" type="password" autoComplete="off" spellCheck={false} disabled={busy} value={confirmation} onChange={(e) => setConfirmation(e.target.value)} />{confirmation && confirmation !== passphrase && <div className="hint error">Passphrases do not match.</div>}</div>
          <label className="parking-check"><input type="checkbox" checked={unrecoverable} disabled={busy} onChange={(e) => setUnrecoverable(e.target.checked)} /><span>I understand that <b>a lost passphrase is unrecoverable</b>. Pelago never saves it.</span></label>
          <div className="field"><label htmlFor="parking-delete">Delete this local project after verified upload</label><input id="parking-delete" className="input mono" autoComplete="off" disabled={busy} placeholder={`DELETE ${project.name}`} value={deletion} onChange={(e) => setDeletion(e.target.value)} /><div className="hint">Type <b>DELETE {project.name}</b> to explicitly authorize local removal.</div></div>
        </>}
      </>}
      {error && <div className="notice error" role="alert">{error}</div>}
      {busy && <div className="row" role="status" aria-live="polite"><Spinner dark />{restore ? "Downloading, decrypting and restoring…" : "Encrypting, uploading and verifying before local removal…"} Keep Pelago open.</div>}
    </div>
    <div className="modal-foot"><button className="btn ghost gray" disabled={busy} onClick={onClose}>{restored ? "Done" : "Cancel"}</button>{!restored && <button className="btn" disabled={!ready || busy} onClick={submit}>{restore ? "Restore & open editor" : "Encrypt, park & delete local"}</button>}</div>
  </Modal>;
}
