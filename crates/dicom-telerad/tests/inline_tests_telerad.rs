// Auto-extracted from /home/z/diccy/crates/dicom-telerad/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_telerad::*;

    #[test]
    fn bandwidth_classification() {
        // REQ-TELERAD-100: bandwidth level classification must be deterministic
        assert_eq!(BandwidthLevel::from_bps(500_000), BandwidthLevel::VeryLow);
        assert_eq!(BandwidthLevel::from_bps(3_000_000), BandwidthLevel::Low);
        assert_eq!(BandwidthLevel::from_bps(10_000_000), BandwidthLevel::Medium);
        assert_eq!(BandwidthLevel::from_bps(30_000_000), BandwidthLevel::High);
        assert_eq!(
            BandwidthLevel::from_bps(100_000_000),
            BandwidthLevel::VeryHigh
        );
    }

    #[test]
    fn quality_factor_increases_with_bandwidth() {
        let q_very_low = BandwidthLevel::VeryLow.quality_factor();
        let q_low = BandwidthLevel::Low.quality_factor();
        let q_medium = BandwidthLevel::Medium.quality_factor();
        let q_high = BandwidthLevel::High.quality_factor();
        let q_very_high = BandwidthLevel::VeryHigh.quality_factor();

        assert!(q_very_low < q_low);
        assert!(q_low < q_medium);
        assert!(q_medium < q_high);
        assert!(q_high < q_very_high);
    }

    #[test]
    fn resolution_scale_for_very_low_bandwidth() {
        assert_eq!(BandwidthLevel::VeryLow.resolution_scale(), 0.25);
        assert_eq!(BandwidthLevel::VeryHigh.resolution_scale(), 1.0);
    }

    #[test]
    fn streaming_config_from_bandwidth() {
        let config = StreamingConfig::from_bandwidth(BandwidthLevel::Medium);
        assert_eq!(config.quality_factor, 60.0);
        assert_eq!(config.resolution_scale, 0.75);
        assert_eq!(config.max_fps, 20);
        assert!(config.lossless_upgrade);
    }

    #[test]
    fn gateway_creation_default() {
        let gateway = TeleradGateway::new();
        assert_eq!(gateway.bandwidth(), BandwidthLevel::Medium);
        assert!(matches!(gateway.sync_status(), SyncStatus::Online));
    }

    #[test]
    fn gateway_bandwidth_adaptation() {
        let mut gateway = TeleradGateway::new();
        gateway.record_network_sample(NetworkSample {
            rtt_ms: 100.0,
            bandwidth_bps: 2_000_000,
            packet_loss_pct: 1.0,
            timestamp: 1,
        });
        assert_eq!(gateway.bandwidth(), BandwidthLevel::Low);
        assert_eq!(gateway.config().quality_factor, 30.0);
    }

    #[test]
    fn interaction_enqueue_dequeue_online() {
        let mut gateway = TeleradGateway::new();
        gateway.enqueue_interaction(InteractionEvent::Pan { dx: 10.0, dy: 20.0 });
        gateway.enqueue_interaction(InteractionEvent::Zoom {
            factor: 2.0,
            center: (100.0, 100.0),
        });

        let event1 = gateway.dequeue_interaction().unwrap();
        assert!(matches!(event1, InteractionEvent::Pan { .. }));
        let event2 = gateway.dequeue_interaction().unwrap();
        assert!(matches!(event2, InteractionEvent::Zoom { .. }));
        assert!(gateway.dequeue_interaction().is_none());
    }

    #[test]
    fn offline_mode_queues_operations() {
        let mut gateway = TeleradGateway::new();
        gateway.go_offline();

        gateway.enqueue_interaction(InteractionEvent::WindowLevel {
            center: 40.0,
            width: 400.0,
        });

        assert!(matches!(gateway.sync_status(), SyncStatus::Offline { .. }));
        assert_eq!(gateway.pending_offline_count(), 1);
    }

    #[test]
    fn offline_sync_recovery() {
        let mut gateway = TeleradGateway::new();
        gateway.go_offline();

        gateway.enqueue_interaction(InteractionEvent::Pan { dx: 5.0, dy: 5.0 });
        gateway.enqueue_interaction(InteractionEvent::Scroll { frame_index: 10 });

        assert_eq!(gateway.pending_offline_count(), 2);

        // Go online triggers sync
        gateway.go_online();
        assert!(matches!(gateway.sync_status(), SyncStatus::Syncing { .. }));

        // Sync one batch
        let synced = gateway.sync_next_batch(1);
        assert_eq!(synced, 1);
        assert_eq!(gateway.pending_offline_count(), 1);

        // Sync remaining
        let synced = gateway.sync_next_batch(10);
        assert_eq!(synced, 1);
        assert!(matches!(gateway.sync_status(), SyncStatus::Online));
    }

    #[test]
    fn progressive_frame_encoding() {
        let gateway = TeleradGateway::with_bandwidth(BandwidthLevel::Medium);
        let image_data = vec![42u8; 1024];

        let low_quality = gateway.encode_progressive_frame(&image_data, 512, 512, 0);
        let high_quality = gateway.encode_progressive_frame(&image_data, 512, 512, 3);

        // Higher quality layer should produce larger or equal output
        assert!(high_quality.len() >= low_quality.len());
    }

    #[test]
    fn interaction_event_size_estimates() {
        let pan = InteractionEvent::Pan { dx: 1.0, dy: 1.0 };
        let wl = InteractionEvent::WindowLevel {
            center: 0.0,
            width: 0.0,
        };
        assert!(pan.estimated_size() > 0);
        assert!(wl.estimated_size() > 0);
    }

    #[test]
    fn bytes_sent_tracked() {
        let mut gateway = TeleradGateway::new();
        gateway.enqueue_interaction(InteractionEvent::Pan { dx: 1.0, dy: 1.0 });
        assert!(gateway.bytes_sent() > 0);
    }

    #[test]
    fn average_rtt_with_no_samples() {
        let gateway = TeleradGateway::new();
        assert_eq!(gateway.average_rtt(), 0.0);
    }

    #[test]
    fn average_rtt_with_samples() {
        let mut gateway = TeleradGateway::new();
        gateway.record_network_sample(NetworkSample {
            rtt_ms: 50.0,
            bandwidth_bps: 10_000_000,
            packet_loss_pct: 0.0,
            timestamp: 1,
        });
        gateway.record_network_sample(NetworkSample {
            rtt_ms: 100.0,
            bandwidth_bps: 10_000_000,
            packet_loss_pct: 0.0,
            timestamp: 2,
        });
        assert_eq!(gateway.average_rtt(), 75.0);
    }

    #[test]
    fn network_history_trimming() {
        let mut gateway = TeleradGateway::new();
        for i in 0..200 {
            gateway.record_network_sample(NetworkSample {
                rtt_ms: i as f64,
                bandwidth_bps: 10_000_000,
                packet_loss_pct: 0.0,
                timestamp: i,
            });
        }
        // Should be trimmed to max_history (100)
        assert!(gateway.network_history_len() <= 100);
    }
