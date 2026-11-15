use qrab_operator::{
    api::{CloudInit, VirtualMachine},
    resources::{self, NicAddress, Settings},
    Error,
};
use serde_json::{json, Value};

fn settings() -> Settings {
    Settings {
        namespace: "qrab".into(),
        vm_image: format!("zot.yapplesauce.com/qrab/manager@sha256:{}", "a".repeat(64)),
        oidc_issuer: "https://idm.yaple.net/oauth2/openid/qrab-bmc".into(),
        oidc_client_id: "qrab-bmc".into(),
        idp_ip: None,
        redfish_oidc_issuer: "https://idm.yaple.net/oauth2/openid/qrab-redfish".into(),
        redfish_oidc_audience: "qrab-redfish".into(),
        bmc_bearer: None,
        nic_addresses: None,
    }
}

fn vm(cloud_init: Option<Value>) -> VirtualMachine {
    let mut value = json!({"apiVersion":"qrab.yapplesauce.com/v1alpha1","kind":"VirtualMachine",
        "metadata":{"name":"test-vm","namespace":"qrab","uid":"0b7c4e6a-uid","generation":1},
        "spec":{"stateVolumeClaim":"test-state","console":{"publicUrl":"https://test-vm.yapplesauce.com","allowedSubjects":["6175a2a1-efb9-44e8-8c0b-5fe7ace18f9b"]}}});
    if let Some(cloud_init) = cloud_init {
        value["spec"]["cloudInit"] = cloud_init;
    }
    serde_json::from_value(value).unwrap()
}

fn vm_json(desired: &resources::Desired) -> Value {
    serde_json::from_str(&desired.config.data.clone().unwrap()["vm.json"]).unwrap()
}

#[test]
fn seed_is_rendered_only_with_cloud_init_and_the_plain_template_is_unchanged() {
    let plain = resources::render(&vm(None), &settings()).unwrap();
    assert!(vm_json(&plain).get("seed").is_none());
    assert!(!serde_json::to_string(&vm(None).spec)
        .unwrap()
        .contains("cloudInit"));

    let user_data = "#cloud-config\npassword: hunter2\nchpasswd: { expire: False }\n";
    let seeded = resources::render(
        &vm(Some(json!({"hostname":"seedtest","userData":user_data}))),
        &settings(),
    )
    .unwrap();
    let seed = &vm_json(&seeded)["seed"];
    assert_eq!(
        seed,
        &json!({"meta_data":"{\"instance-id\":\"0b7c4e6a-uid\",\"local-hostname\":\"seedtest\"}\n","user_data":user_data})
    );
    // The seed changes the instance, so it changes the template hash; the Pod
    // spec itself (volumes, mounts, sidecars) is the same.
    assert_ne!(seeded.hash, plain.hash);
    assert_eq!(
        serde_json::to_value(&seeded.pod.spec).unwrap(),
        serde_json::to_value(&plain.pod.spec).unwrap()
    );
    // user-data goes only into the ConfigMap, never the Pod.
    assert!(!serde_json::to_string(&seeded.pod)
        .unwrap()
        .contains("hunter2"));

    // Removing cloudInit restores exactly the plain rendering.
    let mut removed = vm(Some(json!({"hostname":"seedtest"})));
    removed.spec.cloud_init = None;
    let again = resources::render(&removed, &settings()).unwrap();
    assert_eq!(again.hash, plain.hash);
    assert_eq!(again.config.data, plain.config.data);
}

#[test]
fn hostname_defaults_to_the_vm_name_and_user_data_is_optional() {
    let desired = resources::render(&vm(Some(json!({}))), &settings()).unwrap();
    assert_eq!(
        vm_json(&desired)["seed"],
        json!({"meta_data":"{\"instance-id\":\"0b7c4e6a-uid\",\"local-hostname\":\"test-vm\"}\n"})
    );
    // Hostnames that YAML would read as booleans or numbers stay strings.
    assert_eq!(
        resources::seed_meta_data("u", "no"),
        "{\"instance-id\":\"u\",\"local-hostname\":\"no\"}\n"
    );
}

