//! Per-VM roles, mirroring the BMC. The BMC still enforces its own roles on
//! every bearer request; this decides what the dashboard shows and offers.
use qrab_operator::{api::VirtualMachine, resources::CREATOR};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Viewer,
    Admin,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Self::Viewer => "Viewer",
            Self::Admin => "Admin",
        }
    }
}

fn listed(list: &[String], value: &str) -> bool {
    list.iter()
        .any(|item| item.trim().eq_ignore_ascii_case(value))
}

fn any_listed(list: &[String], groups: &[String]) -> bool {
    groups.iter().any(|group| listed(list, group))
}

/// The strongest role `subject` holds on `vm`, directly or through the
/// token's group UUIDs. `None` means the VM is invisible to them.
pub fn role(vm: &VirtualMachine, subject: &str, groups: &[String]) -> Option<Role> {
    let console = vm.spec.console.as_ref();
    let redfish = &vm.spec.components.redfish;
    let empty: &[String] = &[];
    let console_list = |pick: fn(&qrab_operator::api::Console) -> &[String]| -> &[String] {
        console.map_or(empty, pick)
    };
    let admin = listed(console_list(|c| &c.allowed_subjects), subject)
        || listed(&redfish.admin_subjects, subject)
        || any_listed(console_list(|c| &c.admin_groups), groups)
        || any_listed(&redfish.admin_groups, groups);
    if admin {
        return Some(Role::Admin);
    }
    let creator = vm
        .metadata
        .annotations
        .as_ref()
        .and_then(|a| a.get(CREATOR))
        .is_some_and(|c| c.trim().eq_ignore_ascii_case(subject));
    let viewer = creator
        || listed(console_list(|c| &c.viewer_subjects), subject)
        || listed(&redfish.viewer_subjects, subject)
        || any_listed(console_list(|c| &c.viewer_groups), groups)
        || any_listed(&redfish.viewer_groups, groups);
    viewer.then_some(Role::Viewer)
}

/// The VMs a user may see, each with their role, sorted by name.
pub fn visible<'a>(
    vms: &'a [VirtualMachine],
    subject: &str,
    groups: &[String],
) -> Vec<(&'a VirtualMachine, Role)> {
    let mut seen: Vec<_> = vms
        .iter()
        .filter_map(|vm| role(vm, subject, groups).map(|role| (vm, role)))
        .collect();
    seen.sort_by(|a, b| a.0.metadata.name.cmp(&b.0.metadata.name));
    seen
}

