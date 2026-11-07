//! Linux monolithic kernel ABI and syscall emulation
//!
//! Subsystem component of SigmaOS following core architectural principles:
//! DRY, KISS, SOLID, AAA testing, Separation of Concerns, Gall's Law.

/// Configuration contract for SigmaOS-kernel-linux
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub enabled: bool,
    pub component_name: String,
    pub tier: u8,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            enabled: true,
            component_name: String::from("SigmaOS-kernel-linux"),
            tier: 1,
        }
    }
}

/// State descriptor for SigmaOS-kernel-linux
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub initialized: bool,
    pub healthy: bool,
}

/// Lifecycle supervisor for SigmaOS-kernel-linux
pub struct Supervisor {
    config: Config,
    state: State,
}

impl Supervisor {
    /// Create a new supervisor instance
    pub fn new(config: Config) -> Self {
        Supervisor {
            config,
            state: State {
                initialized: false,
                healthy: true,
            },
        }
    }

    /// Start and initialize the component
    pub fn initialize(&mut self) -> Result<String, String> {
        if !self.config.enabled {
            return Err(String::from("Component is administratively disabled"));
        }
        self.state.initialized = true;
        Ok(format!("{} initialized successfully at tier {}", self.config.component_name, self.config.tier))
    }

    /// Query current health
    pub fn is_healthy(&self) -> bool {
        self.state.initialized && self.state.healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_configuration() {
        // Arrange
        let config = Config::default();

        // Act
        let is_enabled = config.enabled;
        let tier_val = config.tier;

        // Assert
        assert!(is_enabled, "component must be enabled by default");
        assert_eq!(tier_val, 1, "tier must match component contract");
    }

    #[test]
    fn test_successful_initialization() {
        // Arrange
        let config = Config::default();
        let mut supervisor = Supervisor::new(config);

        // Act
        let result = supervisor.initialize();

        // Assert
        assert!(result.is_ok(), "initialization should succeed with valid config");
        assert!(supervisor.is_healthy(), "component should report healthy state after init");
    }

    #[test]
    fn test_disabled_initialization_failure() {
        // Arrange
        let config = Config {
            enabled: false,
            component_name: String::from("SigmaOS-kernel-linux"),
            tier: 1,
        };
        let mut supervisor = Supervisor::new(config);

        // Act
        let result = supervisor.initialize();

        // Assert
        assert!(result.is_err(), "disabled component should reject initialization");
        assert!(!supervisor.is_healthy(), "disabled component must not be marked healthy");
    }
}

// =========================================================================
// ARCH LINUX KERNEL ARCHITECTURE & KCONFIG EXTENSIONS
// =========================================================================

use std::collections::BTreeMap;

/// Linux Kernel Version descriptor (e.g. `6.8.9-arch1-1`)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct KernelVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub extraversion: String,
}

impl KernelVersion {
    pub fn new(major: u32, minor: u32, patch: u32, extraversion: &str) -> Self {
        Self {
            major,
            minor,
            patch,
            extraversion: extraversion.to_string(),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let (ver_part, extra) = if let Some(dash) = s.find('-') {
            (&s[..dash], &s[dash + 1..])
        } else {
            (s, "")
        };

        let parts: Vec<&str> = ver_part.split('.').collect();
        if parts.len() < 2 {
            return None;
        }

        let major = parts[0].parse::<u32>().ok()?;
        let minor = parts[1].parse::<u32>().ok()?;
        let patch = if parts.len() >= 3 {
            parts[2].parse::<u32>().ok()?
        } else {
            0
        };

        Some(Self {
            major,
            minor,
            patch,
            extraversion: extra.to_string(),
        })
    }

    /// Long Term Support (LTS) kernel series checker
    pub fn is_lts(&self) -> bool {
        match (self.major, self.minor) {
            (6, 1) | (6, 6) | (6, 12) | (5, 15) | (5, 10) | (5, 4) => true,
            _ => false,
        }
    }

    pub fn format_uname(&self) -> String {
        if self.extraversion.is_empty() {
            format!("{}.{}.{}", self.major, self.minor, self.patch)
        } else {
            format!("{}.{}.{}-{}", self.major, self.minor, self.patch, self.extraversion)
        }
    }
}

/// Kconfig option values (matches Linux kernel `.config` formats)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelConfigValue {
    Yes,
    No,
    Module,
    Value(String),
}

/// Linux `.config` configuration manager
#[derive(Debug, Clone, Default)]
pub struct KernelConfig {
    options: BTreeMap<String, KernelConfigValue>,
}

impl KernelConfig {
    pub fn new() -> Self {
        Self {
            options: BTreeMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, val: KernelConfigValue) {
        self.options.insert(key.to_string(), val);
    }

    pub fn get(&self, key: &str) -> Option<&KernelConfigValue> {
        self.options.get(key)
    }

    pub fn is_enabled(&self, key: &str) -> bool {
        matches!(self.options.get(key), Some(KernelConfigValue::Yes))
    }

    pub fn is_module(&self, key: &str) -> bool {
        matches!(self.options.get(key), Some(KernelConfigValue::Module))
    }

    pub fn parse_config(&mut self, content: &str) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with("# CONFIG_") && trimmed.ends_with("is not set") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    let key = parts[1];
                    self.options.insert(key.to_string(), KernelConfigValue::No);
                }
                continue;
            }
            if trimmed.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = trimmed.split_once('=') {
                let parsed_val = match val {
                    "y" => KernelConfigValue::Yes,
                    "m" => KernelConfigValue::Module,
                    "n" => KernelConfigValue::No,
                    other => {
                        let stripped = other.trim_matches('"');
                        KernelConfigValue::Value(stripped.to_string())
                    }
                };
                self.options.insert(key.to_string(), parsed_val);
            }
        }
    }
}

