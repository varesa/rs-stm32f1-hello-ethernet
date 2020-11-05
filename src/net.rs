use cortex_m::{iprintln};

use enc28j60::{Enc28j60};
use heapless::{FnvIndexMap, ArrayLength, Bucket, PowerOfTwo};
use jnet::{arp, /*ether,*/ icmp, ipv4, mac, udp, Buffer};

/* Configuration */
//const MAC: mac::Addr = mac::Addr([0x20, 0x18, 0x72, 0x75, 0x73, 0x74]);
const MAC: mac::Addr = mac::Addr([0x48, 0x69, 0x52, 0x75, 0x73, 0x74]);
const IP: ipv4::Addr = ipv4::Addr([192, 168, 1, 33]);

/* Constants */
const MTU: usize = 1518;

pub fn handle_ipv4(
    _stim: &mut cortex_m::peripheral::itm::Stim,
    enc28j60: &mut Enc28j60<
        impl embedded_hal::blocking::spi::Transfer<u8, Error = stm32f1xx_hal::spi::Error> + embedded_hal::blocking::spi::Write<u8, Error = stm32f1xx_hal::spi::Error>,
        impl embedded_hal::digital::v2::OutputPin,
        impl enc28j60::IntPin,
        impl enc28j60::ResetPin,
    >,
    eth: &mut jnet::ether::Frame<Buffer<&mut [u8; MTU]>>,
    arp_cache: &mut FnvIndexMap<
        jnet::ipv4::Addr,
        jnet::mac::Addr,
        impl ArrayLength<Bucket<jnet::ipv4::Addr, jnet::mac::Addr>> + ArrayLength<Option<heapless::Pos>> + PowerOfTwo,
    >,
) {
    let src_mac = eth.get_source();

    if let Ok(mut ip) = ipv4::Packet::parse(eth.payload_mut()) {
        iprintln!(_stim, "** {:?}", ip);

        let src_ip = ip.get_source();

        if !src_mac.is_broadcast() {
            arp_cache.insert(src_ip, src_mac).ok();
        }

        match ip.get_protocol() {
            ipv4::Protocol::Icmp => {
                if let Ok(icmp) = icmp::Packet::parse(ip.payload_mut()) {
                    match icmp.downcast::<icmp::EchoRequest>() {
                        Ok(request) => {
                            // is an echo request
                            iprintln!(_stim, "*** {:?}", request);

                            let src_mac = arp_cache
                                .get(&src_ip)
                                .unwrap_or_else(|| unimplemented!());

                            let _reply: icmp::Packet<_, icmp::EchoReply, _> =
                                request.into();
                            iprintln!(_stim, "\n*** {:?}", _reply);

                            // update the IP header
                            let mut ip = ip.set_source(IP);
                            ip.set_destination(src_ip);
                            let _ip = ip.update_checksum();
                            iprintln!(_stim, "** {:?}", _ip);

                            // update the Ethernet header
                            eth.set_destination(*src_mac);
                            eth.set_source(MAC);
                            iprintln!(_stim, "* {:?}", eth);

                            //led.toggle().unwrap();
                            iprintln!(_stim, "Tx({})", eth.as_bytes().len());
                            enc28j60.transmit(eth.as_bytes()).ok().unwrap();
                        }
                        Err(_icmp) => {
                            iprintln!(_stim, "*** {:?}", _icmp);
                        }
                    }
                } else {
                    // Malformed ICMP packet
                    iprintln!(_stim, "Err(B)");
                }
            }
            ipv4::Protocol::Udp => {
                if let Ok(mut udp) = udp::Packet::parse(ip.payload_mut()) {
                    iprintln!(_stim, "*** {:?}", udp);

                    if let Some(src_mac) = arp_cache.get(&src_ip) {
                        let src_port = udp.get_source();
                        let dst_port = udp.get_destination();

                        // update the UDP header
                        udp.set_source(dst_port);
                        udp.set_destination(src_port);
                        udp.zero_checksum();
                        iprintln!(_stim, "\n*** {:?}", udp);

                        // update the IP header
                        let mut ip = ip.set_source(IP);
                        ip.set_destination(src_ip);
                        let ip = ip.update_checksum();
                        let ip_len = ip.len();
                        iprintln!(_stim, "** {:?}", ip);

                        // update the Ethernet header
                        eth.set_destination(*src_mac);
                        eth.set_source(MAC);
                        eth.truncate(ip_len);
                        iprintln!(_stim, "* {:?}", eth);

                        //led.toggle().unwrap();
                        iprintln!(_stim, "Tx({})", eth.as_bytes().len());
                        enc28j60.transmit(eth.as_bytes()).ok().unwrap();
                    }
                } else {
                    // malformed UDP packet
                    iprintln!(_stim, "Err(C)");
                }
            }
            _ => {}
        }
    } else {
        // malformed IPv4 packet
        iprintln!(_stim, "Err(D)");
    }

}

pub fn handle_arp(
    _stim: &mut cortex_m::peripheral::itm::Stim,
    enc28j60: &mut Enc28j60<
        impl embedded_hal::blocking::spi::Transfer<u8, Error = stm32f1xx_hal::spi::Error> + embedded_hal::blocking::spi::Write<u8, Error = stm32f1xx_hal::spi::Error>,
        impl embedded_hal::digital::v2::OutputPin,
        impl enc28j60::IntPin,
        impl enc28j60::ResetPin,
    >,
    eth: &mut jnet::ether::Frame<Buffer<&mut [u8; MTU]>>,
    arp_cache: &mut FnvIndexMap<
        jnet::ipv4::Addr,
        jnet::mac::Addr,
        impl ArrayLength<Bucket<jnet::ipv4::Addr, jnet::mac::Addr>> + ArrayLength<Option<heapless::Pos>> + PowerOfTwo,
    >,
) {
    if let Ok(arp) = arp::Packet::parse(eth.payload_mut()) {
        match arp.downcast() {
            Ok(mut arp) => {
                iprintln!(_stim, "** {:?}", arp);

                if !arp.is_a_probe() {
                    arp_cache.insert(arp.get_spa(), arp.get_sha()).ok();
                }

                // are they asking for us?
                if arp.get_oper() == arp::Operation::Request && arp.get_tpa() == IP
                {
                    // reply to the ARP request
                    let tha = arp.get_sha();
                    let tpa = arp.get_spa();

                    arp.set_oper(arp::Operation::Reply);
                    arp.set_sha(MAC);
                    arp.set_spa(IP);
                    arp.set_tha(tha);
                    arp.set_tpa(tpa);
                    iprintln!(_stim, "\n** {:?}", arp);
                    let arp_len = arp.len();

                    // update the Ethernet header
                    eth.set_destination(tha);
                    eth.set_source(MAC);
                    eth.truncate(arp_len);
                    iprintln!(_stim, "* {:?}", eth);

                    iprintln!(_stim, "Tx({})", eth.as_bytes().len());
                    enc28j60.transmit(eth.as_bytes()).ok().unwrap();
                }
            }
            Err(_arp) => {
                // Not a Ethernet/IPv4 ARP packet
                iprintln!(_stim, "** {:?}", _arp);
            }
        }
    } else {
        // malformed ARP packet
        iprintln!(_stim, "Err(A)");
    }
}