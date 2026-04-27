// Auto-extracted from /home/z/diccy/crates/dicom-encapsulate/src/lib.rs
// S13-T8: Move inline tests to tests/ directories


    use dicom_encapsulate::*;
    use dicom_core::Tag;

    #[test]
    fn mime_type_strings() {
        assert_eq!(EncapsulatedMimeType::Pdf.mime_type(), "application/pdf");
        assert_eq!(EncapsulatedMimeType::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(EncapsulatedMimeType::Mp4.mime_type(), "video/mp4");
        assert_eq!(EncapsulatedMimeType::Cda.mime_type(), "text/xml");
    }

    #[test]
    fn sop_class_uid_mapping() {
        assert_eq!(
            EncapsulatedMimeType::Pdf.sop_class_uid(),
            "1.2.840.10008.5.1.4.1.1.104.1"
        );
        assert_eq!(
            EncapsulatedMimeType::Jpeg.sop_class_uid(),
            "1.2.840.10008.5.1.4.1.1.7"
        );
        assert_eq!(
            EncapsulatedMimeType::Mp4.sop_class_uid(),
            "1.2.840.10008.5.1.4.1.1.77.1.1.1"
        );
        assert_eq!(
            EncapsulatedMimeType::Cda.sop_class_uid(),
            "1.2.840.10008.5.1.4.1.1.104.2"
        );
    }

    #[test]
    fn detect_pdf_from_magic_bytes() {
        let pdf_data = b"%PDF-1.7 rest of document...";
        assert_eq!(
            EncapsulatedMimeType::from_magic_bytes(pdf_data),
            Some(EncapsulatedMimeType::Pdf)
        );
    }

    #[test]
    fn detect_jpeg_from_magic_bytes() {
        let jpeg_data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        assert_eq!(
            EncapsulatedMimeType::from_magic_bytes(&jpeg_data),
            Some(EncapsulatedMimeType::Jpeg)
        );
    }

    #[test]
    fn detect_png_from_magic_bytes() {
        let png_data = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A];
        assert_eq!(
            EncapsulatedMimeType::from_magic_bytes(&png_data),
            Some(EncapsulatedMimeType::Png)
        );
    }

    #[test]
    fn detect_mp4_from_magic_bytes() {
        let mut mp4_data = vec![0u8; 12];
        mp4_data[4..8].copy_from_slice(b"ftyp");
        assert_eq!(
            EncapsulatedMimeType::from_magic_bytes(&mp4_data),
            Some(EncapsulatedMimeType::Mp4)
        );
    }

    #[test]
    fn detect_cda_from_content() {
        let cda_data = b"<?xml version=\"1.0\"?><ClinicalDocument xmlns=\"urn:hl7-org:v3\"></ClinicalDocument>";
        assert_eq!(
            EncapsulatedMimeType::from_magic_bytes(cda_data),
            Some(EncapsulatedMimeType::Cda)
        );
    }

    #[test]
    fn unknown_magic_bytes() {
        let unknown = b"random data that is not any known format";
        assert_eq!(EncapsulatedMimeType::from_magic_bytes(unknown), None);
    }

    #[test]
    fn encapsulation_request_validation() {
        let valid = EncapsulateRequest::new(
            "P001",
            "Doe^John",
            "1.2.3",
            "4.5.6",
            "7.8.9",
            EncapsulatedMimeType::Pdf,
            vec![1, 2, 3],
        );
        assert!(valid.validate().is_ok());

        let empty_patient = EncapsulateRequest::new(
            "",
            "Doe^John",
            "1.2.3",
            "4.5.6",
            "7.8.9",
            EncapsulatedMimeType::Pdf,
            vec![1, 2, 3],
        );
        assert!(empty_patient.validate().is_err());

        let empty_data = EncapsulateRequest::new(
            "P001",
            "Doe^John",
            "1.2.3",
            "4.5.6",
            "7.8.9",
            EncapsulatedMimeType::Pdf,
            vec![],
        );
        assert!(empty_data.validate().is_err());
    }

    #[test]
    fn encapsulate_pdf_creates_valid_dataset() {
        let pdf_data = b"%PDF-1.7 fake pdf content";
        let ds = encapsulate_pdf(
            pdf_data,
            "P001",
            "Doe^John",
            "1.2.3",
            "4.5.6",
            "7.8.9",
            "Lab Report",
        )
        .unwrap();

        assert_eq!(
            ds.get_uid(Tag(0x0008, 0x0016)),
            Some("1.2.840.10008.5.1.4.1.1.104.1")
        );
        assert_eq!(ds.get_uid(Tag(0x0020, 0x000D)), Some("1.2.3"));
    }

    #[test]
    fn encapsulate_image_creates_secondary_capture() {
        let jpeg_data = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10].to_vec();
        let ds = encapsulate_image(
            &jpeg_data,
            EncapsulatedMimeType::Jpeg,
            "P001",
            "Doe^John",
            "1.2.3",
            "4.5.6",
            "7.8.9",
        )
        .unwrap();

        assert_eq!(
            ds.get_uid(Tag(0x0008, 0x0016)),
            Some("1.2.840.10008.5.1.4.1.1.7")
        );
    }

    #[test]
    fn encapsulate_video_rejects_non_video() {
        let data = vec![1, 2, 3];
        let result = encapsulate_video(
            &data,
            EncapsulatedMimeType::Pdf,
            "P001",
            "Doe^John",
            "1.2.3",
            "4.5.6",
            "7.8.9",
        );
        assert!(result.is_err());
    }

    #[test]
    fn encapsulate_cda_creates_dataset() {
        let cda_data = b"<?xml version=\"1.0\"?><ClinicalDocument></ClinicalDocument>";
        let ds = encapsulate_cda(cda_data, "P001", "Doe^John", "1.2.3", "4.5.6", "7.8.9").unwrap();

        assert_eq!(
            ds.get_uid(Tag(0x0008, 0x0016)),
            Some("1.2.840.10008.5.1.4.1.1.104.2")
        );
    }
