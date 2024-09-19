use std::{io::stdout, sync::Arc};

use clap::Command;
use common::obs::assemble_logger;
use netlink::{
	rtnetlink::{Interface, InterfaceFlags, NetlinkRoute, RTNetlink, RTNetlinkGroups},
	NetlinkSocket,
};
use slog::{debug, error};

fn main() {
	let _matches = Command::new("netd")
		.about("Manages network connections")
		.author("Colin Douch <colin@quirl.co.nz>")
		.get_matches();

	let logger = assemble_logger(stdout());
	let netlink_socket = NetlinkSocket::<NetlinkRoute>::new(RTNetlinkGroups::RTMGRP_LINK).unwrap();
	let global_flow = netlink_socket.global_flow().unwrap();

	let mut handled_links = Vec::new();
	match netlink_socket.get_links() {
		Ok(links) => {
			for link in links {
				handled_links.push(link.index);
				handle_new_link(&logger, netlink_socket.clone(), link);
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
			handle_new_link(&logger, netlink_socket.clone(), link);
		}
	}
}

fn handle_new_link(logger: &slog::Logger, netlink_socket: Arc<NetlinkSocket<NetlinkRoute>>, mut link: Interface) {
	debug!(logger, "handling new link"; "link" => format!("{:?}", link));
	link.flags |= InterfaceFlags::IFF_UP;
	if let Err(e) = netlink_socket.new_link(link) {
		error!(logger, "failed to bring link up"; "error" => format!("{:?}", e));
	}
}
