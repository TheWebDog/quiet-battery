//! On-demand, selected-device support reports; no vendor commands or input reads.
use super::{
    Cancellation, DeviceDescriptor, DeviceError, Reading, bas,
    diagnostics::{self, Collection},
    vendor_hid::number,
};
use crate::app::json_string as quote;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
const MAX_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct SupportContext {
    pub descriptor: DeviceDescriptor,
    pub reading: Option<Reading>,
    pub last_success: Option<u64>,
    pub error: Option<DeviceError>,
    pub connected: bool,
    pub connection_known: bool,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
/// Strings describing a failure may contain paths from Windows or hidapi. Keep
/// operation/reason text, but never copy raw endpoint or filesystem tokens.
fn scrub(text: &str) -> String {
    text.split_whitespace()
        .map(|part| {
            if part.contains('\\')
                || part.contains('#')
                || part.contains("container:")
                || part.contains("bas:")
                || part.contains("aep:")
                || part.contains("hidpp:")
            {
                "[redacted]".to_owned()
            } else {
                part.chars().take(512).collect()
            }
        })
        .take(64)
        .collect::<Vec<_>>()
        .join(" ")
}
pub(super) fn error_json(error: Option<&DeviceError>) -> String {
    error.map_or_else(
        || "null".into(),
        |error| {
            let kind = match error {
                DeviceError::Cancelled => "cancelled",
                DeviceError::NotFound => "not_found",
                DeviceError::Unavailable(_) => "unavailable",
                DeviceError::Unsupported(_) => "unsupported",
                DeviceError::Protocol(_) => "protocol",
                DeviceError::Io(_) => "io",
            };
            format!(
                "{{\"kind\":{},\"message\":{}}}",
                quote(kind),
                quote(&scrub(&error.to_string()))
            )
        },
    )
}
fn matching(collections: &[Collection]) -> String {
    let matches = |vid, page, usage| {
        collections.iter().any(|c| {
            number(c, "vid") == Some(vid)
                && number(c, "usage_page") == Some(page)
                && number(c, "usage") == Some(usage)
        })
    };
    let hidpp = matches(0x046d, 0xff00, 1) || matches(0x046d, 0xff00, 2);
    let n3_profile = collections
        .iter()
        .any(|c| number(c, "vid") == Some(0x248a) && number(c, "pid") == Some(0xff30));
    let mut result = vec![
        format!(
            "{{\"adapter\":\"hidpp\",\"control_endpoint_matched\":{hidpp},\"reason\":{}}}",
            quote(if hidpp {
                "HID++ vendor collection present; features and paired slots are not queried by this report"
            } else {
                "No 046D FF00:0001/0002 collection in selected HID group"
            })
        ),
        format!(
            "{{\"adapter\":\"darmoshark\",\"profile_matched\":{n3_profile},\"control_endpoint_matched\":{},\"reason\":{}}}",
            collections.iter().any(super::darmoshark::target),
            quote(
                "Requires N3 receiver profile and paired-device confirmation; paired device is not queried by this report"
            )
        ),
    ];
    for adapter in [
        &super::compx::ADAPTER,
        &super::razer::ADAPTER,
        &super::rog_pelta::ADAPTER,
        &super::razer_headset::ADAPTER,
        &super::aula::ADAPTER,
        &super::ajazz::ADAPTER,
        &super::xiaomi::ADAPTER,
    ] {
        let known: Vec<_> = collections
            .iter()
            .filter_map(|c| (adapter.identify)(c).map(|p| (c, p)))
            .collect();
        let eligible = known.iter().filter(|(c, _)| (adapter.accepts)(c)).count();
        result.push(format!("{{\"adapter\":{},\"profile_matched\":{},\"control_endpoint_matched\":{},\"reason\":{}}}",
            quote(adapter.name), !known.is_empty(), eligible>0,
            quote(if known.is_empty() { "VID/PID has no built-in profile" } else if eligible==0 {
                "Profile exists, but required usage/interface/report lengths do not match or are unavailable"
            } else { "Profile and control collection match; no battery command sent by this report" })));
    }
    result.push(format!(
        "{{\"adapter\":\"delux\",\"receiver_profile_matched\":{},\"status_endpoint_matched\":{},\"reason\":{}}}",
        collections.iter().any(super::delux::receiver),
        collections.iter().any(super::delux::target),
        quote("Requires 1D57 FA60/FA65, dedicated 000A:0000 five-byte input-only collection, then a recognized frame model/event. Shared USB IDs alone do not identify DELUX. This support report does not read the input stream.")
    ));
    format!("[{}]", result.join(","))
}
fn os_json() -> String {
    #[repr(C)]
    struct Version {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        csd: [u16; 128],
    }
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn RtlGetVersion(version: *mut Version) -> i32;
    }
    let mut version = Version {
        size: std::mem::size_of::<Version>() as u32,
        major: 0,
        minor: 0,
        build: 0,
        platform: 0,
        csd: [0; 128],
    };
    if unsafe { RtlGetVersion(&mut version) } >= 0 {
        format!(
            "{{\"family\":\"Windows\",\"major\":{},\"minor\":{},\"build\":{},\"architecture\":{}}}",
            version.major,
            version.minor,
            version.build,
            quote(std::env::consts::ARCH)
        )
    } else {
        "{\"family\":\"Windows\",\"version_error\":\"RtlGetVersion failed\"}".into()
    }
}
fn assemble(
    context: &SupportContext,
    hid: &str,
    checks: &str,
    bluetooth: &str,
    collected: u64,
) -> Result<String, DeviceError> {
    let reading = context.reading.as_ref().map_or_else(
        || "null".into(),
        |r| {
            format!(
                "{{\"percent\":{},\"source\":{},\"power_state\":{}}}",
                r.normalized_percent(),
                quote(&format!("{:?}", r.source)),
                quote(&format!("{:?}", r.power_state))
            )
        },
    );
    let json = format!(
        r#"{{
  "schema": 1,
  "mode": "selected-device-support",
  "application": {{"name":"Quiet Battery","version":{version}}},
  "system": {os},
  "collected_unix": {collected},
  "privacy": {{"scope":"selected physical device only","raw_paths_included":false,"container_ids_included":false,"bluetooth_addresses_included":false,"serial_numbers_queried":false}},
  "user_supplied": {{"brand":null,"exact_model":null,"connection":null,"receiver_variant":null,"official_driver_url":null}},
  "device": {{"label":{label},"kind":{kind},"adapter":{adapter},"logical_endpoint":{endpoint},"supported_last_discovery":{supported}}},
  "last_monitor_observation": {{"source":"UI snapshot at request; not a fresh vendor battery query","connected":{connected},"connection_known":{known},"last_success_unix":{last_success},"reading":{reading},"error":{error}}},
  "adapter_checks": {checks},
  "hid": {hid},
  "bluetooth": {bluetooth},
  "limits": {{"report_bytes":2097152,"hid_scope":"selected physical group, max 32 collections","bluetooth_scope":"paired BLE endpoint with matching physical identity; standard BAS only"}}
}}
"#,
        version = quote(env!("CARGO_PKG_VERSION")),
        os = os_json(),
        label = quote(&scrub(&context.descriptor.name)),
        kind = quote(context.descriptor.kind.key()),
        adapter = quote(&context.descriptor.provider),
        endpoint = context
            .descriptor
            .endpoint
            .as_deref()
            .map_or_else(|| "null".into(), |s| quote(&scrub(s))),
        supported = context.descriptor.supported,
        connected = context.connected,
        known = context.connection_known,
        last_success = context
            .last_success
            .map_or_else(|| "null".into(), |n| n.to_string()),
        error = error_json(context.error.as_ref())
    );
    if json.len() > MAX_BYTES {
        return Err(DeviceError::Unavailable("适配信息超过大小上限".into()));
    }
    Ok(json)
}
pub fn collect(context: &SupportContext, cancel: &Cancellation) -> Result<String, DeviceError> {
    cancel.check()?;
    let (hid, checks) = match diagnostics::inventory(cancel) {
        Ok(hid) => {
            let hid = hid.selected(&context.descriptor.physical_id);
            let checks = matching(&hid.collections);
            (hid.json(false), checks)
        }
        Err(DeviceError::Cancelled) => return Err(DeviceError::Cancelled),
        Err(error) => (
            format!(
                "{{\"complete\":false,\"error\":{}}}",
                error_json(Some(&error))
            ),
            "[]".into(),
        ),
    };
    let bluetooth = match bas::support_information(&context.descriptor, cancel) {
        Ok(report) => report,
        Err(DeviceError::Cancelled) => return Err(DeviceError::Cancelled),
        Err(error) => format!(
            "{{\"status\":\"query_failed\",\"error\":{}}}",
            error_json(Some(&error))
        ),
    };
    cancel.check()?;
    assemble(context, &hid, &checks, &bluetooth, now())
}
/// No directory creation, log files or EXE-side data. Atomic replacement is
/// confined to the path explicitly chosen by the user, temporary removed on error.
pub fn save(path: &Path, json: &str, cancel: &Cancellation) -> Result<(), DeviceError> {
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    cancel.check()?;
    if json.len() > MAX_BYTES {
        return Err(DeviceError::Protocol("适配信息超过大小上限".into()));
    }
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| DeviceError::Io("导出路径无效".into()))?;
    let temporary = directory.join(format!(
        ".quiet-battery-{}-{}.tmp",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|e| DeviceError::Io(e.to_string()))?;
    let result = (|| {
        file.write_all(json.as_bytes())
            .map_err(|e| DeviceError::Io(e.to_string()))?;
        file.sync_all()
            .map_err(|e| DeviceError::Io(e.to_string()))?;
        drop(file);
        #[cfg(test)]
        crate::fault_tests::pause("export-before-publish");
        cancel.check()?;
        let from = crate::startup::wide(&temporary.to_string_lossy());
        let to = crate::startup::wide(&path.to_string_lossy());
        if unsafe {
            MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(DeviceError::Io(format!(
                "导出替换失败，Windows错误{}",
                unsafe { windows_sys::Win32::Foundation::GetLastError() }
            )));
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::super::{DeviceKind, PowerState, diagnostics::Field};
    use super::*;
    fn context() -> SupportContext {
        SupportContext {
            descriptor: DeviceDescriptor {
                id: "hid:container:PRIVATE".into(),
                physical_id: "container:PRIVATE".into(),
                endpoint: None,
                recoverable: false,
                name: "fixture mouse".into(),
                kind: DeviceKind::Mouse,
                supported: false,
                provider: "unsupported".into(),
            },
            reading: Some(
                Reading::from_percent("fixture mouse".into(), 35, PowerState::Discharging).unwrap(),
            ),
            last_success: Some(123),
            error: Some(DeviceError::Io(
                r"open \\?\hid#SECRET C:\Users\PRIVATE\file".into(),
            )),
            connected: true,
            connection_known: true,
        }
    }
    #[test]
    fn report_keeps_last_observation_separate_and_redacts_id_and_error_paths() {
        let c = context();
        let json = assemble(&c, "{}", "[]", "{}", 456).unwrap();
        assert!(json.contains("\"collected_unix\": 456"));
        assert!(json.contains("\"last_success_unix\":123"));
        assert!(json.contains("\"percent\":35"));
        assert!(!json.contains("PRIVATE") && !json.contains("SECRET"));
        assert!(
            json.contains("user_supplied") && json.contains("not a fresh vendor battery query")
        );
        assert_eq!(c.last_success, Some(123));
    }
    #[test]
    fn matching_distinguishes_unknown_profile_from_wrong_control_endpoint() {
        let mut c = Collection::default();
        c.fields.insert("vid", Field::number(0x3554, "fixture"));
        c.fields.insert("pid", Field::number(0xf58a, "fixture"));
        let first = matching(&[c.clone()]);
        assert!(first.contains("Profile exists, but required"));
        for (k, v) in [
            ("usage_page", 0xff02),
            ("usage", 2),
            ("input_report_bytes", 17),
            ("output_report_bytes", 17),
        ] {
            c.fields.insert(k, Field::number(v, "fixture"));
        }
        let matched = matching(&[c]);
        assert!(matched.contains("Profile and control collection match"));
        assert!(matching(&[]).contains("VID/PID has no built-in profile"));
    }
    #[test]
    fn issue_adapters_are_reported_without_querying_any_vendor_protocol() {
        for (adapter, vid, pid, page, usage, length, feature, interface) in [
            ("rog-pelta", 0x0b05, 0x1b84, 0xff00, 1, 64, 91, 3),
            ("razer-headset", 0x1532, 0x0555, 0xff00, 1, 64, 91, 3),
            ("aula", 0x0c45, 0xfefe, 0xff60, 0x61, 33, 91, 0),
            ("razer", 0x1532, 0x009c, 1, 2, 0, 91, 0),
            ("ajazz", 0x0c45, 0xfefc, 0xff80, 1, 33, 0, 2),
            ("xiaomi", 0x2717, 0x5111, 0xffef, 0, 64, 64, 3),
        ] {
            let mut c = Collection::default();
            for (k, v) in [
                ("vid", vid),
                ("pid", pid),
                ("usage_page", page),
                ("usage", usage),
                ("interface", interface),
                ("input_report_bytes", length),
                ("output_report_bytes", length),
                ("feature_report_bytes", feature),
            ] {
                c.fields.insert(k, Field::number(v, "fixture"));
            }
            let report = matching(&[c.clone()]);
            assert!(report.contains(&format!("\"adapter\":\"{adapter}\",\"profile_matched\":true,\"control_endpoint_matched\":true")));
            c.fields.remove(if adapter == "razer" {
                "feature_report_bytes"
            } else {
                "input_report_bytes"
            });
            let report = matching(&[c]);
            assert!(report.contains(&format!("\"adapter\":\"{adapter}\",\"profile_matched\":true,\"control_endpoint_matched\":false")));
        }
    }
    #[test]
    fn cancelled_collection_does_not_enumerate_or_touch_hardware() {
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(matches!(
            collect(&context(), &cancel),
            Err(DeviceError::Cancelled)
        ));
    }
    #[test]
    fn export_replaces_only_selected_file_and_cleans_failed_temporary() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(".build/tmp")
            .join(format!("support-export-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let target = directory.join("chosen.json");
        let other = directory.join("untouched.txt");
        fs::write(&target, "old").unwrap();
        fs::write(&other, "keep").unwrap();
        save(&target, "{\"测试\":35}", &Cancellation::default()).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "{\"测试\":35}");
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(save(&target, "bad", &cancel).is_err());
        let guard = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(".guard"))
            .unwrap();
        drop(guard);
        assert!(save(&directory, "bad", &Cancellation::default()).is_err());
        assert_eq!(fs::read_to_string(other).unwrap(), "keep");
        assert!(
            !fs::read_dir(&directory).unwrap().any(|e| e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp"))
        );
        fs::remove_file(target).unwrap();
        fs::remove_file(directory.join("untouched.txt")).unwrap();
        fs::remove_file(directory.join(".guard")).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
