#!/bin/bash

BRIDGE_NAME=qemubr0
NETWORK=10.0.128.0/24
GATEWAY=10.0.128.1
DHCPRANGE=10.0.128.2,10.0.128.254

up(){
  ip link add name "${BRIDGE_NAME}" type bridge
  ip addr add "${GATEWAY}/24" dev "${BRIDGE_NAME}"
  ip link set "${BRIDGE_NAME}" up

  ip tuntap add dev qemutap0 mode tap user "${USER}"
  ip link set qemutap0 up promisc on
  ip link set qemutap0 master qemubr0

  sysctl net.ipv4.ip_forward=1
  sysctl net.ipv6.conf.default.forwarding=1
  sysctl net.ipv6.conf.all.forwarding=1

  sysctl net.bridge.bridge-nf-call-ip6tables=0
  sysctl net.bridge.bridge-nf-call-iptables=0
  sysctl net.bridge.bridge-nf-call-arptables=0

  iptables -A FORWARD -i qemubr0 -o wifi0 -j ACCEPT
  iptables -A FORWARD -i wifi0 -o qemubr0 -m state --state RELATED,ESTABLISHED -j ACCEPT 
  iptables -t nat -A POSTROUTING -o wifi0 -j MASQUERADE

  start_dnsmasq
}

down(){
  ip link del qemutap0
  ip link del qemubr0

  ps aux | grep 'dnsmasq --strict-order --except-interface=lo --interface=qemubr0' | grep -v grep | awk '{print $2}' | xargs kill
}

start_dnsmasq() {
  dnsmasq \
    --strict-order \
    --except-interface=lo \
    --interface=$BRIDGE_NAME \
    --listen-address=$GATEWAY \
    --bind-interfaces \
    --dhcp-range=$DHCPRANGE \
    --pid-file=/var/run/qemu-dnsmasq-$BRIDGE_NAME.pid \
    --dhcp-leasefile=/var/run/qemu-dnsmasq-$BRIDGE_NAME.leases \
    --dhcp-no-override \
    --log-dhcp \
    --log-debug \
    --log-facility=/var/log/dnsmasq.log
}

case "${1}" in
  up) up
  ;;

  down) down
  ;;

  *) echo "Unknown command: $1" >&2
esac