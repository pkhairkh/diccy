# Fuzz target inventory

This inventory lists each fuzz target and the crate boundary it exercises, per REQ-TEST-731.

## Targets

- `dicom_io_p10`: DICOM Part 10 parsing boundary (`crates/dicom-io`), including meta header parsing, transfer syntax switching, and dataset decoding (VR/VL, sequences, length handling).
- `dimse_pdu`: DIMSE UL PDU parsing and association state machine sequencing boundary (`crates/dicom-net`).
- `dimse_command`: DIMSE command set parsing boundary (`crates/dicom-dimse`).
- `dicomweb_request`: DICOMweb request parsing boundary (`crates/dicom-web`).
- `storage_index_metadata`: Metadata extraction boundary (`crates/dicom-index`) using datasets parsed via `crates/dicom-io`.
- `dicom_pixel_pipeline`: Pixel pipeline entrypoint boundary (`crates/dicom-pixel`) using datasets parsed via `crates/dicom-io`.
- `dicom_pixel_rle`: Pixel codec boundary for RLE Lossless decoding (`crates/dicom-pixel`).
- `dicom_pixel_jpeg_baseline`: Pixel codec boundary for JPEG Baseline decoding (`crates/dicom-pixel`).
- `dicom_pixel_jpegls`: Pixel codec boundary for JPEG-LS decoding (`crates/dicom-pixel`, feature `codec-jpegls`).
- `dicom_pixel_j2k`: Pixel codec boundary for JPEG 2000 decoding (`crates/dicom-pixel`, feature `codec-j2k`).
- `pack_enhanced_groups`: Enhanced CT/MR functional group parsing boundary (`crates/pack-enhanced`, feature `pack-enhanced`).
- `pack_gsps`: GSPS presentation state parsing boundary (`crates/pack-gsps`, feature `gsps`).
- `pack_gsps_graphics`: GSPS graphic annotation parsing boundary (`crates/pack-gsps`, feature `gsps`).
- `pack_seg`: Segmentation parsing boundary (`crates/pack-seg`, feature `pack-seg`).
- `pack_rt_dose`: RT Dose parsing boundary (`crates/pack-rt`, feature `pack-rt`).
- `pack_rt_structure`: RT Structure Set parsing boundary (`crates/pack-rt`, feature `pack-rt`).
- `pack_rt_plan`: RT Plan parsing boundary (`crates/pack-rt`, feature `pack-rt`).
- `pack_sr`: Structured report parsing boundary (`crates/pack-sr`, feature `pack-sr`).
- `pack_us`: Ultrasound measurement calibration boundary (`crates/pack-us`, feature `pack-us`).
- `pack_nm`: Nuclear medicine measurement calibration boundary (`crates/pack-nm`, feature `pack-nm`).
- `pack_xa`: XA/XRF measurement calibration boundary (`crates/pack-xa`, feature `pack-xa`).
- `viewer_core_volume_mpr`: Volume assembly and MPR request/reslice boundary (`crates/viewer-core`).
- `modality_pet_fusion`: PET/CT fusion precondition + resample/blend boundary (`crates/modality-pet`).
- `workflow_sr_store`: SR workflow snapshot parse/create boundary (`crates/dicom-workflow-server` + `crates/pack-sr`).

## Notes

- Codec-boundary fuzz targets are implemented alongside the decoders in `crates/dicom-pixel`.
