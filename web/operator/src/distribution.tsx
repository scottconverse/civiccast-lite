import { FormEvent, useEffect, useState } from "react";
import { apiRequest } from "./api-client";

interface Channel { channel_id: string; display_name: string }

export function DistributionPanel({ channels }: { channels: Channel[] }) {
  const [selected, select] = useState("");
  const id = channels.some(channel => channel.channel_id === selected) ? selected : channels[0]?.channel_id;
  const [configured, setConfigured] = useState<boolean | null>(null);
  const [url, setUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const endpoint = "/api/v1/channels/" + encodeURIComponent(id ?? "") + "/distribution";

  useEffect(() => {
    const controller = new AbortController();
    setConfigured(null);
    setError("");
    setUrl("");
    if (id) apiRequest<{ rtmp_configured: boolean }>(endpoint, { signal: controller.signal })
      .then(result => { if (!controller.signal.aborted) setConfigured(result.rtmp_configured); })
      .catch((e: Error) => { if (!controller.signal.aborted) setError(e.message); });
    return () => controller.abort();
  }, [id, endpoint]);

  async function save(enabled: boolean) {
    setBusy(true);
    setError("");
    try {
      const body = enabled ? { enabled, rtmp_url: url.trim() } : { enabled };
      const result = await apiRequest<{ rtmp_configured: boolean }>(endpoint, { method: "PUT", body: JSON.stringify(body) });
      setUrl("");
      setConfigured(result.rtmp_configured);
    } catch (e) { setError(e instanceof Error ? e.message : "Could not update the publisher."); }
    finally { setBusy(false); }
  }

  if (!id) return null;
  return <section className="panel"><h2>Continuous streaming output</h2>
    <label>Channel<select value={id} disabled={busy} onChange={e => select(e.target.value)}>
      {channels.map(c => <option key={c.channel_id} value={c.channel_id}>{c.display_name}</option>)}
    </select></label>
    <p aria-live="polite">{configured === null ? "" : configured ? "A streaming destination is configured." : "Streaming destination is off."}</p>
    <form onSubmit={(e: FormEvent) => { e.preventDefault(); void save(true); }}>
      <label>RTMP or RTMPS URL including the stream key<input type="password" autoComplete="off" spellCheck={false} value={url}
        onChange={e => setUrl(e.target.value)} required disabled={busy || configured === null} /></label>
      <button disabled={busy || configured === null || !url.trim()}>Save destination</button>
      {configured && <button type="button" disabled={busy} onClick={() => void save(false)}>Turn off streaming</button>}
    </form>
    {error && <p role="alert">{error}</p>}
  </section>;
}
