use viewer_core::{sort_overlay_render_items, OverlayLayer, OverlayRenderItem};

#[test]
fn overlay_layers_follow_required_precedence() {
    // REQ-HI-159, REQ-UI-062, REQ-GSPS-302
    let mut items = vec![
        OverlayRenderItem {
            layer: OverlayLayer::Annotation,
            tie_break: 0,
            stable_id: "ann-1".to_string(),
        },
        OverlayRenderItem {
            layer: OverlayLayer::PixelData,
            tie_break: 0,
            stable_id: "px-1".to_string(),
        },
        OverlayRenderItem {
            layer: OverlayLayer::Segmentation,
            tie_break: 0,
            stable_id: "seg-1".to_string(),
        },
        OverlayRenderItem {
            layer: OverlayLayer::GspsGraphics,
            tie_break: 0,
            stable_id: "gsps-gfx-1".to_string(),
        },
        OverlayRenderItem {
            layer: OverlayLayer::GspsShutter,
            tie_break: 0,
            stable_id: "gsps-shutter-1".to_string(),
        },
    ];

    sort_overlay_render_items(&mut items);

    let order = items.into_iter().map(|item| item.layer).collect::<Vec<_>>();
    assert_eq!(
        order,
        vec![
            OverlayLayer::PixelData,
            OverlayLayer::GspsShutter,
            OverlayLayer::GspsGraphics,
            OverlayLayer::Segmentation,
            OverlayLayer::Annotation,
        ]
    );
}

#[test]
fn segmentation_tie_breaking_is_deterministic() {
    // REQ-HI-160, REQ-HI-161, REQ-UI-063
    let mut items = vec![
        OverlayRenderItem {
            layer: OverlayLayer::Segmentation,
            tie_break: 4,
            stable_id: "seg-b".to_string(),
        },
        OverlayRenderItem {
            layer: OverlayLayer::Segmentation,
            tie_break: 4,
            stable_id: "seg-a".to_string(),
        },
        OverlayRenderItem {
            layer: OverlayLayer::Segmentation,
            tie_break: 1,
            stable_id: "seg-z".to_string(),
        },
    ];

    sort_overlay_render_items(&mut items);

    let ids = items
        .into_iter()
        .map(|item| item.stable_id)
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["seg-z", "seg-a", "seg-b"]);
}

#[test]
fn sorting_same_overlay_set_multiple_times_is_stable() {
    // REQ-HI-159, REQ-HI-161, REQ-HI-244
    let input = vec![
        OverlayRenderItem {
            layer: OverlayLayer::Annotation,
            tie_break: 9,
            stable_id: "ann-9".to_string(),
        },
        OverlayRenderItem {
            layer: OverlayLayer::PixelData,
            tie_break: 1,
            stable_id: "px-1".to_string(),
        },
        OverlayRenderItem {
            layer: OverlayLayer::Segmentation,
            tie_break: 5,
            stable_id: "seg-2".to_string(),
        },
        OverlayRenderItem {
            layer: OverlayLayer::Segmentation,
            tie_break: 5,
            stable_id: "seg-1".to_string(),
        },
    ];

    let mut first = input.clone();
    let mut second = input;

    sort_overlay_render_items(&mut first);
    sort_overlay_render_items(&mut second);

    assert_eq!(first, second);
}
