use crate::{api::VirtualMachine, Error, Result};
use k8s_openapi::api::core::v1::{ConfigMap, Pod};
use kube::{Resource, ResourceExt};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const HASH: &str = "qrab.yapplesauce.com/template-hash";

#[derive(Clone)]
pub struct Settings {
    pub namespace: String,
    pub vm_image: String,
    pub oidc_issuer: String,
    pub oidc_client_id: String,
    pub idp_ip: Option<String>,
}

pub fn pinned_image(image: &str) -> bool {
    image
        .strip_prefix("zot.yapplesauce.com/qrab/manager@sha256:")
        .is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
        })
}

fn dns_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

pub fn origin(raw: &str) -> Result<String> {
    let url = url::Url::parse(raw).map_err(|e| Error::Invalid(e.to_string()))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Invalid("console.publicUrl must be an HTTPS origin without credentials, path, query or fragment".into()));
    }
    Ok(url.origin().ascii_serialization())
}

pub fn validate(vm: &VirtualMachine, settings: &Settings) -> Result<()> {
    if vm.name_any().len() > 50
        || !dns_name(&vm.name_any())
        || !dns_name(&vm.spec.state_volume_claim)
    {
        return Err(Error::Invalid(
            "invalid VM name (max 50) or stateVolumeClaim".into(),
        ));
    }
    if !(256..=1_048_576).contains(&vm.spec.memory_mib)
        || !(1..=256).contains(&vm.spec.cpus)
        || vm.spec.restart_nonce.len() > 128
    {
        return Err(Error::Invalid(
            "invalid memory, CPU count or restartNonce".into(),
        ));
    }
    if !pinned_image(vm.spec.image.as_deref().unwrap_or(&settings.vm_image)) {
        return Err(Error::Invalid(
            "image must pin an immutable qrab/manager digest".into(),
        ));
    }
    if !vm.spec.boot_disk.starts_with('/')
        || vm.spec.boot_disk.len() > 4096
        || vm.spec.boot_disk.chars().any(char::is_control)
    {
        return Err(Error::Invalid(
            "bootDisk must be an absolute image path".into(),
        ));
    }
    origin(&vm.spec.console.public_url)?;
    if vm.spec.console.allowed_subjects.is_empty()
        || vm.spec.console.allowed_subjects.len() > 128
        || vm
            .spec
            .console
            .allowed_subjects
            .iter()
            .any(|s| uuid::Uuid::parse_str(s).is_err())
    {
        return Err(Error::Invalid(
            "console.allowedSubjects must contain operator UUIDs".into(),
        ));
    }
    if vm.uid().is_none() {
        return Err(Error::Invalid("VM UID is required".into()));
    }
    Ok(())
}

pub struct Desired {
    pub config: ConfigMap,
    pub pod: Pod,
    pub hash: String,
}

fn memory_quantity(mib: u32) -> String {
    if mib.is_multiple_of(1024) {
        format!("{}Gi", mib / 1024)
    } else {
        format!("{mib}Mi")
    }
}

