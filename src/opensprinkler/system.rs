use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs, UdpSocket};

use zbus::{dbus_proxy, Connection};

#[dbus_proxy(interface = "org.freedesktop.login1.Manager", gen_blocking = false, default_service = "org.freedesktop.login1", default_path = "/org/freedesktop/login1")]
trait Manager {
    /// [📖](https://www.freedesktop.org/software/systemd/man/systemd.directives.html#Reboot()) Call interface method `Reboot`.
    #[dbus_proxy(name = "Reboot")]
    fn reboot(&self, interactive: bool) -> zbus::Result<()>;
}

pub async fn reboot() -> Result<(), Box<dyn std::error::Error>> {
    let connection = Connection::session().await?;

    let proxy = ManagerProxy::new(&connection).await?;
    let reply = proxy.reboot(false).await?;
    dbg!(reply);

    Ok(())
}

pub enum Error {
    NotFound,
    OSError(nix::errno::Errno),
}

impl From<nix::errno::Errno> for Error {
    fn from(err: nix::errno::Errno) -> Self {
        Self::OSError(err)
    }
}

pub fn is_interface_online(interface_name: &str) -> Result<bool, Error> {
    let mut addrs = nix::ifaddrs::getifaddrs()?;
    if let Some(ifaddr) = addrs.find(|a| a.interface_name == interface_name) {
        return Ok(ifaddr.flags.contains(nix::net::if_::InterfaceFlags::IFF_RUNNING));
    }
    Err(Error::NotFound)
}

/// Get the uptime of the system
///
/// Will return [None] if the uptime could not be obtained.
pub fn get_system_uptime() -> Option<std::time::Duration> {
    if let Ok(timespec) = nix::time::clock_gettime(nix::time::ClockId::CLOCK_MONOTONIC) {
        return Some(std::time::Duration::from(timespec));
    }

    None
}

pub fn get_external_ip() -> Result<Option<IpAddr>, stunclient::Error> {
    if let Some(stun_server) = "openrelay.metered.ca:80".to_socket_addrs()?.filter(|x| x.is_ipv4()).next() {
        let udp = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 0))?;
        let client = stunclient::StunClient::new(stun_server);
        return Ok(Some(client.query_external_address(&udp)?.ip()));
    }

    Ok(None)
}
