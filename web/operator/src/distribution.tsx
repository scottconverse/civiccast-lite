import { FormEvent, useEffect, useState } from "react";
import { apiRequest } from "./api-client";

interface Channel { channel_id: string; display_name: string }

interface Progress {
  rtmp_configured: boolean;
  rtmp_last_progress_unix: number | null;
  portal_last_progress_unix: number | null;
}

function observed(time: number | null) {
  if (time === null) return "Awaiting output";
  return Date.now() / 1000 - time < 15 ? "Sending" : "Last sent " + new Date(time * 1000).toLocaleTimeString();
}

export function DistributionPanel({ channels }: { channels: Channel[] }) {
  const [selected, select] = useState("");
  const id = channels.some(channel => channel.channel_id === selected) ? selected : channels[0]?.channel_id;
  const [configured, setConfigured] = useState<boolean | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const endpoint = "/api/v1/channels/" + encodeURIComponent(id ?? "") + "/distribution";

  useEffect(() => {
    const controller = new AbortController();
    setConfigured(null);
    setError("");
    setUrl("");
    setProgress(null);
    let timer: ReturnType<typeof setTimeout>;
    const refresh = () => apiRequest<Progress>(endpoint, { signal: controller.signal })
      .then(result => { if (!controller.signal.aborted) {setConfigured(result.rtmp_configured); setProgress(result); } })
      .catch((e: Error) => { if (!controller.signal.aborted) setError(e.message); })
      .finally(() => { if (!controller.signal.aborted) timer = setTimeout(refresh, 5000); });
    if (id) void refresh();
    return () => {controller.abort(); clearTimeout(timer);};
  }, [id, endpoint]);

  async function save(enabled: boolean) {
    setBusy(true);
    setError("");
    try {
      const body = enabled ? { enabled, rtmp_url: url.trim() } : { enabled };
      const result = await apiRequest<Progress>(endpoint, { method: "PUT", body: JSON.stringify(body) });
      setUrl("");
      setConfigured(result.rtmp_configured);
      setProgress(result);
    } catch (e) { setError(e instanceof Error ? e.message : "Could not update the publisher."); }
    finally { setBusy(false); }
  }

  if (!id) return null;
  return <section className="panel"><h2>Continuous streaming output</h2>
    <label>Channel<select value={id} disabled={busy} onChange={e => select(e.target.value)}>
      {channels.map(c => <option key={c.channel_id} value={c.channel_id}>{c.display_name}</option>)}
    </select></label>
    <p aria-live="polite">{configured === null ? "" : configured ? "A streaming destination is configured." : "Streaming destination is off."}</p>
    {progress && <dl><dt>Streaming output</dt><dd>{configured ? observed(progress.rtmp_last_progress_unix) : "Off"}</dd><dt>Portal feed</dt><dd>{observed(progress.portal_last_progress_unix)}</dd></dl>}
    <form onSubmit={(e: FormEvent) => { e.preventDefault(); void save(true); }}>
      <label>RTMP or RTMPS URL including the stream key<input type="password" autoComplete="off" spellCheck={false} value={url}
        onChange={e => setUrl(e.target.value)} required disabled={busy || configured === null} /></label>
      <button disabled={busy || configured === null || !url.trim()}>Save destination</button>
      {configured && <button type="button" disabled={busy} onClick={() => void save(false)}>Turn off streaming</button>}
    </form>
    {error && <p role="alert">{error}</p>}
  </section>;
}
