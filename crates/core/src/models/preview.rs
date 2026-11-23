//! Response preview classification and lightweight image header probing.

/// Supported image formats for the native response preview renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImagePreviewFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
    Svg,
    Bmp,
    Ico,
    Tiff,
}

impl ImagePreviewFormat {
    /// Uppercase display label for the image preview metadata bar.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::Webp => "WebP",
            Self::Gif => "GIF",
            Self::Svg => "SVG",
            Self::Bmp => "BMP",
            Self::Ico => "ICO",
            Self::Tiff => "TIFF",
        }
    }

    /// Preferred file extension (without leading dot) when saving a response.
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Webp => "webp",
            Self::Gif => "gif",
            Self::Svg => "svg",
            Self::Bmp => "bmp",
            Self::Ico => "ico",
            Self::Tiff => "tiff",
        }
    }

    /// Returns `true` for binary raster formats (everything except SVG).
    pub const fn is_raster(self) -> bool {
        !matches!(self, Self::Svg)
    }
}

/// Previewable response body classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponsePreviewKind {
    Image(ImagePreviewFormat),
}

impl ResponsePreviewKind {
    /// Suggested file extension (without leading dot) for fallback filenames.
    pub const fn fallback_extension(self) -> &'static str {
        match self {
            Self::Image(fmt) => fmt.extension(),
        }
    }
}

/// Classifies whether an HTTP response can be rendered in Preview mode.
///
/// Precedence:
/// 1. `Content-Type` response header (media type before `;`, case-insensitive).
/// 2. Magic-byte / SVG-prefix sniffing when `Content-Type` is absent or
///    `application/octet-stream`.
pub fn detect_preview_kind(
    headers: &[(String, String)],
    body: &[u8],
) -> Option<ResponsePreviewKind> {
    let content_type = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .map(|(_, v)| {
            v.split(';')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase()
        });

    if let Some(ref mime) = content_type {
        if let Some(fmt) = image_format_from_mime(mime) {
            return Some(ResponsePreviewKind::Image(fmt));
        }
        // Only fall through to magic-byte sniffing when the server sent a
        // generic/empty content type; never override explicit text/json/pdf.
        if !mime.is_empty() && mime != "application/octet-stream" {
            return None;
        }
    }

    sniff_image_format(body).map(ResponsePreviewKind::Image)
}

fn image_format_from_mime(mime: &str) -> Option<ImagePreviewFormat> {
    match mime {
        "image/png" => Some(ImagePreviewFormat::Png),
        "image/jpeg" | "image/jpg" => Some(ImagePreviewFormat::Jpeg),
        "image/webp" => Some(ImagePreviewFormat::Webp),
        "image/gif" => Some(ImagePreviewFormat::Gif),
        "image/svg+xml" => Some(ImagePreviewFormat::Svg),
        "image/bmp" | "image/x-ms-bmp" => Some(ImagePreviewFormat::Bmp),
        "image/x-icon" | "image/vnd.microsoft.icon" | "image/ico" => Some(ImagePreviewFormat::Ico),
        "image/tiff" => Some(ImagePreviewFormat::Tiff),
        _ => None,
    }
}

fn sniff_image_format(bytes: &[u8]) -> Option<ImagePreviewFormat> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(ImagePreviewFormat::Png);
    }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some(ImagePreviewFormat::Jpeg);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(ImagePreviewFormat::Gif);
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some(ImagePreviewFormat::Webp);
    }
    if bytes.len() >= 2 && &bytes[0..2] == b"BM" {
        return Some(ImagePreviewFormat::Bmp);
    }
    if bytes.len() >= 4 && bytes[0..4] == [0x00, 0x00, 0x01, 0x00] {
        return Some(ImagePreviewFormat::Ico);
    }
    if bytes.len() >= 4 && (&bytes[0..4] == b"II*\x00" || &bytes[0..4] == b"MM\x00*") {
        return Some(ImagePreviewFormat::Tiff);
    }
    if looks_like_svg(bytes) {
        return Some(ImagePreviewFormat::Svg);
    }
    None
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let prefix_len = bytes.len().min(512);
    let Ok(prefix) = std::str::from_utf8(&bytes[..prefix_len]) else {
        return false;
    };
    let trimmed = prefix.trim_start_matches('\u{feff}').trim_start();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("<svg") {
        return true;
    }
    if lower.starts_with("<?xml") && lower.contains("<svg") {
        return true;
    }
    false
}