/// Launchers are the members of the configured launcher group.
pub fn can_launch(groups: &[String], launcher_group: &str) -> bool {
    !launcher_group.is_empty() && listed(groups, launcher_group)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub const ALICE: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
    pub const BOB: &str = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
    pub const ADMINS: &str = "11111111-1111-4111-8111-111111111111";
    pub const VIEWERS: &str = "22222222-2222-4222-8222-222222222222";
    pub const RF_ADMINS: &str = "33333333-3333-4333-8333-333333333333";
    pub const RF_VIEWERS: &str = "44444444-4444-4444-8444-444444444444";

    pub fn vm(name: &str, spec: serde_json::Value, creator: Option<&str>) -> VirtualMachine {
        let mut metadata =
            serde_json::json!({"name": name, "namespace": "qrab", "uid": format!("uid-{name}")});
        if let Some(c) = creator {
            metadata["annotations"] = serde_json::json!({ CREATOR: c });
        }
        let mut spec_value = serde_json::json!({"stateVolumeClaim": format!("{name}-state")});
        spec_value
            .as_object_mut()
            .unwrap()
            .extend(spec.as_object().unwrap().clone());
        serde_json::from_value(serde_json::json!({
            "apiVersion": "qrab.yapplesauce.com/v1alpha1",
            "kind": "VirtualMachine",
            "metadata": metadata,
            "spec": spec_value,
        }))
        .unwrap()
    }

    fn console(fields: serde_json::Value) -> serde_json::Value {
        let mut c = serde_json::json!({"publicUrl": "https://vm.example"});
        c.as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        serde_json::json!({ "console": c })
    }

    #[test]
    fn subjects_groups_and_creator_resolve_roles() {
        let g = |ids: &[&str]| ids.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let by_subject = vm(
            "a",
            console(
                serde_json::json!({"allowedSubjects": [ALICE.to_uppercase()], "viewerSubjects": [BOB]}),
            ),
            None,
        );
        assert_eq!(role(&by_subject, ALICE, &[]), Some(Role::Admin));
        assert_eq!(role(&by_subject, BOB, &[]), Some(Role::Viewer));
        let by_group = vm(
            "b",
            console(serde_json::json!({"adminGroups": [ADMINS], "viewerGroups": [VIEWERS]})),
            None,
        );
        assert_eq!(role(&by_group, BOB, &g(&[ADMINS])), Some(Role::Admin));
        assert_eq!(role(&by_group, BOB, &g(&[VIEWERS])), Some(Role::Viewer));
        assert_eq!(role(&by_group, BOB, &g(&[RF_ADMINS])), None);
        let mut redfish = console(serde_json::json!({}));
        redfish["components"] = serde_json::json!({"redfish": {"enabled": true,
            "adminSubjects": [ALICE], "viewerSubjects": [BOB],
            "adminGroups": [RF_ADMINS], "viewerGroups": [RF_VIEWERS]}});
        let redfish = vm("c", redfish, None);
        assert_eq!(role(&redfish, ALICE, &[]), Some(Role::Admin));
        assert_eq!(role(&redfish, BOB, &[]), Some(Role::Viewer));
        let carol = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
        assert_eq!(role(&redfish, carol, &g(&[RF_ADMINS])), Some(Role::Admin));
        assert_eq!(role(&redfish, carol, &g(&[RF_VIEWERS])), Some(Role::Viewer));
        assert_eq!(role(&redfish, carol, &[]), None);
        // The creator is a viewer, and admin wins over every viewer grant.
        let created = vm(
            "d",
            console(serde_json::json!({"adminGroups": [ADMINS], "viewerSubjects": [ALICE]})),
            Some(ALICE),
        );
        assert_eq!(role(&created, ALICE, &[]), Some(Role::Viewer));
        assert_eq!(role(&created, ALICE, &g(&[ADMINS])), Some(Role::Admin));
        assert_eq!(role(&created, BOB, &[]), None);
        // No console and no grants: invisible to everyone.
        let bare = vm(
            "e",
            serde_json::json!({"components": {"bmc": {"enabled": false}}}),
            None,
        );
        assert_eq!(role(&bare, ALICE, &g(&[ADMINS, VIEWERS])), None);
    }

    #[test]
    fn listing_hides_invisible_vms() {
        let vms = vec![
            vm(
                "zeta",
                console(serde_json::json!({"viewerGroups": [VIEWERS]})),
                None,
            ),
            vm(
                "alpha",
                console(serde_json::json!({"allowedSubjects": [ALICE]})),
                None,
            ),
            vm(
                "hidden",
                console(serde_json::json!({"allowedSubjects": [BOB]})),
                None,
            ),
            vm(
                "mine",
                console(serde_json::json!({"allowedSubjects": [BOB]})),
                Some(ALICE),
            ),
        ];
        let seen: Vec<_> = visible(&vms, ALICE, &[VIEWERS.into()])
            .into_iter()
            .map(|(vm, role)| (vm.metadata.name.clone().unwrap(), role))
            .collect();
        assert_eq!(
            seen,
            [
                ("alpha".into(), Role::Admin),
                ("mine".into(), Role::Viewer),
                ("zeta".into(), Role::Viewer)
            ]
        );
        assert!(visible(&vms, "dddddddd-dddd-4ddd-8ddd-dddddddddddd", &[]).is_empty());
    }

    #[test]
    fn launching_requires_the_launcher_group() {
        assert!(can_launch(&[VIEWERS.into(), ADMINS.into()], ADMINS));
        assert!(!can_launch(&[VIEWERS.into()], ADMINS));
        assert!(!can_launch(&[], ADMINS));
        assert!(!can_launch(&[String::new()], ""));
    }
}
