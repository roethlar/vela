#![allow(dead_code)] // Provider endpoint candidates consume this in Slice 2.

use serde::Serialize;
use std::net::{IpAddr, SocketAddr};

/// Locality order is product behavior: smaller values are preferred by
/// Prefer Fastest Source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum EndpointLocality {
    SameMachine,
    Lan,
    Internet,
}

fn is_lan_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => {
            address.is_private() || address.is_link_local() || address.is_broadcast()
        }
        IpAddr::V6(address) => {
            let first = address.octets()[0];
            address.is_unicast_link_local() || first & 0xfe == 0xfc
        }
    }
}

/// Pure address classification. DNS and interface enumeration are kept outside
/// so tests do not depend on the machine running them.
pub(crate) fn classify_addresses(
    resolved: &[IpAddr],
    interface_addresses: &[IpAddr],
    provider_verified_local: bool,
) -> EndpointLocality {
    if resolved.iter().any(|address| {
        address.is_loopback() || interface_addresses.iter().any(|local| local == address)
    }) {
        return EndpointLocality::SameMachine;
    }
    if provider_verified_local || resolved.iter().copied().any(is_lan_address) {
        return EndpointLocality::Lan;
    }
    EndpointLocality::Internet
}

/// Resolve one configured endpoint at play time. Interface enumeration is
/// blocking and therefore runs away from Tokio workers. DNS failures and empty
/// answers conservatively remain Internet unless the provider itself verified
/// the connection as local.
pub(crate) async fn classify_endpoint(
    endpoint: &url::Url,
    provider_verified_local: bool,
) -> EndpointLocality {
    let interface_addresses = tauri::async_runtime::spawn_blocking(|| {
        if_addrs::get_if_addrs()
            .map(|interfaces| {
                interfaces
                    .into_iter()
                    .map(|interface| interface.ip())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    let Some(host) = endpoint.host_str() else {
        return classify_addresses(&[], &interface_addresses, provider_verified_local);
    };
    let port = endpoint.port_or_known_default().unwrap_or(80);
    let resolved: Vec<IpAddr> = tokio::net::lookup_host((host, port))
        .await
        .map(|answers| answers.map(|answer: SocketAddr| answer.ip()).collect())
        .unwrap_or_default();
    classify_addresses(&resolved, &interface_addresses, provider_verified_local)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn loopback_and_assigned_addresses_are_same_machine() {
        let assigned = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 22));
        assert_eq!(
            classify_addresses(&[IpAddr::V4(Ipv4Addr::LOCALHOST)], &[], false),
            EndpointLocality::SameMachine
        );
        assert_eq!(
            classify_addresses(&[assigned], &[assigned], false),
            EndpointLocality::SameMachine
        );
    }

    #[test]
    fn private_link_local_and_ula_addresses_are_lan() {
        for address in [
            IpAddr::V4(Ipv4Addr::new(10, 20, 30, 40)),
            IpAddr::V4(Ipv4Addr::new(169, 254, 2, 3)),
            IpAddr::V6("fd12::1".parse::<Ipv6Addr>().unwrap()),
            IpAddr::V6("fe80::1".parse::<Ipv6Addr>().unwrap()),
        ] {
            assert_eq!(
                classify_addresses(&[address], &[], false),
                EndpointLocality::Lan
            );
        }
    }

    #[test]
    fn verified_local_overrides_public_or_unknown_dns_but_not_to_same_machine() {
        assert_eq!(classify_addresses(&[], &[], true), EndpointLocality::Lan);
        assert_eq!(
            classify_addresses(&[IpAddr::V4(Ipv4Addr::new(203, 0, 113, 4))], &[], true),
            EndpointLocality::Lan
        );
    }

    #[test]
    fn public_and_unknown_addresses_are_internet() {
        assert_eq!(
            classify_addresses(&[], &[], false),
            EndpointLocality::Internet
        );
        assert_eq!(
            classify_addresses(&[IpAddr::V4(Ipv4Addr::new(203, 0, 113, 4))], &[], false),
            EndpointLocality::Internet
        );
    }
}
