use crate::error::EsrError;
use base64::Engine;

pub fn deflate(input: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec(input, 6)
}

pub fn inflate(input: &[u8]) -> Result<Vec<u8>, EsrError> {
    Ok(miniz_oxide::inflate::decompress_to_vec(input)?)
}

pub fn base64url_encode(input: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(input)
}

pub fn base64url_decode(input: &str) -> Result<Vec<u8>, EsrError> {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(input)
        .map_err(|e| EsrError::InvalidUri(format!("base64: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deflate_roundtrip() {
        let data = b"hello world, this is a test of the deflate roundtrip path";
        let compressed = deflate(data);
        let decompressed = inflate(&compressed).unwrap();
        assert_eq!(&decompressed[..], data);
    }

    #[test]
    fn base64url_roundtrip() {
        let data: &[u8] = &[0xde, 0xad, 0xbe, 0xef, 0x00, 0xff];
        let encoded = base64url_encode(data);
        assert!(!encoded.contains('='), "expected unpadded base64url");
        let decoded = base64url_decode(&encoded).unwrap();
        assert_eq!(&decoded[..], data);
    }

    #[test]
    fn pako_fixture_decompresses() {
        let compressed = hex::decode("2b492d2e0100").unwrap();
        let decompressed = inflate(&compressed).unwrap();
        assert_eq!(&decompressed[..], b"test");
    }
}
