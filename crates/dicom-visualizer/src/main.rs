#![deny(missing_docs)]

//! Multi-file DICOM preview exporter.
//!
//! The binary decodes first frames from a folder of DICOM Part 10 files and writes:
//! - PNG previews,
//! - a `manifest.csv` with success/error rows,
//! - an optional `index.html` gallery,
//! - a `manifest.integrity.json` metadata sidecar with deterministic hashes.

use dicom_core::{Error as CoreError, ErrorKind, Limits};
use dicom_io::{BytesSource, P10Reader};
use dicom_pixel::{DisplayFrame, PixelFormat, PixelPipeline, PixelPipelineConfig};
use dicom_visualizer::{deterministic_measurement_ids, extract_export_context};
use image::{ImageBuffer, ImageFormat, Rgba};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_LIMIT: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    input_dir: PathBuf,
    output_dir: PathBuf,
    max_files: usize,
    recursive: bool,
    write_index: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExportRecord {
    source_path: String,
    output_path: Option<String>,
    status: &'static str,
    detail: String,
    width: Option<u32>,
    height: Option<u32>,
    study_uid: Option<String>,
    series_uid: Option<String>,
    sop_instance_uid: Option<String>,
    frame_index: Option<u32>,
    measurement_ids: Vec<String>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let options = match parse_args(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(2);
        }
    };
    println!(
        "dicom-visualizer config: recursive={} max_files={} write_index={}",
        options.recursive, options.max_files, options.write_index
    );

    if let Err(err) = fs::create_dir_all(&options.output_dir) {
        eprintln!("failed to create output directory: {err}");
        std::process::exit(1);
    }

    let files = match collect_input_files(&options.input_dir, options.recursive) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("failed to collect input files: {err}");
            std::process::exit(1);
        }
    };

    let pipeline = PixelPipeline::new(PixelPipelineConfig::default());
    let limits = Limits::default();
    let mut records = Vec::new();
    let mut exported = 0usize;
    let mut failed = 0usize;

    for (index, file_path) in files.iter().take(options.max_files).enumerate() {
        let relative = file_path
            .strip_prefix(&options.input_dir)
            .unwrap_or(file_path);
        let file_label = relative.to_string_lossy().to_string();
        match export_preview(
            file_path,
            relative,
            &options.output_dir,
            index,
            &limits,
            &pipeline,
        ) {
            Ok(record) => {
                exported += 1;
                records.push(record);
            }
            Err(detail) => {
                failed += 1;
                records.push(ExportRecord {
                    source_path: file_label,
                    output_path: None,
                    status: "error",
                    detail,
                    width: None,
                    height: None,
                    study_uid: None,
                    series_uid: None,
                    sop_instance_uid: None,
                    frame_index: None,
                    measurement_ids: Vec::new(),
                });
            }
        }
    }

    let manifest_path = options.output_dir.join("manifest.csv");
    let manifest_bytes = match write_manifest(&manifest_path, &records) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("failed to write manifest: {err}");
            std::process::exit(1);
        }
    };

    let mut index_bytes = None;
    if options.write_index {
        let index_path = options.output_dir.join("index.html");
        let bytes = match write_index_html(&index_path, &records) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("failed to write index: {err}");
                std::process::exit(1);
            }
        };
        index_bytes = Some(bytes);
    }

    let integrity_path = options.output_dir.join("manifest.integrity.json");
    if let Err(err) = write_integrity_metadata(
        &integrity_path,
        &manifest_path,
        &manifest_bytes,
        index_bytes.as_deref(),
        &records,
    ) {
        eprintln!("failed to write integrity metadata: {err}");
        std::process::exit(1);
    }

    println!(
        "dicom-visualizer complete: exported={exported} failed={failed} manifest={} integrity={}",
        manifest_path.display(),
        integrity_path.display()
    );
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    if args.len() < 3 {
        return Err(
            "usage: dicom-visualizer <input_dir> <output_dir> [--limit N] [--no-recursive] [--no-index]"
                .to_string(),
        );
    }
    let mut options = Options {
        input_dir: PathBuf::from(&args[1]),
        output_dir: PathBuf::from(&args[2]),
        max_files: DEFAULT_LIMIT,
        recursive: true,
        write_index: true,
    };

    let mut index = 3usize;
    while index < args.len() {
        match args[index].as_str() {
            "--limit" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--limit requires a value".to_string())?;
                options.max_files = value
                    .parse::<usize>()
                    .map_err(|_| "--limit must be a positive integer".to_string())?;
                index += 2;
            }
            "--no-recursive" => {
                options.recursive = false;
                index += 1;
            }
            "--no-index" => {
                options.write_index = false;
                index += 1;
            }
            other => return Err(format!("unknown option: {other}")),
        }
    }
    if options.max_files == 0 {
        return Err("--limit must be greater than 0".to_string());
    }
    Ok(options)
}

