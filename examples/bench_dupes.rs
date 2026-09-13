// Benchmark the duplicate scan: cargo run --release --example bench_dupes -- <dir>
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "C:\\Users\\LENOVO".to_string());
    let (tx, rx) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));
    let t0 = Instant::now();
    let d = dir.clone();
    std::thread::spawn(move || {
        cleaner::workers::duplicates_worker(d, vec![], cancel, tx)
    });
    let mut groups = 0;
    let mut last = String::new();
    for msg in rx {
        match msg {
            cleaner::workers::WorkerMessage::Duplicates(g) => {
                groups = g.len();
            }
            cleaner::workers::WorkerMessage::Log(s) => {
                if s != last {
                    last = s;
                }
            }
            cleaner::workers::WorkerMessage::Done { summary } => {
                println!("{} ({} groups) in {:.2?}", summary, groups, t0.elapsed());
                return;
            }
            cleaner::workers::WorkerMessage::Error(e) => {
                println!("ERROR: {}", e);
                return;
            }
            _ => {}
        }
    }
}
