#pragma once

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Called during a download with the fraction done (0..1)
typedef void (*spotify_progress_cb)(void* user, double fraction);

/// Downloads a Spotify track (base62 id) as Ogg Vorbis (320 kbit/s with
/// Premium) to `out_path`, logging in to Spotify with the OAuth access token
/// of the user (it needs the "streaming" scope). `client_id` is the Spotify
/// client id the token was issued to; `tmp_dir` holds partial downloads.
/// Blocks until done; call it from a worker thread. Returns NULL on success,
/// otherwise an error message to free with spotify_free_string().
char* spotify_download(const char* access_token,
        const char* client_id,
        const char* track_id,
        const char* out_path,
        const char* tmp_dir,
        spotify_progress_cb progress,
        void* user);

void spotify_free_string(char* message);

#ifdef __cplusplus
}
#endif
