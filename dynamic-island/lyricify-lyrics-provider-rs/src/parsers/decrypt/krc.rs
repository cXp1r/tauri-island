pub fn krc_decrypt(encoded: &str) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
    use flate2::read::{DeflateDecoder, ZlibDecoder};
    use std::io::Read;

    const KEY: &[u8] = &[
        0x40, 0x47, 0x61, 0x77, 0x5e, 0x32, 0x74, 0x47,
        0x51, 0x36, 0x31, 0x2d, 0xce, 0xd2, 0x6e, 0x69,
    ];

    let clean: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
    let decoded = B64.decode(&clean)
        .map_err(|e| format!("Decryptor: base64 decode failed (len={}): {}", clean.len(), e))?;
    if decoded.len() <= 4 {
        return Err(format!("Decryptor: decoded too short: {} bytes", decoded.len()));
    }
    let mut data = decoded[4..].to_vec();
    for (i, byte) in data.iter_mut().enumerate() {
        *byte ^= KEY[i % KEY.len()];
    }
    let head4: Vec<String> = data[..4.min(data.len())].iter().map(|b| format!("{:02x}", b)).collect();
    let inflated = {
        let mut out = Vec::new();
        if ZlibDecoder::new(&data[..]).read_to_end(&mut out).is_ok() && !out.is_empty() {
            out
        } else {
            let mut out2 = Vec::new();
            DeflateDecoder::new(&data[..]).read_to_end(&mut out2)
                .map_err(|e| format!("Decryptor: inflate failed (xor_head=[{}]): {}", head4.join(","), e))?;
            if out2.is_empty() {
                return Err(format!("Decryptor: inflate produced empty output (xor_head=[{}])", head4.join(",")));
            }
            out2
        }
    };
    let skip = if inflated.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 1 };
    if inflated.len() <= skip {
        return Err(format!("Decryptor: inflated too short after skip({}): {} bytes", skip, inflated.len()));
    }
    String::from_utf8(inflated[skip..].to_vec())
        .map_err(|e| format!("Decryptor: utf8 decode failed: {}", e))
}
