use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io;
use tokio::sync::mpsc;
use tokio::time::error::Elapsed;
use tracing::instrument;

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct Target {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerResult {
    pub target: Target,
    pub timestamp: u64,
    pub response: Option<CrawlerResponse>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerResponse {
    pub body: String,
}

pub async fn start_scan<S>(targets: S, tx: mpsc::Sender<CrawlerResult>, max_concurrent_scans: usize)
where
    S: futures::Stream<Item = Target>,
{
    let client = reqwest::Client::new();
    let detections = targets
        .map(|target| async { scan(&client, target).await })
        .buffered(max_concurrent_scans);

    detections
        .for_each(|d| async {
            tx.send(d).await.expect("failed to send");
        })
        .await;
}

#[derive(Debug)]
pub enum CrawlerError {
    Io(io::Error),
    Elapsed(Elapsed),
    Http(reqwest::Error),
}

impl fmt::Display for CrawlerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            CrawlerError::Io(ref err) => err.fmt(f),
            CrawlerError::Elapsed(ref err) => err.fmt(f),
            CrawlerError::Http(ref err) => err.fmt(f),
        }
    }
}

impl From<io::Error> for CrawlerError {
    fn from(err: io::Error) -> CrawlerError {
        CrawlerError::Io(err)
    }
}

impl From<reqwest::Error> for CrawlerError {
    fn from(err: reqwest::Error) -> CrawlerError {
        CrawlerError::Http(err)
    }
}

impl From<Elapsed> for CrawlerError {
    fn from(err: Elapsed) -> CrawlerError {
        CrawlerError::Elapsed(err)
    }
}

pub async fn scan(client: &reqwest::Client, target: Target) -> CrawlerResult {
    match run_scan(client, &target).await {
        Ok(response) => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System time before unix epoch")
                .as_secs();
            CrawlerResult {
                target,
                timestamp,
                response: Some(response),
                error: None,
            }
        }
        Err(e) => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("System time before unix epoch")
                .as_secs();
            CrawlerResult {
                target,
                timestamp,
                response: None,
                error: Some(e.to_string()),
            }
        }
    }
}

#[instrument]
async fn run_scan(
    client: &reqwest::Client,
    target: &Target,
) -> Result<CrawlerResponse, CrawlerError> {
    let response = client.get(&target.url).send().await?;

    Ok(CrawlerResponse {
        body: response
            .text()
            .await
            .expect("failed to parse response body"),
    })
}