#[test]
fn hostname_and_user_data_are_validated_without_echoing_user_data() {
    let invalid = |cloud_init: Value| match resources::render(&vm(Some(cloud_init)), &settings()) {
        Err(Error::Invalid(message)) => message,
        Err(other) => panic!("unexpected error {other}"),
        Ok(_) => panic!("accepted"),
    };
    for hostname in [
        "",
        "Upper",
        "-lead",
        "trail-",
        "dotted.name",
        "under_score",
        &"a".repeat(64),
    ] {
        assert!(invalid(json!({ "hostname": hostname })).contains("cloudInit.hostname"));
    }
    resources::render(&vm(Some(json!({"hostname":"a".repeat(63)}))), &settings()).unwrap();

    let limit = resources::USER_DATA_LIMIT;
    resources::render(
        &vm(Some(json!({"userData":"x".repeat(limit)}))),
        &settings(),
    )
    .unwrap();
    let message = invalid(json!({"userData":format!("secret-{}", "x".repeat(limit))}));
    assert!(message.contains("cloudInit.userData"));
    assert!(!message.contains("secret"));

    // Unknown fields are refused rather than ignored.
    assert!(serde_json::from_value::<CloudInit>(json!({"metaData":"x"})).is_err());
}

#[test]
fn crd_publishes_cloud_init_bounds() {
    let crd = qrab_operator::api::crd();
    let schema = &crd["spec"]["versions"][0]["schema"]["openAPIV3Schema"]["properties"]["spec"]
        ["properties"]["cloudInit"];
    assert_eq!(
        schema["properties"]["userData"]["maxLength"],
        json!(resources::USER_DATA_LIMIT)
    );
    assert_eq!(
        schema["properties"]["hostname"]["pattern"],
        "^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$"
    );
}

#[test]
fn network_config_matches_by_mac_and_routes_host_addresses_on_link() {
    let yaml = resources::seed_network_config(&[
        NicAddress {
            mac: "52:54:00:AB:CD:01".into(),
            address_cidr: "10.203.4.5/32".into(),
            gateway: Some("10.203.4.1".into()),
            nameservers: vec!["10.203.0.53".into(), "10.203.0.54".into()],
            dhcp: false,
        },
        NicAddress {
            mac: "52:54:00:ab:cd:02".into(),
            address_cidr: "192.168.10.7/24".into(),
            gateway: None,
            nameservers: vec![],
            dhcp: false,
        },
        NicAddress {
            mac: "52:54:00:ab:cd:03".into(),
            address_cidr: "fd00::5/128".into(),
            gateway: Some("fd00::1".into()),
            nameservers: vec![],
            dhcp: false,
        },
    ]);
    assert_eq!(
        yaml,
        r#"version: 2
ethernets:
  eth0:
    match:
      macaddress: "52:54:00:ab:cd:01"
    set-name: "eth0"
    addresses:
      - "10.203.4.5/32"
    routes:
      - to: "0.0.0.0/0"
        via: "10.203.4.1"
        on-link: true
    nameservers:
      addresses:
        - "10.203.0.53"
        - "10.203.0.54"
  eth1:
    match:
      macaddress: "52:54:00:ab:cd:02"
    set-name: "eth1"
    addresses:
      - "192.168.10.7/24"
  eth2:
    match:
      macaddress: "52:54:00:ab:cd:03"
    set-name: "eth2"
    addresses:
      - "fd00::5/128"
    routes:
      - to: "::/0"
        via: "fd00::1"
        on-link: true
"#
    );
    let routed = resources::seed_network_config(&[NicAddress {
        mac: "52:54:00:ab:cd:04".into(),
        address_cidr: "10.0.0.5/24".into(),
        gateway: Some("10.0.0.1".into()),
        nameservers: vec![],
        dhcp: false,
    }]);
    assert!(routed.contains("via: \"10.0.0.1\"\n") && !routed.contains("on-link"));
    assert_eq!(
        resources::seed_network_config(&[]),
        "version: 2\nethernets: {}\n"
    );
}
