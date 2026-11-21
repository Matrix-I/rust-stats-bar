use statsbar_core::network::{RouteError, default_route_interface, is_interface_name};

const HEADER: &str =
    "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n";

/// `/proc/net/route` on this laptop on 2026-11-19.
fn this_laptop() -> String {
    format!(
        "{HEADER}wlo1\t00000000\t0164A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0\n\
         wlo1\t0064A8C0\t00000000\t0001\t0\t0\t600\t00FFFFFF\t0\t0\t0\n"
    )
}

fn default_route(interface: &str, flags: &str, metric: u32) -> String {
    format!("{interface}\t00000000\t0164A8C0\t{flags}\t0\t0\t{metric}\t00000000\t0\t0\t0\n")
}

fn interface(text: &str) -> Result<Option<String>, RouteError> {
    default_route_interface(text)
}

#[test]
fn finds_this_laptops_default_route() {
    assert_eq!(interface(&this_laptop()), Ok(Some("wlo1".to_owned())));
}

#[test]
fn the_lowest_metric_wins() {
    let text = format!(
        "{HEADER}{}{}",
        default_route("wlo1", "0003", 600),
        default_route("eno2", "0003", 100)
    );
    assert_eq!(interface(&text), Ok(Some("eno2".to_owned())));
}

#[test]
fn a_tie_goes_to_the_first_line() {
    let text = format!(
        "{HEADER}{}{}",
        default_route("eno2", "0003", 100),
        default_route("wlo1", "0003", 100)
    );
    assert_eq!(interface(&text), Ok(Some("eno2".to_owned())));
}

#[test]
fn a_route_that_is_not_up_is_ignored() {
    let text = format!(
        "{HEADER}{}{}",
        default_route("eno2", "0002", 100),
        default_route("wlo1", "0003", 600)
    );
    assert_eq!(interface(&text), Ok(Some("wlo1".to_owned())));
}

#[test]
fn no_default_route_or_an_empty_table_is_none() {
    let subnet_only =
        format!("{HEADER}wlo1\t0064A8C0\t00000000\t0001\t0\t0\t600\t00FFFFFF\t0\t0\t0\n");
    for text in ["", HEADER, subnet_only.as_str()] {
        assert_eq!(interface(text), Ok(None), "{text:?}");
    }
}

#[test]
fn a_malformed_line_is_an_error_naming_it() {
    let bad_hex = format!(
        "{HEADER}{}wlo1\tZZ\t0\t0003\t0\t0\t600\t00000000\n",
        default_route("eno2", "0003", 100)
    );
    assert_eq!(interface(&bad_hex), Err(RouteError { line: 3 }));
    let short = format!("{HEADER}wlo1\t00000000\t0\n");
    assert_eq!(interface(&short), Err(RouteError { line: 2 }));
    assert_eq!(
        RouteError { line: 2 }.to_string(),
        "proc/net/route line 2 is malformed"
    );
}

#[test]
fn an_interface_name_that_could_escape_sys_class_net_is_an_error() {
    let text = format!("{HEADER}{}", default_route("..", "0003", 100));
    assert_eq!(interface(&text), Err(RouteError { line: 2 }));
}

#[test]
fn interface_names_follow_the_kernels_rules() {
    for name in ["wlo1", "eno2", "wwp0s20f0u5i12", "a-b_c.d"] {
        assert!(is_interface_name(name), "{name}");
    }
    for name in [
        "",
        ".",
        "..",
        "a/b",
        "a:b",
        "a b",
        "sixteen-chars-xx",
        "wlö1",
    ] {
        assert!(!is_interface_name(name), "{name}");
    }
}
