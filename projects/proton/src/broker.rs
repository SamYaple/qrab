//! The in-cluster broker: the only holder of the Proton session. For each
//! [`VpnProfile`] it keeps a key, a certificate and Secret `vpn-<profile>`.
//!
//! It never reads a Secret other than `proton-session` and `proton-keys`;
//! profile Secrets are written blind (server-side apply, every pass, which is
//! a no-op when nothing changed), so RBAC needs no read access to Secrets.
use crate::{
    api::{Certificate, Error},
    catalog::{Catalog, Current, Feature},
    crd::{secret_name, Condition, VpnProfile, VpnProfileStatus},
    keys::ProfileKey,
    wg::{wg_conf, Features},
};
use sha2::{Digest, Sha256};
use std::future::Future;
use zeroize::Zeroizing;

pub const KEYS_SECRET: &str = "proton-keys";
pub const CATALOG_CONFIGMAP: &str = "proton-catalog";
pub const CATALOG_KEY: &str = "catalog.json.gz";
/// Certificates last a week (as ProtonVPN's own client asks) and are renewed
/// at half-life, or when Proton says so.
pub const CERTIFICATE_MINUTES: u32 = 7 * 24 * 60;
pub const COMPONENT: &str = "proton-vpn";
pub const PROFILE_LABEL: &str = "qrab.yapplesauce.com/vpn-profile";
pub const FIELD_MANAGER: &str = "qrab-proton-broker";

/// What a pass knows about the account and its servers.
pub struct World<'a> {
    pub catalog: &'a Catalog,
    pub max_tier: u8,
    pub max_connect: u32,
    /// Unix seconds.
    pub now: i64,
}

/// What reconciling one profile produced: a config to apply (if a server is
/// selected and a valid certificate covers the key) and the new status.
pub struct Outcome {
    pub conf: Option<Zeroizing<String>>,
    pub status: VpnProfileStatus,
}

pub fn ready(status: bool, reason: &str, message: impl Into<String>) -> Condition {
    Condition {
        type_: "Ready".into(),
        status: if status { "True" } else { "False" }.into(),
        reason: reason.into(),
        message: message.into(),
    }
}

/// Status for every profile when no pass can run (no session, retired app
/// version, Proton unreachable): selection and certificate fields stay as
/// they were, and existing Secrets are left alone.
pub fn blocked(profile: &VpnProfile, reason: &str, message: &str) -> VpnProfileStatus {
    let mut status = profile.status.clone().unwrap_or_default();
    status.conditions = vec![ready(false, reason, message)];
    status
}

fn when_to_renew(certificate: &Certificate, now: i64) -> i64 {
    let half = now + (certificate.expiration_time - now) / 2;
    if certificate.refresh_time > now {
        certificate.refresh_time.min(half)
    } else {
        half
    }
}

/// Reconcile one profile: select a server, (re)certify the key when the
/// certificate is missing, due, for another key or for other features, and
/// render the config. `rank` is the profile's place among all profiles by
/// age, to flag those beyond the plan's connection limit.
pub async fn reconcile<C, Fut>(
    profile: &VpnProfile,
    key: &ProfileKey,
    world: &World<'_>,
    rank: usize,
    certify: C,
) -> Outcome
where
    C: FnOnce(Features) -> Fut,
    Fut: Future<Output = Result<Certificate, Error>>,
{
    let spec = &profile.spec.proton;
    let old = profile.status.clone().unwrap_or_default();
    let mut status = VpnProfileStatus {
        observed_generation: profile.metadata.generation,
        secret_name: Some(secret_name(&kube::ResourceExt::name_any(profile))),
        ..old.clone()
    };
    let current = match (&old.logical_id, &old.physical_id) {
        (Some(logical), Some(physical)) => Some(Current { logical, physical }),
        _ => None,
    };
    let choice = match world
        .catalog
        .select(&spec.criteria(), world.max_tier, current)
    {
        Ok(choice) => choice,
        Err(why) => {
            status.conditions = vec![ready(false, "NoServer", why)];
            return Outcome { conf: None, status };
        }
    };
    let features = spec.connection_features();
    let public_key = key.wireguard_public();
    let valid = old
        .certificate_expires
        .is_some_and(|expires| expires > world.now + 600);
    let due = !valid
        || old.certificate_renews.is_none_or(|at| world.now >= at)
        || old.certified_features.as_ref() != Some(&features)
        || old.public_key.as_deref() != Some(public_key.as_str());
    let mut certificate_error = None;
    if due {
        match certify(features.clone()).await {
            Ok(certificate) => {
                status.certificate_expires = Some(certificate.expiration_time);
                status.certificate_renews = Some(when_to_renew(&certificate, world.now));
                status.certified_features = Some(features.clone());
                status.public_key = Some(public_key.clone());
            }
            Err(error) => certificate_error = Some(error.to_string()),
        }
    }
    let covered = status.public_key.as_deref() == Some(public_key.as_str())
        && status
            .certificate_expires
            .is_some_and(|expires| expires > world.now);
    if !covered {
        status.conditions = vec![ready(
            false,
            "CertificateFailed",
            certificate_error.unwrap_or_else(|| "no valid certificate for the key".into()),
        )];
        return Outcome { conf: None, status };
    }
    // Features changed but could not be re-certified: the old certificate
    // still carries the old features, so render what is actually certified.
    let applied = status.certified_features.clone().unwrap_or(features);
    let conf = wg_conf(key, &choice, &applied);
    status.config_hash = Some(format!("{:x}", Sha256::digest(conf.as_bytes())));
    status.server = Some(choice.name.clone());
    status.logical_id = Some(choice.logical_id.clone());
    status.physical_id = Some(choice.physical_id.clone());
    status.entry_country = Some(choice.entry_country.clone());
    status.exit_country = Some(choice.exit_country.clone());
    status.city = choice.city.clone();
    status.entry_ip = Some(choice.entry_ip.to_string());
    status.load = Some(choice.load);
    status.score = Some(choice.score);
    status.server_features = Feature::of(choice.features);
    let mut message = format!(
        "{} ({}), Secret {}",
        choice.name,
        choice.exit_country,
        status.secret_name.as_deref().unwrap_or_default()
    );
    if let Some(error) = certificate_error {
        message += &format!("; certificate renewal failed, the current one still holds: {error}");
    }
    if world.max_connect > 0 && rank >= world.max_connect as usize {
        message += &format!(
            "; beyond the plan's {} simultaneous connections",
            world.max_connect
        );
    }
    status.conditions = vec![ready(true, "Configured", message)];
    Outcome {
        conf: Some(conf),
        status,
    }
}

