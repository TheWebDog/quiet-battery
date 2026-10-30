//! Razer read-only power commands (class 07, battery 80, charging 84).
//! Wire specification, source revisions and validation limits: docs/DEVICE-SUPPORT.md.
use super::{
    Cancellation, DeviceError, PowerState, Reading,
    diagnostics::Collection,
    vendor_hid::{Adapter, Profile, Transport, number},
};
use std::time::Duration;
const MODELS: &[(&str, u16, u16)] = &[
    ("Razer Viper V2 Pro", 0x00a6, 0x00a5),
    ("Razer Viper V3 Pro", 0x00c1, 0x00c0),
    ("Razer DeathAdder V3 Pro", 0x00b7, 0x00b6),
    ("Razer DeathAdder V3 Pro", 0x00c3, 0x00c2),
    ("Razer Basilisk V3 Pro", 0x00ab, 0x00aa),
];
fn identify(c: &Collection) -> Option<Profile> {
    let vid = number(c, "vid")?;
    let pid = number(c, "pid")?;
    if vid != 0x1532 {
        return None;
    }
    if pid == 0x009c {
        return Some(Profile {
            name: "Razer DeathAdder V2 X HyperSpeed",
            vid,
            pid,
            wireless: true,
        });
    }
    MODELS.iter().find_map(|&(name, receiver, cable)| {
        (pid == receiver || pid == cable).then_some(Profile {
            name,
            vid,
            pid,
            wireless: pid == receiver,
        })
    })
}
fn accepts(c: &Collection) -> bool {
    // OpenRazer targets USB interface 0 for these mouse models. Do not walk
    // arbitrary interfaces or consume their input streams to find a response.
    number(c, "interface") == Some(0) && number(c, "feature_report_bytes") == Some(91)
}
pub(super) const ADAPTER: Adapter = Adapter {
    name: "razer",
    kind: super::DeviceKind::Mouse,
    unconfirmed_name: None,
    identify,
    accepts,
    feature_only: true,
    query,
};
fn checksum(payload: &[u8]) -> u8 {
    payload[2..88].iter().fold(0, |crc, b| crc ^ b)
}
fn request(command: u8) -> [u8; 91] {
    let mut b = [0; 91];
    // Windows report-ID slot precedes the 90-byte unnumbered wire payload.
    b[2] = 0x1f;
    b[6] = 2;
    b[7] = 7;
    b[8] = command;
    b[89] = checksum(&b[1..]);
    b
}
fn parse(b: &[u8], command: u8) -> Result<Option<u8>, DeviceError> {
    // Windows may omit only the final reserved byte for this unnumbered
    // report. Both lengths still include the zero report-ID and the full CRC.
    if !matches!(b.len(), 90 | 91) || b[0] != 0 {
        return Err(DeviceError::Protocol(
            "Razer Feature长度或report-ID异常".into(),
        ));
    }
    let p = &b[1..];
    // Ignore foreign command buffers (including vendor lighting software).
    if p[1] != 0x1f || p[2..5] != [0, 0, 0] || p[6] != 7 || p[7] != command {
        return Ok(None);
    }
    if p[88] != checksum(p) {
        return Err(DeviceError::Protocol("Razer回复校验和不匹配".into()));
    }
    match p[0] {
        0 | 1 => Ok(None), // request echo / busy, not a battery reading
        2 if p[5] == 2 => Ok(Some(p[9])),
        2 => Err(DeviceError::Protocol("Razer电量参数长度异常".into())),
        5 => Err(DeviceError::Unsupported("Razer不支持此电源查询".into())),
        _ => Err(DeviceError::Unavailable(format!(
            "Razer电源查询状态{}",
            p[0]
        ))),
    }
}
fn command(io: &dyn Transport, id: u8, cancel: &Cancellation) -> Result<u8, DeviceError> {
    cancel.check()?;
    io.set_feature(&request(id))?;
    // Shared native transaction deadline also bounds all these reads and waits.
    for _ in 0..24 {
        cancel.check()?;
        io.pause(Duration::from_millis(30))?;
        if let Some(value) = parse(&io.get_feature(0, 91)?, id)? {
            return Ok(value);
        }
    }
    Err(DeviceError::Unavailable("Razer未收到匹配的电源回复".into()))
}
fn query(p: Profile, io: &dyn Transport, cancel: &Cancellation) -> Result<Reading, DeviceError> {
    let raw = command(io, 0x80, cancel)?;
    // 009C uses a replaceable AA/AAA cell; no charging command is needed.
    let power = if p.pid == 0x009c {
        PowerState::Discharging
    } else {
        match command(io, 0x84, cancel) {
            Ok(0) => PowerState::Discharging,
            Ok(1) => PowerState::Charging,
            Ok(n) => PowerState::Unknown(n),
            Err(DeviceError::Cancelled) => return Err(DeviceError::Cancelled),
            Err(_) => PowerState::Unknown(255), // basic battery remains useful
        }
    };
    let percent = ((u16::from(raw) * 100 + 127) / 255) as u8;
    Reading::from_percent(p.name.into(), percent, power)
}
#[cfg(test)]
mod tests {
    use super::super::{
        diagnostics::Field,
        vendor_hid::tests::{Script, Step, collection},
    };
    use super::*;
    fn profile() -> Profile {
        identify(&collection(0x1532, 0x00a6, "fixture")).unwrap()
    }
    fn reply(command: u8, value: u8, status: u8) -> Vec<u8> {
        let mut b = request(command).to_vec();
        b[1] = status;
        b[10] = value;
        b[89] = checksum(&b[1..]);
        b
    }
    #[test]
    fn model_routes_require_expected_control_interface_and_feature_length() {
        for &(name, receiver, cable) in MODELS {
            for (pid, wireless) in [(receiver, true), (cable, false)] {
                let mut c = collection(0x1532, pid, "fixture");
                let p = identify(&c).unwrap();
                assert_eq!((p.name, p.wireless), (name, wireless));
                c.fields.insert("interface", Field::number(0, "fixture"));
                c.fields
                    .insert("feature_report_bytes", Field::number(91, "fixture"));
                assert!(accepts(&c));
                c.fields.insert("interface", Field::number(1, "fixture"));
                assert!(!accepts(&c));
                c.fields.insert("interface", Field::number(0, "fixture"));
                c.fields
                    .insert("feature_report_bytes", Field::number(90, "fixture"));
                assert!(!accepts(&c));
            }
        }
        assert!(identify(&collection(0x1532, 0x00b3, "fixture")).is_none()); // shared HyperPolling dongle needs model identity
        assert!(identify(&collection(0x1533, 0x00a6, "fixture")).is_none());
    }
    #[test]
    fn request_layout_matches_power_class_and_commands_not_swapped_header() {
        for (id, crc) in [(0x80, 0x85), (0x84, 0x81)] {
            let b = request(id);
            assert_eq!(&b[..11], &[0, 0, 0x1f, 0, 0, 0, 2, 7, id, 0, 0]);
            assert_eq!(b[89], crc);
            assert_eq!(b[90], 0);
        }
    }
    #[test]
    fn parse_rejects_corruption_truncation_and_does_not_guess_sleep_from_zero() {
        for raw in 0..=255 {
            assert_eq!(parse(&reply(0x80, raw, 2), 0x80).unwrap(), Some(raw));
        }
        let b = reply(0x80, 171, 2);
        for len in 0..90 {
            assert!(parse(&b[..len], 0x80).is_err());
        }
        for index in [6, 9, 10, 40, 89] {
            let mut bad = b.clone();
            bad[index] ^= 1;
            assert!(parse(&bad, 0x80).is_err());
        }
        for index in [2, 3, 4, 5, 7, 8] {
            let mut foreign = b.clone();
            foreign[index] ^= 1;
            assert!(parse(&foreign, 0x80).unwrap().is_none());
        }
        assert!(parse(&reply(0x80, 90, 1), 0x80).unwrap().is_none());
        assert!(parse(&reply(0x80, 90, 0), 0x80).unwrap().is_none());
        assert!(matches!(
            parse(&reply(0x80, 90, 4), 0x80),
            Err(DeviceError::Unavailable(_))
        ));
        assert!(matches!(
            parse(&reply(0x80, 90, 5), 0x80),
            Err(DeviceError::Unsupported(_))
        ));
    }
    #[test]
    fn transaction_handles_foreign_busy_then_battery_and_independent_charging() {
        let io = Script::new(vec![
            Step::Set(request(0x80).to_vec()),
            Step::Pause,
            Step::Get(0, 91, reply(0x84, 1, 2)),
            Step::Pause,
            Step::Get(0, 91, reply(0x80, 0, 1)),
            Step::Pause,
            Step::Get(0, 91, reply(0x80, 171, 2)),
            Step::Set(request(0x84).to_vec()),
            Step::Pause,
            Step::Get(0, 91, reply(0x84, 1, 2)),
        ]);
        let r = query(profile(), &io, &Cancellation::default()).unwrap();
        io.done();
        assert_eq!(
            (r.normalized_percent(), r.power_state),
            (67, PowerState::Charging)
        );
    }
    #[test]
    fn optional_charge_failure_keeps_battery_but_never_claims_discharging() {
        let io = Script::new(vec![
            Step::Set(request(0x80).to_vec()),
            Step::Pause,
            Step::Get(0, 91, reply(0x80, 0, 2)),
            Step::Set(request(0x84).to_vec()),
            Step::Pause,
            Step::Failure,
        ]);
        let r = query(profile(), &io, &Cancellation::default()).unwrap();
        io.done();
        assert_eq!(
            (r.normalized_percent(), r.power_state),
            (0, PowerState::Unknown(255))
        );
        let cancel = Cancellation::default();
        cancel.cancel();
        let io = Script::new(vec![]);
        assert!(matches!(
            query(profile(), &io, &cancel),
            Err(DeviceError::Cancelled)
        ));
        io.done();
    }
    #[test]
    fn deathadder_v2x_is_receiver_only_and_accepts_short_windows_feature() {
        let p = identify(&collection(0x1532, 0x009c, "fixture")).unwrap();
        assert!(p.wireless);
        assert_eq!(p.name, "Razer DeathAdder V2 X HyperSpeed");
        let mut short = reply(0x80, 26, 2);
        short.pop();
        let io = Script::new(vec![
            Step::Set(request(0x80).to_vec()),
            Step::Pause,
            Step::Get(0, 91, short.clone()),
        ]);
        let r = query(p, &io, &Cancellation::default()).unwrap();
        io.done();
        assert_eq!(
            (r.normalized_percent(), r.power_state),
            (10, PowerState::Discharging)
        );
        short[0] = 1;
        assert!(parse(&short, 0x80).is_err());
        short[0] = 0;
        short[89] ^= 1;
        assert!(parse(&short, 0x80).is_err());
    }
    #[test]
    fn endless_foreign_stream_is_bounded_and_cannot_become_success() {
        let mut steps = vec![Step::Set(request(0x80).to_vec())];
        for _ in 0..24 {
            steps.extend([Step::Pause, Step::Get(0, 91, reply(0x84, 255, 2))]);
        }
        let io = Script::new(steps);
        assert!(query(profile(), &io, &Cancellation::default()).is_err());
        io.done();
    }
}
