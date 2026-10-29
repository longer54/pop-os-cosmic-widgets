//! Przygotowanie treści podglądu — wszystko, co dotyka dysku.
//!
//! Każda funkcja tutaj BLOKUJE (czyta plik, woła `pdftoppm`, dekoduje
//! obraz), więc wołamy je z `spawn_blocking`, nigdy z pętli iced. Efektem
//! jest gotowy [`Preview`]: nazwa, wiersz z metadanymi i `Body`, który
//! `app.rs` tylko rysuje.
//!
//! Zewnętrzne narzędzia świadomie zamiast bibliotek: `pdftoppm` (poppler),
//! `ffmpegthumbnailer`, `ffprobe` i `magick` są u autora zainstalowane, a
//! wciągnięcie PDF-a czy dekodera wideo jako crate'a zamieniłoby minutowy
//! build w kilkunastominutowy.

use std::path::{Path, PathBuf};

use cosmic::widget::image::Handle as ImgHandle;

/// Ile razy więcej pikseli renderujemy, niż zajmie karta. Iced zmniejszy
/// obraz przy rysowaniu, więc na ekranie HiDPI (i przy skalowaniu COSMIC-a)
/// podgląd nie jest rozmyty.
const RENDER_SCALE: f32 = 2.0;

#[derive(Debug, Clone, Copy)]
pub struct Opts {
    pub text_max_kb: usize,
    pub pdf_dpi: u32,
}

#[derive(Debug, Clone)]
pub struct Preview {
    /// Nazwa pliku (nagłówek karty).
    pub title: String,
    /// Wiersz pod nazwą: typ, rozmiar, data.
    pub info: String,
    pub body: Body,
}

#[derive(Debug, Clone)]
pub enum Body {
    Image {
        handle: ImgHandle,
        w: f32,
        h: f32,
    },
    /// SVG rysuje libcosmic sam z pliku — nie ma po co dekodować.
    Svg {
        path: PathBuf,
        w: f32,
        h: f32,
    },
    Pdf {
        handle: ImgHandle,
        w: f32,
        h: f32,
        page: usize,
        pages: usize,
    },
    Text {
        text: String,
        mono: bool,
        truncated: bool,
    },
    /// Wideo/audio: klatka (jeśli się udała) plus opis strumienia.
    Media {
        thumb: Option<(ImgHandle, f32, f32)>,
        lines: Vec<String>,
    },
    Dir {
        entries: Vec<String>,
        more: usize,
    },
    /// Nie umiemy pokazać treści — zostaje sam opis.
    Info(String),
}

impl Body {
    /// Czy treść ma własne proporcje (obraz), czy ma wypełnić kartę (tekst).
    pub fn natural_size(&self) -> Option<(f32, f32)> {
        match self {
            Body::Image { w, h, .. } | Body::Svg { w, h, .. } | Body::Pdf { w, h, .. } => {
                Some((*w, *h))
            }
            // Klatka plus wiersze opisu pod nią — bez tego zapasu karta
            // przycina „1920×1080 · 0:12 · h264”.
            Body::Media {
                thumb: Some((_, w, h)),
                lines,
            } => Some((*w, *h + 26.0 * lines.len() as f32)),
            _ => None,
        }
    }
}

/// Główne wejście: co pokazać dla tego pliku.
///
/// `page` ma znaczenie tylko dla PDF-ów (0 = pierwsza strona).
pub fn load(path: &Path, page: usize, box_w: f32, box_h: f32, opts: Opts) -> Preview {
    let title = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let meta = std::fs::metadata(path).ok();
    let mime = mime_of(path);
    let info = info_line(path, meta.as_ref(), &mime);

    let body = if meta.as_ref().is_some_and(std::fs::Metadata::is_dir) {
        dir_body(path)
    } else if mime == "image/svg+xml" {
        svg_body(path, box_w, box_h)
    } else if mime.starts_with("image/") {
        image_body(path, box_w, box_h)
    } else if mime == "application/pdf" {
        pdf_body(path, page, box_w, box_h, opts.pdf_dpi)
    } else if mime.starts_with("video/") || mime.starts_with("audio/") {
        media_body(path, &mime, box_w, box_h)
    } else if is_texty(&mime, path) {
        text_body(path, opts.text_max_kb)
    } else {
        Body::Info(format!("Typ {mime} — brak podglądu.\nEnter otwiera w aplikacji."))
    };

    Preview {
        title,
        info,
        body,
    }
}

