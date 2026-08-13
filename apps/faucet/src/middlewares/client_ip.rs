use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::Response,
};
use faucet_config::ProxyConfig;
use real::RealIp;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub async fn insert_client_ip(
    State(proxy): State<ProxyConfig>,
    mut request: Request,
    next: Next,
) -> Response {
    let client_ip = client_ip(&request, &proxy);
    request.extensions_mut().insert(RealIp(client_ip));
    next.run(request).await
}

fn client_ip(request: &Request, proxy: &ProxyConfig) -> IpAddr {
    let Some(peer_ip) = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|connect_info| connect_info.0.ip())
    else {
        return Ipv4Addr::LOCALHOST.into();
    };

    if proxy.enabled
        && proxy.ips.iter().any(|network| network.contains(&peer_ip))
        && let Some(forwarded_ip) = request
            .headers()
            .get(proxy.header.as_str())
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse().ok())
    {
        return forwarded_ip;
    }

    peer_ip
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, extract::Request, http::HeaderName};

    use super::*;

    fn trusted_proxy(enabled: bool, header: &str) -> ProxyConfig {
        ProxyConfig {
            enabled,
            header: header.to_string(),
            ips: vec![IpAddr::V4(Ipv4Addr::new(172, 18, 0, 1)).into()],
        }
    }

    fn request_from(peer_ip: IpAddr, header: &str, forwarded_ip: &str) -> Request {
        let mut request = Request::builder()
            .header(
                HeaderName::from_bytes(header.as_bytes()).unwrap(),
                forwarded_ip,
            )
            .body(Body::empty())
            .unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::new(peer_ip, 12345)));
        request
    }

    #[test]
    fn ignores_forwarded_ip_when_proxy_headers_are_disabled() {
        let peer_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10));
        let request = request_from(peer_ip, "X-Real-IP", "198.51.100.20");
        let proxy = trusted_proxy(false, "X-Real-IP");

        assert_eq!(client_ip(&request, &proxy), peer_ip);
    }

    #[test]
    fn ignores_forwarded_ip_from_untrusted_public_peer() {
        let peer_ip = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 10));
        let request = request_from(peer_ip, "X-Real-IP", "198.51.100.20");
        let proxy = trusted_proxy(true, "X-Real-IP");

        assert_eq!(client_ip(&request, &proxy), peer_ip);
    }

    #[test]
    fn accepts_forwarded_ip_from_explicitly_trusted_proxy() {
        let peer_ip = IpAddr::V4(Ipv4Addr::new(172, 18, 0, 1));
        let request = request_from(peer_ip, "X-Real-IP", "198.51.100.20");
        let proxy = trusted_proxy(true, "X-Real-IP");

        assert_eq!(
            client_ip(&request, &proxy),
            IpAddr::V4(Ipv4Addr::new(198, 51, 100, 20))
        );
    }

    #[test]
    fn accepts_forwarded_ip_from_trusted_proxy_network() {
        let peer_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 100, 42));
        let request = request_from(peer_ip, "X-Real-IP", "198.51.100.20");
        let mut proxy = trusted_proxy(true, "X-Real-IP");
        proxy.ips = vec!["192.168.100.0/24".parse().unwrap()];

        assert_eq!(
            client_ip(&request, &proxy),
            IpAddr::V4(Ipv4Addr::new(198, 51, 100, 20))
        );
    }

    #[test]
    fn ignores_forwarded_ip_from_unlisted_private_peer() {
        let peer_ip = IpAddr::V4(Ipv4Addr::new(172, 18, 0, 2));
        let request = request_from(peer_ip, "X-Real-IP", "198.51.100.20");
        let proxy = trusted_proxy(true, "X-Real-IP");

        assert_eq!(client_ip(&request, &proxy), peer_ip);
    }

    #[test]
    fn accepts_ip_from_configured_proxy_header() {
        let peer_ip = IpAddr::V4(Ipv4Addr::new(172, 18, 0, 1));
        let request = request_from(peer_ip, "CF-Connecting-IP", "198.51.100.30");
        let proxy = trusted_proxy(true, "CF-Connecting-IP");

        assert_eq!(
            client_ip(&request, &proxy),
            IpAddr::V4(Ipv4Addr::new(198, 51, 100, 30))
        );
    }
}
