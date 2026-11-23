//! The wire format shared with `statsbar-panel`. It lives here because `statsbar-core` may not
//! take `serde_json`, not even for its tests.

use statsbar_core::cpu::CpuLoad;
use statsbar_core::ipc::{
    CpuPage, CpuProcess, ErrorReason, Event, Hello, MemoryPage, MemoryProcess, PROTOCOL_VERSION,
    Page, ProcessInfo, Request,
};
use statsbar_core::memory::{MemoryBuckets, Pressure};
use statsbar_core::system::Thermal;

fn firefox() -> ProcessInfo {
    ProcessInfo {
        pid: 4242,
        start_ticks: 9001,
        name: "firefox".to_owned(),
        can_quit: true,
    }
}

fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("serialises")
}

#[test]
fn the_first_line_names_protocol_1() {
    assert_eq!(PROTOCOL_VERSION, 1);
    assert_eq!(json(&Hello::current()), r#"{"protocol":1}"#);
}

#[test]
fn requests_have_these_exact_lines() {
    for (request, line) in [
        (
            Request::Subscribe { page: Page::Cpu },
            r#"{"type":"subscribe","page":"cpu"}"#,
        ),
        (
            Request::Subscribe { page: Page::Memory },
            r#"{"type":"subscribe","page":"memory"}"#,
        ),
        (Request::Unsubscribe, r#"{"type":"unsubscribe"}"#),
        (Request::Refresh, r#"{"type":"refresh"}"#),
    ] {
        assert_eq!(json(&request), line);
        assert_eq!(
            serde_json::from_str::<Request>(line).expect("parses"),
            request
        );
    }
}

#[test]
fn a_cpu_event_has_this_exact_line() {
    let event = Event::Cpu(CpuPage {
        model: Some("i5".to_owned()),
        load: Some(CpuLoad {
            system_percent: 10.0,
            user_percent: 20.5,
            idle_percent: 69.5,
            per_core_busy: vec![30.5],
        }),
        temperature_c: Some(84.0),
        thermal: Some(Thermal::Fair),
        core_temperatures_c: vec![None],
        frequency_mhz: Some(1900.0),
        load_per_cpu: Some([1.0, 0.5, 0.25]),
        uptime_seconds: Some(3600.0),
        power_saver: false,
        processes: Some(vec![CpuProcess {
            process: firefox(),
            cpu_percent: 12.5,
        }]),
    });
    let line = concat!(
        r#"{"type":"cpu","model":"i5","load":{"system_percent":10.0,"user_percent":20.5,"#,
        r#""idle_percent":69.5,"per_core_busy":[30.5]},"temperature_c":84.0,"thermal":"fair","#,
        r#""core_temperatures_c":[null],"frequency_mhz":1900.0,"load_per_cpu":[1.0,0.5,0.25],"#,
        r#""uptime_seconds":3600.0,"power_saver":false,"processes":[{"process":{"pid":4242,"#,
        r#""start_ticks":9001,"name":"firefox","can_quit":true},"cpu_percent":12.5}]}"#,
    );
    assert_eq!(json(&event), line);
    assert_eq!(serde_json::from_str::<Event>(line).expect("parses"), event);
}

#[test]
fn a_memory_event_has_this_exact_line() {
    let event = Event::Memory(MemoryPage {
        buckets: MemoryBuckets {
            total: 1000,
            app: 400,
            kernel: 150,
            cached: 300,
            free: 150,
            swap_used: 7,
            compressed: 100,
        },
        pressure: Some(Pressure::Warning),
        processes: Some(vec![MemoryProcess {
            process: firefox(),
            bytes: 4096,
        }]),
    });
    let line = concat!(
        r#"{"type":"memory","buckets":{"total":1000,"app":400,"kernel":150,"cached":300,"#,
        r#""free":150,"swap_used":7,"compressed":100},"pressure":"warning","processes":["#,
        r#"{"process":{"pid":4242,"start_ticks":9001,"name":"firefox","can_quit":true},"#,
        r#""bytes":4096}]}"#,
    );
    assert_eq!(json(&event), line);
    assert_eq!(serde_json::from_str::<Event>(line).expect("parses"), event);
}

#[test]
fn errors_have_these_exact_reasons() {
    for (reason, name) in [
        (ErrorReason::VersionMismatch, "version_mismatch"),
        (ErrorReason::BadRequest, "bad_request"),
        (ErrorReason::Busy, "busy"),
        (ErrorReason::HandshakeTimeout, "handshake_timeout"),
    ] {
        let event = Event::Error {
            reason,
            message: "why".to_owned(),
        };
        let line = format!(r#"{{"type":"error","reason":"{name}","message":"why"}}"#);
        assert_eq!(json(&event), line);
        assert_eq!(serde_json::from_str::<Event>(&line).expect("parses"), event);
    }
}

#[test]
fn unknown_fields_are_ignored() {
    let hello: Hello = serde_json::from_str(r#"{"protocol":1,"client":"x"}"#).expect("parses");
    assert_eq!(hello, Hello::current());
    let request: Request =
        serde_json::from_str(r#"{"type":"refresh","later":true}"#).expect("parses");
    assert_eq!(request, Request::Refresh);
    let request: Request =
        serde_json::from_str(r#"{"type":"subscribe","page":"cpu","later":1}"#).expect("parses");
    assert_eq!(request, Request::Subscribe { page: Page::Cpu });
}

#[test]
fn an_absent_optional_field_reads_as_none() {
    let line = r#"{"type":"cpu","core_temperatures_c":[],"power_saver":true}"#;
    let Event::Cpu(page) = serde_json::from_str(line).expect("parses") else {
        panic!("not a cpu event");
    };
    assert_eq!(
        (page.load, page.processes, page.thermal),
        (None, None, None)
    );
    assert!(page.power_saver);
}

#[test]
fn an_unknown_request_or_page_does_not_parse() {
    for line in [
        r#"{"type":"kill","pid":1}"#,
        r#"{"type":"subscribe","page":"gpu"}"#,
    ] {
        assert!(serde_json::from_str::<Request>(line).is_err(), "{line}");
    }
}