// =========================================================================
// Poszczególne rodzaje treści
// =========================================================================

fn image_body(path: &Path, box_w: f32, box_h: f32) -> Body {
    match decode_fit_path(path, box_w, box_h) {
        Some((handle, w, h)) => Body::Image { handle, w, h },
        None => match magick_png(path, box_w, box_h) {
            Some((handle, w, h)) => Body::Image { handle, w, h },
            None => Body::Info("Nie udało się otworzyć tego obrazu.".into()),
        },
    }
}

fn svg_body(path: &Path, box_w: f32, box_h: f32) -> Body {
    Body::Svg {
        path: path.to_path_buf(),
        w: box_w,
        h: box_h,
    }
}

fn pdf_body(path: &Path, page: usize, box_w: f32, box_h: f32, dpi: u32) -> Body {
    let pages = pdf_pages(path).unwrap_or(1);
    let page = page.min(pages.saturating_sub(1));
    let n = (page + 1).to_string();
    let out = std::process::Command::new("pdftoppm")
        // BEZ przedrostka pliku wyjściowego — wtedy poppler pisze PNG na
        // stdout. Wariant z `-` na końcu (spotykany w poradnikach) w
        // poppler 26 kończy się sukcesem i PUSTYM wyjściem.
        .args([
            "-png",
            "-r",
            &dpi.to_string(),
            "-f",
            &n,
            "-l",
            &n,
            &path.to_string_lossy(),
        ])
        .output();
    match out {
        Ok(o) if o.status.success() && !o.stdout.is_empty() => {
            match decode_fit_bytes(&o.stdout, box_w, box_h) {
                Some((handle, w, h)) => Body::Pdf {
                    handle,
                    w,
                    h,
                    page,
                    pages,
                },
                None => Body::Info("PDF się wyrenderował, ale nie dał się wczytać.".into()),
            }
        }
        _ => Body::Info("Nie udało się wyrenderować PDF-a (pdftoppm).".into()),
    }
}

fn media_body(path: &Path, mime: &str, box_w: f32, box_h: f32) -> Body {
    let thumb = ffmpeg_thumb(path, box_w, box_h);
    let mut lines = Vec::new();
    if let Some(desc) = ffprobe_desc(path) {
        lines.push(desc);
    }
    if lines.is_empty() {
        lines.push(if mime.starts_with("audio/") {
            "Plik dźwiękowy".to_string()
        } else {
            "Plik wideo".to_string()
        });
    }
    Body::Media { thumb, lines }
}

fn text_body(path: &Path, max_kb: usize) -> Body {
    let limit = max_kb.max(1) * 1024;
    let bytes = match read_head(path, limit + 1) {
        Ok(b) => b,
        Err(e) => return Body::Info(format!("Nie mogę przeczytać pliku: {e}")),
    };
    let truncated = bytes.len() > limit;
    let text = String::from_utf8_lossy(&bytes[..bytes.len().min(limit)]).into_owned();
    let mono = !matches!(
        path.extension().and_then(|e| e.to_str()).unwrap_or(""),
        "md" | "markdown" | "txt" | "text"
    );
    Body::Text {
        text,
        mono,
        truncated,
    }
}

fn dir_body(path: &Path) -> Body {
    const MAX: usize = 40;
    let mut names: Vec<String> = match std::fs::read_dir(path) {
        Ok(rd) => rd
            .filter_map(Result::ok)
            .map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    format!("{name}/")
                } else {
                    name
                }
            })
            .filter(|n| !n.starts_with('.'))
            .collect(),
        Err(e) => return Body::Info(format!("Nie mogę przeczytać katalogu: {e}")),
    };
    names.sort_by_key(|n| (!n.ends_with('/'), n.to_lowercase()));
    let more = names.len().saturating_sub(MAX);
    names.truncate(MAX);
    Body::Dir {
        entries: names,
        more,
    }
}

// =========================================================================
// Narzędzia
// =========================================================================

