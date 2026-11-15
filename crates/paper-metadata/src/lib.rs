mod parse;
use chrono::Utc;
use paper_core::{model::*, ports::MetadataResolver};
pub use parse::{parse_arxiv, parse_crossref};
use reqwest::{Url, blocking::Client};
use std::{
    io::Read,
    sync::Mutex,
    time::{Duration, Instant},
};

const LIMIT: u64 = 2 * 1024 * 1024;
static LAST_REQUEST: Mutex<Option<Instant>> = Mutex::new(None);
#[derive(Debug, thiserror::Error)]
pub enum LookupError {
    #[error("DOI または arXiv ID の形式を確認してください。")]
    Identifier,
    #[error("この識別子の書誌情報は見つかりませんでした。")]
    NotFound,
    #[error("取得先が混雑しています。しばらく待ってから再取得してください。")]
    RateLimited,
    #[error("通信に失敗しました。接続を確認して再取得してください。")]
    Network,
    #[error("取得先から正常な応答がありませんでした（HTTP {0}）。")]
    Http(u16),
    #[error("取得先の応答を解釈できませんでした。入力や保存済みの情報は変更していません。")]
    Format,
    #[error("取得結果の識別子が要求と一致しません。")]
    Mismatch,
    #[error("取得結果が大きすぎるため中止しました。")]
    TooLarge,
}
pub fn parse_identifier(s: &str) -> Result<PaperIdentifier, LookupError> {
    Doi::parse(s)
        .map(PaperIdentifier::Doi)
        .or_else(|_| ArxivId::parse(s).map(PaperIdentifier::Arxiv))
        .map_err(|_| LookupError::Identifier)
}
#[derive(Clone)]
pub struct HttpMetadataResolver {
    client: Client,
}
impl HttpMetadataResolver {
    pub fn new() -> Result<Self, LookupError> {
        let client = Client::builder()
            .user_agent(concat!("PaperInbox/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(8))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| LookupError::Network)?;
        Ok(Self { client })
    }
    /// Call from a dedicated worker; no library transaction is held during I/O.
    pub fn resolve_blocking(
        &self,
        identifier: &PaperIdentifier,
    ) -> Result<Vec<MetadataCandidate>, LookupError> {
        let url = match identifier {
            PaperIdentifier::Doi(id) => {
                let mut u = Url::parse("https://api.crossref.org/works/").expect("fixed URL");
                u.path_segments_mut().expect("base").push(id.as_str());
                u
            }
            PaperIdentifier::Arxiv(id) => {
                let mut u = Url::parse("https://export.arxiv.org/api/query").expect("fixed URL");
                u.query_pairs_mut()
                    .append_pair("id_list", id.as_str())
                    .append_pair("max_results", "1");
                u
            }
            _ => return Err(LookupError::Identifier),
        };
        // Serial requests, >=3 seconds apart across resolver instances.
        let mut last = LAST_REQUEST.lock().map_err(|_| LookupError::Network)?;
        if let Some(previous) = *last {
            std::thread::sleep(Duration::from_secs(3).saturating_sub(previous.elapsed()));
        }
        *last = Some(Instant::now());
        let body = self.get(url)?;
        drop(last);
        let now = Utc::now();
        let candidate = match identifier {
            PaperIdentifier::Doi(id) => parse_crossref(&body, id, now)?,
            PaperIdentifier::Arxiv(id) => parse_arxiv(&body, id, now)?,
            _ => return Err(LookupError::Identifier),
        };
        Ok(vec![candidate])
    }
    fn get(&self, url: Url) -> Result<String, LookupError> {
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|_| LookupError::Network)?;
        match response.status().as_u16() {
            200 => {}
            404 => return Err(LookupError::NotFound),
            429 => return Err(LookupError::RateLimited),
            status => return Err(LookupError::Http(status)),
        };
        if response.content_length().is_some_and(|n| n > LIMIT) {
            return Err(LookupError::TooLarge);
        }
        let mut bytes = Vec::new();
        response
            .take(LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| LookupError::Network)?;
        if bytes.len() as u64 > LIMIT {
            return Err(LookupError::TooLarge);
        }
        String::from_utf8(bytes).map_err(|_| LookupError::Format)
    }
}
impl MetadataResolver for HttpMetadataResolver {
    type Error = LookupError;
    async fn resolve(
        &self,
        identifier: &PaperIdentifier,
    ) -> Result<Vec<MetadataCandidate>, LookupError> {
        let resolver = self.clone();
        let identifier = identifier.clone();
        let (sender, receiver) = futures_channel::oneshot::channel();
        std::thread::Builder::new()
            .name("metadata-resolver".into())
            .spawn(move || {
                let _ = sender.send(resolver.resolve_blocking(&identifier));
            })
            .map_err(|_| LookupError::Network)?;
        receiver.await.map_err(|_| LookupError::Network)?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    fn serve(response: Vec<u8>) -> (Url, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 8192];
            let size = socket.read(&mut request).unwrap();
            let _ = socket.write_all(&response);
            String::from_utf8_lossy(&request[..size]).into()
        });
        (
            Url::parse(&format!("http://{address}/metadata")).unwrap(),
            handle,
        )
    }
    #[test]
    fn transport_bounds_response_rejects_redirects_and_reports_http_errors() {
        let resolver = HttpMetadataResolver::new().unwrap();
        for (status, expected) in [
            (404, "notfound"),
            (429, "rate"),
            (503, "server"),
            (302, "redirect"),
        ] {
            let (url,worker)=serve(format!("HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nLocation: http://127.0.0.1:1/\r\nConnection: close\r\n\r\n").into_bytes());
            let error = resolver.get(url).unwrap_err();
            assert!(matches!(
                (expected, error),
                ("notfound", LookupError::NotFound)
                    | ("rate", LookupError::RateLimited)
                    | ("server", LookupError::Http(503))
                    | ("redirect", LookupError::Http(302))
            ));
            assert!(worker.join().unwrap().contains("PaperInbox/"));
        }
        let (url, worker) = serve(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                LIMIT + 1
            )
            .into_bytes(),
        );
        assert!(matches!(resolver.get(url), Err(LookupError::TooLarge)));
        worker.join().unwrap();
        let (url, worker) =
            serve(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".to_vec());
        assert_eq!(resolver.get(url).unwrap(), "{}");
        worker.join().unwrap();
    }
    #[test]
    fn streaming_size_and_response_deadline_are_enforced() {
        let resolver = HttpMetadataResolver::new().unwrap();
        let mut response = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
        response.resize(response.len() + LIMIT as usize + 1, b'x');
        let (url, worker) = serve(response);
        assert!(matches!(resolver.get(url), Err(LookupError::TooLarge)));
        worker.join().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (_socket, _) = listener.accept().unwrap();
            std::thread::sleep(Duration::from_millis(100));
        });
        let short = HttpMetadataResolver {
            client: Client::builder()
                .timeout(Duration::from_millis(20))
                .build()
                .unwrap(),
        };
        assert!(matches!(
            short.get(Url::parse(&format!("http://{address}/")).unwrap()),
            Err(LookupError::Network)
        ));
        worker.join().unwrap();
    }
}
