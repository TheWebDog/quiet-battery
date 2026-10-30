//! Read-only protocol adapters and common, device-independent battery values.
mod ajazz;
mod aula;
mod bas;
mod compx;
mod darmoshark;
mod delux;
mod diagnostics;
mod discovery;
mod hid_io;
mod logitech;
mod razer;
mod razer_headset;
mod rog_pelta;
pub mod support;
mod vendor_hid;
mod xiaomi;
use std::{
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub use support::SupportContext;
#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);
impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn check(&self) -> Result<(), DeviceError> {
        if self.0.load(Ordering::Acquire) {
            Err(DeviceError::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    Mouse,
    Keyboard,
    Numpad,
    Headset,
    Earbuds,
    Gamepad,
    Touchpad,
    Trackball,
    Tablet,
    Pen,
    Microphone,
    Speaker,
    Remote,
    Camera,
    Presenter,
    Generic,
}
impl DeviceKind {
    pub const ALL: [Self; 16] = [
        Self::Mouse,
        Self::Keyboard,
        Self::Numpad,
        Self::Headset,
        Self::Earbuds,
        Self::Gamepad,
        Self::Touchpad,
        Self::Trackball,
        Self::Tablet,
        Self::Pen,
        Self::Microphone,
        Self::Speaker,
        Self::Remote,
        Self::Camera,
        Self::Presenter,
        Self::Generic,
    ];
    pub fn key(self) -> &'static str {
        match self {
            Self::Mouse => "mouse",
            Self::Keyboard => "keyboard",
            Self::Numpad => "numpad",
            Self::Headset => "headset",
            Self::Earbuds => "earbuds",
            Self::Gamepad => "gamepad",
            Self::Touchpad => "touchpad",
            Self::Trackball => "trackball",
            Self::Tablet => "tablet",
            Self::Pen => "pen",
            Self::Microphone => "microphone",
            Self::Speaker => "speaker",
            Self::Remote => "remote",
            Self::Camera => "camera",
            Self::Presenter => "presenter",
            Self::Generic => "generic",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.key() == key)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDescriptor {
    pub id: String,
    pub physical_id: String,
    pub endpoint: Option<String>,
    pub recoverable: bool,
    pub name: String,
    pub kind: DeviceKind,
    pub supported: bool,
    pub provider: String,
}
impl DeviceDescriptor {
    pub fn short_name(&self) -> String {
        let name = self.name.trim();
        let name = name
            .strip_suffix(" Lightspeed Wireless Gaming Mouse")
            .or_else(|| name.strip_suffix(" LIGHTSPEED Wireless Gaming Mouse"))
            .unwrap_or(name);
        name.chars().take(24).collect()
    }
}
#[derive(Debug, Clone)]
pub struct Candidate {
    pub descriptor: DeviceDescriptor,
    pub reading: Option<Reading>,
    pub error: Option<DeviceError>,
}
#[derive(Debug, Default)]
pub struct DiscoveryReport {
    pub candidates: Vec<Candidate>,
    pub failures: Vec<(String, String)>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Full,
    Medium,
    Low,
    Critical,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    Discharging,
    Charging,
    Full,
    Unknown(u8),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatterySource {
    ReportedPercent,
    Levels { current: u8, total: u8 },
}
#[derive(Debug, Clone)]
pub struct Reading {
    pub device_name: String,
    pub protocol: (u8, u8),
    pub metadata_known: bool,
    pub level: Level,
    pub reported_percent: u8,
    pub next_reported_percent: u8,
    pub level_count: u8,
    pub capability_flags: u8,
    pub battery_capability_mask: Option<u8>,
    pub power_state: PowerState,
    pub source: BatterySource,
}
impl Reading {
    pub fn has_fine_estimate(&self) -> bool {
        self.metadata_known
            && matches!(self.source, BatterySource::ReportedPercent)
            && self.capability_flags & 2 != 0
            && (self.battery_capability_mask.is_some() || self.level_count >= 10)
    }
    pub fn normalized_percent(&self) -> u8 {
        self.reported_percent
    }
    pub fn from_percent(
        name: String,
        percent: u8,
        power_state: PowerState,
    ) -> Result<Self, DeviceError> {
        if percent > 100 {
            return Err(DeviceError::Protocol("电量百分比超出范围".into()));
        }
        Ok(Self {
            device_name: name,
            protocol: (0, 0),
            metadata_known: false,
            level: level_for(percent),
            reported_percent: percent,
            next_reported_percent: 0,
            level_count: 0,
            capability_flags: 0,
            battery_capability_mask: None,
            power_state,
            source: BatterySource::ReportedPercent,
        })
    }
    pub fn from_levels(name: String, current: u8, total: u8) -> Result<Self, DeviceError> {
        if total == 0 || current > total {
            return Err(DeviceError::Protocol("电量档位范围异常".into()));
        }
        let percent = (u16::from(current) * 100 / u16::from(total)) as u8;
        Ok(Self {
            device_name: name,
            protocol: (0, 0),
            metadata_known: false,
            level: level_for(percent),
            reported_percent: percent,
            next_reported_percent: 0,
            level_count: total,
            capability_flags: 0,
            battery_capability_mask: None,
            power_state: PowerState::Unknown(255),
            source: BatterySource::Levels { current, total },
        })
    }
}
pub fn level_for(percent: u8) -> Level {
    match percent {
        0..=10 => Level::Critical,
        11..=29 => Level::Low,
        30..=80 => Level::Medium,
        _ => Level::Full,
    }
}
#[derive(Debug, Clone)]
pub enum DeviceError {
    Cancelled,
    NotFound,
    Unavailable(String),
    Unsupported(String),
    Protocol(String),
    Io(String),
}
impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("读取已取消"),
            Self::NotFound => f.write_str("未找到设备"),
            Self::Unavailable(s) => write!(f, "设备暂不可用：{s}"),
            Self::Unsupported(s) => write!(f, "设备尚未适配：{s}"),
            Self::Protocol(s) => write!(f, "设备回复异常：{s}"),
            Self::Io(s) => write!(f, "设备通信失败：{s}"),
        }
    }
}
impl std::error::Error for DeviceError {}
/// A statically registered, query-only family. Adding one never changes the UI.
pub trait ProtocolAdapter: Sync {
    fn name(&self) -> &'static str;
    fn owns(&self, id: &str) -> bool;
    fn discover(&self, cancel: &Cancellation) -> Result<Vec<Candidate>, DeviceError>;
    fn discover_report(&self, cancel: &Cancellation) -> Result<DiscoveryReport, DeviceError> {
        self.discover(cancel).map(|candidates| DiscoveryReport {
            candidates,
            failures: Vec::new(),
        })
    }
    fn read(&self, id: &str, cancel: &Cancellation) -> Result<Reading, DeviceError>;
}
struct HidppAdapter;
impl ProtocolAdapter for HidppAdapter {
    fn name(&self) -> &'static str {
        "hidpp"
    }
    fn owns(&self, id: &str) -> bool {
        id.starts_with("hidpp:")
    }
    fn discover(&self, cancel: &Cancellation) -> Result<Vec<Candidate>, DeviceError> {
        logitech::discover(cancel)
    }
    fn read(&self, id: &str, cancel: &Cancellation) -> Result<Reading, DeviceError> {
        logitech::read_device(id, cancel)
    }
}
static ADAPTERS: &[&dyn ProtocolAdapter] = &[
    &HidppAdapter,
    &bas::BasAdapter,
    &darmoshark::DarmosharkAdapter,
    &compx::ADAPTER,
    &delux::DeluxAdapter,
    &razer::ADAPTER,
    &rog_pelta::ADAPTER,
    &razer_headset::ADAPTER,
    &aula::ADAPTER,
    &ajazz::ADAPTER,
    &xiaomi::ADAPTER,
];
pub struct Registry;
impl Registry {
    #[cfg(test)]
    pub fn diagnostic_fixture() -> Candidate {
        discovery::publish_diagnostic_fixture()
    }
    /// Independent passive HID inventory. Does not initialize/query adapters.
    pub fn diagnose(cancel: &Cancellation, identifiers: bool) -> Result<String, DeviceError> {
        diagnostics::inventory(cancel).map(|report| report.json(identifiers))
    }
    pub fn diagnostic_information(descriptor: &DeviceDescriptor) -> Option<String> {
        discovery::diagnostic_information(&descriptor.physical_id)
    }
    pub fn runtime() -> Result<bas::Runtime, DeviceError> {
        bas::Runtime::init()
    }
    pub fn set_change_handler(
        handler: Arc<dyn Fn(String, Result<Reading, DeviceError>) + Send + Sync>,
    ) {
        bas::set_change_handler(handler);
    }
    pub fn set_refresh_handler(handler: Arc<dyn Fn(String) + Send + Sync>) {
        bas::set_refresh_handler(handler);
    }
    pub fn shutdown() {
        bas::shutdown();
    }
    pub fn discover() -> Result<Vec<Candidate>, DeviceError> {
        Self::discover_cancelled(&Cancellation::default()).map(|report| report.candidates)
    }
    pub fn discover_cancelled(cancel: &Cancellation) -> Result<DiscoveryReport, DeviceError> {
        let mut report = discover_adapters(ADAPTERS, cancel)?;
        let claimed = report
            .candidates
            .iter()
            .map(|c| c.descriptor.physical_id.clone())
            .collect();
        cancel.check()?;
        match discovery::generic_candidates(&claimed, cancel) {
            Ok(candidates) => report.candidates.extend(candidates),
            Err(DeviceError::Cancelled) => return Err(DeviceError::Cancelled),
            Err(error) => report
                .failures
                .push(("unsupported".into(), error.to_string())),
        }
        cancel.check()?;
        Ok(report)
    }
    pub fn read(id: &str) -> Result<Reading, DeviceError> {
        Self::read_cancelled(id, &Cancellation::default())
    }
    pub fn read_cancelled(id: &str, cancel: &Cancellation) -> Result<Reading, DeviceError> {
        cancel.check()?;
        ADAPTERS
            .iter()
            .find(|adapter| adapter.owns(id))
            .ok_or_else(|| DeviceError::Unsupported("没有对应的查询适配器".into()))?
            .read(id, cancel)
    }
}
fn discover_adapters(
    adapters: &[&dyn ProtocolAdapter],
    cancel: &Cancellation,
) -> Result<DiscoveryReport, DeviceError> {
    let mut report = DiscoveryReport::default();
    for adapter in adapters {
        cancel.check()?;
        match adapter.discover_report(cancel) {
            Ok(found) => {
                report.candidates.extend(found.candidates);
                report.failures.extend(found.failures);
            }
            Err(DeviceError::Cancelled) => return Err(DeviceError::Cancelled),
            Err(error) => report
                .failures
                .push((adapter.name().into(), error.to_string())),
        }
    }
    cancel.check()?;
    Ok(report)
}
#[cfg(test)]
mod tests {
    use super::*;
    struct FailingAdapter;
    impl ProtocolAdapter for FailingAdapter {
        fn name(&self) -> &'static str {
            "failure"
        }
        fn owns(&self, _: &str) -> bool {
            false
        }
        fn discover(&self, _: &Cancellation) -> Result<Vec<Candidate>, DeviceError> {
            Err(DeviceError::Io("failed".into()))
        }
        fn read(&self, _: &str, _: &Cancellation) -> Result<Reading, DeviceError> {
            Err(DeviceError::NotFound)
        }
    }
    struct EmptyAdapter;
    impl ProtocolAdapter for EmptyAdapter {
        fn name(&self) -> &'static str {
            "healthy"
        }
        fn owns(&self, _: &str) -> bool {
            false
        }
        fn discover(&self, _: &Cancellation) -> Result<Vec<Candidate>, DeviceError> {
            Ok(Vec::new())
        }
        fn read(&self, _: &str, _: &Cancellation) -> Result<Reading, DeviceError> {
            Err(DeviceError::NotFound)
        }
    }
    #[test]
    fn failed_adapters_are_isolated_and_cancellation_is_not_completion() {
        let report =
            discover_adapters(&[&FailingAdapter, &EmptyAdapter], &Cancellation::default()).unwrap();
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].0, "failure");
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(matches!(
            discover_adapters(&[&EmptyAdapter], &cancel),
            Err(DeviceError::Cancelled)
        ));
    }
    #[test]
    fn normalization_checks_range_and_preserves_source() {
        let value = Reading::from_levels("Keyboard".into(), 3, 4).unwrap();
        assert_eq!(value.normalized_percent(), 75);
        assert_eq!(
            value.source,
            BatterySource::Levels {
                current: 3,
                total: 4
            }
        );
        assert!(Reading::from_levels("".into(), 0, 0).is_err());
        assert!(Reading::from_levels("".into(), 5, 4).is_err());
    }
}
