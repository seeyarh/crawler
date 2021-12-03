//! Crawler
use crawler::crawler::{start_scan, CrawlerResult, Target};
use futures::stream::StreamExt;
use std::error::Error;
use std::time::Instant;
use structopt::StructOpt;
use tokio::fs::File;
use tokio::io;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::mpsc;

/// Run HTTP Crawler
#[derive(Debug, StructOpt)]
#[structopt(name = "example", about = "An example of StructOpt usage.")]
struct Opts {
    /// Path to output file, defaults to stdout
    #[structopt(short, long)]
    out_file: Option<String>,

    /// Path to log file, defaults to stderr
    #[structopt(short, long)]
    log_file: Option<String>,

    /// Max concurrent scans
    #[structopt(short, long, default_value = "50000")]
    max_concurrent_scans: usize,
}

const MAX_BUFFERED_RESULTS: usize = 10000;
async fn run(opts: Opts) -> Result<(), Box<dyn Error>> {
    let start = Instant::now();

    let f = io::stdin();

    let mut rdr = csv_async::AsyncReaderBuilder::new()
        .has_headers(false)
        .create_deserializer(f);

    let f = opts.out_file.unwrap();
    let f = File::create(&f).await?;
    let writer = io::BufWriter::new(f);

    let (tx, rx) = mpsc::channel(MAX_BUFFERED_RESULTS);
    let writer_task = tokio::spawn(async move { write_results(writer, rx).await });

    let records = rdr.deserialize::<Target>();

    let targets = records.filter_map(|record| async move {
        match record {
            Ok(target) => Some(target),
            Err(e) => {
                tracing::warn!("failed to parse input {:?}", e);
                None
            }
        }
    });

    start_scan(targets, tx, opts.max_concurrent_scans).await;
    let n_targets = writer_task.await??;

    let duration = start.elapsed();
    tracing::info!(
        "scanned {} targets in {} seconds",
        n_targets,
        duration.as_secs_f64()
    );

    Ok(())
}

async fn write_results<T>(
    mut writer: BufWriter<T>,
    mut rx: mpsc::Receiver<CrawlerResult>,
) -> io::Result<u64>
where
    T: AsyncWriteExt + Unpin,
{
    let mut n = 0;
    while let Some(result) = rx.recv().await {
        writer.write_all(&serde_json::to_vec(&result)?).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;
        n += 1;
    }
    Ok(n)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let opts = Opts::from_args();
    run(opts).await.expect("fail");
}
