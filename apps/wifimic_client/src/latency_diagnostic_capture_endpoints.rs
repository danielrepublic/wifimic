use wasapi::{Device, DeviceEnumerator, Direction};

use super::CaptureError;

pub(super) fn select_device(
    enumerator: &DeviceEnumerator,
    expected: &str,
) -> Result<Device, CaptureError> {
    if expected.trim().is_empty() {
        return Err(CaptureError::InvalidEndpointName);
    }
    let collection = enumerator
        .get_device_collection(&Direction::Capture)
        .map_err(|source| CaptureError::Wasapi {
            operation: "enumerate active capture endpoints",
            source,
        })?;
    let count = collection
        .get_nbr_devices()
        .map_err(|source| CaptureError::Wasapi {
            operation: "count active capture endpoints",
            source,
        })?;
    let mut names = Vec::new();
    let mut devices = Vec::new();
    for index in 0..count {
        let device =
            collection
                .get_device_at_index(index)
                .map_err(|source| CaptureError::Wasapi {
                    operation: "read active capture endpoint",
                    source,
                })?;
        names.push(
            device
                .get_friendlyname()
                .map_err(|source| CaptureError::Wasapi {
                    operation: "read capture endpoint friendly name",
                    source,
                })?,
        );
        devices.push(device);
    }
    let selected = names
        .iter()
        .position(|name| name == expected)
        .ok_or_else(|| CaptureError::EndpointNotFound {
            expected: expected.to_owned(),
            available: names.clone(),
        })?;
    devices
        .into_iter()
        .nth(selected)
        .ok_or(CaptureError::EndpointNotFound {
            expected: expected.to_owned(),
            available: names,
        })
}
