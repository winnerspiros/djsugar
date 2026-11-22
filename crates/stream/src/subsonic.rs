//! Subsonic API servers: Navidrome, Airsonic, Gonic and the rest, the DJ's
//! own music at home. Signs every call with a salted token, never the password.

use serde_json::Value;

use crate::{agent, enc, json, Error, Hit, Result, Stream};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Server {
    pub base: String,
    pub user: String,
    pub password: String,
}

impl Server {
    pub fn new(base: &str, user: &str, password: &str) -> Self {
        let base = base.trim().trim_end_matches('/');
        let base = if base.starts_with("http://") || base.starts_with("https://") {
            base.to_string()
        } else {
            format!("http://{base}")
        };
        Self {
            base,
            user: user.into(),
            password: password.into(),
        }
    }

    fn salt() -> String {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        format!("{:x}", t ^ (std::process::id() as u128) << 64)
    }

    /// URL of a call, signed with `salt`.
    pub fn url_with(&self, method: &str, args: &str, salt: &str) -> String {
        let token = format!("{:x}", md5::compute(format!("{}{salt}", self.password)));
        format!(
            "{}/rest/{method}.view?u={}&t={token}&s={salt}&v=1.16.1&c=DJSugar&f=json{args}",
            self.base,
            enc(&self.user)
        )
    }

    fn call(&self, method: &str, args: &str) -> Result<Value> {
        let v = json(
            agent()
                .get(&self.url_with(method, args, &Self::salt()))
                .call(),
        )?;
        let r = &v["subsonic-response"];
        if r["status"].as_str() != Some("ok") {
            return Err(Error::Refused(
                r["error"]["message"]
                    .as_str()
                    .unwrap_or("server refused")
                    .to_string(),
            ));
        }
        Ok(r.clone())
    }

    pub fn ping(&self) -> Result<()> {
        self.call("ping", "").map(|_| ())
    }

    pub fn search(&self, query: &str) -> Result<Vec<Hit>> {
        let r = self.call(
            "search3",
            &format!(
                "&query={}&songCount=100&albumCount=0&artistCount=0",
                enc(query)
            ),
        )?;
        Ok(songs(&r["searchResult3"]["song"]))
    }

    pub fn random(&self) -> Result<Vec<Hit>> {
        Ok(songs(
            &self.call("getRandomSongs", "&size=100")?["randomSongs"]["song"],
        ))
    }

    pub fn stream(&self, id: &str) -> Stream {
        Stream {
            url: self.url_with("stream", &format!("&id={}", enc(id)), &Self::salt()),
            headers: Vec::new(),
            ext: "audio",
        }
    }
}

fn songs(v: &Value) -> Vec<Hit> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    Some(Hit {
                        id: s["id"].as_str()?.to_string(),
                        title: s["title"].as_str().unwrap_or_default().to_string(),
                        artist: s["artist"].as_str().unwrap_or_default().to_string(),
                        seconds: s["duration"].as_u64().map(|d| d as u32),
                        artwork: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_with_a_salted_token() {
        let s = Server::new("music.local:4533/", "dj", "sesame");
        let u = s.url_with("ping", "", "c19b2d");
        // md5("sesamec19b2d"), as in the Subsonic API documentation
        assert!(u.starts_with("http://music.local:4533/rest/ping.view?u=dj&t=26719a1196d2a940705a59634eb18eab&s=c19b2d"));
        assert!(!u.contains("sesame&"));
    }

    #[test]
    fn reads_songs() {
        let v = serde_json::json!([{"id": "7", "title": "T", "artist": "A", "duration": 61}]);
        assert_eq!(songs(&v)[0].seconds, Some(61));
    }
}
