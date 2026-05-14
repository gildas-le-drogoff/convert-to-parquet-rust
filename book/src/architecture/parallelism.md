# Parallel Processing

The project uses two complementary parallelism libraries: **rayon** and **crossbeam**.

## Parallel Pipeline Architecture

```
                    ┌────────────────────────┐
                    │   Sequential reading   │
                    │  (blocks of 100k rows) │
                    └──────────┬─────────────┘
                               │
                    ┌──────────▼────────────┐
                    │  block_sender         │
                    │  (crossbeam, capacity │
                    │   8 blocks)           │
                    └──────────┬────────────┘
                               │
                ┌──────────────┼──────────────┐
                │              │              │
         ┌──────▼──────┐  ┌────▼─────┐ ┌──────▼───┐
         │ Worker 0    │  │ Worker 1 │ │ Worker N │
         │ (rayon)     │  │ (rayon)  │ │ (rayon)  │
         └──────┬──────┘  └────┬─────┘ └──────┬───┘
                │              │              │
                └──────────────┼──────────────┘
                               │
                    ┌──────────▼─────────────┐
                    │  batch_sender          │
                    │  (crossbeam, capacity  │
                    │   8 batches)           │
                    └──────────┬─────────────┘
                               │
                    ┌──────────▼────────────┐
                    │  Ordered writing      │
                    │  (BTreeMap + Parquet) │
                    └───────────────────────┘
```

## Rayon: Data Parallelism

**Rayon** converts each CSV block to an Arrow `RecordBatch` in parallel via `par_bridge()`:

```rust
block_receiver
    .into_iter()
    .par_bridge()
    .try_for_each(|block| {
        let result = analyze_block(&block.records, schema.clone(), force_utf8)?;
        batch_sender.send((block.index, result.batch, block.bytes_read))?;
        Ok(())
    })
```

Each block is processed independently on a rayon thread pool, allowing efficient use of all CPU cores.

## Crossbeam: Bounded Channels

**Crossbeam** provides inter-thread communication channels with bounded capacity:

- `block_sender` (capacity: 8 blocks) — from reader to workers
- `batch_sender` (capacity: 8 batches) — from workers to writer

The advantage of bounded channels is **backpressure**: if workers are too slow, reading slows down; if the writer is too slow, workers slow down. The system naturally balances itself.

## Strict Ordering

Blocks are numbered sequentially (`block.index`). Since parallelism does not preserve order, each `RecordBatch` is sent with its original index.

The writing thread maintains a **BTreeMap** that reorders batches:

```rust
let mut pending: BTreeMap<usize, (RecordBatch, u64)> = BTreeMap::new();
let mut next_index = 0;

// On each reception:
pending.insert(index, (batch, bytes));
// Write all ready batches in order
while let Some((batch, bytes)) = pending.remove(&next_index) {
    writer.write(&batch)?;
    next_index += 1;
}
```

This ensures that the row order from the source file is **strictly preserved** in the final Parquet file.

## Progress Ticker

A dedicated thread (`ticker`) updates the progress bar in real-time:

- Display in **MB/s** (megabytes per second)
- 2-second smoothing window
- Clean shutdown via `AtomicBool`
- Cleaned up even on error (thanks to `TickerStopGuard`)

## Excel Sheet Concurrency

For multi-sheet workbooks, each sheet is processed in a separate rayon pool, configurable with `--sheet-concurrency`. Default: `ncpu / 2` threads.

Sheets are independent: each produces its own Parquet file.
