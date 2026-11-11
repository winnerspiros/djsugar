#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Spotify through librespot (src/lib.rs). `cache_dir` holds the stored
// account (credentials.json) and partial downloads. The calls that return
// char* block: run them on a worker thread. They return NULL on success,
// otherwise an error message ("cancelled" after spotify_cancel_login) to free
// with spotify_free_string(), as every string handed out here.

/// Login on Spotify's web page: `open_url` is called with the page to show
/// (any thread); it comes back to http://127.0.0.1:5588/login, or
/// through spotify_login_redirect(). Waits until
/// then or spotify_cancel_login().
char* spotify_login_web(const char* cache_dir,
        void (*open_url)(void* user, const char* url),
        void* user,
        char** out_user);

/// The login page shown in the app reached the redirect URI (`url`, with
/// its query): finishes the running login like the loopback request would
void spotify_login_redirect(const char* url);

/// Ends the running login
void spotify_cancel_login(void);

/// An account is stored (no login needed)
bool spotify_logged_in(const char* cache_dir);

/// Forgets the stored account
void spotify_logout(const char* cache_dir);

/// A list of the account as JSON in `*out_json`: request "profile" ({"name"}),
/// "playlists", "liked", "playlist:<id>" or "search:<query>"; lists are
/// {"items": [...], "next": offset of the next page or -1}, tracks `limit`
/// at a time from `offset`. Blocks until done
char* spotify_browse(const char* cache_dir,
        const char* request,
        int32_t offset,
        int32_t limit,
        char** out_json);

/// Called during a download with the fraction done (0..1)
typedef void (*spotify_progress_cb)(void* user, double fraction);

/// Downloads a Spotify track (base62 id) as Ogg Vorbis (320 kbit/s; needs
/// Spotify Premium) to `out_path`
char* spotify_download(const char* cache_dir,
        const char* track_id,
        const char* out_path,
        spotify_progress_cb progress,
        void* user);

void spotify_free_string(char* message);

#ifdef __cplusplus
}
#endif