/// A NIC anchor Pod as the broker sees it: which Secret it mounts and its
/// qrab-netd port-forward condition (status, message).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Anchor {
    pub name: String,
    pub vm: String,
    pub nic: String,
    pub node: Option<String>,
    pub secret: String,
    pub port_forward: Option<(bool, String)>,
}

/// Fill a profile's attachments and forwarded port from the anchors that
/// mount its Secret. One profile is one key and one Proton connection, so
/// anchors of two different VMs make the profile not ready.
///
/// ```
/// use qrab_proton::{broker::{attach, Anchor}, crd::VpnProfileStatus};
/// let anchor = |name: &str, node: &str, port: Option<(bool, &str)>| Anchor {
///     name: name.into(), vm: "vm1".into(), nic: "vpn".into(), node: Some(node.into()),
///     secret: "vpn-ch".into(), port_forward: port.map(|(ok, m)| (ok, m.into())),
/// };
/// let mut status = VpnProfileStatus::default();
/// // Mid-migration: the standby anchor gets no reply, the active one maps.
/// attach(&mut status, "vpn-ch", true, &[
///     anchor("vm1-nic-vpn-kube01", "kube01", Some((false, "no NAT-PMP reply"))),
///     anchor("vm1-nic-vpn-kube03", "kube03", Some((true, "41234"))),
/// ]);
/// assert_eq!(status.forwarded_port, Some(41234));
/// assert_eq!(status.attached.len(), 2);
/// ```
pub fn attach(
    status: &mut VpnProfileStatus,
    secret: &str,
    port_forwarding: bool,
    anchors: &[Anchor],
) {
    let mine: Vec<&Anchor> = anchors.iter().filter(|a| a.secret == secret).collect();
    status.attached = mine
        .iter()
        .map(|a| crate::crd::Attachment {
            vm: a.vm.clone(),
            nic: a.nic.clone(),
            node: a.node.clone(),
            anchor: a.name.clone(),
        })
        .collect();
    status.attached.sort_by(|a, b| a.anchor.cmp(&b.anchor));
    status.forwarded_port = None;
    status.port_forward_message = None;
    if port_forwarding {
        let mapped = mine.iter().find_map(|a| match &a.port_forward {
            Some((true, port)) => port.parse::<u16>().ok(),
            _ => None,
        });
        status.forwarded_port = mapped;
        if mapped.is_none() {
            status.port_forward_message = Some(
                mine.iter()
                    .find_map(|a| a.port_forward.as_ref().map(|(_, m)| m.clone()))
                    .unwrap_or_else(|| {
                        if mine.is_empty() {
                            "no NIC uses the profile yet".into()
                        } else {
                            "waiting for qrab-netd's first mapping".into()
                        }
                    }),
            );
        }
    }
    let mut vms: Vec<&str> = mine.iter().map(|a| a.vm.as_str()).collect();
    vms.sort();
    vms.dedup();
    if vms.len() > 1 {
        status.conditions = vec![ready(
            false,
            "SharedProfile",
            format!(
                "NICs of {} all use {secret}; one profile is one connection, give each NIC its own",
                vms.join(", ")
            ),
        )];
    }
}

pub mod runner;