/// Dekodowanie obrazu z pliku i zmniejszenie go do rozmiaru karty.
///
/// Zmniejszamy SAMI, bo inaczej zdjęcie 6000×4000 idzie na GPU jako
/// tekstura 96 MB — a i tak zobaczymy je w kilkuset pikselach.
fn decode_fit_path(path: &Path, box_w: f32, box_h: f32) -> Option<(ImgHandle, f32, f32)> {
    let img = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?;
    fit(img.decode().ok()?, box_w, box_h)
}

fn decode_fit_bytes(bytes: &[u8], box_w: f32, box_h: f32) -> Option<(ImgHandle, f32, f32)> {
    let img = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?
        .decode()
        .ok()?;
    fit(img, box_w, box_h)
}

fn fit(img: image::DynamicImage, box_w: f32, box_h: f32) -> Option<(ImgHandle, f32, f32)> {
    let (w, h) = (img.width() as f32, img.height() as f32);
    if w < 1.0 || h < 1.0 {
        return None;
    }
    // Małego obrazka nie rozciągamy na całą kartę — w macOS-owym Quick Looku
    // ikonka 48×48 też zostaje ikonką.
    let scale = (box_w / w).min(box_h / h).min(1.0);
    let (lw, lh) = ((w * scale).max(1.0), (h * scale).max(1.0));
    let (pw, ph) = (
        (lw * RENDER_SCALE).round() as u32,
        (lh * RENDER_SCALE).round() as u32,
    );
    let rgba = if pw < img.width() || ph < img.height() {
        image::imageops::thumbnail(&img.to_rgba8(), pw.max(1), ph.max(1))
    } else {
        img.to_rgba8()
    };
    let (fw, fh) = (rgba.width(), rgba.height());
    Some((ImgHandle::from_rgba(fw, fh, rgba.into_raw()), lw, lh))
}

/// Ostatnia deska ratunku dla formatów, których crate `image` nie zna
/// (HEIC, RAW z aparatu, PSD): ImageMagick zamienia je na PNG.
fn magick_png(path: &Path, box_w: f32, box_h: f32) -> Option<(ImgHandle, f32, f32)> {
    let geom = format!(
        "{}x{}",
        (box_w * RENDER_SCALE) as u32,
        (box_h * RENDER_SCALE) as u32
    );
    let out = std::process::Command::new("magick")
        .args([
            &format!("{}[0]", path.to_string_lossy()),
            "-resize",
            &geom,
            "png:-",
        ])
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    decode_fit_bytes(&out.stdout, box_w, box_h)
}

fn ffmpeg_thumb(path: &Path, box_w: f32, box_h: f32) -> Option<(ImgHandle, f32, f32)> {
    let size = ((box_w.max(box_h)) * RENDER_SCALE) as u32;
    let out = std::process::Command::new("ffmpegthumbnailer")
        .args([
            "-i",
            &path.to_string_lossy(),
            "-o",
            "-",
            "-c",
            "png",
            "-s",
            &size.to_string(),
        ])
        .output()
        .ok()?;
    if !out.status.success() || out.stdout.is_empty() {
        return None;
    }
    decode_fit_bytes(&out.stdout, box_w, box_h)
}

fn ffprobe_desc(path: &Path) -> Option<String> {
    let out = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration:stream=width,height,codec_name,codec_type",
            "-of",
            "default=noprint_wrappers=1",
            &path.to_string_lossy(),
        ])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut codec = None;
    let mut w = None;
    let mut h = None;
    let mut dur = None;
    for line in text.lines() {
        let (k, v) = line.split_once('=')?;
        match k {
            "codec_name" if codec.is_none() => codec = Some(v.to_string()),
            "width" => w = v.parse::<u32>().ok(),
            "height" => h = v.parse::<u32>().ok(),
            "duration" => dur = v.parse::<f64>().ok(),
            _ => {}
        }
    }
    let mut parts = Vec::new();
    if let (Some(w), Some(h)) = (w, h) {
        parts.push(format!("{w}×{h}"));
    }
    if let Some(d) = dur {
        parts.push(hms(d));
    }
    if let Some(c) = codec {
        parts.push(c);
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn pdf_pages(path: &Path) -> Option<usize> {
    let out = std::process::Command::new("pdfinfo")
        .arg(path.as_os_str())
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("Pages:"))
        .and_then(|v| v.trim().parse().ok())
}

