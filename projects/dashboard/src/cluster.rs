//! Namespace-scoped Kubernetes access through the dashboard's ServiceAccount.
use anyhow::Result;
use k8s_openapi::api::core::v1::{ConfigMap, PersistentVolumeClaim, Pod};
use kube::{
    Api, Client,
    api::{
        ApiResource, DeleteParams, DynamicObject, ListParams, Patch, PatchParams, PostParams,
        Preconditions,
    },
};
use qrab_operator::api::VirtualMachine;
use qrab_proton::crd::{ProtonSpec, VpnProfile};
use std::{collections::HashMap, net::IpAddr};

pub const FIELD_MANAGER: &str = "qrab-dashboard";

/// Live usage summed over a Pod's containers, from metrics.k8s.io.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Usage {
    pub cpu_cores: f64,
    pub memory_bytes: f64,
}

#[async_trait::async_trait]
pub trait Cluster: Send + Sync {
    async fn list_vms(&self) -> Result<Vec<VirtualMachine>>;
    async fn get_vm(&self, name: &str) -> Result<Option<VirtualMachine>>;
    async fn get_pod(&self, name: &str) -> Result<Option<Pod>>;
    /// Usage keyed by Pod name.
    async fn pod_usage(&self) -> Result<HashMap<String, Usage>>;
    async fn get_pvc(&self, name: &str) -> Result<Option<PersistentVolumeClaim>>;
    /// The raw `images.json` of ConfigMap `qrab-base-images`, if present.
    async fn base_image_catalog(&self) -> Result<Option<String>>;
    async fn create_pvc(&self, pvc: &PersistentVolumeClaim) -> Result<()>;
    async fn create_vm(&self, vm: &serde_json::Value) -> Result<()>;
    /// Deletes exactly the VM with this UID.
    async fn delete_vm(&self, name: &str, uid: &str) -> Result<()>;
    /// Sets `spec.running` on exactly the VM with this UID, and nothing else.
    async fn set_running(&self, name: &str, uid: &str, running: bool) -> Result<()>;
    /// ProtonVPN profiles (docs/networking.md, "ProtonVPN").
    async fn vpn_profiles(&self) -> Result<Vec<VpnProfile>>;
    /// The broker's compressed server catalog (ConfigMap `proton-catalog`).
    async fn vpn_catalog(&self) -> Result<Option<Vec<u8>>>;
    async fn create_vpn_profile(&self, profile: &VpnProfile) -> Result<()>;
    /// Replaces `spec.proton` of exactly the profile with this UID.
    async fn patch_vpn_profile(&self, name: &str, uid: &str, proton: &ProtonSpec) -> Result<()>;
    /// Deletes exactly the profile with this UID.
    async fn delete_vpn_profile(&self, name: &str, uid: &str) -> Result<()>;
}

/// The merge patch for [`Cluster::patch_vpn_profile`]: every optional
/// field is written, unset ones as null, so a cleared choice (Auto) clears.
///
/// ```
/// let spec = qrab_proton::crd::ProtonSpec { net_shield: 1, ..Default::default() };
/// let patch = qrab_dashboard::cluster::vpn_patch("uid-a", &spec);
/// assert_eq!(patch["metadata"]["uid"], "uid-a");
/// assert!(patch["spec"]["proton"]["server"].is_null() && patch["spec"]["proton"]["country"].is_null());
/// assert_eq!(patch["spec"]["proton"]["netShield"], 1);
/// ```
pub fn vpn_patch(uid: &str, proton: &ProtonSpec) -> serde_json::Value {
    let mut spec = serde_json::to_value(proton).unwrap_or_default();
    for key in ["server", "country", "city", "features"] {
        spec.as_object_mut()
            .map(|o| o.entry(key).or_insert(serde_json::Value::Null));
    }
    serde_json::json!({"metadata": {"uid": uid}, "spec": {"proton": spec}})
}

