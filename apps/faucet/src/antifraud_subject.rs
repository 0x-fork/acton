use std::net::{IpAddr, Ipv6Addr};

pub(crate) fn wallet(address: &str) -> String {
    format!("wallet:{address}")
}

pub(crate) fn github(github_user_id: u64) -> String {
    format!("github:{github_user_id}")
}

pub(crate) fn client_ip(ip: IpAddr) -> String {
    let ip = match ip {
        IpAddr::V6(ip) if ip.to_ipv4_mapped().is_some() => {
            IpAddr::V4(ip.to_ipv4_mapped().expect("checked IPv4-mapped address"))
        }
        IpAddr::V6(ip) => {
            let network = u128::from(ip) & (u128::MAX << 64);
            IpAddr::V6(Ipv6Addr::from(network))
        }
        IpAddr::V4(ip) => IpAddr::V4(ip),
    };
    format!("client-ip:{ip}")
}

pub(crate) fn device_uid(device_uid: &str) -> String {
    format!("device-uid:{}", device_uid.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use super::{client_ip, device_uid, wallet};

    #[test]
    fn builds_wallet_subject() {
        assert_eq!(wallet("0:abc"), "wallet:0:abc");
    }

    #[test]
    fn builds_client_subjects_from_peer_ip() {
        assert_eq!(
            client_ip(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))),
            "client-ip:203.0.113.7"
        );
        assert_eq!(
            client_ip(IpAddr::V6(
                "2001:db8:1234:5678:abcd::1".parse::<Ipv6Addr>().unwrap()
            )),
            "client-ip:2001:db8:1234:5678::"
        );
        assert_eq!(
            client_ip(IpAddr::V6("::ffff:192.0.2.44".parse::<Ipv6Addr>().unwrap())),
            "client-ip:192.0.2.44"
        );
    }

    #[test]
    fn builds_case_normalized_device_subject() {
        assert_eq!(
            device_uid("550E8400-E29B-41D4-A716-446655440000"),
            "device-uid:550e8400-e29b-41d4-a716-446655440000"
        );
    }
}