/// Reads `(width, height)` in pixels directly from image headers without
/// decoding pixel buffers. Returns `None` for vector formats (SVG) or
/// truncated/unsupported headers.
pub fn probe_image_dimensions(format: ImagePreviewFormat, bytes: &[u8]) -> Option<(u32, u32)> {
    match format {
        ImagePreviewFormat::Png => probe_png(bytes),
        ImagePreviewFormat::Gif => probe_gif(bytes),
        ImagePreviewFormat::Bmp => probe_bmp(bytes),
        ImagePreviewFormat::Webp => probe_webp(bytes),
        ImagePreviewFormat::Jpeg => probe_jpeg(bytes),
        ImagePreviewFormat::Svg | ImagePreviewFormat::Ico | ImagePreviewFormat::Tiff => None,
    }
}

fn probe_png(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    (w > 0 && h > 0).then_some((w, h))
}

fn probe_gif(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 10 || !(bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return None;
    }
    let w = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
    let h = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
    (w > 0 && h > 0).then_some((w, h))
}

fn probe_bmp(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 26 || &bytes[0..2] != b"BM" {
        return None;
    }
    let w = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]).unsigned_abs();
    let h = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]).unsigned_abs();
    (w > 0 && h > 0).then_some((w, h))
}

fn probe_webp(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 30 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WEBP" {
        return None;
    }
    match &bytes[12..16] {
        b"VP8 " => {
            if bytes[23..26] != [0x9d, 0x01, 0x2a] {
                return None;
            }
            let w = (u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3fff) as u32;
            let h = (u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3fff) as u32;
            (w > 0 && h > 0).then_some((w, h))
        }
        b"VP8L" => {
            if bytes.len() < 25 || bytes[20] != 0x2f {
                return None;
            }
            let bits = u32::from_le_bytes([bytes[21], bytes[22], bytes[23], bytes[24]]);
            let w = (bits & 0x3fff) + 1;
            let h = ((bits >> 14) & 0x3fff) + 1;
            Some((w, h))
        }
        b"VP8X" => {
            let w = (bytes[24] as u32 | ((bytes[25] as u32) << 8) | ((bytes[26] as u32) << 16)) + 1;
            let h = (bytes[27] as u32 | ((bytes[28] as u32) << 8) | ((bytes[29] as u32) << 16)) + 1;
            Some((w, h))
        }
        _ => None,
    }
}

