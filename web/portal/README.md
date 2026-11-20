# Watch live on the station website

Set `CIVICCAST_PORTAL_ROOT` in the station service environment to the absolute path of a dedicated website directory. Its parent must exist and the service account must have write access. Keep the site in a separate tree from the private station database, media library and service credentials.

Start the station and enable a channel in the operator console. The service puts the player, its local HLS library and a public channel list in the site directory. Each running channel writes a rolling playlist and transport stream segments in its own channel-ID directory. The player uses the channel display name from the console.

Serve this directory as a static HTTPS website on the station's web server. Use `index.html` as the default document. Serve `.m3u8` as `application/vnd.apple.mpegurl` and `.ts` as `video/mp2t`. Require revalidation for playlists and `channels.json`. Give the web server read access. Disable directory listings. The operator API and database belong on the private station network.

Open the site, choose a channel and press Play. A link such as `/?channel=<channel-uuid>` selects a specific channel. The page, scripts, channel list and media all load from the station website. A YouTube account is optional for website playback.

To send the same channel to a streaming service, use the channel distribution controls in the operator console as a Setup Admin. Enter the full RTMP or RTMPS ingest URL for the intended broadcast, including its stream key. The console returns whether a destination is configured. Disabling that destination leaves the channel's cable and portal outputs enabled.

The bundled browser library is pinned in package-lock.json. To refresh it after changing the version, run `npm ci` in this directory and copy `node_modules/hls.js/dist/hls.light.min.js` and its package LICENSE to the corresponding tracked files. The station embeds the public assets at build time.
