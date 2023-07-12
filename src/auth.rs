use github_device_flow::authorize;
use github_device_flow::{Credential, DeviceFlowError};

pub fn device_auth() -> Result<Credential, DeviceFlowError> {
  authorize(
    "Iv1.b507a08c87ecfe98".to_string(),
    None
  )
}
