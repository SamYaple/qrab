use anyhow::Context;
use qrab_kvm_device_plugin::{run, Options};

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).is_some() {
        anyhow::bail!("usage: qrab-kvm-device-plugin (configure with QRAB_KVM_DEVICES)");
    }
    let mut options = Options::default();
    if let Ok(count) = std::env::var("QRAB_KVM_DEVICES") {
        options.devices = count
            .parse()
            .ok()
            .filter(|n| (1..=1024).contains(n))
            .context("QRAB_KVM_DEVICES must be 1..1024")?;
    }
    let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    run(options, async {
        tokio::select! { _ = term.recv() => {}, _ = tokio::signal::ctrl_c() => {} }
    })
    .await
}
