//! A pasted link: a file on the web or an internet radio stream.

use crate::{Hit, Stream};

pub fn hit(link: &str) -> Option<Hit> {
    let link = link.trim();
    if !(link.starts_with("http://") || link.starts_with("https://")) {
        return None;
    }
    let path = link.split(['?', '#']).next().unwrap_or(link);
    let name = path.rsplit('/').find(|s| !s.is_empty()).unwrap_or(link);
    let title = name
        .rsplit_once('.')
        .map_or(name, |(n, _)| n)
        .replace("%20", " ");
    Some(Hit {
        id: link.to_string(),
        title,
        ..Hit::default()
    })
}

pub fn stream(link: &str) -> Stream {
    let path = link.split(['?', '#']).next().unwrap_or(link);
    let ext = match path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()) {
        Some(e) if e == "mp3" => "mp3",
        Some(e) if e == "flac" => "flac",
        Some(e) if e == "wav" => "wav",
        Some(e) if e == "ogg" => "ogg",
        Some(e) if e == "m4a" || e == "aac" || e == "mp4" => "m4a",
        _ => "audio",
    };
    Stream {
        url: link.trim().to_string(),
        headers: Vec::new(),
        ext,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_links() {
        let h = super::hit("https://x.org/a/My%20Mix.FLAC?dl=1").unwrap();
        assert_eq!(h.title, "My Mix");
        assert_eq!(super::stream(&h.id).ext, "flac");
        assert!(super::hit("ftp://x").is_none());
    }
}