/// Subsystems present in the modular Linux monolithic kernel
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KernelSubsystem {
    Networking,
    Storage,
    Usb,
    Pci,
    Acpi,
    Drm,
    Sound,
    Bluetooth,
    Wireless,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemState {
    Disabled,
    Builtin,
    ModuleLoaded,
    Unloaded,
}

/// Dynamic subsystem orchestrator
#[derive(Debug, Clone, Default)]
pub struct SubsystemLoader {
    states: BTreeMap<KernelSubsystem, SubsystemState>,
}

impl SubsystemLoader {
    pub fn new() -> Self {
        let mut states = BTreeMap::new();
        // Core buses default to builtin
        states.insert(KernelSubsystem::Pci, SubsystemState::Builtin);
        states.insert(KernelSubsystem::Acpi, SubsystemState::Builtin);
        states.insert(KernelSubsystem::Storage, SubsystemState::Builtin);
        states.insert(KernelSubsystem::Networking, SubsystemState::Builtin);
        // Peripherals default to unloaded
        states.insert(KernelSubsystem::Usb, SubsystemState::Unloaded);
        states.insert(KernelSubsystem::Drm, SubsystemState::Unloaded);
        states.insert(KernelSubsystem::Sound, SubsystemState::Unloaded);
        states.insert(KernelSubsystem::Bluetooth, SubsystemState::Disabled);
        states.insert(KernelSubsystem::Wireless, SubsystemState::Unloaded);

        Self { states }
    }

    pub fn load(&mut self, sub: KernelSubsystem) -> Result<(), &'static str> {
        match self.states.get(&sub) {
            Some(SubsystemState::Builtin) => Ok(()),
            Some(SubsystemState::Disabled) => Err("Subsystem is disabled in kernel config"),
            Some(SubsystemState::ModuleLoaded) => Ok(()),
            Some(SubsystemState::Unloaded) => {
                self.states.insert(sub, SubsystemState::ModuleLoaded);
                Ok(())
            }
            None => Err("Unknown subsystem"),
        }
    }

    pub fn unload(&mut self, sub: KernelSubsystem) -> Result<(), &'static str> {
        match self.states.get(&sub) {
            Some(SubsystemState::Builtin) => Err("Cannot unload builtin kernel subsystem"),
            Some(SubsystemState::ModuleLoaded) => {
                self.states.insert(sub, SubsystemState::Unloaded);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    pub fn is_operational(&self, sub: KernelSubsystem) -> bool {
        matches!(self.states.get(&sub), Some(SubsystemState::Builtin) | Some(SubsystemState::ModuleLoaded))
    }
}

#[cfg(test)]
mod arch_kernel_tests {
    use super::*;

    // --- AAA Pattern: Kernel Version Parsing & LTS Check ---
    #[test]
    fn test_kernel_version_parsing_and_lts() {
        // Arrange
        let arch_kernel = "6.6.28-1-lts";
        let zen_kernel = "6.8.9-zen1-1-zen";

        // Act
        let v_lts = KernelVersion::parse(arch_kernel).unwrap();
        let v_zen = KernelVersion::parse(zen_kernel).unwrap();

        // Assert
        assert!(v_lts.is_lts(), "6.6 must be identified as LTS");
        assert_eq!(v_lts.format_uname(), "6.6.28-1-lts");

        assert!(!v_zen.is_lts(), "6.8 is not an LTS release");
        assert_eq!(v_zen.major, 6);
        assert_eq!(v_zen.minor, 8);
        assert_eq!(v_zen.patch, 9);
    }

    // --- AAA Pattern: Kconfig Parser ---
    #[test]
    fn test_kernel_config_parser() {
        // Arrange
        let mut kconfig = KernelConfig::new();
        let raw_config = r#"
# Linux/x86_64 6.8.9 Kernel Configuration
CONFIG_64BIT=y
CONFIG_SMP=y
CONFIG_BTRFS_FS=m
CONFIG_EXT4_FS=y
# CONFIG_USB_STORAGE is not set
CONFIG_DEFAULT_HOSTNAME="sigmaos"
"#;

        // Act
        kconfig.parse_config(raw_config);

        // Assert
        assert!(kconfig.is_enabled("CONFIG_64BIT"));
        assert!(kconfig.is_enabled("CONFIG_SMP"));
        assert!(kconfig.is_module("CONFIG_BTRFS_FS"));
        assert_eq!(kconfig.get("CONFIG_USB_STORAGE"), Some(&KernelConfigValue::No));
        assert_eq!(
            kconfig.get("CONFIG_DEFAULT_HOSTNAME"),
            Some(&KernelConfigValue::Value("sigmaos".to_string()))
        );
    }

    // --- AAA Pattern: Subsystem Loader Lifecycle ---
    #[test]
    fn test_subsystem_loader_lifecycle() {
        // Arrange
        let mut loader = SubsystemLoader::new();

        // Act & Assert Builtin
        assert!(loader.is_operational(KernelSubsystem::Pci));
        assert!(loader.unload(KernelSubsystem::Pci).is_err(), "builtin subsystem must refuse unload");

        // Act & Assert Module Loading
        assert!(!loader.is_operational(KernelSubsystem::Drm));
        assert!(loader.load(KernelSubsystem::Drm).is_ok());
        assert!(loader.is_operational(KernelSubsystem::Drm));

        // Act & Assert Unload
        assert!(loader.unload(KernelSubsystem::Drm).is_ok());
        assert!(!loader.is_operational(KernelSubsystem::Drm));
    }
}
