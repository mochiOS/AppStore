pub const MAX_INSPECTION_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageInfo {
    pub media_type: &'static str,
    pub width: u32,
    pub height: u32,
}

pub fn inspect(bytes: &[u8]) -> Option<ImageInfo> {
    if bytes.len() >= 24 && bytes[..8] == [137, 80, 78, 71, 13, 10, 26, 10] {
        return Some(ImageInfo {
            media_type: "image/png",
            width: u32::from_be_bytes(bytes[16..20].try_into().ok()?),
            height: u32::from_be_bytes(bytes[20..24].try_into().ok()?),
        });
    }
    inspect_jpeg(bytes)
}

fn inspect_jpeg(bytes: &[u8]) -> Option<ImageInfo> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        return None;
    }
    let mut offset = 2;
    while offset + 4 <= bytes.len() {
        while offset < bytes.len() && bytes[offset] == 0xff {
            offset += 1;
        }
        let marker = *bytes.get(offset)?;
        offset += 1;
        if matches!(marker, 0xd8 | 0xd9) {
            continue;
        }
        if marker == 0xda {
            return None;
        }
        let length = u16::from_be_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?) as usize;
        if length < 2 || offset + length > bytes.len() {
            return None;
        }
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            let height = u16::from_be_bytes(bytes.get(offset + 3..offset + 5)?.try_into().ok()?);
            let width = u16::from_be_bytes(bytes.get(offset + 5..offset + 7)?.try_into().ok()?);
            return Some(ImageInfo {
                media_type: "image/jpeg",
                width: width.into(),
                height: height.into(),
            });
        }
        offset += length;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_png_dimensions_from_ihdr() {
        let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82];
        png.extend_from_slice(&512_u32.to_be_bytes());
        png.extend_from_slice(&512_u32.to_be_bytes());
        assert_eq!(
            inspect(&png),
            Some(ImageInfo {
                media_type: "image/png",
                width: 512,
                height: 512
            })
        );
    }

    #[test]
    fn reads_jpeg_dimensions_from_start_of_frame() {
        let jpeg = [0xff, 0xd8, 0xff, 0xc0, 0, 11, 8, 2, 0, 1, 0, 3, 1, 0, 0];
        assert_eq!(
            inspect(&jpeg),
            Some(ImageInfo {
                media_type: "image/jpeg",
                width: 256,
                height: 512
            })
        );
    }

    #[test]
    fn rejects_unrecognized_and_truncated_images() {
        assert_eq!(inspect(b"not an image"), None);
        assert_eq!(inspect(&[0xff, 0xd8, 0xff, 0xc0, 0, 11]), None);
    }
}