/// The merge patch for [`Cluster::set_running`]. The API server refuses a
/// patch whose `metadata.uid` differs from the object's, so it cannot reach a
/// recreated VM of the same name.
///
/// ```
/// let patch = qrab_dashboard::cluster::running_patch("uid-a", false);
/// assert_eq!(patch, serde_json::json!({"metadata": {"uid": "uid-a"}, "spec": {"running": false}}));
/// ```
pub fn running_patch(uid: &str, running: bool) -> serde_json::Value {
    serde_json::json!({"metadata": {"uid": uid}, "spec": {"running": running}})
}

/// The API server's own message for a failed request.
pub fn describe(error: &anyhow::Error) -> String {
    match error.downcast_ref::<kube::Error>() {
        Some(kube::Error::Api(status)) => {
            format!("{} ({}): {}", status.reason, status.code, status.message)
        }
        _ => error.to_string(),
    }
}

/// A Kubernetes quantity as a number of base units (cores or bytes).
///
/// ```
/// use qrab_dashboard::cluster::quantity;
/// assert_eq!(quantity("250m"), Some(0.25));
/// assert_eq!(quantity("1500000n"), Some(0.0015));
/// assert_eq!(quantity("2"), Some(2.0));
/// assert_eq!(quantity("64Mi"), Some(67108864.0));
/// assert_eq!(quantity("1k"), Some(1000.0));
/// assert_eq!(quantity("12x"), None);
/// ```
pub fn quantity(raw: &str) -> Option<f64> {
    let raw = raw.trim();
    let split = raw
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(raw.len());
    let (number, suffix) = raw.split_at(split);
    let number: f64 = number.parse().ok()?;
    let scale = match suffix {
        "" => 1.0,
        "n" => 1e-9,
        "u" => 1e-6,
        "m" => 1e-3,
        "k" => 1e3,
        "M" => 1e6,
        "G" => 1e9,
        "T" => 1e12,
        "P" => 1e15,
        "E" => 1e18,
        "Ki" => 1024.0,
        "Mi" => 1024f64.powi(2),
        "Gi" => 1024f64.powi(3),
        "Ti" => 1024f64.powi(4),
        "Pi" => 1024f64.powi(5),
        "Ei" => 1024f64.powi(6),
        _ => return None,
    };
    number.is_finite().then_some(number * scale)
}

/// Sums a PodMetrics object's container usage.
pub fn pod_metrics_usage(object: &serde_json::Value) -> Usage {
    let mut usage = Usage::default();
    for container in object["containers"].as_array().into_iter().flatten() {
        let read = |key: &str| container["usage"][key].as_str().and_then(quantity);
        usage.cpu_cores += read("cpu").unwrap_or(0.0);
        usage.memory_bytes += read("memory").unwrap_or(0.0);
    }
    usage
}

/// The VM's Pod address, only when the Pod is controlled by this exact VM
/// (a controller ownerReference with its kind and UID, as the operator
/// checks) and carries the operator's labels for it, and reports an IP.
pub fn vm_pod_ip(vm: &VirtualMachine, pod: &Pod) -> Option<IpAddr> {
    use kube::Resource;
    let labels = pod.metadata.labels.as_ref()?;
    let label = |key: &str| labels.get(key).map(String::as_str);
    let uid = vm.metadata.uid.as_deref()?;
    let api_version = VirtualMachine::api_version(&());
    let controlled = pod.metadata.owner_references.iter().flatten().any(|o| {
        o.controller == Some(true)
            && o.kind == "VirtualMachine"
            && o.api_version == api_version
            && o.uid == uid
            && Some(o.name.as_str()) == vm.metadata.name.as_deref()
    });
    if !controlled
        || label("app.kubernetes.io/name") != Some("qrab")
        || label("app.kubernetes.io/component") != Some("vm")
        || label("app.kubernetes.io/instance") != vm.metadata.name.as_deref()
        || label("qrab.yapplesauce.com/owner-uid") != Some(uid)
        || pod.metadata.deletion_timestamp.is_some()
    {
        return None;
    }
    pod.status.as_ref()?.pod_ip.as_deref()?.parse().ok()
}

pub struct Kube {
    vms: Api<DynamicObject>,
    pods: Api<Pod>,
    pvcs: Api<PersistentVolumeClaim>,
    metrics: Api<DynamicObject>,
    configs: Api<ConfigMap>,
    vpn: Api<VpnProfile>,
}

