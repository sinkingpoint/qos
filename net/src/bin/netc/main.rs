use netlink::{
	rtnetlink::{NetlinkRoute, RTNetlink},
	NetlinkSocket,
};

#[tokio::main]
async fn main() {
	let mut netlink_socket = NetlinkSocket::<NetlinkRoute>::new(0).unwrap();
	println!("{:?}", netlink_socket.get_links());
}
