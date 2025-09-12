use crate::workloads::WorkloadResult;
use std::fs::File;
use std::io::Write;

pub fn print_text_report(results: &[WorkloadResult]) {
    println!("{:-<80}", "");
    println!("{:^80}", "KVSTORE++ BENCHMARK RESULTS");
    println!("{:-<80}", "");
    println!(
        "{:<15} {:<10} {:<12} {:<12} {:<12} {:<12} {:<10}",
        "WORKLOAD", "OPS/SEC", "P50(ms)", "P95(ms)", "P99(ms)", "ERROR RATE", "TOTAL OPS"
    );
    println!("{:-<80}", "");

    for result in results {
        println!(
            "{:<15} {:<10.2} {:<12.2} {:<12.2} {:<12.2} {:<12.2%} {:<10}",
            result.workload_type,
            result.ops_per_sec,
            result.latency_p50_ms,
            result.latency_p95_ms,
            result.latency_p99_ms,
            result.error_rate,
            result.total_ops
        );
    }
    println!("{:-<80}", "");
}

pub fn save_json_report(results: &[WorkloadResult], filename: &str) -> std::io::Result<()> {
    let file = File::create(filename)?;
    serde_json::to_writer_pretty(file, results)?;
    Ok(())
}

pub fn save_csv_report(results: &[WorkloadResult], filename: &str) -> std::io::Result<()> {
    let mut w = csv::Writer::from_path(filename)?;

    for result in results {
        w.serialize(result)?;
    }

    w.flush()?;
    Ok(())
}
