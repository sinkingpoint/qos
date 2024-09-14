use std::ops::Deref;

use clap::{Arg, ArgMatches, Command};
use netlink::{
	rtnetlink::{Interface, InterfaceFlags, NetlinkRoute, RTNetlink},
	NetlinkSocket,
};

fn main() {
	let link_set_command = Command::new("set")
		.about("set the state of a link")
		.arg(
			Arg::new("device")
				.help("the name of the link to set the state of")
				.short('d')
				.long("dev")
				.num_args(1)
				.required(true),
		)
		.arg(
			Arg::new("state")
				.help("the state to set the link to (up or down)")
				.num_args(1)
				.required(true),
		);

	let link_command = Command::new("link")
		.about("manage network links")
		.subcommand(Command::new("show").about("show the currently active links"))
		.subcommand(link_set_command)
		.subcommand_required(true);

	let address_command = Command::new("addr")
		.about("manage network addresses")
		.subcommand(Command::new("show").about("show the currently active addresses"))
		.subcommand_required(true);

	let app = Command::new("netc")
		.about("Provides network information")
		.author("Colin Douch <colin@quirl.co.nz>")
		.subcommand(link_command)
		.subcommand(address_command)
		.subcommand_required(true)
		.get_matches();

	let mut netlink_socket = NetlinkSocket::<NetlinkRoute>::new(0).unwrap();
	match app.subcommand() {
		Some(("link", matches)) => match matches.subcommand() {
			Some(("show", _matches)) => show_links(&mut netlink_socket),
			Some(("set", matches)) => set_link(&mut netlink_socket, matches),
			_ => panic!("unknown links subcommand"),
		},
		Some(("addr", matches)) => match matches.subcommand() {
			Some(("show", _matches)) => show_addresses(&mut netlink_socket),
			_ => panic!("unknown addr subcommand"),
		},
		_ => panic!("unknown subcommand"),
	}
}

/// Returns the link with the given name, if it exists.
fn get_link_by_name(netlink_socket: &mut NetlinkSocket<NetlinkRoute>, name: &str) -> Option<Interface> {
	netlink_socket
		.get_links()
		.unwrap()
		.into_iter()
		.find(|l| matches!(&l.attributes.name, Some(s) if s == name))
}

fn set_link(netlink_socket: &mut NetlinkSocket<NetlinkRoute>, matches: &ArgMatches) {
	let link_name: &String = match matches.get_one("device") {
		Some(l) => l,
		None => panic!("BUG: missing links"),
	};

	let state: &String = match matches.get_one("state") {
		Some(l) => l,
		None => panic!("BUG: missing links"),
	};

	let mut link = match get_link_by_name(netlink_socket, link_name) {
		Some(l) => l,
		None => {
			eprintln!("no such device: {}", link_name);
			return;
		}
	};

	match state.deref() {
		"up" => link.flags |= InterfaceFlags::IFF_UP,
		"down" => link.flags &= !InterfaceFlags::IFF_UP,
		s => {
			eprintln!("invalid operational state: `{}`", s);
			return;
		}
	};

	let err = netlink_socket.new_link(link);
	println!("{:?}", err);
}

fn show_links(netlink_socket: &mut NetlinkSocket<NetlinkRoute>) {
	let mut table = tables::Table::new_with_headers(["Index", "Name", "Flags", "State", "MTU", "QDisc"])
		.with_setting(tables::TableSetting::ColumnSeperators)
		.with_setting(tables::TableSetting::HeaderSeperator);

	let links = netlink_socket.get_links().unwrap();
	for i in links {
		let index = &format!("{}", i.index);
		let name = i.attributes.name.as_deref().unwrap_or("<unknown>");
		let flags = &format!("{}", i.flags);
		let mtu = &format!("{}", i.attributes.mtu.unwrap_or(0));
		let qdisc = i.attributes.qdisc.as_deref().unwrap_or("<unknown>");
		let state = i.attributes.operational_state.as_ref().map(ToString::to_string);
		table.add_row([index, name, flags, state.as_deref().unwrap_or("<unknown>"), mtu, qdisc])
	}

	print!("{}", table);
}

fn show_addresses(netlink_socket: &mut NetlinkSocket<NetlinkRoute>) {
	let mut table = tables::Table::new_with_headers(["Interface", "Address", "Broadcast", "Scope", "Proto", "Flags"])
		.with_setting(tables::TableSetting::ColumnSeperators)
		.with_setting(tables::TableSetting::HeaderSeperator);

	let addresses = netlink_socket.get_addrs(1).unwrap();
	for addr in addresses {
		let interface = &format!("{}", addr.interface_index);
		let address = &format!(
			"{}/{}",
			addr.attributes.address.expect("ip address"),
			addr.prefix_length
		);

		let broadcast = if let Some(addr) = addr.attributes.broadcast_address {
			&format!("{}", addr)
		} else {
			"<None>"
		};

		let scope = &format!("{:?}", addr.scope);
		let proto = if let Some(proto) = addr.attributes.protocol {
			&format!("{:?}", proto)
		} else {
			"<None>"
		};

		let flags = &format!("{}", addr.flags);

		table.add_row([interface, address, broadcast, scope, proto, flags]);
	}

	println!("{}", table);
}
