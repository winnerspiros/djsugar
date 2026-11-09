//! Spotify audio for the DJ Sugar decks: logs in to Spotify with the user's
//! access token (librespot), fetches the track's encrypted Ogg Vorbis file,
//! decrypts it and writes a plain .ogg that Mixxx loads like any local file.
//! Playing it needs Spotify Premium, as in every librespot client.

use std::ffi::{c_char, c_void, CStr, CString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use futures_util::stream::{FuturesUnordered, StreamExt};
use librespot_audio::{AudioDecrypt, AudioFile};
use librespot_core::authentication::Credentials;
use librespot_core::config::SessionConfig;
use librespot_core::session::Session;
use librespot_core::spotify_id::SpotifyId;
use librespot_core::spotify_uri::SpotifyUri;
use librespot_metadata::audio::{AudioFileFormat, AudioFiles, AudioItem};
use tokio::runtime::Runtime;

/// Spotify's Ogg files start with a custom packet (normalisation data) the
/// Vorbis decoders don't know; the playable stream begins after it
const SPOTIFY_OGG_HEADER_END: u64 = 0xa7;

/// Best first. Ogg only: Mixxx decodes it everywhere (MP3 files are
/// encrypted differently and FLAC is not served to third-party clients)
const FORMATS: [AudioFileFormat; 3] = [
    AudioFileFormat::OGG_VORBIS_320,
    AudioFileFormat::OGG_VORBIS_160,
    AudioFileFormat::OGG_VORBIS_96,
];

type Progress = Box<dyn Fn(f64) + Send>;

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("spotify")
            .enable_all()
            .build()
            .expect("tokio runtime")
    })
}

/// The logged-in session, reused while the access token stays the same
struct CachedSession {
    token: String,
    session: Session,
}

fn session_cache() -> &'static Mutex<Option<CachedSession>> {
    static CACHE: OnceLock<Mutex<Option<CachedSession>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

async fn session_for(token: &str, client_id: &str, tmp_dir: &Path) -> Result<Session, String> {
    if let Some(cached) = session_cache().lock().unwrap().as_ref() {
        if cached.token == token && !cached.session.is_invalid() {
            return Ok(cached.session.clone());
        }
    }
    let mut config = SessionConfig::default();
    if !client_id.is_empty() {
        config.client_id = client_id.to_owned();
    }
    config.tmp_dir = tmp_dir.to_path_buf();
    let session = Session::new(config, None);
    session
        .connect(Credentials::with_access_token(token), false)
        .await
        .map_err(|e| format!("Spotify login: {e}"))?;
    if let Some(old) = session_cache().lock().unwrap().replace(CachedSession {
        token: token.to_owned(),
        session: session.clone(),
    }) {
        old.session.shutdown();
    }
    Ok(session)
}

/// The track, or a playable alternative of it (another release of the same
/// recording) when it is not available in the user's country
async fn playable_item(session: &Session, uri: SpotifyUri) -> Result<AudioItem, String> {
    let item = AudioItem::get_file(session, uri)
        .await
        .map_err(|e| format!("Track info: {e}"))?;
    if item.availability.is_ok() && !item.files.is_empty() {
        return Ok(item);
    }
    let Some(alternatives) = item.alternatives.clone() else {
        return Err(match item.availability {
            Err(e) => format!("Not playable: {e}"),
            Ok(()) => "Not playable here".to_owned(),
        });
    };
    let mut lookups: FuturesUnordered<_> = alternatives
        .0
        .into_iter()
        .map(|alt| AudioItem::get_file(session, alt))
        .collect();
    while let Some(alt) = lookups.next().await {
        if let Ok(alt) = alt {
            if alt.availability.is_ok() && !alt.files.is_empty() {
                return Ok(alt);
            }
        }
    }
    Err("Not playable in this country".to_owned())
}

