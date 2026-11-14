//! Local service resource accounting. Container totals are explicitly opt-in:
//! standalone processes must never report host/root-cgroup usage as their own.
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ResourceUsage {
    pub observed_unix_ms: u64,
    pub process: ProcessResources,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<ContainerResources>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProcessResources {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_time_ticks: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clock_ticks_per_second: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_time_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resident_memory_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ContainerResources {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_usage_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_throttled_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_limit_cores: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_current_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_working_set_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_limit_bytes: Option<u64>,
}

impl ResourceUsage {
    /// Process counters always refer to this service process. Container totals
    /// require an operator-provided `QRAB_RESOURCE_SCOPE=container` and a v2
    /// cgroup whose process-membership list contains this process's PID.
    pub fn sample() -> Self {
        sample_at(
            Path::new("/proc"),
            Path::new("/sys/fs/cgroup"),
            std::env::var("QRAB_RESOURCE_SCOPE").as_deref() == Ok("container"),
            std::process::id(),
        )
    }
}

fn read(path: impl AsRef<Path>) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(128 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 128 * 1024 {
        return Err(io::Error::other("resource accounting file exceeds bound"));
    }
    Ok(bytes)
}

fn text(path: impl AsRef<Path>) -> Option<String> {
    String::from_utf8(read(path).ok()?).ok()
}

fn cpu_ticks(stat: &str) -> Option<u64> {
    // comm may contain spaces and ')' characters; the final ')' ends field 2.
    let (_, tail) = stat.rsplit_once(") ")?;
    let fields = tail.split_whitespace().collect::<Vec<_>>();
    fields
        .get(11)?
        .parse::<u64>()
        .ok()?
        .checked_add(fields.get(12)?.parse().ok()?)
}

fn clock_ticks(auxv: &[u8]) -> Option<u64> {
    const WORD: usize = std::mem::size_of::<usize>();
    for pair in auxv.chunks_exact(WORD * 2) {
        let tag = usize::from_ne_bytes(pair[..WORD].try_into().ok()?);
        let value = usize::from_ne_bytes(pair[WORD..].try_into().ok()?);
        if tag == 0 {
            break;
        }
        if tag == 17 {
            return u64::try_from(value).ok().filter(|v| *v > 0);
        } // AT_CLKTCK
    }
    None
}

fn rss(status: &str) -> Option<u64> {
    let mut fields = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .split_whitespace();
    let kib = fields.next()?.parse::<u64>().ok()?;
    (fields.next()? == "kB").then_some(kib)?.checked_mul(1024)
}

fn key_value(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        (fields.next()? == key)
            .then(|| fields.next()?.parse().ok())
            .flatten()
    })
}

fn cpu_limit(text: &str) -> Option<f64> {
    let mut fields = text.split_whitespace();
    let quota = fields.next()?.parse::<u64>().ok()?;
    let period = fields.next()?.parse::<u64>().ok()?;
    (period > 0).then_some(quota as f64 / period as f64)
}

fn container_path(proc_root: &Path, cgroup_root: &Path, pid: u32) -> Option<PathBuf> {
    let cgroups = text(proc_root.join("self/cgroup"))?;
    let relative = cgroups.lines().find_map(|line| line.strip_prefix("0::"))?;
    let path = Path::new(relative);
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return None;
    }
    let path = cgroup_root.join(path.strip_prefix("/").ok()?);
    let processes = text(path.join("cgroup.procs"))?;
    processes
        .lines()
        .any(|line| line.trim().parse::<u32>() == Ok(pid))
        .then_some(path)
}

fn container(path: &Path) -> ContainerResources {
    let cpu = text(path.join("cpu.stat")).unwrap_or_default();
    let memory_current_bytes =
        text(path.join("memory.current")).and_then(|s| s.trim().parse::<u64>().ok());
    let inactive = text(path.join("memory.stat")).and_then(|s| key_value(&s, "inactive_file"));
    ContainerResources {
        cpu_usage_seconds: key_value(&cpu, "usage_usec").map(|n| n as f64 / 1_000_000.0),
        cpu_throttled_seconds: key_value(&cpu, "throttled_usec").map(|n| n as f64 / 1_000_000.0),
        cpu_limit_cores: text(path.join("cpu.max")).and_then(|s| cpu_limit(&s)),
        memory_current_bytes,
        memory_working_set_bytes: memory_current_bytes
            .zip(inactive)
            .map(|(current, inactive)| current.saturating_sub(inactive)),
        memory_limit_bytes: text(path.join("memory.max")).and_then(|s| s.trim().parse().ok()),
    }
}