pub fn render(vm: &VirtualMachine, settings: &Settings) -> Result<Desired> {
    validate(vm, settings)?;
    let name = vm.name_any();
    let config_name = format!("{name}-config");
    let uid = vm.uid().expect("validated UID");
    let mut owner = vm.controller_owner_ref(&()).expect("VM has a UID");
    // Background GC needs no delete/finalizer authority on the parent CR.
    owner.block_owner_deletion = Some(false);
    let labels = json!({"app.kubernetes.io/name":"qrab", "app.kubernetes.io/component":"vm", "app.kubernetes.io/instance":name, "qrab.yapplesauce.com/owner-uid":uid});
    let config = json!({"name":name,"memory_mib":vm.spec.memory_mib,"cpus":vm.spec.cpus,"accelerator":"tcg","console":true,"uefi":{"boot_disk":vm.spec.boot_disk},"tpm":{},"state_dir":"/var/lib/qrab","state_id":uid,"runtime_dir":"/run/qrab","startup_timeout_secs":30,"shutdown_grace_secs":5});
    let config_data = serde_json::to_string_pretty(&config)?;
    let config_map: ConfigMap = serde_json::from_value(
        json!({"apiVersion":"v1","kind":"ConfigMap","metadata":{"name":config_name,"namespace":settings.namespace,"labels":labels,"ownerReferences":[owner]},"data":{"vm.json":config_data}}),
    )?;
    let mut pod = json!({"apiVersion":"v1","kind":"Pod","metadata":{"name":name,"namespace":settings.namespace,"labels":labels,"ownerReferences":[owner]},"spec":{
        "serviceAccountName":"qrab-manager","automountServiceAccountToken":false,"enableServiceLinks":false,
        "hostNetwork":false,"hostPID":false,"hostIPC":false,
        "restartPolicy":"Never","terminationGracePeriodSeconds":20,
        "securityContext":{"runAsNonRoot":true,"runAsUser":10001,"runAsGroup":10001,"fsGroup":10001,"fsGroupChangePolicy":"OnRootMismatch","seccompProfile":{"type":"RuntimeDefault"}},
        "tolerations":[{"key":"node-role.kubernetes.io/control-plane","operator":"Exists","effect":"NoSchedule"}],
        "containers":[{"name":"manager","image":vm.spec.image.as_deref().unwrap_or(&settings.vm_image),"imagePullPolicy":"Always","command":["qrab-bmc"],"args":["--config","/etc/qrab/vm.json"],
            "ports":[{"name":"http","containerPort":8080}],
            "env":[{"name":"BMC_PUBLIC_URL","value":origin(&vm.spec.console.public_url)?},{"name":"BMC_OIDC_ISSUER","value":settings.oidc_issuer},{"name":"BMC_OIDC_CLIENT_ID","value":settings.oidc_client_id},{"name":"BMC_ALLOWED_SUBJECTS","value":vm.spec.console.allowed_subjects.join(",")}],
            "resources":{"requests":{"cpu":vm.spec.cpus.to_string(),"memory":memory_quantity(vm.spec.memory_mib+256)},"limits":{"cpu":(vm.spec.cpus+1).to_string(),"memory":memory_quantity(vm.spec.memory_mib+512)}},
            "securityContext":{"allowPrivilegeEscalation":false,"privileged":false,"readOnlyRootFilesystem":true,"capabilities":{"drop":["ALL"]}},"terminationMessagePolicy":"FallbackToLogsOnError",
            "volumeMounts":[{"name":"config","mountPath":"/etc/qrab","readOnly":true},{"name":"state","mountPath":"/var/lib/qrab"},{"name":"runtime","mountPath":"/run/qrab"},{"name":"tmp","mountPath":"/tmp"}],
            "startupProbe":{"httpGet":{"path":"/readyz","port":"http"},"periodSeconds":1,"failureThreshold":90,"timeoutSeconds":2},
            "readinessProbe":{"httpGet":{"path":"/readyz","port":"http"},"periodSeconds":2,"failureThreshold":3,"timeoutSeconds":2},
            "livenessProbe":{"httpGet":{"path":"/readyz","port":"http"},"periodSeconds":5,"failureThreshold":3,"timeoutSeconds":2}}],
        "volumes":[{"name":"config","configMap":{"name":config_name}},{"name":"state","persistentVolumeClaim":{"claimName":vm.spec.state_volume_claim}},{"name":"runtime","emptyDir":{"medium":"Memory","sizeLimit":"64Mi"}},{"name":"tmp","emptyDir":{"medium":"Memory","sizeLimit":"64Mi"}}]
    }});
    if let Some(ip) = &settings.idp_ip {
        let issuer =
            url::Url::parse(&settings.oidc_issuer).map_err(|e| Error::Invalid(e.to_string()))?;
        pod["spec"]["hostAliases"] = json!([{"ip":ip,"hostnames":[issuer.host_str().ok_or_else(||Error::Invalid("OIDC issuer has no host".into()))?]}]);
    }
    let hash = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(
            &json!({"pod":pod,"config":config_data,"restartNonce":vm.spec.restart_nonce})
        )?)
    );
    pod["metadata"]["annotations"] = json!({HASH:hash});
    Ok(Desired {
        config: config_map,
        pod: serde_json::from_value(pod)?,
        hash,
    })
}

