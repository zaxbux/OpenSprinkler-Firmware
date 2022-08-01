use std::error::Error;

use zbus::{Connection, dbus_proxy};

#[dbus_proxy(
	interface = "org.freedesktop.login1.Manager",
	gen_blocking = false,
	default_service = "org.freedesktop.login1",
	default_path = "/org/freedesktop/login1"
)]
trait Manager {
	/// [📖](https://www.freedesktop.org/software/systemd/man/systemd.directives.html#Reboot()) Call interface method `Reboot`.
	#[dbus_proxy(name = "Reboot")]
	fn reboot(&self, interactive: bool) -> zbus::Result<()>;
}

pub async fn reboot() -> Result<(), Box<dyn Error>> {
	let connection = Connection::session().await?;
	
	let proxy = ManagerProxy::new(&connection).await?;
	let reply = proxy.reboot(false).await?;
	dbg!(reply);

	Ok(())
}

pub fn is_interface_online(interface_name: String) -> bool {
	let addrs = nix::ifaddrs::getifaddrs()?;
	let ifaddr = addrs.find(|&a| a.interface_name == interface_name);
	ifaddr.flags.contains(nix::net::if_::InterfaceFlags::IFF_RUNNING)
}