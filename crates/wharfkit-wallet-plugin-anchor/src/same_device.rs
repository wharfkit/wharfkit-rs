//! iOS same-device info keys shared by the login and sign flows.

use wharfkit_signing_request::SigningRequest;

/// Marks `request` as iOS same-device so Anchor returns to the calling app instead of staying foregrounded.
pub(crate) fn apply_ios_same_device_info(request: &mut SigningRequest, return_path: Option<&str>) {
    // ABI-packed bool: a single 0x01 byte.
    request.set_info_bytes("same_device", vec![0x01]);
    if let Some(rp) = return_path {
        let _ = request.set_info_key("return_path", rp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use antelope::chain::checksum::Checksum256;
    use wharfkit_signing_request::{EsrOptions, SigningRequestCreateArgs};

    fn empty_request() -> SigningRequest {
        SigningRequest::create(
            SigningRequestCreateArgs {
                chain_id: Checksum256::default(),
                actions: vec![],
                callback: None,
                expiration: None,
            },
            &EsrOptions::offline(),
        )
        .expect("create request")
    }

    #[test]
    fn apply_ios_same_device_info_sets_same_device_and_return_path() {
        let mut request = empty_request();
        apply_ios_same_device_info(&mut request, Some("myapp://callback"));
        assert!(request.info.iter().any(|kv| kv.key == "same_device"));
        assert!(request.info.iter().any(|kv| kv.key == "return_path"));
    }

    #[test]
    fn apply_ios_same_device_info_omits_return_path_when_absent() {
        let mut request = empty_request();
        apply_ios_same_device_info(&mut request, None);
        assert!(request.info.iter().any(|kv| kv.key == "same_device"));
        assert!(!request.info.iter().any(|kv| kv.key == "return_path"));
    }
}
