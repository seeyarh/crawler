use crawler::crawler::{start_scan, Target};
use futures::stream::{self};
use tokio::sync::mpsc;
use warp::Filter;

const MAX_BUFFERED_RESULTS: usize = 10000;

fn init_server() {
    std::thread::spawn(|| {
        let rt = tokio::runtime::Runtime::new().expect("runtime starts");
        rt.block_on(async {
            // Match any request and return hello world!
            eprintln!("starting webserver");
            let routes = warp::any().map(|| "Hello, World!");

            warp::serve(routes).run(([127, 0, 0, 1], 8080)).await;
        });
    });
}

#[tokio::test]
async fn e2e_test() {
    init_server();
    std::thread::sleep(std::time::Duration::from_millis(100));

    eprintln!("running scan");
    let (tx, mut rx) = mpsc::channel(MAX_BUFFERED_RESULTS);

    let target = Target {
        url: "http://127.0.0.1:8080/".into(),
    };

    let max_concurrent_scans = 100;
    let n_targets = 100;
    let targets: Vec<Target> = (0..n_targets).map(|_| target.clone()).collect();
    let targets = stream::iter(targets);
    start_scan(targets, tx, max_concurrent_scans).await;

    let mut n = 0;
    while let Some(result) = rx.recv().await {
        assert_eq!(result.target, target);
        assert_eq!(result.error, None);
        assert_eq!(result.response.unwrap().body, "Hello, World!");
        n += 1;
    }

    assert_eq!(n, n_targets);
}