fn collect_input_files(root: &Path, recursive: bool) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if recursive {
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.is_file() && looks_like_dicom_candidate(&path) {
                    files.push(path);
                }
            }
        }
    } else {
        for entry in fs::read_dir(root)? {
            let path = entry?.path();
            if path.is_file() && looks_like_dicom_candidate(&path) {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn looks_like_dicom_candidate(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) => {
            let lower = ext.to_ascii_lowercase();
            lower == "dcm" || lower == "dicom"
        }
        None => true,
    }
}

fn export_preview(
    file_path: &Path,
    relative_path: &Path,
    output_dir: &Path,
    index: usize,
    limits: &Limits,
    pipeline: &PixelPipeline,
) -> Result<ExportRecord, String> {
    let bytes = fs::read(file_path).map_err(|err| format!("read failed: {err}"))?;
    let mut meta_reader = P10Reader::with_limits(BytesSource::new(bytes.clone()), limits.clone());
    let transfer_syntax_uid = meta_reader
        .read_meta()
        .map_err(|err| format!("meta parse failed: {}", describe_core_error(&err)))?
        .transfer_syntax_uid;

    let mut dataset_reader = P10Reader::with_limits(BytesSource::new(bytes), limits.clone());
    let dataset = dataset_reader
        .read_dataset()
        .map_err(|err| format!("dataset parse failed: {}", describe_core_error(&err)))?;
    let context = extract_export_context(&dataset)
        .map_err(|err| format!("context extract failed: {}", describe_core_error(&err)))?;
    let frame = pipeline
        .decode_frame(&dataset, &transfer_syntax_uid, 0)
        .map_err(|err| format!("pixel decode failed: {}", describe_core_error(&err)))?;

    let rgba = frame_to_rgba8(&frame).map_err(|err| format!("frame conversion failed: {err}"))?;
    let image = ImageBuffer::<Rgba<u8>, _>::from_vec(frame.width, frame.height, rgba)
        .ok_or_else(|| "image buffer shape mismatch".to_string())?;
    let filename = format!("{index:05}_{}.png", sanitize_path_component(relative_path));
    let output_path = output_dir.join(filename);
    image
        .save_with_format(&output_path, ImageFormat::Png)
        .map_err(|err| format!("png write failed: {err}"))?;

    Ok(ExportRecord {
        source_path: relative_path.to_string_lossy().to_string(),
        output_path: Some(
            output_path
                .strip_prefix(output_dir)
                .unwrap_or(&output_path)
                .to_string_lossy()
                .to_string(),
        ),
        status: "ok",
        detail: String::new(),
        width: Some(frame.width),
        height: Some(frame.height),
        study_uid: Some(context.study_uid),
        series_uid: Some(context.series_uid),
        sop_instance_uid: Some(context.instance_uid),
        frame_index: context.frame_index,
        measurement_ids: deterministic_measurement_ids(&[]),
    })
}

fn frame_to_rgba8(frame: &DisplayFrame) -> Result<Vec<u8>, String> {
    match frame.format {
        PixelFormat::Rgba8 => Ok(frame.bytes.clone()),
        PixelFormat::Luma8 => {
            let mut rgba = Vec::with_capacity(frame.bytes.len() * 4);
            for value in &frame.bytes {
                rgba.extend_from_slice(&[*value, *value, *value, 255u8]);
            }
            Ok(rgba)
        }
        PixelFormat::Luma16 => {
            if !frame.bytes.len().is_multiple_of(2) {
                return Err("Luma16 byte length must be even".to_string());
            }
            let mut values = Vec::with_capacity(frame.bytes.len() / 2);
            let mut min = u16::MAX;
            let mut max = 0u16;
            for chunk in frame.bytes.chunks_exact(2) {
                let value = u16::from_le_bytes([chunk[0], chunk[1]]);
                min = min.min(value);
                max = max.max(value);
                values.push(value);
            }
            let range = max.saturating_sub(min);
            let mut rgba = Vec::with_capacity(values.len() * 4);
            for value in values {
                let scaled = if range == 0 {
                    0u8
                } else {
                    (((value - min) as f32 / range as f32) * 255.0).round() as u8
                };
                rgba.extend_from_slice(&[scaled, scaled, scaled, 255u8]);
            }
            Ok(rgba)
        }
    }
}

fn describe_core_error(err: &CoreError) -> String {
    match &err.kind() {
        ErrorKind::InvalidTagValue { tag, detail } => {
            format!(
                "{} tag=({:04X},{:04X}) detail={detail}",
                err.code(), tag.0, tag.1
            )
        }
        ErrorKind::MissingRequiredTag { tag } => {
            format!("{} tag=({:04X},{:04X})", err.code(), tag.0, tag.1)
        }
        ErrorKind::DecodeError { stage, detail } => {
            format!("{} stage={stage} detail={detail}", err.code())
        }
        ErrorKind::LimitExceeded {
            limit_name,
            observed,
            allowed,
        } => format!(
            "{} limit={limit_name} observed={observed} allowed={allowed}",
            err.code()
        ),
        ErrorKind::UnsupportedSopClass { sop_class_uid } => {
            format!("{} sop_class_uid={sop_class_uid}", err.code())
        }
        ErrorKind::UnsupportedTransferSyntax {
            transfer_syntax_uid,
        } => {
            format!("{} transfer_syntax_uid={transfer_syntax_uid}", err.code())
        }
        ErrorKind::InvalidGeometry { detail }
        | ErrorKind::InvalidPixelTransform { detail, .. }
        | ErrorKind::IoError { detail }
        | ErrorKind::IntegrityError { detail }
        | ErrorKind::InternalError { detail } => format!("{} detail={detail}", err.code()),
    }
}

fn sanitize_path_component(path: &Path) -> String {
    path.to_string_lossy()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch,
            _ => '_',
        })
        .collect()
}

