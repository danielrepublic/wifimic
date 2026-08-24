use wasapi::{Device, DeviceEnumerator, Direction};

use super::super::{select_endpoint_index, RenderError};

pub(super) struct ComApartment;

impl ComApartment {
    pub(super) fn new() -> Result<Self, RenderError> {
        let result = wasapi::initialize_mta();
        if result.0 < 0 {
            return Err(RenderError::ComInitialization { hresult: result.0 });
        }
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        wasapi::deinitialize();
    }
}

pub(super) fn select_device(
    enumerator: &DeviceEnumerator,
    expected: &str,
) -> Result<Device, RenderError> {
    let collection = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|source| RenderError::Wasapi {
            operation: "enumerate active render endpoints",
            source,
        })?;
    let count = collection
        .get_nbr_devices()
        .map_err(|source| RenderError::Wasapi {
            operation: "count active render endpoints",
            source,
        })?;
    let mut names = Vec::new();
    let mut devices = Vec::new();
    for index in 0..count {
        let device =
            collection
                .get_device_at_index(index)
                .map_err(|source| RenderError::Wasapi {
                    operation: "read active render endpoint",
                    source,
                })?;
        names.push(
            device
                .get_friendlyname()
                .map_err(|source| RenderError::Wasapi {
                    operation: "read render endpoint friendly name",
                    source,
                })?,
        );
        devices.push(device);
    }
    let name_refs = names.iter().map(String::as_str).collect::<Vec<_>>();
    let selected = select_endpoint_index(expected, &name_refs)?;
    devices
        .into_iter()
        .nth(selected)
        .ok_or_else(|| RenderError::EndpointNotFound {
            expected: expected.to_owned(),
            available: names,
        })
}

/// Enumerates active render endpoint friendly names at startup.
pub fn enumerate_render_endpoints() -> Result<Vec<String>, RenderError> {
    let _com = ComApartment::new()?;
    let enumerator = DeviceEnumerator::new().map_err(|source| RenderError::Wasapi {
        operation: "create render endpoint enumerator",
        source,
    })?;
    let collection = enumerator
        .get_device_collection(&Direction::Render)
        .map_err(|source| RenderError::Wasapi {
            operation: "enumerate active render endpoints",
            source,
        })?;
    let count = collection
        .get_nbr_devices()
        .map_err(|source| RenderError::Wasapi {
            operation: "count active render endpoints",
            source,
        })?;
    (0..count)
        .map(|index| {
            collection
                .get_device_at_index(index)
                .map_err(|source| RenderError::Wasapi {
                    operation: "read active render endpoint",
                    source,
                })?
                .get_friendlyname()
                .map_err(|source| RenderError::Wasapi {
                    operation: "read render endpoint friendly name",
                    source,
                })
        })
        .collect()
}