async fn download(
    token: String,
    client_id: String,
    track_id: String,
    out_path: PathBuf,
    tmp_dir: PathBuf,
    progress: Progress,
) -> Result<(), String> {
    let id = SpotifyId::from_base62(&track_id).map_err(|e| format!("Track id: {e}"))?;
    let session = session_for(&token, &client_id, &tmp_dir).await?;
    let item = playable_item(&session, SpotifyUri::Track { id }).await?;
    let file_id_track = SpotifyId::try_from(&item.track_id).map_err(|e| e.to_string())?;
    let (format, file_id) = FORMATS
        .iter()
        .find_map(|f| item.files.get(f).map(|id| (*f, *id)))
        .ok_or_else(|| "No Ogg Vorbis file for this track".to_owned())?;
    debug_assert!(AudioFiles::is_ogg_vorbis(format));
    progress(0.05);

    // Ahead of the file: the key comes from the access point, the file
    // from the CDN. Without a key (no Premium) there is nothing to play.
    let key = session
        .audio_key()
        .request(file_id_track, file_id)
        .await
        .map_err(|e| format!("No audio key (Spotify Premium needed): {e}"))?;
    let encrypted = AudioFile::open(&session, file_id, 40 * 1024)
        .await
        .map_err(|e| format!("Audio file: {e}"))?;
    let controller = encrypted
        .get_stream_loader_controller()
        .map_err(|e| e.to_string())?;
    let total = controller.len() as u64;
    // Whole file at once, not paced like playback
    controller.set_random_access_mode();

    // Reading blocks on the download: off the async workers
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let mut decrypted = AudioDecrypt::new(Some(key), encrypted);
        decrypted
            .seek(SeekFrom::Start(SPOTIFY_OGG_HEADER_END))
            .map_err(|e| e.to_string())?;
        let partial = out_path.with_extension("ogg.part");
        let mut out = File::create(&partial).map_err(|e| format!("{}: {e}", partial.display()))?;
        let mut buffer = vec![0u8; 64 * 1024];
        let mut done = SPOTIFY_OGG_HEADER_END;
        loop {
            let n = decrypted.read(&mut buffer).map_err(|e| format!("Download: {e}"))?;
            if n == 0 {
                break;
            }
            out.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
            done += n as u64;
            if total > 0 {
                progress(0.05 + 0.9 * (done as f64 / total as f64).min(1.0));
            }
        }
        out.flush().map_err(|e| e.to_string())?;
        drop(out);
        if total > 0 && done < total {
            let _ = std::fs::remove_file(&partial);
            return Err("Download ended early".to_owned());
        }
        std::fs::rename(&partial, &out_path).map_err(|e| e.to_string())?;
        progress(1.0);
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

unsafe fn arg(value: *const c_char) -> String {
    if value.is_null() {
        String::new()
    } else {
        CStr::from_ptr(value).to_string_lossy().into_owned()
    }
}

struct UserPtr(*mut c_void);
// The C++ side guarantees `user` outlives the blocking call
unsafe impl Send for UserPtr {}

/// # Safety
/// Strings must be valid NUL-terminated UTF-8 (or NULL); see spotifybridge.h
#[no_mangle]
pub unsafe extern "C" fn spotify_download(
    access_token: *const c_char,
    client_id: *const c_char,
    track_id: *const c_char,
    out_path: *const c_char,
    tmp_dir: *const c_char,
    progress: Option<extern "C" fn(*mut c_void, f64)>,
    user: *mut c_void,
) -> *mut c_char {
    let user = UserPtr(user);
    let report: Progress = Box::new(move |fraction| {
        let user = &user;
        if let Some(callback) = progress {
            callback(user.0, fraction);
        }
    });
    let result = runtime().block_on(download(
        arg(access_token),
        arg(client_id),
        arg(track_id),
        PathBuf::from(arg(out_path)),
        PathBuf::from(arg(tmp_dir)),
        report,
    ));
    match result {
        Ok(()) => std::ptr::null_mut(),
        Err(message) => CString::new(message.replace('\0', " "))
            .unwrap_or_default()
            .into_raw(),
    }
}

/// # Safety
/// `message` must come from spotify_download (or be NULL)
#[no_mangle]
pub unsafe extern "C" fn spotify_free_string(message: *mut c_char) {
    if !message.is_null() {
        drop(CString::from_raw(message));
    }
}
