use malbox_machinery::machine::{DiskType, MachineSpec, Network, NetworkMode, Platform};
use serde::{Deserialize, Serialize};

/// Domain structure that serializes/deserializes to/from libvirt XML format.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "domain")]
pub struct Domain {
    #[serde(rename = "@type")]
    pub domain_type: String,
    pub name: String,
    pub memory: Memory,
    pub vcpu: Vcpu,
    pub os: Os,
    pub features: Features,
    pub cpu: Cpu,
    pub clock: Clock,
    pub on_poweroff: String,
    pub on_reboot: String,
    pub on_crash: String,
    pub devices: Devices,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Memory {
    #[serde(rename = "@unit")]
    pub unit: String,
    #[serde(rename = "$text")]
    pub value: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Vcpu {
    #[serde(rename = "@placement")]
    pub placement: String,
    #[serde(rename = "$text")]
    pub count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Os {
    #[serde(rename = "type")]
    pub os_type: OsType,
    pub boot: Boot,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OsType {
    #[serde(rename = "@arch")]
    pub arch: String,
    #[serde(rename = "@machine")]
    pub machine: String,
    #[serde(rename = "$text")]
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Boot {
    #[serde(rename = "@dev")]
    pub dev: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Features {
    pub acpi: EmptyElement,
    pub apic: EmptyElement,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmptyElement {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Cpu {
    #[serde(rename = "@mode")]
    pub mode: String,
    #[serde(rename = "@check")]
    pub check: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Clock {
    #[serde(rename = "@offset")]
    pub offset: String,
    #[serde(rename = "timer")]
    pub timers: Vec<Timer>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Timer {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@tickpolicy", skip_serializing_if = "Option::is_none")]
    pub tickpolicy: Option<String>,
    #[serde(rename = "@present", skip_serializing_if = "Option::is_none")]
    pub present: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Devices {
    pub emulator: String,
    pub disk: Disk,
    pub interface: Interface,
    pub console: Console,
    pub channel: Channel,
    pub graphics: Graphics,
    pub video: Video,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Disk {
    #[serde(rename = "@type")]
    pub disk_type: String,
    #[serde(rename = "@device")]
    pub device: String,
    pub driver: DiskDriver,
    pub source: DiskSource,
    pub target: DiskTarget,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiskDriver {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@type")]
    pub driver_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiskSource {
    #[serde(rename = "@file")]
    pub file: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiskTarget {
    #[serde(rename = "@dev")]
    pub dev: String,
    #[serde(rename = "@bus")]
    pub bus: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Interface {
    #[serde(rename = "@type")]
    pub interface_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<MacAddress>,
    pub source: InterfaceSource,
    pub model: InterfaceModel,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MacAddress {
    #[serde(rename = "@address")]
    pub address: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InterfaceSource {
    #[serde(rename = "@network", skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(rename = "@bridge", skip_serializing_if = "Option::is_none")]
    pub bridge: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InterfaceModel {
    #[serde(rename = "@type")]
    pub model_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Console {
    #[serde(rename = "@type")]
    pub console_type: String,
    pub target: ConsoleTarget,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsoleTarget {
    #[serde(rename = "@type")]
    pub target_type: String,
    #[serde(rename = "@port")]
    pub port: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Channel {
    #[serde(rename = "@type")]
    pub channel_type: String,
    pub target: ChannelTarget,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelTarget {
    #[serde(rename = "@type")]
    pub target_type: String,
    #[serde(rename = "@name")]
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Graphics {
    #[serde(rename = "@type")]
    pub graphics_type: String,
    #[serde(rename = "@port")]
    pub port: String,
    #[serde(rename = "@autoport")]
    pub autoport: String,
    #[serde(rename = "@listen")]
    pub listen: String,
    pub listen_element: ListenElement,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "listen")]
pub struct ListenElement {
    #[serde(rename = "@type")]
    pub listen_type: String,
    #[serde(rename = "@address")]
    pub address: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Video {
    pub model: VideoModel,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VideoModel {
    #[serde(rename = "@type")]
    pub model_type: String,
    #[serde(rename = "@ram")]
    pub ram: String,
    #[serde(rename = "@vram")]
    pub vram: String,
    #[serde(rename = "@vgamem")]
    pub vgamem: String,
    #[serde(rename = "@heads")]
    pub heads: String,
}

impl Domain {
    /// Build a Domain from a MachineSpec.
    pub fn from_spec(name: String, spec: &MachineSpec, disk_path: String) -> Self {
        let (os_type, arch, machine_type) = match spec.platform {
            Platform::Windows => ("hvm", "x86_64", "pc-q35-5.2"),
            Platform::Linux => ("hvm", "x86_64", "pc-q35-5.2"),
        };

        let disk_bus = match spec.platform {
            Platform::Linux => "virtio",
            Platform::Windows => "sata",
        };

        let disk_format = match spec.storage.disk_type {
            DiskType::Qcow2 => "qcow2",
            DiskType::Raw => "raw",
            DiskType::Vmdk => "vmdk",
        };

        Domain {
            domain_type: "kvm".to_string(),
            name,
            memory: Memory {
                unit: "MiB".to_string(),
                value: spec.resources.memory_mb,
            },
            vcpu: Vcpu {
                placement: "static".to_string(),
                count: spec.resources.cpus,
            },
            os: Os {
                os_type: OsType {
                    arch: arch.to_string(),
                    machine: machine_type.to_string(),
                    value: os_type.to_string(),
                },
                boot: Boot {
                    dev: "hd".to_string(),
                },
            },
            features: Features {
                acpi: EmptyElement {},
                apic: EmptyElement {},
            },
            cpu: Cpu {
                mode: "host-passthrough".to_string(),
                check: "none".to_string(),
            },
            clock: Clock {
                offset: "utc".to_string(),
                timers: vec![
                    Timer {
                        name: "rtc".to_string(),
                        tickpolicy: Some("catchup".to_string()),
                        present: None,
                    },
                    Timer {
                        name: "pit".to_string(),
                        tickpolicy: Some("delay".to_string()),
                        present: None,
                    },
                    Timer {
                        name: "hpet".to_string(),
                        tickpolicy: None,
                        present: Some("no".to_string()),
                    },
                ],
            },
            on_poweroff: "destroy".to_string(),
            on_reboot: "restart".to_string(),
            on_crash: "destroy".to_string(),
            devices: Devices {
                emulator: "/usr/bin/qemu-system-x86_64".to_string(),
                disk: Disk {
                    disk_type: "file".to_string(),
                    device: "disk".to_string(),
                    driver: DiskDriver {
                        name: "qemu".to_string(),
                        driver_type: disk_format.to_string(),
                    },
                    source: DiskSource { file: disk_path },
                    target: DiskTarget {
                        dev: "vda".to_string(),
                        bus: disk_bus.to_string(),
                    },
                },
                interface: Self::build_interface(&spec.network),
                console: Console {
                    console_type: "pty".to_string(),
                    target: ConsoleTarget {
                        target_type: "serial".to_string(),
                        port: "0".to_string(),
                    },
                },
                channel: Channel {
                    channel_type: "unix".to_string(),
                    target: ChannelTarget {
                        target_type: "virtio".to_string(),
                        name: "org.qemu.guest_agent.0".to_string(),
                    },
                },
                graphics: Graphics {
                    graphics_type: "vnc".to_string(),
                    port: "-1".to_string(),
                    autoport: "yes".to_string(),
                    listen: "127.0.0.1".to_string(),
                    listen_element: ListenElement {
                        listen_type: "address".to_string(),
                        address: "127.0.0.1".to_string(),
                    },
                },
                video: Video {
                    model: VideoModel {
                        model_type: "qxl".to_string(),
                        ram: "65536".to_string(),
                        vram: "65536".to_string(),
                        vgamem: "16384".to_string(),
                        heads: "1".to_string(),
                    },
                },
            },
        }
    }

    fn build_interface(network: &Network) -> Interface {
        let (interface_type, source) = match &network.mode {
            NetworkMode::Nat => (
                "network",
                InterfaceSource {
                    network: Some("default".to_string()),
                    bridge: None,
                },
            ),
            NetworkMode::Isolated { subnet } => (
                "network",
                InterfaceSource {
                    network: Some(format!("malbox-isolated-{}", subnet.replace('/', "-"))),
                    bridge: None,
                },
            ),
            NetworkMode::Bridged { interface } => (
                "bridge",
                InterfaceSource {
                    network: None,
                    bridge: Some(interface.clone()),
                },
            ),
        };

        Interface {
            interface_type: interface_type.to_string(),
            mac: network
                .mac
                .as_ref()
                .map(|m| MacAddress { address: m.clone() }),
            source,
            model: InterfaceModel {
                model_type: "virtio".to_string(),
            },
        }
    }

    /// Serialize to XML string.
    pub fn to_xml(&self) -> Result<String, quick_xml::SeError> {
        quick_xml::se::to_string(self)
    }

    /// Deserialize from XML string.
    pub fn from_xml(xml: &str) -> Result<Self, quick_xml::DeError> {
        quick_xml::de::from_str(xml)
    }

    /// Extract disk file path from domain.
    pub fn disk_path(&self) -> Option<String> {
        Some(self.devices.disk.source.file.clone())
    }
}
