#!/usr/bin/env python3
"""Compare isolated previous Redfish hot paths with the current implementation.

No QEMU, guest, cluster, network download or external Python package is needed.
Both variants use the current manager control transport and a local fake Unix
socket. The legacy variant reproduces the previous JSON history and per-event
checkpoint strategy; it is not a checkout or benchmark of the whole old binary.
"""
import argparse
import json
import os
from pathlib import Path
import statistics
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--iterations", type=int, default=20000)
    parser.add_argument("--requests", type=int, default=128)
    parser.add_argument("--concurrency", type=int, default=4)
    parser.add_argument("--delay-ms", type=int, default=2)
    parser.add_argument("--release", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if not 1 <= args.runs <= 20:
        parser.error("--runs must be between 1 and 20")
    root = Path(__file__).resolve().parents[2]
    results = []
    command = ["cargo", "test", "--locked", "-p", "qrab-redfish", "--lib"]
    if args.release:
        command.append("--release")
    command += ["telemetry::performance::redfish_performance", "--", "--ignored", "--exact", "--nocapture", "--test-threads=1"]
    for run in range(args.runs):
        for mode in ("legacy", "optimized"):
            env = os.environ.copy()
            env.update(QRAB_PERF_MODE=mode, QRAB_PERF_ITERATIONS=str(args.iterations), QRAB_PERF_REQUESTS=str(args.requests), QRAB_PERF_DELAY_MS=str(args.delay_ms), QRAB_PERF_CONCURRENCY=str(args.concurrency), QRAB_PERF_PROFILE="release" if args.release else "debug")
            completed = subprocess.run(command, cwd=root, env=env, text=True, capture_output=True, timeout=300)
            if completed.returncode:
                raise SystemExit(completed.stdout + completed.stderr)
            reports = [line.partition("REDFISH_PERFORMANCE ")[2] for line in completed.stdout.splitlines() if "REDFISH_PERFORMANCE " in line]
            if len(reports) != 1:
                raise SystemExit("Expected exactly one performance result:\n" + completed.stdout)
            value = json.loads(reports[0])
            value["run"] = run + 1
            results.append(value)
    summary = {}
    for mode in ("legacy", "optimized"):
        selected = [value for value in results if value["mode"] == mode]
        summary[mode] = {
            "duplicate_record_elapsed_us_median": statistics.median(value["duplicate_record_elapsed_us"] for value in selected),
            "duplicate_record_allocations_median": statistics.median(value["duplicate_record_allocations"]["allocations"] for value in selected),
            "duplicate_record_allocated_bytes_median": statistics.median(value["duplicate_record_allocations"]["allocated_bytes"] for value in selected),
            "concurrent_p95_us_median": statistics.median(value["concurrent_fake_control"]["p95_us"] for value in selected),
            "concurrent_failed_total": sum(value["concurrent_fake_control"]["failed"] for value in selected),
            "idle_control_queries_per_six_ticks": selected[0]["concurrent_fake_control"]["idle_six_scheduler_ticks_control_queries"],
            "durable_writes_per_128_events": selected[0]["persistent_delivery_checkpoint"]["durable_writes"],
            "checkpoint_elapsed_us_median": statistics.median(value["persistent_delivery_checkpoint"]["elapsed_us"] for value in selected),
            "process_peak_rss_kib_median": statistics.median(value["memory"]["peak_rss_kib"] for value in selected),
        }
    report = {"scope": "isolated Redfish hot paths through current transport; local fake control; not a production VM benchmark", "profile": "release" if args.release else "debug", "summary": summary, "runs": results}
    text = json.dumps(report, indent=2) + "\n"
    if args.output:
        args.output.write_text(text)
    print(text, end="")


if __name__ == "__main__":
    main()