fn probe_jpeg(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2usize;
    while i + 1 < bytes.len() {
        // Skip fill 0xFF bytes before marker.
        if bytes[i] != 0xFF {
            return None;
        }
        while i < bytes.len() && bytes[i] == 0xFF {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        let marker = bytes[i];
        i += 1;

        // Standalone markers without length field (TEM, RST0..RST7, SOI, EOI).
        if marker == 0x01 || (0xD0..=0xD9).contains(&marker) {
            if marker == 0xD9 {
                return None;
            }
            continue;
        }

        if i + 2 > bytes.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes([bytes[i], bytes[i + 1]]) as usize;
        if seg_len < 2 || i + seg_len > bytes.len() {
            return None;
        }

        let is_sof = matches!(
            marker,
            0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF
        );
        if is_sof {
            if seg_len < 7 || i + 7 > bytes.len() {
                return None;
            }
            let h = u16::from_be_bytes([bytes[i + 3], bytes[i + 4]]) as u32;
            let w = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            return (w > 0 && h > 0).then_some((w, h));
        }

        // Stop scanning once entropy-coded image data begins.
        if marker == 0xDA {
            return None;
        }

        i += seg_len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ct(val: &str) -> Vec<(String, String)> {
        vec![("Content-Type".to_string(), val.to_string())]
    }

    #[test]
    fn detects_image_formats_from_content_type() {
        assert_eq!(
            detect_preview_kind(&ct("image/png"), &[]),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Png))
        );
        assert_eq!(
            detect_preview_kind(&ct("Image/JPEG; charset=binary"), &[]),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Jpeg))
        );
        assert_eq!(
            detect_preview_kind(&ct("image/webp"), &[]),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Webp))
        );
        assert_eq!(
            detect_preview_kind(&ct("image/gif"), &[]),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Gif))
        );
        assert_eq!(
            detect_preview_kind(&ct("image/svg+xml; charset=utf-8"), &[]),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Svg))
        );
        assert_eq!(
            detect_preview_kind(&ct("image/bmp"), &[]),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Bmp))
        );
        assert_eq!(
            detect_preview_kind(&ct("image/x-icon"), &[]),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Ico))
        );
        assert_eq!(
            detect_preview_kind(&ct("image/tiff"), &[]),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Tiff))
        );
    }

    #[test]
    fn non_image_content_types_return_none() {
        assert_eq!(detect_preview_kind(&ct("application/json"), b"{}"), None);
        assert_eq!(detect_preview_kind(&ct("text/plain"), b"hello"), None);
        assert_eq!(
            detect_preview_kind(&ct("application/pdf"), b"%PDF-1.4"),
            None
        );
        assert_eq!(
            detect_preview_kind(&ct("application/octet-stream"), b"random bytes"),
            None
        );
    }

    #[test]
    fn sniffs_magic_bytes_when_content_type_missing_or_octet_stream() {
        let png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        assert_eq!(
            detect_preview_kind(&[], png),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Png))
        );
        assert_eq!(
            detect_preview_kind(&ct("application/octet-stream"), png),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Png))
        );
        assert_eq!(
            detect_preview_kind(&[], &[0xFF, 0xD8, 0xFF, 0xE0]),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Jpeg))
        );
        assert_eq!(
            detect_preview_kind(&[], b"GIF89a\x01\x00\x01\x00"),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Gif))
        );
        assert_eq!(
            detect_preview_kind(&[], b"RIFF\x18\x00\x00\x00WEBP"),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Webp))
        );
        assert_eq!(
            detect_preview_kind(&[], b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Svg))
        );
        assert_eq!(
            detect_preview_kind(&[], b"<?xml version=\"1.0\"?><svg></svg>"),
            Some(ResponsePreviewKind::Image(ImagePreviewFormat::Svg))
        );
        // Explicit text/plain should not be sniffed as SVG.
        assert_eq!(detect_preview_kind(&ct("text/plain"), b"<svg></svg>"), None);
    }

    #[test]
    fn probes_png_gif_bmp_webp_jpeg_dimensions() {
        // PNG 640x480
        let mut png = Vec::from(*b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR");
        png.extend_from_slice(&640u32.to_be_bytes());
        png.extend_from_slice(&480u32.to_be_bytes());
        assert_eq!(
            probe_image_dimensions(ImagePreviewFormat::Png, &png),
            Some((640, 480))
        );

        // GIF 320x240
        let mut gif = Vec::from(*b"GIF89a");
        gif.extend_from_slice(&320u16.to_le_bytes());
        gif.extend_from_slice(&240u16.to_le_bytes());
        assert_eq!(
            probe_image_dimensions(ImagePreviewFormat::Gif, &gif),
            Some((320, 240))
        );

        // BMP 100x50 (top-down negative height)
        let mut bmp = vec![0u8; 26];
        bmp[0..2].copy_from_slice(b"BM");
        bmp[18..22].copy_from_slice(&100i32.to_le_bytes());
        bmp[22..26].copy_from_slice(&(-50i32).to_le_bytes());
        assert_eq!(
            probe_image_dimensions(ImagePreviewFormat::Bmp, &bmp),
            Some((100, 50))
        );

        // WebP VP8X 800x600
        let mut webp = vec![0u8; 30];
        webp[0..4].copy_from_slice(b"RIFF");
        webp[8..12].copy_from_slice(b"WEBP");
        webp[12..16].copy_from_slice(b"VP8X");
        let w_m1 = 799u32;
        let h_m1 = 599u32;
        webp[24] = (w_m1 & 0xff) as u8;
        webp[25] = ((w_m1 >> 8) & 0xff) as u8;
        webp[26] = ((w_m1 >> 16) & 0xff) as u8;
        webp[27] = (h_m1 & 0xff) as u8;
        webp[28] = ((h_m1 >> 8) & 0xff) as u8;
        webp[29] = ((h_m1 >> 16) & 0xff) as u8;
        assert_eq!(
            probe_image_dimensions(ImagePreviewFormat::Webp, &webp),
            Some((800, 600))
        );

        // JPEG SOF0 1920x1080
        let jpeg = [
            0xFF, 0xD8, // SOI
            0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00, // APP0 (len=4)
            0xFF, 0xC0, 0x00, 0x0B, 0x08, // SOF0, len=11, precision=8
            0x04, 0x38, // height = 1080
            0x07, 0x80, // width = 1920
            0x03, 0x01, 0x11, 0x00, // dummy component bytes
        ];
        assert_eq!(
            probe_image_dimensions(ImagePreviewFormat::Jpeg, &jpeg),
            Some((1920, 1080))
        );
    }

    #[test]
    fn malformed_headers_return_none_without_panicking() {
        for fmt in [
            ImagePreviewFormat::Png,
            ImagePreviewFormat::Jpeg,
            ImagePreviewFormat::Webp,
            ImagePreviewFormat::Gif,
            ImagePreviewFormat::Bmp,
            ImagePreviewFormat::Svg,
        ] {
            assert_eq!(probe_image_dimensions(fmt, &[]), None);
            assert_eq!(probe_image_dimensions(fmt, &[0xFF, 0xD8, 0xFF]), None);
            assert_eq!(probe_image_dimensions(fmt, b"garbage"), None);
        }
    }
}
