// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{InspectionError, OutputDevice, OutputDiscovery};

struct Fixture(Result<Vec<OutputDevice>, InspectionError>);
impl OutputDiscovery for Fixture {
    fn output_devices(&self) -> Result<Vec<OutputDevice>, InspectionError> {
        self.0.clone()
    }
}

#[test]
fn consumers_distinguish_empty_failure_and_partial_inspection() {
    let failure = InspectionError {
        operation: "fixture",
        message: "unavailable".into(),
    };
    let empty: &dyn OutputDiscovery = &Fixture(Ok(vec![]));
    assert!(empty.output_devices().unwrap().is_empty());
    assert_eq!(
        Fixture(Err(failure.clone())).output_devices(),
        Err(failure.clone())
    );
    let devices = Fixture(Ok(vec![
        OutputDevice {
            ordinal: 0,
            name: Ok("duplicate names are permitted".into()),
            configurations: Err(failure.clone()),
            default_configuration: Err(failure.clone()),
        },
        OutputDevice {
            ordinal: 1,
            name: Err(failure.clone()),
            configurations: Ok(vec![]),
            default_configuration: Err(failure),
        },
    ]))
    .output_devices()
    .unwrap();
    assert_eq!(devices.len(), 2);
    assert!(devices[0].name.is_ok());
    assert!(devices[0].configurations.is_err());
    assert_eq!(devices[1].configurations, Ok(vec![]));
}
