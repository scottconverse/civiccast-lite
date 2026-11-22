"use strict";
const select = document.querySelector("#channel");
const video = document.querySelector("video");
const status = document.querySelector("#status");
const requested = () => new URL(location.href).searchParams.get("channel");
let player;
let timer;
let active;
let version = 0;
let intent = false;
let resumeOnCanPlay = false;
function tune(id, resume = false) {
  clearTimeout(timer);
  const v = ++version;
  if (player) player.destroy();
  player = undefined;
  active = id;
  video.removeAttribute("src");
  video.load();
  intent = resume;
  resumeOnCanPlay = resume;
  if (!id) return;
  select.value = id;
  const url = "./" + encodeURIComponent(id) + "/live.m3u8";
  status.textContent = "Connecting to " + select.selectedOptions[0].textContent + "…";
  if (window.Hls && Hls.isSupported()) {
    player = new Hls();
    player.on(Hls.Events.ERROR, (_, data) => { if (data.fatal && v === version) reconnect(); });
    player.loadSource(url);
    player.attachMedia(video);
  } else if (video.canPlayType("application/vnd.apple.mpegurl")) {
    video.src = url;
  } else {
    status.textContent = "This browser cannot play the station’s live video.";
  }
}
function reconnect() {
  if (!active) return;
  status.textContent = "Waiting for the channel to reconnect…";
  clearTimeout(timer);
  const v = version;
  timer = setTimeout(() => { if (v === version) tune(active, intent); }, 3500);
}
video.addEventListener("error", reconnect);
video.addEventListener("play", () => { intent = true; });
video.addEventListener("pause", () => { if (video.readyState >= 1 && !video.error) intent = false; });
video.addEventListener("playing", () => { status.textContent = select.selectedOptions[0]?.textContent || ""; });
video.addEventListener("canplay", () => {
  const v = version;
  if (resumeOnCanPlay) {
    resumeOnCanPlay = false;
    video.play().catch(() => {
      if (v === version && video.paused) status.textContent = "Press play to watch " + select.selectedOptions[0]?.textContent + ".";
    });
  } else if (video.paused) {
    status.textContent = "Press play to watch " + select.selectedOptions[0]?.textContent + ".";
  }
});
select.addEventListener("change", () => {
  const url = new URL(location.href);
  url.searchParams.set("channel", select.value);
  history.pushState({}, "", url);
  tune(select.value);
});
async function refresh() {
  try {
    const response = await fetch("./channels.json", {cache:"no-store"});
    if (!response.ok) throw Error();
    const channels = await response.json();
    const selected = requested() || active || channels[0]?.id;
    select.replaceChildren(...channels.map(c => new Option(c.name, c.id)));
    select.disabled = !channels.length;
    if (channels.some(c => c.id === selected)) {
      select.value = selected;
      if (selected !== active) tune(selected);
    } else {
      tune(null);
      select.selectedIndex = -1;
      status.textContent = channels.length ? "This channel is unavailable. Choose another channel." : "No channels are on air.";
    }
  } catch {
    if (!active) status.textContent = "The channel list is unavailable. Retrying…";
  }
}
window.addEventListener("popstate", refresh);
async function poll() { await refresh(); setTimeout(poll, 15000); }
poll();
