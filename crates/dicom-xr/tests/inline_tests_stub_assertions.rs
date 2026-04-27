// Auto-extracted from /home/z/diccy/crates/dicom-xr/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_xr::*;

    #[test]
    fn xr_renderer_stub_returns_error() {
        let renderer = XrRenderer::new();
        let config = XrRenderConfig::default();
        let session = XrViewerSession::new(config, "1.2.3").expect("session");
        let result = renderer.render_frame(Eye::Left, &session);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("stub"),
            "error should mention stub: {err}"
        );
    }

    #[test]
    fn ar_overlay_engine_stub_returns_error() {
        let engine = ArOverlayEngine::new();
        let session = ArOverlaySession::new(ArPlatform::HoloLens2, "1.2.3");
        let result = engine.render_overlay(&session);
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.to_lowercase().contains("stub"),
            "error should mention stub: {err}"
        );
    }

    #[test]
    fn xr_and_ar_stubs_are_not_initialized_by_default() {
        assert!(!XrRenderer::new().initialized);
        assert!(!ArOverlayEngine::new().initialized);
    }
