use super::*;

#[test]
fn test_image_response_auto_switches_to_preview_and_falls_back_on_json() {
    let mut state = ResponseState::new();
    let mut png = Vec::from(*b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR");
    png.extend_from_slice(&320u32.to_be_bytes());
    png.extend_from_slice(&240u32.to_be_bytes());
    let img_rec = Arc::new(HttpResponseRecord {
        status_code: 200,
        status_text: "OK".to_string(),
        size_bytes: png.len(),
        headers: vec![("content-type".to_string(), "image/png".to_string())],
        body: png,
        ..Default::default()
    });

    state.current_response = Some(img_rec);
    state.view.image_zoom = ImageZoomMode::Scale(200);
    state.refresh_formatted_body();
    state.reconcile_view_mode_for_response();

    assert_eq!(state.view.mode, ResponseViewMode::Preview);
    assert_eq!(state.view.image_zoom, ImageZoomMode::Fit);
    let fmt = state.view.formatted_body.as_ref().unwrap();
    assert_eq!(
        fmt.preview_kind,
        Some(zest_core::models::ResponsePreviewKind::Image(
            zest_core::models::ImagePreviewFormat::Png
        ))
    );
    assert!(fmt.preview_image.is_some());
    assert_eq!(fmt.image_dimensions, Some((320, 240)));
    assert!(fmt.display_text.contains("Binary PNG image"));

    // Switching to a JSON response must return to Pretty mode.
    let json_rec = Arc::new(HttpResponseRecord {
        status_code: 200,
        status_text: "OK".to_string(),
        size_bytes: 7,
        headers: vec![("content-type".to_string(), "application/json".to_string())],
        body: b"{\"a\":1}".to_vec(),
        ..Default::default()
    });
    state.current_response = Some(json_rec);
    state.refresh_formatted_body();
    state.reconcile_view_mode_for_response();
    assert_eq!(state.view.mode, ResponseViewMode::Pretty);
    assert_eq!(
        state.view.formatted_body.as_ref().unwrap().preview_kind,
        None
    );
}
