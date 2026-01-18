use std::{collections::HashMap, ops::Deref, sync::Arc};

use anyhow::anyhow;
use clap::{Arg, ArgMatches, Command};
use netlink::{
	rtnetlink::{Address, IPAddress, Interface, InterfaceFlags, NetlinkRoute, RTNetlink, RTNetlinkGroups},
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

	let addr_add_command = Command::new("add")
		.about("Add an address to a given link")
		.arg(
			Arg::new("device")
				.help("the name of the link to set the state of")
				.short('d')
				.long("dev")
				.num_args(1)
				.required(true),
		)
		.arg(
			Arg::new("address")
				.help("the address to add")
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
		.subcommand(addr_add_command)
		.subcommand_required(true);

	let app = Command::new("netc")
		.about("Provides network information")
		.author("Colin Douch <colin@quirl.co.nz>")
		.subcommand(link_command)
		.subcommand(address_command)
		.subcommand_required(true)
		.get_matches();

	let netlink_socket = NetlinkSocket::<NetlinkRoute>::new(RTNetlinkGroups::RTMGRP_NONE).unwrap();
	match app.subcommand() {
		Some(("link", matches)) => match matches.subcommand() {
			Some(("show", _matches)) => show_links(netlink_socket.clone()),
			Some(("set", matches)) => set_link(netlink_socket, matches),
			_ => panic!("unknown links subcommand"),
		},
		Some(("addr", matches)) => match matches.subcommand() {
			Some(("show", _matches)) => show_addresses(netlink_socket),
			Some(("add", matches)) => add_address(netlink_socket, matches),
			_ => panic!("unknown addr subcommand"),
		},
		_ => panic!("unknown subcommand"),
	}
}

/// Returns the link with the given name, if it exists.
fn get_link_by_name(netlink_socket: &Arc<NetlinkSocket<NetlinkRoute>>, name: &str) -> Option<Interface> {
	netlink_socket
		.get_links()
		.unwrap()
		.into_iter()
		.find(|l| matches!(&l.attributes.name, Some(s) if s == name))
}

/// Sets the link state (up/down) for the given device.
fn set_link(netlink_socket: Arc<NetlinkSocket<NetlinkRoute>>, matches: &ArgMatches) {
	let link_name: &String = matches.get_one("device").expect("required device");
	let state: &String = matches.get_one("state").expect("required state");

	let mut link = match get_link_by_name(&netlink_socket, link_name) {
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

	if let Err(e) = netlink_socket.new_link(link) {
		println!("{:?}", e);
	}
}

/// Displays the currently active links.
fn show_links(netlink_socket: Arc<NetlinkSocket<NetlinkRoute>>) {
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

fn show_addresses(netlink_socket: Arc<NetlinkSocket<NetlinkRoute>>) {
	let mut table = tables::Table::new_with_headers(["Interface", "Address", "Broadcast", "Scope", "Proto", "Flags"])
		.with_setting(tables::TableSetting::ColumnSeperators)
		.with_setting(tables::TableSetting::HeaderSeperator);

	let links: HashMap<_, _> = netlink_socket
		.get_links()
		.unwrap()
		.into_iter()
		.map(|i| (i.index, i.attributes.name.to_owned().unwrap_or(format!("{}", i.index))))
		.collect();

	let mut addresses = netlink_socket.get_addrs().unwrap();
	addresses.sort_by_key(|a| a.interface_index);

	for addr in addresses {
		let interface = links
			.get(&addr.interface_index)
			.map(|s| s.as_str())
			.unwrap_or("<unknown>");
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

fn parse_scoped_address(raw_address: &str) -> anyhow::Result<(IPAddress, u8)> {
	let (raw_addr, raw_scope) = match raw_address.split_once("/") {
		Some(p) => p,
		None => {
			return Err(anyhow!("expected <address>/<scope>"));
		}
	};

	let addr = IPAddress::try_from(raw_addr).map_err(|s| anyhow!(s))?;
	let scope = raw_scope.parse()?;

	Ok((addr, scope))
}

fn add_address(netlink_socket: Arc<NetlinkSocket<NetlinkRoute>>, matches: &ArgMatches) {
	let raw_address: &String = matches.get_one("address").expect("required address");
	let (addr, scope) = match parse_scoped_address(raw_address) {
		Ok(i) => i,
		Err(e) => {
			println!("invalid IP address: {}: {}", raw_address, e);
			return;
		}
	};

	let link_name: &String = matches.get_one("device").expect("required device");
	let interface = match get_link_by_name(&netlink_socket, link_name) {
		Some(i) => i,
		None => {
			println!("no such interface: {}", link_name);
			return;
		}
	};

	let address = Address::new(addr, scope, interface.index);

	if let Err(e) = netlink_socket.new_address(address) {
		println!("failed to add address: {:?}", e);
	}
}
