use super::*;

#[test]
fn test_svg_response_keeps_xml_source_and_builds_preview_image() {
    let svg = b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\"></svg>";
    let rec = Arc::new(HttpResponseRecord {
        status_code: 200,
        status_text: "OK".to_string(),
        size_bytes: svg.len(),
        headers: vec![("content-type".to_string(), "image/svg+xml".to_string())],
        body: svg.to_vec(),
        ..Default::default()
    });

    let fmt = build_cached_format(&rec);
    assert!(fmt.is_xml);
    assert!(fmt.preview_image.is_some());
    assert!(fmt.raw_text.contains("<svg"));
}

#[test]
fn test_image_zoom_mode_steps_and_bounds() {
    assert_eq!(ImageZoomMode::Fit.label(), "Fit");
    assert_eq!(ImageZoomMode::Fit.zoom_in(), ImageZoomMode::Scale(150));
    assert_eq!(ImageZoomMode::Fit.zoom_out(), ImageZoomMode::Scale(75));
    assert_eq!(
        ImageZoomMode::Scale(400).zoom_in(),
        ImageZoomMode::Scale(400)
    );
    assert_eq!(
        ImageZoomMode::Scale(25).zoom_out(),
        ImageZoomMode::Scale(25)
    );
    assert_eq!(ImageZoomMode::Scale(100).label(), "100%");
}

#[test]
fn test_large_binary_response_uses_placeholder_without_decoding() {
    let body = vec![0x80u8; 4 * 1024 * 1024]; // 4 MB non-UTF-8 binary payload
    let rec = Arc::new(HttpResponseRecord {
        id: "resp-bin".to_string(),
        request_id: "req-bin".to_string(),
        status_code: 200,
        status_text: "OK".to_string(),
        duration_ms: 100,
        size_bytes: body.len(),
        headers: vec![(
            "Content-Type".to_string(),
            "application/octet-stream".to_string(),
        )],
        cookies: vec![],
        body,
        timing: Default::default(),
        request_sent: None,
        test_results: vec![],
        console_logs: vec![],
        created_at: chrono::Utc::now(),
    });

    let fmt = build_cached_format(&rec);
    assert!(fmt.is_binary);
    assert!(!fmt.is_truncated);
    assert!(!fmt.is_json);
    assert!(fmt.raw_text.contains("Binary response"));
    assert!(fmt.raw_text.contains("Save to File"));
    assert!(fmt.raw_text.len() < 256);
    assert!(Arc::ptr_eq(&fmt.raw_text, &fmt.pretty_text));
}

#[test]
fn test_nul_byte_payload_detected_as_binary_even_without_content_type() {
    let mut body = vec![b'A'; 1024];
    body[512] = 0;
    let rec = Arc::new(HttpResponseRecord {
        status_code: 200,
        status_text: "OK".to_string(),
        size_bytes: body.len(),
        headers: vec![],
        body,
        ..Default::default()
    });

    let fmt = build_cached_format(&rec);
    assert!(fmt.is_binary);
    assert!(fmt.raw_text.contains("Binary response"));
}

#[test]
fn test_oversized_text_response_bounds_preview_and_shares_arcs() {
    let body = "hello world\n".repeat(300_000); // ~3.6 MB text
    let rec = Arc::new(HttpResponseRecord {
        status_code: 200,
        status_text: "OK".to_string(),
        size_bytes: body.len(),
        headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
        body: body.into_bytes(),
        ..Default::default()
    });

    let fmt = build_cached_format(&rec);
    assert!(!fmt.is_binary);
    assert!(fmt.is_truncated);
    assert!(fmt.raw_text.len() <= MAX_OVERSIZED_PREVIEW_BYTES);
    assert!(Arc::ptr_eq(&fmt.raw_text, &fmt.pretty_text));
    assert!(Arc::ptr_eq(&fmt.raw_lines, &fmt.pretty_lines));
    assert!(fmt
        .raw_lines
        .last()
        .is_some_and(|l| l.contains("truncated") && l.contains("Save to File")));
}

#[test]
fn test_decode_bounded_utf8_prefix_handles_multibyte_boundary() {
    // 'é' is 2 bytes (0xC3, 0xA9); slicing at byte 5 splits the 3rd 'é'.
    let text = "éééé";
    let decoded = decode_bounded_utf8_prefix(text.as_bytes(), 5);
    assert_eq!(decoded, "éé");
}
