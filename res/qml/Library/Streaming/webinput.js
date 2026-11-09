.pragma library

// What the streaming hub's smart box was fed:
//   {kind: "search", query}  words: search Navidrome (when connected) or Audius
//   {kind: "audius", query}  an audius.co link: search Audius for its track
//   {kind: "url", url}       an audio file or a radio/stream link: load it
//   {kind: "server", url}    anything else with an address: a Navidrome /
//                            Subsonic server to log in to
const kAudio = /\.(mp3|m4a|aac|flac|ogg|oga|opus|wav|aiff?|wma|pls|m3u8?|asx|xspf)$/i;
const kStreamPath = /(^|\/)(stream|listen|live|radio|icecast|;)(\/|$|[.;?])/i;
const kStreamHost = /^(stream|streams|listen|live|radio|icecast|shoutcast|ice\d*)[.-]/i;
const kServerHost = /navidrome|subsonic|airsonic|gonic|ampache|funkwhale/i;

function classify(input) {
    const text = (input || "").trim();
    if (text === "")
        return {"kind": "none"};
    let url = "";
    if (/^[a-z][a-z0-9+.-]*:\/\//i.test(text))
        url = text;
    else if (!/\s/.test(text) && /^[\w-]+(\.[\w-]+)+(:\d+)?([/?#]\S*)?$/.test(text) && /\.[a-z]{2,}(:\d+)?([/?#]|$)|^\d+(\.\d+){3}/i.test(text))
        url = "http" + (/^\d+(\.\d+){3}/.test(text) ? "" : "s") + "://" + text;
    else if (/^localhost(:\d+)?(\/|$)/i.test(text))
        url = "http://" + text;
    if (url === "")
        return {"kind": "search", "query": text};
    const m = url.match(/^([a-z][a-z0-9+.-]*):\/\/([^/?#]+)([^?#]*)/i);
    if (!m)
        return {"kind": "search", "query": text};
    const scheme = m[1].toLowerCase();
    const host = m[2].toLowerCase();
    const path = decodeURIComponent(m[3]);
    if (/(^|\.)audius\.co$/.test(host)) {
        const slug = path.split("/").filter(s => s !== "").pop() || "";
        return {"kind": "audius", "query": slug.replace(/-\d+$/, "").replace(/[-_]+/g, " ").trim()};
    }
    if (scheme !== "http" && scheme !== "https")
        return {"kind": "url", "url": url}; // icy://, rtsp://, mms://...
    if (kAudio.test(path) || kStreamPath.test(path) || kStreamHost.test(host))
        return {"kind": "url", "url": url};
    const app = path.search(/\/(app|rest)(\/|$)/i);
    if (path === "" || path === "/" || app >= 0 || kServerHost.test(host))
        return {"kind": "server", "url": scheme + "://" + m[2] + (app >= 0 ? path.slice(0, app) : path.replace(/\/+$/, ""))};
    return {"kind": "url", "url": url};
}