fn write_integrity_metadata(
    path: &Path,
    manifest_path: &Path,
    manifest_bytes: &[u8],
    index_bytes: Option<&[u8]>,
    records: &[ExportRecord],
) -> std::io::Result<()> {
    let manifest_name = manifest_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("manifest.csv");
    let manifest_sha256 = sha256_hex(manifest_bytes);
    let index_sha256 = index_bytes.map(sha256_hex);
    let app_version = env!("CARGO_PKG_VERSION");
    let build_id = option_env!("DICOM_BUILD_ID").unwrap_or("build-unknown");
    let envelope_version =
        env::var("DICOM_ENVELOPE_VERSION").unwrap_or_else(|_| "envelope-unknown".to_string());
    let exported_count = records
        .iter()
        .filter(|record| record.status == "ok")
        .count();
    let failed_count = records.len().saturating_sub(exported_count);

    let mut text = String::from("{\n");
    text.push_str("  \"format\":\"hi_artifact_integrity_v1\",\n");
    text.push_str("  \"hash_algorithm\":\"sha256\",\n");
    text.push_str(&format!(
        "  \"application_version\":\"{}\",\n",
        json_escape(app_version)
    ));
    text.push_str(&format!("  \"build_id\":\"{}\",\n", json_escape(build_id)));
    text.push_str(&format!(
        "  \"envelope_version\":\"{}\",\n",
        json_escape(&envelope_version)
    ));
    text.push_str(&format!(
        "  \"manifest_file\":\"{}\",\n",
        json_escape(manifest_name)
    ));
    text.push_str(&format!("  \"manifest_sha256\":\"{manifest_sha256}\",\n"));
    match index_sha256 {
        Some(index_sha256) => {
            text.push_str("  \"index_file\":\"index.html\",\n");
            text.push_str(&format!("  \"index_sha256\":\"{index_sha256}\",\n"));
        }
        None => {
            text.push_str("  \"index_file\":\"\",\n");
            text.push_str("  \"index_sha256\":\"\",\n");
        }
    }
    text.push_str(&format!("  \"record_count\":{},\n", records.len()));
    text.push_str(&format!("  \"exported_count\":{exported_count},\n"));
    text.push_str(&format!("  \"failed_count\":{failed_count}\n"));
    text.push('}');
    text.push('\n');

    fs::write(path, text)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\u{08}' => escaped.push_str("\\b"),
            '\u{0C}' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn write_manifest(path: &Path, records: &[ExportRecord]) -> std::io::Result<Vec<u8>> {
    let mut out = String::from(
        "source_path,output_path,status,width,height,detail,study_uid,series_uid,sop_instance_uid,frame_index,measurement_ids\n",
    );
    for record in records {
        out.push_str(&csv_escape(&record.source_path));
        out.push(',');
        out.push_str(&csv_escape(record.output_path.as_deref().unwrap_or("")));
        out.push(',');
        out.push_str(record.status);
        out.push(',');
        out.push_str(&record.width.map(|v| v.to_string()).unwrap_or_default());
        out.push(',');
        out.push_str(&record.height.map(|v| v.to_string()).unwrap_or_default());
        out.push(',');
        out.push_str(&csv_escape(&record.detail));
        out.push(',');
        out.push_str(&csv_escape(record.study_uid.as_deref().unwrap_or("")));
        out.push(',');
        out.push_str(&csv_escape(record.series_uid.as_deref().unwrap_or("")));
        out.push(',');
        out.push_str(&csv_escape(
            record.sop_instance_uid.as_deref().unwrap_or(""),
        ));
        out.push(',');
        out.push_str(
            &record
                .frame_index
                .map(|v| v.to_string())
                .unwrap_or_default(),
        );
        out.push(',');
        out.push_str(&csv_escape(&record.measurement_ids.join(";")));
        out.push('\n');
    }
    let bytes = out.into_bytes();
    fs::write(path, &bytes)?;
    Ok(bytes)
}

fn write_index_html(path: &Path, records: &[ExportRecord]) -> std::io::Result<Vec<u8>> {
    let mut html = String::from(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>DICOM Preview</title>\
<style>body{font-family:system-ui,sans-serif;padding:16px}\
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:12px}\
.card{border:1px solid #ddd;border-radius:8px;padding:8px;background:#fff}\
img{width:100%;height:auto;display:block;background:#000}\
.meta{font-size:12px;word-break:break-all}</style></head><body><h1>DICOM Preview</h1><div class=\"grid\">",
    );
    for record in records {
        if record.status != "ok" {
            continue;
        }
        let output = record.output_path.as_deref().unwrap_or_default();
        html.push_str("<div class=\"card\">");
        html.push_str(&format!(
            "<img src=\"{}\" alt=\"{}\">",
            escape_html(output),
            escape_html(&record.source_path)
        ));
        html.push_str("<div class=\"meta\">");
        html.push_str(&escape_html(&record.source_path));
        if let (Some(width), Some(height)) = (record.width, record.height) {
            html.push_str(&format!("<br>{width}x{height}"));
        }
        html.push_str("</div></div>");
    }
    html.push_str("</div></body></html>");
    let bytes = html.into_bytes();
    fs::write(path, &bytes)?;
    Ok(bytes)
}

fn csv_escape(value: &str) -> String {
    let guarded = match value.chars().next() {
        Some('=' | '+' | '-' | '@') => format!("'{value}"),
        _ => value.to_string(),
    };
    if guarded.contains(',') || guarded.contains('"') || guarded.contains('\n') {
        format!("\"{}\"", guarded.replace('"', "\"\""))
    } else {
        guarded
    }
}

fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_path_component_replaces_non_alnum() {
        assert_eq!(sanitize_path_component(Path::new("a/b-c.dcm")), "a_b_c_dcm");
    }

    #[test]
    fn frame_to_rgba8_from_luma8_expands_channels() {
        let frame = DisplayFrame {
            width: 1,
            height: 1,
            format: PixelFormat::Luma8,
            bytes: vec![7],
        };
        let rgba = frame_to_rgba8(&frame).expect("rgba");
        assert_eq!(rgba, vec![7, 7, 7, 255]);
    }

    #[test]
    fn frame_to_rgba8_rejects_odd_luma16_buffer() {
        let frame = DisplayFrame {
            width: 1,
            height: 1,
            format: PixelFormat::Luma16,
            bytes: vec![1],
        };
        assert!(frame_to_rgba8(&frame).is_err());
    }

    #[test]
    fn csv_escape_guards_formula_prefixes() {
        assert_eq!(csv_escape("=SUM(A1:A2)"), "'=SUM(A1:A2)");
        assert_eq!(csv_escape("+cmd"), "'+cmd");
    }

    #[test]
    fn escape_html_escapes_attribute_sensitive_characters() {
        assert_eq!(
            escape_html("<img src=\"x\" onerror='a'>"),
            "&lt;img src=&quot;x&quot; onerror=&#x27;a&#x27;&gt;"
        );
    }
}