impl Kube {
    pub fn new(client: Client, namespace: &str) -> Self {
        let vm = ApiResource::erase::<VirtualMachine>(&());
        let metrics = ApiResource {
            group: "metrics.k8s.io".into(),
            version: "v1beta1".into(),
            api_version: "metrics.k8s.io/v1beta1".into(),
            kind: "PodMetrics".into(),
            plural: "pods".into(),
        };
        Self {
            vms: Api::namespaced_with(client.clone(), namespace, &vm),
            pods: Api::namespaced(client.clone(), namespace),
            pvcs: Api::namespaced(client.clone(), namespace),
            metrics: Api::namespaced_with(client.clone(), namespace, &metrics),
            configs: Api::namespaced(client.clone(), namespace),
            vpn: Api::namespaced(client, namespace),
        }
    }
}

/// VMs are read untyped and converted one by one, so one VM the dashboard
/// cannot parse is skipped rather than hiding every VM.
fn typed(object: DynamicObject) -> Option<VirtualMachine> {
    let name = object.metadata.name.clone().unwrap_or_default();
    serde_json::to_value(object)
        .and_then(serde_json::from_value)
        .map_err(|error| tracing::warn!(vm=%name, %error, "skipping unreadable VirtualMachine"))
        .ok()
}

#[async_trait::async_trait]
impl Cluster for Kube {
    async fn list_vms(&self) -> Result<Vec<VirtualMachine>> {
        Ok(self
            .vms
            .list(&ListParams::default())
            .await?
            .items
            .into_iter()
            .filter_map(typed)
            .collect())
    }
    async fn get_vm(&self, name: &str) -> Result<Option<VirtualMachine>> {
        Ok(self.vms.get_opt(name).await?.and_then(typed))
    }
    async fn get_pod(&self, name: &str) -> Result<Option<Pod>> {
        Ok(self.pods.get_opt(name).await?)
    }
    async fn pod_usage(&self) -> Result<HashMap<String, Usage>> {
        Ok(self
            .metrics
            .list(&ListParams::default())
            .await?
            .items
            .into_iter()
            .filter_map(|object| {
                let name = object.metadata.name.clone()?;
                Some((name, pod_metrics_usage(&object.data)))
            })
            .collect())
    }
    async fn base_image_catalog(&self) -> Result<Option<String>> {
        Ok(self
            .configs
            .get_opt(crate::images::CONFIGMAP)
            .await?
            .and_then(|c| c.data)
            .and_then(|mut d| d.remove(crate::images::KEY)))
    }
    async fn get_pvc(&self, name: &str) -> Result<Option<PersistentVolumeClaim>> {
        Ok(self.pvcs.get_opt(name).await?)
    }
    async fn create_pvc(&self, pvc: &PersistentVolumeClaim) -> Result<()> {
        let params = PostParams {
            field_manager: Some(FIELD_MANAGER.into()),
            ..Default::default()
        };
        self.pvcs.create(&params, pvc).await?;
        Ok(())
    }
    async fn create_vm(&self, vm: &serde_json::Value) -> Result<()> {
        let params = PostParams {
            field_manager: Some(FIELD_MANAGER.into()),
            ..Default::default()
        };
        let object: DynamicObject = serde_json::from_value(vm.clone())?;
        self.vms.create(&params, &object).await?;
        Ok(())
    }
    async fn delete_vm(&self, name: &str, uid: &str) -> Result<()> {
        let params = DeleteParams {
            preconditions: Some(Preconditions {
                uid: Some(uid.into()),
                resource_version: None,
            }),
            ..Default::default()
        };
        self.vms.delete(name, &params).await?;
        Ok(())
    }
    async fn set_running(&self, name: &str, uid: &str, running: bool) -> Result<()> {
        let params = PatchParams {
            field_manager: Some(FIELD_MANAGER.into()),
            ..Default::default()
        };
        self.vms
            .patch(name, &params, &Patch::Merge(running_patch(uid, running)))
            .await?;
        Ok(())
    }
    async fn vpn_profiles(&self) -> Result<Vec<VpnProfile>> {
        Ok(self.vpn.list(&ListParams::default()).await?.items)
    }
    async fn vpn_catalog(&self) -> Result<Option<Vec<u8>>> {
        Ok(self
            .configs
            .get_opt(qrab_proton::broker::CATALOG_CONFIGMAP)
            .await?
            .and_then(|c| c.binary_data)
            .and_then(|mut d| d.remove(qrab_proton::broker::CATALOG_KEY))
            .map(|b| b.0))
    }
    async fn create_vpn_profile(&self, profile: &VpnProfile) -> Result<()> {
        let params = PostParams {
            field_manager: Some(FIELD_MANAGER.into()),
            ..Default::default()
        };
        self.vpn.create(&params, profile).await?;
        Ok(())
    }
    async fn patch_vpn_profile(&self, name: &str, uid: &str, proton: &ProtonSpec) -> Result<()> {
        let params = PatchParams {
            field_manager: Some(FIELD_MANAGER.into()),
            ..Default::default()
        };
        self.vpn
            .patch(name, &params, &Patch::Merge(vpn_patch(uid, proton)))
            .await?;
        Ok(())
    }
    async fn delete_vpn_profile(&self, name: &str, uid: &str) -> Result<()> {
        let params = DeleteParams {
            preconditions: Some(Preconditions {
                uid: Some(uid.into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        self.vpn.delete(name, &params).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access::tests::vm;

    fn owner(uid: &str, controller: bool) -> serde_json::Value {
        serde_json::json!([{"apiVersion": "qrab.yapplesauce.com/v1alpha1", "kind": "VirtualMachine",
            "name": "a", "uid": uid, "controller": controller}])
    }

    fn pod(labels: serde_json::Value, ip: &str) -> Pod {
        owned(labels, ip, owner("uid-a", true))
    }

    fn owned(labels: serde_json::Value, ip: &str, owners: serde_json::Value) -> Pod {
        serde_json::from_value(serde_json::json!({
            "metadata": {"name": "a", "labels": labels, "ownerReferences": owners},
            "status": {"podIP": ip}
        }))
        .unwrap()
    }

    #[test]
    fn pod_ip_requires_the_operator_labels_for_this_vm() {
        let vm = vm("a", serde_json::json!({}), None);
        let labels = serde_json::json!({"app.kubernetes.io/name": "qrab", "app.kubernetes.io/component": "vm",
            "app.kubernetes.io/instance": "a", "qrab.yapplesauce.com/owner-uid": "uid-a"});
        assert_eq!(
            vm_pod_ip(&vm, &pod(labels.clone(), "10.0.0.7")),
            Some("10.0.0.7".parse().unwrap())
        );
        for (key, value) in [
            ("app.kubernetes.io/instance", "b"),
            ("qrab.yapplesauce.com/owner-uid", "uid-b"),
            ("app.kubernetes.io/component", "dashboard"),
        ] {
            let mut wrong = labels.clone();
            wrong[key] = value.into();
            assert_eq!(vm_pod_ip(&vm, &pod(wrong, "10.0.0.7")), None, "{key}");
        }
        assert_eq!(vm_pod_ip(&vm, &pod(labels.clone(), "not-an-ip")), None);
        // The labels alone are not enough: the VM must control the Pod.
        for owners in [
            owner("uid-b", true),
            owner("uid-a", false),
            serde_json::json!([]),
        ] {
            assert_eq!(
                vm_pod_ip(&vm, &owned(labels.clone(), "10.0.0.7", owners)),
                None
            );
        }
    }

    #[test]
    fn pod_metrics_sum_containers() {
        let usage = pod_metrics_usage(&serde_json::json!({"containers": [
            {"name": "manager", "usage": {"cpu": "250m", "memory": "512Mi"}},
            {"name": "bmc", "usage": {"cpu": "1000000n", "memory": "1Mi"}}
        ]}));
        assert!((usage.cpu_cores - 0.251).abs() < 1e-9);
        assert_eq!(usage.memory_bytes, 513.0 * 1024.0 * 1024.0);
    }
}
