use anyhow::Result;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use std::time::Duration;

const SERVICE_TYPE: &str = "_clipchamp._tcp.local.";

pub struct MdnsGuard {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Drop for MdnsGuard {
    fn drop(&mut self) {
        let _ = self.daemon.unregister(&self.fullname);
        let _ = self.daemon.shutdown();
    }
}

pub fn advertise(port: u16) -> Result<MdnsGuard> {
    let daemon = ServiceDaemon::new()?;

    let hostname = hostname::get()
        .unwrap_or_else(|_| "clipchamp".into())
        .to_string_lossy()
        .to_string();

    let instance_name = format!("clipchamp-{hostname}");

    let service = ServiceInfo::new(SERVICE_TYPE, &instance_name, &format!("{hostname}.local."), "", port, None)?;

    let fullname = service.get_fullname().to_string();
    daemon.register(service)?;

    Ok(MdnsGuard { daemon, fullname })
}

pub async fn discover() -> Result<(String, u16)> {
    let daemon = ServiceDaemon::new()?;
    let receiver = daemon.browse(SERVICE_TYPE)?;

    let timeout = Duration::from_secs(10);
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            daemon.shutdown()?;
            anyhow::bail!("mDNS discovery timed out after {timeout:?}");
        }

        let recv = receiver.clone();
        let event = tokio::task::spawn_blocking(move || recv.recv_timeout(remaining)).await?;

        match event {
            Ok(ServiceEvent::ServiceResolved(info)) => {
                let addrs = info.get_addresses();
                if let Some(addr) = addrs.iter().next() {
                    let port = info.get_port();
                    daemon.shutdown()?;
                    return Ok((addr.to_string(), port));
                }
            }
            Ok(_) => continue,
            Err(e) => {
                daemon.shutdown()?;
                anyhow::bail!("mDNS discovery error: {e}");
            }
        }
    }
}
