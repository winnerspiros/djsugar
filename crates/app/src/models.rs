//! The engine's models (stem separation, the listening for lyrics), downloaded the first time one is needed.

const URL: &str = "https://github.com/winnerspiros/djsugar/releases/download/stem-models";

/// Lets the engine fetch a model it lacks, checked against the hash it knows it by.
/// Has the models the settings call for on hand before a track needs them, quietly.
pub fn ready(stems: bool, lyrics: bool) {
    std::thread::spawn(move || {
        let got = [stems.then(djsugar_engine::separation::fetch), lyrics.then(djsugar_engine::word_timing::fetch)];
        for why in got.into_iter().flatten().filter_map(Result::err) {
            crate::log::note(&why);
        }
    });
}

pub fn install() {
    djsugar_engine::separation::set_fetch(Box::new(|name, hash, to| {
        sugar_net::http::download(&format!("{URL}/{name}"), &[], to, None, 4, &|_, _| true)
            .map_err(|e| e.to_string())?;
        let got = sugar_net::http::sha256(to).map_err(|e| e.to_string())?;
        if got == hash { Ok(()) } else { Err("the download is damaged".into()) }
    }));
}
