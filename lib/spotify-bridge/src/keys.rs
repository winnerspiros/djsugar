//! Paced audio key requests. Spotify refuses audio keys in bursts for some
//! accounts (librespot-org/librespot#1649); asking again at once prolongs
//! the burst, a few seconds of quiet ends it. So: one request at a time, at
//! least 200 ms apart, and after a refusal 3, then 5, then 10 seconds of
//! quiet for every request (the approach of librespot PR #1844)

use std::future::Future;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use librespot_core::audio_key::{AudioKey, AudioKeyError};
use librespot_core::Error;
use tokio::time::Instant;

const MIN_REQUEST_GAP: Duration = Duration::from_millis(200);
const REFUSAL_BACKOFF: [Duration; 3] = [
    Duration::from_secs(3),
    Duration::from_secs(5),
    Duration::from_secs(10),
];
/// Times a refused key is asked again, each after the back-off
const REFUSAL_RETRIES: usize = 3;

#[derive(Default)]
struct Pacer {
    /// Held for a whole request: one in flight; when the last one was sent
    turn: tokio::sync::Mutex<Option<Instant>>,
    /// Refusals in a row, and no request before the instant
    backoff: Mutex<(usize, Option<Instant>)>,
}

fn pacer() -> &'static Pacer {
    static PACER: OnceLock<Pacer> = OnceLock::new();
    PACER.get_or_init(Pacer::default)
}

fn is_refusal(error: &Error) -> bool {
    matches!(
        error.error.downcast_ref::<AudioKeyError>(),
        Some(AudioKeyError::AesKey)
    )
}

impl Pacer {
    async fn ask<F, Fut>(&self, ask: F) -> Result<AudioKey, Error>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<AudioKey, Error>>,
    {
        let mut last_sent = self.turn.lock().await;
        let until = self.backoff.lock().unwrap().1;
        if let Some(until) = until {
            tokio::time::sleep_until(until).await;
        }
        if let Some(last_sent) = *last_sent {
            tokio::time::sleep_until(last_sent + MIN_REQUEST_GAP).await;
        }
        *last_sent = Some(Instant::now());
        let result = ask().await;
        let mut backoff = self.backoff.lock().unwrap();
        match &result {
            Ok(_) => *backoff = (0, None),
            Err(e) if is_refusal(e) => {
                backoff.0 += 1;
                let delay = REFUSAL_BACKOFF[backoff.0.min(REFUSAL_BACKOFF.len()) - 1];
                backoff.1 = Some(Instant::now() + delay);
            }
            // A timeout or lost connection says nothing about the key
            Err(_) => {}
        }
        result
    }

    async fn request<F, Fut>(&self, mut ask: F) -> Result<AudioKey, Error>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<AudioKey, Error>>,
    {
        let mut retries = 0;
        loop {
            match self.ask(&mut ask).await {
                Err(e) if is_refusal(&e) && retries < REFUSAL_RETRIES => retries += 1,
                result => return result,
            }
        }
    }
}

/// The key `ask` gets, paced with every other key request of the app
pub async fn request<F, Fut>(ask: F) -> Result<AudioKey, Error>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<AudioKey, Error>>,
{
    pacer().request(ask).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Answers each request from `answers` in turn (refusing once they run
    /// out) and notes when it was asked, in ms since the start
    struct Server {
        answers: Mutex<Vec<bool>>,
        asked: Mutex<Vec<u128>>,
        start: Instant,
    }

    impl Server {
        fn new(answers: &[bool]) -> Arc<Self> {
            Arc::new(Self {
                answers: Mutex::new(answers.iter().rev().copied().collect()),
                asked: Mutex::default(),
                start: Instant::now(),
            })
        }
        async fn ask(self: Arc<Self>) -> Result<AudioKey, Error> {
            self.asked
                .lock()
                .unwrap()
                .push((Instant::now() - self.start).as_millis());
            tokio::time::sleep(Duration::from_millis(50)).await;
            if self.answers.lock().unwrap().pop().unwrap_or(false) {
                Ok(AudioKey([7; 16]))
            } else {
                Err(AudioKeyError::AesKey.into())
            }
        }
        fn asked(&self) -> Vec<u128> {
            self.asked.lock().unwrap().clone()
        }
    }

    #[tokio::test(start_paused = true)]
    async fn refusals_are_asked_again_after_growing_pauses() {
        let pacer = Pacer::default();
        let server = Server::new(&[false, false, false, true]);
        assert!(pacer.request(|| server.clone().ask()).await.is_ok());
        assert_eq!(server.asked(), [0, 3050, 8100, 18150]);
    }

    #[tokio::test(start_paused = true)]
    async fn requests_are_one_at_a_time_and_spaced() {
        let pacer = Arc::new(Pacer::default());
        let server = Server::new(&[true; 3]);
        let all = (0..3).map(|_| {
            let (pacer, server) = (pacer.clone(), server.clone());
            async move { pacer.request(|| server.clone().ask()).await }
        });
        let results = futures_util::future::join_all(all).await;
        assert!(results.iter().all(Result::is_ok));
        assert_eq!(server.asked(), [0, 200, 400]);
    }

    #[tokio::test(start_paused = true)]
    async fn gives_up_after_the_retries() {
        let pacer = Pacer::default();
        let server = Server::new(&[]);
        let result = pacer.request(|| server.clone().ask()).await;
        assert!(result.as_ref().is_err_and(is_refusal));
        assert_eq!(server.asked().len(), REFUSAL_RETRIES + 1);
    }
}
