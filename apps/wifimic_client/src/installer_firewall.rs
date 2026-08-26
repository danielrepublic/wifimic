//! Direct Windows Firewall COM access for the owned UDP rule.

#![cfg(windows)]

use std::net::Ipv4Addr;

use windows::core::{BSTR, GUID};
use windows::Win32::Foundation::VARIANT_BOOL;
use windows::Win32::NetworkManagement::WindowsFirewall::{
    INetFwPolicy2, INetFwRule, NET_FW_ACTION_ALLOW, NET_FW_RULE_DIR_IN,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};

use crate::installer::{FirewallSnapshot, InstallerError, FIREWALL_NAME, PEER_ADDRESS, UDP_PORT};

struct ComApartment;
impl ComApartment {
    fn initialize() -> Result<Self, InstallerError> {
        // SAFETY: This thread owns the COM apartment for this guard's lifetime.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
            .ok()
            .map_err(|error| InstallerError::Operation {
                operation: "firewall-com",
                message: error.to_string(),
            })?;
        Ok(Self)
    }
}
impl Drop for ComApartment {
    fn drop(&mut self) {
        /* SAFETY: initialization succeeded for this guard. */
        unsafe { CoUninitialize() };
    }
}

fn policy() -> Result<INetFwPolicy2, InstallerError> {
    // SAFETY: The caller initializes COM before constructing the policy object.
    unsafe {
        CoCreateInstance(
            &GUID::from_u128(0xe2b3c97f_6ae1_41ac_817a_f6f92166d7dd),
            None,
            CLSCTX_INPROC_SERVER,
        )
    }
    .map_err(|error| InstallerError::Operation {
        operation: "firewall-policy",
        message: error.to_string(),
    })
}

fn rule() -> Result<INetFwRule, InstallerError> {
    let _com = ComApartment::initialize()?;
    let policy = policy()?;
    // SAFETY: The policy interface is live and returns its owned rule collection.
    let rules = unsafe { policy.Rules() }.map_err(|error| InstallerError::Operation {
        operation: "firewall-rules",
        message: error.to_string(),
    })?;
    let name = BSTR::from(FIREWALL_NAME);
    // SAFETY: The fixed rule name is a valid BSTR and Item performs a read-only lookup.
    unsafe { rules.Item(&name) }.map_err(|error| InstallerError::Operation {
        operation: "firewall-rule",
        message: error.to_string(),
    })
}

/// Parses and normalizes only the two accepted peer representations.
pub fn normalize_peer(value: &str) -> Result<String, InstallerError> {
    let (address, prefix) = value
        .split_once('/')
        .map_or((value, None), |(address, prefix)| (address, Some(prefix)));
    let parsed = address
        .parse::<Ipv4Addr>()
        .map_err(|_| InstallerError::Operation {
            operation: "firewall-address",
            message: "remote address is not IPv4".to_owned(),
        })?;
    if prefix.is_some_and(|value| value != "32") || parsed != Ipv4Addr::new(192, 168, 0, 210) {
        return Err(InstallerError::Operation {
            operation: "firewall-address",
            message: "remote address is not the fixed peer".to_owned(),
        });
    }
    Ok(format!("{parsed}/32"))
}

/// Reads the owned rule and rejects duplicates or a conflicting contract.
pub fn snapshot() -> Result<Option<FirewallSnapshot>, InstallerError> {
    match rule() {
        Ok(rule) => {
            // SAFETY: All getters are invoked on a live COM rule object.
            let name = unsafe { rule.Name() }.map_err(|error| InstallerError::Operation {
                operation: "firewall-name",
                message: error.to_string(),
            })?;
            // SAFETY: The display label getter is read-only on the live object.
            let display = unsafe { rule.Description() }.unwrap_or_default();
            // SAFETY: The remote-address getter is read-only on the live object.
            let remote =
                unsafe { rule.RemoteAddresses() }.map_err(|error| InstallerError::Operation {
                    operation: "firewall-remote",
                    message: error.to_string(),
                })?;
            // SAFETY: The enabled getter is read-only on the live object.
            let enabled = unsafe { rule.Enabled() }
                .map_err(|error| InstallerError::Operation {
                    operation: "firewall-enabled",
                    message: error.to_string(),
                })?
                .as_bool();
            let normalized = normalize_peer(&remote.to_string())?;
            Ok(Some(FirewallSnapshot {
                name: name.to_string(),
                display_name: display.to_string(),
                remote_address: normalized,
                enabled,
            }))
        }
        Err(InstallerError::Operation {
            operation: "firewall-rule",
            ..
        }) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Creates the canonical inbound UDP rule without shelling out to `netsh`.
pub fn set() -> Result<(), InstallerError> {
    let _com = ComApartment::initialize()?;
    let policy = policy()?;
    // SAFETY: The firewall policy returns a live rule collection.
    let rules = unsafe { policy.Rules() }.map_err(|error| InstallerError::Operation {
        operation: "firewall-rules",
        message: error.to_string(),
    })?;
    let name = BSTR::from(FIREWALL_NAME);
    let _ = unsafe { rules.Remove(&name) };
    // SAFETY: The class factory creates a live firewall rule in this COM apartment.
    let rule: INetFwRule = unsafe {
        CoCreateInstance(
            &GUID::from_u128(0x2c5bc43e_3369_4c33_ab0c_be9469677af4),
            None,
            CLSCTX_INPROC_SERVER,
        )
    }
    .map_err(|error| InstallerError::Operation {
        operation: "firewall-create",
        message: error.to_string(),
    })?;
    let display = BSTR::from(FIREWALL_NAME);
    let remote = BSTR::from(PEER_ADDRESS);
    let local_port = BSTR::from(UDP_PORT);
    // SAFETY: Each setter receives a live rule and a BSTR valid for the duration of the call.
    unsafe {
        rule.SetName(&name)
            .and_then(|_| rule.SetDescription(&display))
            .and_then(|_| rule.SetProtocol(17))
            .and_then(|_| rule.SetLocalPorts(&local_port))
            .and_then(|_| rule.SetRemoteAddresses(&remote))
            .and_then(|_| rule.SetDirection(NET_FW_RULE_DIR_IN))
            .and_then(|_| rule.SetAction(NET_FW_ACTION_ALLOW))
            .and_then(|_| rule.SetEnabled(VARIANT_BOOL(-1)))
    }
    .map_err(|error| InstallerError::Operation {
        operation: "firewall-configure",
        message: error.to_string(),
    })?;
    // SAFETY: The fully-configured rule is added to the live policy collection.
    unsafe { rules.Add(&rule) }.map_err(|error| InstallerError::Operation {
        operation: "firewall-add",
        message: error.to_string(),
    })
}

/// Removes the owned rule.
pub fn remove() -> Result<(), InstallerError> {
    let _com = ComApartment::initialize()?;
    let policy = policy()?;
    // SAFETY: The policy interface returns a live collection for this read-only/remove operation.
    let rules = unsafe { policy.Rules() }.map_err(|error| InstallerError::Operation {
        operation: "firewall-rules",
        message: error.to_string(),
    })?;
    let name = BSTR::from(FIREWALL_NAME);
    // SAFETY: The fixed owned rule name is a valid BSTR.
    unsafe { rules.Remove(&name) }.map_err(|error| InstallerError::Operation {
        operation: "firewall-remove",
        message: error.to_string(),
    })
}
