//! Xiaomi G-HUB M002 feature-report battery query. The 2.4G receiver exposes a
//! 0xFFEF vendor collection; the feature reply is matched before a value is used.
use super::{
    Cancellation, DeviceError, DeviceKind, PowerState, Reading,
    diagnostics::Collection,
    vendor_hid::{Adapter, Profile, Transport, number},
};

const REPORT_ID: u8 = 5;
const LENGTH: usize = 64;
const QUERY: u8 = 0x16;
// M002 receivers (0x5111) use selector 1; the wired 0x5110 variant uses 0.
const SELECTOR: u8 = 1;
fn identify(c: &Collection) -> Option<Profile> {
    (number(c, "vid") == Some(0x2717) && number(c, "pid") == Some(0x5111)).then_some(Profile {
        name: "小米电竞鼠标 2",
        vid: 0x2717,
        pid: 0x5111,
        wireless: true,
    })
}
fn accepts(c: &Collection) -> bool {
    [
        ("interface", 3),
        ("usage_page", 0xffef),
        ("usage", 0),
        ("input_report_bytes", 64),
        ("output_report_bytes", 64),
        ("feature_report_bytes", 64),
    ]
    .iter()
    .all(|&(k, v)| number(c, k) == Some(v))
}
pub(super) const ADAPTER: Adapter = Adapter {
    name: "xiaomi",
    kind: DeviceKind::Mouse,
    // Xiaomi's own receiver ID and vendor page select the device; there is no
    // model field to confirm, so a failed query must not rename the device.
    unconfirmed_name: None,
    identify,
    accepts,
    feature_only: true,
    query,
};
fn request() -> [u8; LENGTH] {
    let mut b = [0; LENGTH];
    b[0] = REPORT_ID;
    b[1] = SELECTOR;
    b[2] = QUERY;
    b
}
// Verified reply on the receiver: `05 01 16 03 01 <percent> ...`, where 05 is
// the Windows report ID, 01 the receiver selector and 16 the query command.
fn decode(reply: &[u8]) -> Result<(u8, PowerState), DeviceError> {
    if reply.len() != LENGTH {
        return Err(DeviceError::Protocol("小米电量回复长度异常".into()));
    }
    if reply[..3] != [REPORT_ID, SELECTOR, QUERY] {
        return Err(DeviceError::Protocol("小米电量回复标识不匹配".into()));
    }
    // The three-byte battery payload starts at byte 4. An echoed request has
    // this length byte zero; it must never become a successful 0% observation.
    if reply[3] != 3 {
        return Err(DeviceError::Protocol("小米电量回复数据长度异常".into()));
    }
    let percent = reply[5];
    if percent > 100 {
        return Err(DeviceError::Protocol("小米电量越界".into()));
    }
    // No byte in this payload has a confirmed charging interpretation yet.
    Ok((percent, PowerState::Unknown(255)))
}
fn query(p: Profile, io: &dyn Transport, cancel: &Cancellation) -> Result<Reading, DeviceError> {
    cancel.check()?;
    io.set_feature(&request())?;
    io.pause(std::time::Duration::from_millis(100))?;
    let reply = io.get_feature(REPORT_ID, LENGTH)?;
    cancel.check()?;
    let (percent, power) = decode(&reply)?;
    Reading::from_percent(p.name.into(), percent, power)
}
#[cfg(test)]
mod tests {
    use super::super::{
        diagnostics::Field,
        vendor_hid::tests::{Script, Step, collection},
    };
    use super::*;
    fn p() -> Profile {
        identify(&collection(0x2717, 0x5111, "fixture")).unwrap()
    }
    fn sample() -> Vec<u8> {
        let mut b = vec![0u8; LENGTH];
        b[0] = REPORT_ID;
        b[1] = SELECTOR;
        b[2] = QUERY;
        b[3] = 3;
        b[4] = 1;
        b[5] = 56;
        b
    }
    #[test]
    fn only_the_documented_receiver_and_vendor_collection_are_eligible() {
        let mut c = collection(0x2717, 0x5111, "fixture");
        assert!(identify(&c).is_some());
        assert!(!accepts(&c));
        for (k, v) in [
            ("interface", 3),
            ("usage_page", 0xffef),
            ("usage", 0),
            ("input_report_bytes", 64),
            ("output_report_bytes", 64),
            ("feature_report_bytes", 64),
        ] {
            c.fields.insert(k, Field::number(v, "fixture"));
        }
        assert!(accepts(&c));
        for (k, v) in [
            ("interface", 2),
            ("usage_page", 0xff60),
            ("usage", 1),
            ("feature_report_bytes", 63),
            ("input_report_bytes", 63),
            ("output_report_bytes", 63),
        ] {
            let mut other = c.clone();
            other.fields.insert(k, Field::number(v, "fixture"));
            assert!(!accepts(&other));
        }
        assert!(identify(&collection(0x2717, 0x5110, "wired")).is_none());
        assert_eq!(ADAPTER.kind, DeviceKind::Mouse);
    }
    #[test]
    fn feature_transaction_requires_request_identity_and_marker() {
        let reply = sample();
        let io = Script::new(vec![
            Step::Set(request().to_vec()),
            Step::Pause,
            Step::Get(5, 64, reply.clone()),
        ]);
        let value = query(p(), &io, &Cancellation::default()).unwrap();
        io.done();
        assert_eq!(
            (value.normalized_percent(), value.power_state),
            (56, PowerState::Unknown(255))
        );
        for index in [0, 1, 2] {
            let mut bad = sample();
            bad[index] ^= 1;
            assert!(decode(&bad).is_err());
        }
        let mut truncated = sample();
        truncated.truncate(32);
        assert!(decode(&truncated).is_err());
        let io = Script::new(vec![Step::Failure]);
        assert!(query(p(), &io, &Cancellation::default()).is_err());
        io.done();
        let cancel = Cancellation::default();
        cancel.cancel();
        let io = Script::new(vec![]);
        assert!(matches!(
            query(p(), &io, &cancel),
            Err(DeviceError::Cancelled)
        ));
        io.done();
    }
    #[test]
    fn percent_out_of_range_is_rejected() {
        for percent in 101..=255 {
            let mut reply = sample();
            reply[5] = percent;
            assert!(decode(&reply).is_err());
        }
        for percent in 0..=100 {
            let mut reply = sample();
            reply[5] = percent;
            assert_eq!(decode(&reply).unwrap().0, percent);
        }
    }
    #[test]
    fn request_echo_and_empty_payload_never_replace_battery_with_zero() {
        assert!(decode(&request()).is_err());
        for length in [0, 1, 2, 4, 255] {
            let mut b = sample();
            b[3] = length;
            assert!(decode(&b).is_err());
        }
        let mut zero = sample();
        zero[5] = 0;
        assert_eq!(decode(&zero).unwrap().0, 0);
        let io = Script::new(vec![
            Step::Set(request().to_vec()),
            Step::Pause,
            Step::Get(5, 64, request().to_vec()),
        ]);
        assert!(query(p(), &io, &Cancellation::default()).is_err());
        io.done();
    }
    // Contributor Mi Gaming Mouse 2 receiver capture, 2026-10-30. One valid
    // feature reply with a 3-byte payload; no path, container or serial data.
    const CAPTURED_REPLY: [u8; 64] = [
        0x05, 0x01, 0x16, 0x03, 0x01, 0x14, 0x15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    #[test]
    fn captured_feature_reply_confirms_length_and_percent() {
        assert_eq!(CAPTURED_REPLY[..4], [REPORT_ID, SELECTOR, QUERY, 3]);
        assert_eq!(
            decode(&CAPTURED_REPLY).unwrap(),
            (20, PowerState::Unknown(255))
        );
        // A real request echo (length byte 0) must never become 0%.
        assert!(decode(&request()).is_err());
    }
}