pub fn owned<K: Resource<DynamicType = ()>>(resource: &K, vm: &VirtualMachine) -> bool {
    resource
        .meta()
        .owner_references
        .as_ref()
        .is_some_and(|owners| {
            owners.iter().any(|o| {
                o.controller == Some(true)
                    && Some(o.uid.clone()) == vm.uid()
                    && o.kind == "VirtualMachine"
                    && o.name == vm.name_any()
            })
        })
}

/// Compare only declared fields: the API server adds defaults to stored Pods.
/// Arrays retain their declared membership, except injected default tolerations.
fn contains(actual: &Value, desired: &Value) -> bool {
    match (actual, desired) {
        (Value::Object(a), Value::Object(d)) => d.iter().all(|(key, value)| {
            if key == "tolerations" {
                return value.as_array().is_some_and(|want| {
                    a.get(key).and_then(Value::as_array).is_some_and(|have| {
                        want.iter().all(|v| have.iter().any(|h| contains(h, v)))
                    })
                });
            }
            a.get(key).is_some_and(|v| contains(v, value))
        }),
        (Value::Array(a), Value::Array(d)) => {
            a.len() == d.len() && a.iter().zip(d).all(|(a, d)| contains(a, d))
        }
        _ => actual == desired,
    }
}

pub fn pod_matches(actual: &Pod, desired: &Desired) -> bool {
    let Some(spec) = &actual.spec else {
        return false;
    };
    if spec.init_containers.as_ref().is_some_and(|v| !v.is_empty())
        || spec
            .image_pull_secrets
            .as_ref()
            .is_some_and(|v| !v.is_empty())
    {
        return false;
    }
    for container in &spec.containers {
        if container.env_from.as_ref().is_some_and(|v| !v.is_empty())
            || container
                .volume_devices
                .as_ref()
                .is_some_and(|v| !v.is_empty())
        {
            return false;
        }
        for mount in container.volume_mounts.iter().flatten() {
            if mount.sub_path.as_ref().is_some_and(|v| !v.is_empty())
                || mount.sub_path_expr.as_ref().is_some_and(|v| !v.is_empty())
                || (mount.name != "config" && mount.read_only.unwrap_or(false))
                || mount
                    .mount_propagation
                    .as_ref()
                    .is_some_and(|v| v != "None")
            {
                return false;
            }
        }
    }
    if spec.volumes.iter().flatten().any(|v| {
        v.persistent_volume_claim
            .as_ref()
            .is_some_and(|v| v.read_only.unwrap_or(false))
    }) {
        return false;
    }
    let mut actual_spec = serde_json::to_value(&actual.spec).expect("Pod serializes");
    // These Go PodSpec bool fields use omitempty; absent means false. Do not
    // apply this normalization to pointer booleans such as token mounting.
    for field in ["hostNetwork", "hostPID", "hostIPC"] {
        if actual_spec.get(field).is_none_or(Value::is_null) {
            actual_spec[field] = Value::Bool(false);
        }
    }
    actual.annotations().get(HASH) == Some(&desired.hash)
        && desired
            .pod
            .labels()
            .iter()
            .all(|(k, v)| actual.labels().get(k) == Some(v))
        && contains(
            &actual_spec,
            &serde_json::to_value(&desired.pod.spec).expect("Pod serializes"),
        )
}

pub fn data(config: &ConfigMap) -> BTreeMap<String, String> {
    config.data.clone().unwrap_or_default()
}
