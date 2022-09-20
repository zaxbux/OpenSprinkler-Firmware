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
