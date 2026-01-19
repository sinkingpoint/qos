mod config;
mod dhcp;

use std::{io::stderr, sync::Arc, thread};

use crate::dhcp::DHCPClient;
use clap::{Arg, Command};
use common::obs::assemble_logger;
use config::Config;
use netlink::{
	rtnetlink::{Address, Interface, InterfaceFlags, NetlinkRoute, RTNetlink, RTNetlinkGroups},
	NetlinkSocket,
};
use slog::{error, info};

fn main() {
	let matches = Command::new("netd")
		.about("Manages network connections")
		.author("Colin Douch <colin@quirl.co.nz>")
		.arg(
			Arg::new("config")
				.help("path to the config file")
				.default_value("/etc/netd/config.toml"),
		)
		.get_matches();

	let logger = assemble_logger(stderr());
	let config_file_path: &String = matches.get_one("config").expect("config file default");
	let config = match Config::read_from(config_file_path) {
		Ok(c) => c,
		Err(e) => {
			error!(logger, "failed to read config file"; "path" => config_file_path, "error" => e.to_string());
			return;
		}
	};

	let netlink_socket = NetlinkSocket::<NetlinkRoute>::new(RTNetlinkGroups::RTMGRP_LINK).unwrap();
	let global_flow = netlink_socket.global_flow().unwrap();

	let mut handled_links = Vec::new();
	match netlink_socket.get_links() {
		Ok(links) => {
			for link in links {
				handled_links.push(link.index);
				handle_new_link(&logger, &config, netlink_socket.clone(), link);
			}
		}
		Err(e) => {
			error!(logger, "failed to get links"; "error" => format!("{:?}", e));
		}
	}

	while let Some((_, body)) = global_flow.read() {
		let link = match Interface::parse(body) {
			Ok(int) => int,
			Err(e) => {
				error!(logger, "failed to parse interface from body"; "error" => e.to_string());
				continue;
			}
		};

		if !handled_links.contains(&link.index) {
			handled_links.push(link.index);
			handle_new_link(&logger, &config, netlink_socket.clone(), link);
		}
	}
}

fn handle_new_link(
	logger: &slog::Logger,
	config: &Config,
	netlink_socket: Arc<NetlinkSocket<NetlinkRoute>>,
	mut link: Interface,
) {
	let config = config.get_actions_for_interface(&link);
	let mut enable_dhcp = false;

	link.flags |= InterfaceFlags::IFF_UP;
	if let Some(config) = config {
		if let Some(name) = &config.rename_to {
			link.attributes.name = Some(name.clone());
		}

		if let Some(addrs) = &config.add_ips {
			let addrs = addrs
				.iter()
				.map(|(addr, scope)| (addr, scope, Address::new(addr.clone(), *scope, link.index)));

			for (raw_addr, scope, addr) in addrs {
				if let Err(e) = netlink_socket.new_address(addr) {
					error!(logger, "failed to add address to link"; "address" => format!("{}/{}", &raw_addr, scope), "error" => format!("{:?}", e));
				}
			}
		}

		enable_dhcp = config.enable_dhcp.unwrap_or(false);
	}

	if let Err(e) = netlink_socket.new_link(link.clone()) {
		error!(logger, "failed to bring link up"; "error" => format!("{:?}", e));
	}

	if enable_dhcp {
		match DHCPClient::new(logger.clone(), link.clone()) {
			Ok(d) => {
				info!(logger, "started DHCP client for link"; "name" => format!("{}", link.attributes.name.as_ref().expect("interface name")));
				thread::spawn(move || d.run());
			}
			Err(e) => {
				error!(logger, "failed to enable dhcp on link"; "error" => format!("{:?}", e));
			}
		};
	}
}
