// What the streaming hub's smart box was fed:
//   {kind: "search", query}  words: search the Navidrome / Subsonic library
//   {kind: "url", url}       an audio file or a radio/stream link: load it
//   {kind: "server", url}    anything else with an address: a Navidrome /
//                            Subsonic server to log in to
const kAudio = /\.(mp3|m4a|aac|flac|ogg|oga|opus|wav|aiff?|wma|pls|m3u8?|asx|xspf)$/i;
const kStreamPath = /(^|\/)(stream|listen|live|radio|icecast|;)(\/|$|[.;?])/i;
const kStreamHost = /^(stream|streams|listen|live|radio|icecast|shoutcast|ice\d*)[.-]/i;
const kServerHost = /navidrome|subsonic|airsonic|gonic|ampache|funkwhale/i;
const kIp = /^\d+(\.\d+){3}/;

/**
 * @param {string} text What was typed or pasted, trimmed
 * @returns {string} The URL it is, or "" for words
 */
const toUrl = function(text) {
    if (/^[a-z][a-z0-9+.-]*:\/\//i.test(text)) {
        return text;
    }
    if (/^localhost(:\d+)?(\/|$)/i.test(text)) {
        return `http://${text}`;
    }
    if (/\s/.test(text) || !/^[\w-]+(\.[\w-]+)+(:\d+)?([/?#]\S*)?$/.test(text)) {
        return "";
    }
    if (kIp.test(text)) {
        return `http://${text}`;
    }
    return /\.[a-z]{2,}(:\d+)?([/?#]|$)/i.test(text) ? `https://${text}` : "";
};

/**
 * @param {string} input What the user typed or pasted
 * @returns {{kind: string, query: (string|undefined), url: (string|undefined)}} What it is
 */
export const classify = function(input) {
    const text = (input || "").trim();
    if (text === "") {
        return {"kind": "none"};
    }
    const url = toUrl(text);
    const m = url.match(/^([a-z][a-z0-9+.-]*):\/\/([^/?#]+)([^?#]*)/i);
    if (!m) {
        return {"kind": "search", "query": text};
    }
    const scheme = m[1].toLowerCase();
    const host = m[2].toLowerCase();
    const path = m[3];
    if (scheme !== "http" && scheme !== "https") {
        return {"kind": "url", url}; // icy://, rtsp://, mms://...
    }
    if (kAudio.test(path) || kStreamPath.test(path) || kStreamHost.test(host)) {
        return {"kind": "url", url};
    }
    const app = path.search(/\/(app|rest)(\/|$)/i);
    if (path === "" || path === "/" || app >= 0 || kServerHost.test(host)) {
        const base = app >= 0 ? path.slice(0, app) : path.replace(/\/+$/, "");
        return {"kind": "server", "url": `${scheme}://${m[2]}${base}`};
    }
    return {"kind": "url", url};
};
