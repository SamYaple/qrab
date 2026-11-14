//! Wire fixtures from current query-cpus-fast, independent of QEMU/hardware.
use qapi::{CpuInfoFast, CpuInfoFastBranch};
use serde_json::json;

#[test]
fn current_cpu_info_fast_accepts_known_targets_with_implicit_empty_branches() {
    qapi::with_version((11, 1, 3), || {
        for target in ["x86_64", "i386", "aarch64", "riscv64", "ppc64", "mips64"] {
            let value = json!({"cpu-index":0,"qom-path":"/machine/unattached/device[0]","qom-type":"max-x86_64-cpu",
                "thread-id":29781,"target":target,"props":{"socket-id":0,"core-id":0,"thread-id":0}});
            let cpu: CpuInfoFast = serde_json::from_value(value.clone())
                .unwrap_or_else(|error| panic!("{target}: {error}"));
            assert_eq!(cpu.cpu_index, 0);
            assert_eq!(cpu.thread_id, 29781);
            assert_eq!(cpu.target.as_ref().unwrap().to_string(), target);
            assert!(
                cpu.u.is_some(),
                "implicit empty branch is represented explicitly"
            );
            assert_eq!(serde_json::to_value(&cpu).unwrap(), value);
        }
        // props is optional on real replies; no synthetic payload keys appear.
        let value = json!({"cpu-index":1,"qom-path":"/machine/unattached/device[1]","qom-type":"max-x86_64-cpu","thread-id":29782,"target":"x86_64"});
        let cpu: CpuInfoFast = serde_json::from_value(value.clone()).unwrap();
        assert!(cpu.props.is_none());
        assert_eq!(serde_json::to_value(cpu).unwrap(), value);
    });
}

#[test]
fn current_s390_cpu_info_keeps_required_branch_data() {
    qapi::with_version((11, 1, 3), || {
        let value = json!({"cpu-index":0,"qom-path":"/machine/unattached/device[0]","qom-type":"gen17-s390x-cpu",
            "thread-id":29800,"target":"s390x","cpu-state":"operating","dedicated":true,"entitlement":"high"});
        let cpu: CpuInfoFast = serde_json::from_value(value.clone()).unwrap();
        assert!(matches!(cpu.u, Some(CpuInfoFastBranch::S390x(_))));
        assert_eq!(serde_json::to_value(cpu).unwrap(), value);
        let mut missing = value;
        missing.as_object_mut().unwrap().remove("cpu-state");
        assert!(
            serde_json::from_value::<CpuInfoFast>(missing).is_err(),
            "explicit payload branch remains required"
        );
    });
}

#[test]
fn unknown_cpu_targets_are_still_rejected() {
    qapi::with_version((11, 1, 3), || {
        let value = json!({"cpu-index":0,"qom-path":"/machine/unattached/device[0]","qom-type":"future-cpu",
            "thread-id":29800,"target":"future-unknown-target"});
        assert!(serde_json::from_value::<CpuInfoFast>(value).is_err());
    });
}
