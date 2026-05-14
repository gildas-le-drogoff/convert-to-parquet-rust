// ============================================================
use crate::conversion::THROUGHPUT_WINDOW;
use indicatif::ProgressBar;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Periodically display throughput (MB/s) based on the byte-based progress
/// position. Exits when `stop` is set or the bar is finished, so error paths
/// do not leave the thread spinning.
pub fn start_ticker(progress_bar: ProgressBar, stop: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut history: VecDeque<(Instant, u64)> = VecDeque::new();
        while !stop.load(Ordering::Relaxed) && !progress_bar.is_finished() {
            thread::sleep(Duration::from_millis(200));
            let now = Instant::now();
            history.push_back((now, progress_bar.position()));
            while history
                .front()
                .is_some_and(|(instant, _)| now.duration_since(*instant) > THROUGHPUT_WINDOW)
            {
                history.pop_front();
            }
            update_throughput_message(&progress_bar, &history);
        }
    })
}

fn update_throughput_message(progress_bar: &ProgressBar, history: &VecDeque<(Instant, u64)>) {
    if history.len() < 2 {
        return;
    }
    let Some((&(start_time, start_bytes), &(end_time, end_bytes))) =
        history.front().zip(history.back())
    else {
        return;
    };
    let elapsed = end_time.duration_since(start_time).as_secs_f64();
    if elapsed > 0.0 && end_bytes > start_bytes {
        let megabytes_per_second = (end_bytes - start_bytes) as f64 / elapsed / 1_000_000.0;
        progress_bar.set_message(format!("{megabytes_per_second:.1} MB/s"));
    }
}