fn sample_at(
    proc_root: &Path,
    cgroup_root: &Path,
    container_scope: bool,
    pid: u32,
) -> ResourceUsage {
    let cpu_time_ticks = text(proc_root.join("self/stat")).and_then(|s| cpu_ticks(&s));
    let clock_ticks_per_second = read(proc_root.join("self/auxv"))
        .ok()
        .and_then(|s| clock_ticks(&s));
    ResourceUsage {
        observed_unix_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX),
        process: ProcessResources {
            cpu_time_ticks,
            clock_ticks_per_second,
            cpu_time_seconds: cpu_time_ticks
                .zip(clock_ticks_per_second)
                .map(|(ticks, rate)| ticks as f64 / rate as f64),
            resident_memory_bytes: text(proc_root.join("self/status")).and_then(|s| rss(&s)),
        },
        container: container_scope
            .then(|| container_path(proc_root, cgroup_root, pid))
            .flatten()
            .map(|p| container(&p)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_units_are_derived_without_assuming_page_size_or_tick_rate() {
        assert_eq!(
            cpu_ticks("7 (name (with spaces)) S 1 2 3 4 5 6 7 8 9 10 123 456 0"),
            Some(579)
        );
        assert_eq!(rss("Name: test\nVmRSS:\t23 kB\n"), Some(23 * 1024));
        let auxv = [
            17usize.to_ne_bytes(),
            250usize.to_ne_bytes(),
            0usize.to_ne_bytes(),
            0usize.to_ne_bytes(),
        ]
        .concat();
        assert_eq!(clock_ticks(&auxv), Some(250));
        assert_eq!(cpu_limit("50000 100000"), Some(0.5));
        assert_eq!(cpu_limit("max 100000"), None);
        assert_eq!(cpu_limit("50000 0"), None);
    }
    #[test]
    fn container_totals_require_opt_in_and_verified_membership() {
        let temp = tempfile::tempdir().unwrap();
        let proc = temp.path().join("proc");
        let cg = temp.path().join("cgroup");
        std::fs::create_dir_all(proc.join("self")).unwrap();
        std::fs::create_dir_all(cg.join("service")).unwrap();
        std::fs::write(proc.join("self/cgroup"), "0::/service\n").unwrap();
        for (file, value) in [
            ("cgroup.procs", "42\n"),
            ("cpu.stat", "usage_usec 1250000\nthrottled_usec 250000\n"),
            ("cpu.max", "50000 100000"),
            ("memory.current", "1000"),
            ("memory.max", "max"),
            ("memory.stat", "inactive_file 200\n"),
        ] {
            std::fs::write(cg.join("service").join(file), value).unwrap();
        }
        assert!(sample_at(&proc, &cg, false, 42).container.is_none());
        assert!(sample_at(&proc, &cg, true, 43).container.is_none());
        let usage = sample_at(&proc, &cg, true, 42).container.unwrap();
        assert_eq!(usage.cpu_usage_seconds, Some(1.25));
        assert_eq!(usage.cpu_limit_cores, Some(0.5));
        assert_eq!(usage.memory_working_set_bytes, Some(800));
        assert_eq!(usage.memory_limit_bytes, None);
        std::fs::write(proc.join("self/cgroup"), "0::/../host\n").unwrap();
        assert!(sample_at(&proc, &cg, true, 42).container.is_none());
    }
    #[test]
    fn missing_measurements_are_absent_not_zero_usage() {
        let temp = tempfile::tempdir().unwrap();
        let sample = sample_at(temp.path(), temp.path(), false, 42);
        let json = serde_json::to_value(sample).unwrap();
        assert_eq!(json["process"], serde_json::json!({}));
        assert!(json.get("container").is_none());
    }
}