fn mime_of(path: &Path) -> String {
    // Po rozszerzeniu jest szybciej i pewniej dla tekstu (`file` potrafi
    // nazwać plik .md „text/plain”, a .rs „text/x-c”) — ale typ obrazu czy
    // wideo lepiej wziąć od `file`, bo rozszerzenia bywają zmyślone.
    let out = std::process::Command::new("file")
        .args(["--mime-type", "-b", "--"])
        .arg(path.as_os_str())
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "application/octet-stream".into(),
    }
}

/// Czy pokazywać jako tekst. `file` mówi „text/...” o źródłach i configach,
/// ale JSON/TOML/CSV potrafi nazwać inaczej — stąd dodatkowa lista
/// rozszerzeń.
fn is_texty(mime: &str, path: &Path) -> bool {
    const EXTS: [&str; 24] = [
        "md", "markdown", "txt", "csv", "tsv", "json", "toml", "yaml", "yml", "ini", "conf", "log",
        "rs", "py", "sh", "fish", "js", "ts", "html", "css", "xml", "sql", "desktop", "service",
    ];
    if mime.starts_with("text/")
        || mime == "application/json"
        || mime == "application/xml"
        || mime == "application/toml"
    {
        return true;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .is_some_and(|e| EXTS.contains(&e.as_str()))
}

fn read_head(path: &Path, limit: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;
    let mut f = std::fs::File::open(path)?;
    let mut buf = vec![0u8; limit];
    let mut total = 0;
    while total < limit {
        match f.read(&mut buf[total..])? {
            0 => break,
            n => total += n,
        }
    }
    buf.truncate(total);
    Ok(buf)
}

fn info_line(path: &Path, meta: Option<&std::fs::Metadata>, mime: &str) -> String {
    let mut parts = vec![mime.to_string()];
    if let Some(m) = meta {
        if m.is_dir() {
            parts = vec!["katalog".to_string()];
        } else {
            parts.push(human_size(m.len()));
        }
        if let Ok(t) = m.modified() {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            parts.push(dt.format("%d.%m.%Y %H:%M").to_string());
        }
    }
    let dir = path
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !dir.is_empty() {
        // Ścieżka bywa dłuższa niż cała karta — zostawiamy początek i koniec.
        parts.push(elide(&skroc_dom(&dir), 52));
    }
    parts.join("  ·  ")
}

/// Skrócenie długiego napisu ze środka: `/home/user/…/Projekty/x`.
pub fn elide(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max || max < 8 {
        return s.to_string();
    }
    let keep_end = max * 2 / 3;
    let keep_start = max - keep_end - 1;
    let start: String = chars[..keep_start].iter().collect();
    let end: String = chars[chars.len() - keep_end..].iter().collect();
    format!("{start}…{end}")
}

fn skroc_dom(p: &str) -> String {
    match dirs::home_dir() {
        Some(h) => match p.strip_prefix(&*h.to_string_lossy()) {
            Some(rest) => format!("~{rest}"),
            None => p.to_string(),
        },
        None => p.to_string(),
    }
}

pub fn human_size(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i + 1 < UNITS.len() {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else if v < 10.0 {
        format!("{v:.1} {}", UNITS[i])
    } else {
        format!("{v:.0} {}", UNITS[i])
    }
}

fn hms(sec: f64) -> String {
    let s = sec.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::{elide, hms, human_size, is_texty};
    use std::path::Path;

    #[test]
    fn rozmiary_po_ludzku() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 kB");
        assert_eq!(human_size(15 * 1024 * 1024), "15 MB");
    }

    #[test]
    fn skracanie_dlugiej_sciezki() {
        assert_eq!(elide("krótka", 20), "krótka");
        let long = "/home/user/Projekty/cosmic-quicklook/src";
        let out = elide(long, 30);
        assert!(out.chars().count() <= 30, "{out}");
        assert!(out.contains('…') && out.ends_with("src"));
    }

    #[test]
    fn czas_trwania() {
        assert_eq!(hms(75.0), "1:15");
        assert_eq!(hms(3725.0), "1:02:05");
    }

    #[test]
    fn rozpoznaje_tekst_po_rozszerzeniu_gdy_mime_klamie() {
        assert!(is_texty("application/octet-stream", Path::new("a.toml")));
        assert!(is_texty("text/plain", Path::new("a.cokolwiek")));
        assert!(!is_texty("application/octet-stream", Path::new("a.bin")));
    }
}
