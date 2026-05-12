// Jackson Coxson
// Tool registry. Each entry maps a URL slug + sidebar label to a route view.

pub mod afc;
pub mod amfi;
pub mod app_service;
pub mod debug_proxy;
pub mod diagnostics;
pub mod diagnosticsservice;
pub mod energy_monitor;
pub mod graphics;
pub mod idevice_id;
pub mod ideviceinfo;
pub mod installation_proxy;
pub mod location_simulation;
pub mod lockdown;
pub mod misagent;
pub mod mounter;
pub mod network_monitor;
pub mod notifications;
pub mod os_trace_relay;
pub mod process_control;
pub mod rppairing;
pub mod screenshot;
pub mod syslog_relay;

#[derive(Clone, Copy)]
pub struct ToolEntry {
    pub slug: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

pub static TOOLS: &[ToolEntry] = &[
    ToolEntry {
        slug: "idevice_id",
        name: "idevice_id",
        description: "List devices visible to the browser via WebUSB.",
    },
    ToolEntry {
        slug: "afc",
        name: "afc",
        description: "Browse and transfer files over Apple File Conduit.",
    },
    ToolEntry {
        slug: "amfi",
        name: "amfi",
        description: "Interact with amfid",
    },
    ToolEntry {
        slug: "app_service",
        name: "app_service",
        description: "RemoteXPC app service: list / launch / uninstall / signal / fetch icons.",
    },
    ToolEntry {
        slug: "debug_proxy",
        name: "debug_proxy",
        description: "Interactive debugserver shell over RemoteXPC.",
    },
    ToolEntry {
        slug: "diagnostics",
        name: "diagnostics",
        description: "Diagnostics relay: IORegistry, MobileGestalt, power state.",
    },
    ToolEntry {
        slug: "diagnosticsservice",
        name: "diagnosticsservice",
        description: "Capture a sysdiagnose tarball over RemoteXPC.",
    },
    ToolEntry {
        slug: "energy_monitor",
        name: "energy_monitor",
        description: "Sample per-PID energy consumption.",
    },
    ToolEntry {
        slug: "graphics",
        name: "graphics",
        description: "Stream FPS and GPU memory samples.",
    },
    ToolEntry {
        slug: "ideviceinfo",
        name: "ideviceinfo",
        description: "Dump lockdown values from the connected device.",
    },
    ToolEntry {
        slug: "installation_proxy",
        name: "installation_proxy",
        description: "Install, upgrade, uninstall, and inspect apps.",
    },
    ToolEntry {
        slug: "location_simulation",
        name: "location_simulation",
        description: "Set or clear a simulated GPS location.",
    },
    ToolEntry {
        slug: "lockdown",
        name: "lockdown",
        description: "Get/set lockdown values, enter recovery mode.",
    },
    ToolEntry {
        slug: "misagent",
        name: "misagent",
        description: "List, install, or remove provisioning profiles.",
    },
    ToolEntry {
        slug: "mounter",
        name: "mounter",
        description: "List / lookup / unmount developer disk images.",
    },
    ToolEntry {
        slug: "network_monitor",
        name: "network_monitor",
        description: "Stream network interface / connection events.",
    },
    ToolEntry {
        slug: "notifications",
        name: "notifications",
        description: "Listen for app/memory notifications over RemoteXPC.",
    },
    ToolEntry {
        slug: "os_trace_relay",
        name: "os_trace_relay",
        description: "Structured os_log stream.",
    },
    ToolEntry {
        slug: "process_control",
        name: "process_control",
        description: "Launch an app via Instruments and disable its memory limit.",
    },
    ToolEntry {
        slug: "rppairing",
        name: "rppairing",
        description: "Inspect the RemoteXPC tunnel and pair iOS 17+ devices.",
    },
    ToolEntry {
        slug: "screenshot",
        name: "screenshot",
        description: "Capture and download a device screenshot.",
    },
    ToolEntry {
        slug: "syslog_relay",
        name: "syslog_relay",
        description: "Stream raw syslog lines.",
    },
];
