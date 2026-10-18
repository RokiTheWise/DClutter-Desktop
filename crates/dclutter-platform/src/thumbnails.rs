//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! Previews from the Windows shell: the same thumbnails File Explorer
//! shows, standing in for QuickLook on macOS.
//!
//! Which types get a real thumbnail depends on what's installed. Images
//! and videos always do, and Office files do when Office provides a
//! handler. Everything else gets its file-type icon, which is still better
//! than a blank well.
//!
//! PDFs are the exception: Explorer only previews them when a reader
//! such as Acrobat has that turned on, so DClutter renders their first
//! page itself with the PDF renderer built into Windows.

use std::path::Path;

/// What the shell could produce for a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailKind {
    /// A picture of the content.
    Thumbnail,
    /// The file type's icon, for types with no thumbnail handler. Worth
    /// showing small: icons top out at 256 px and blur when stretched.
    Icon,
}

#[derive(Debug, Clone)]
pub struct Thumbnail {
    pub kind: ThumbnailKind,
    pub width: u32,
    pub height: u32,
    pub png: Vec<u8>,
}

/// A preview of `path` about `size` pixels on its longer side, or `None`
/// if the shell can't produce one.
#[cfg(windows)]
pub fn thumbnail(path: &Path, size: u32) -> Option<Thumbnail> {
    use windows::Win32::UI::Shell::{SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY, SIIGBF_THUMBNAILONLY};

    // Both the shell and StorageFile reject forward slashes.
    let path = &std::path::PathBuf::from(path.to_string_lossy().replace('/', "\\"));
    shell::ensure_com();
    let is_pdf = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
    if is_pdf {
        // A locked, encrypted or damaged PDF falls through to the shell.
        if let Some(page) = pdf::first_page(path, size) {
            return Some(page);
        }
    }
    let item = shell::image_factory(path)?;
    // A content thumbnail first. If the type has no handler, ask for the
    // icon explicitly; letting the shell pick would hand back an icon
    // labelled as nothing in particular.
    if let Some(bgra) = shell::image(&item, size, SIIGBF_THUMBNAILONLY | SIIGBF_BIGGERSIZEOK) {
        return encode(ThumbnailKind::Thumbnail, bgra);
    }
    // With several previews being made at once, the shell now and then
    // fails an icon it can normally give; one retry covers that.
    let icon = || shell::image(&item, size.min(256), SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK);
    let bgra = icon().or_else(|| {
        std::thread::sleep(std::time::Duration::from_millis(50));
        icon()
    })?;
    encode(ThumbnailKind::Icon, bgra)
}

#[cfg(not(windows))]
pub fn thumbnail(_path: &Path, _size: u32) -> Option<Thumbnail> {
    None
}

/// Pixels as the shell hands them over: rows top to bottom, BGRA.
struct Bgra {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[cfg_attr(not(windows), allow(dead_code))]
fn encode(kind: ThumbnailKind, image: Bgra) -> Option<Thumbnail> {
    let rgba = to_rgba(image.pixels);
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    encoder.write_header().ok()?.write_image_data(&rgba).ok()?;
    Some(Thumbnail { kind, width: image.width, height: image.height, png })
}

/// Converts the shell's premultiplied BGRA to straight RGBA. A bitmap with
/// no alpha at all, which some thumbnail handlers return, is opaque.
fn to_rgba(mut pixels: Vec<u8>) -> Vec<u8> {
    let has_alpha = pixels.chunks_exact(4).any(|p| p[3] != 0);
    for p in pixels.chunks_exact_mut(4) {
        p.swap(0, 2);
        if !has_alpha {
            p[3] = 255;
        } else if p[3] != 0 && p[3] != 255 {
            let a = u32::from(p[3]);
            for c in &mut p[..3] {
                *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    pixels
}

#[cfg(windows)]
mod pdf {
    use std::path::Path;

    use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
    use windows::Storage::StorageFile;
    use windows::Storage::Streams::{DataReader, InMemoryRandomAccessStream};
    use windows::core::HSTRING;

    use super::{Thumbnail, ThumbnailKind};

    /// The first page as a PNG about `size` pixels on its longer side, on
    /// the white background Windows renders pages with.
    ///
    /// Always renders on a fresh single-threaded-apartment thread that ends
    /// as soon as the page is done. A thread that has rendered a page and is
    /// still alive when the process exits, such as the main thread or a
    /// pooled worker, makes Windows' exit teardown crash. A thread that has
    /// already ended leaves nothing behind.
    pub fn first_page(path: &Path, size: u32) -> Option<Thumbnail> {
        let path = path.to_owned();
        std::thread::spawn(move || {
            super::shell::ensure_com();
            render(&path, size)
        })
        .join()
        .ok()
        .flatten()
    }

    fn render(path: &Path, size: u32) -> Option<Thumbnail> {
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path.as_os_str())).ok()?.join().ok()?;
        let document = PdfDocument::LoadFromFileAsync(&file).ok()?.join().ok()?;
        if document.PageCount().ok()? == 0 {
            return None;
        }
        let page = document.GetPage(0).ok()?;
        let page_size = page.Size().ok()?;
        if page_size.Width <= 0.0 || page_size.Height <= 0.0 {
            return None;
        }

        // Fit the longer side to `size`, keeping the page's proportions.
        let scale = size as f32 / page_size.Width.max(page_size.Height);
        let width = ((page_size.Width * scale).round() as u32).max(1);
        let height = ((page_size.Height * scale).round() as u32).max(1);
        let options = PdfPageRenderOptions::new().ok()?;
        options.SetDestinationWidth(width).ok()?;
        options.SetDestinationHeight(height).ok()?;

        // Renders to PNG unless told otherwise.
        let stream = InMemoryRandomAccessStream::new().ok()?;
        page.RenderWithOptionsToStreamAsync(&stream, &options).ok()?.join().ok()?;

        let length = u32::try_from(stream.Size().ok()?).ok()?;
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).ok()?).ok()?;
        reader.LoadAsync(length).ok()?.join().ok()?;
        let mut png = vec![0u8; length as usize];
        reader.ReadBytes(&mut png).ok()?;

        Some(Thumbnail { kind: ThumbnailKind::Thumbnail, width, height, png })
    }
}

#[cfg(windows)]
mod shell {
    use std::ffi::c_void;
    use std::path::Path;

    use windows::Win32::Foundation::SIZE;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
        DeleteObject, GetDIBits, GetObjectW, HGDIOBJ,
    };
    use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx};
    use windows::Win32::UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF};
    use windows::core::HSTRING;

    use super::Bgra;

    /// Thumbnail handlers are COM objects, and many need a single-threaded
    /// apartment. Joining one is harmless if the thread already has.
    pub fn ensure_com() {
        // SAFETY: no pointers; an error just means the thread already
        // chose a COM mode, which is fine for creating shell items.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
    }

    pub fn image_factory(path: &Path) -> Option<IShellItemImageFactory> {
        // SAFETY: the path is a valid HSTRING for the duration of the call.
        unsafe { SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None).ok() }
    }

    pub fn image(item: &IShellItemImageFactory, size: u32, flags: SIIGBF) -> Option<Bgra> {
        let side = size as i32;
        // SAFETY: the bitmap the shell returns is ours to read and delete;
        // every buffer passed to GDI is sized from that bitmap's own
        // dimensions, and both the bitmap and the DC are always released.
        unsafe {
            let bitmap = item.GetImage(SIZE { cx: side, cy: side }, flags).ok()?;
            let object = HGDIOBJ(bitmap.0);

            let mut info = BITMAP::default();
            let read = GetObjectW(
                object,
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut info as *mut BITMAP as *mut c_void),
            );
            let (width, height) = (info.bmWidth, info.bmHeight.abs());
            let mut result = None;
            if read != 0 && width > 0 && height > 0 {
                let mut header = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: width,
                        // Negative: rows top to bottom.
                        biHeight: -height,
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let mut pixels = vec![0u8; width as usize * height as usize * 4];
                let dc = CreateCompatibleDC(None);
                let rows = GetDIBits(
                    dc,
                    bitmap,
                    0,
                    height as u32,
                    Some(pixels.as_mut_ptr() as *mut c_void),
                    &mut header,
                    DIB_RGB_COLORS,
                );
                let _ = DeleteDC(dc);
                if rows == height {
                    result = Some(Bgra { width: width as u32, height: height as u32, pixels });
                }
            }
            let _ = DeleteObject(object);
            result
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpremultiplies_and_swaps_to_rgba() {
        // Half-transparent pure red, premultiplied: B=0 G=0 R=128 A=128.
        assert_eq!(to_rgba(vec![0, 0, 128, 128]), vec![255, 0, 0, 128]);
    }

    #[test]
    fn a_bitmap_without_alpha_is_made_opaque() {
        assert_eq!(to_rgba(vec![10, 20, 30, 0, 40, 50, 60, 0]), vec![30, 20, 10, 255, 60, 50, 40, 255]);
    }

    #[test]
    fn fully_transparent_pixels_stay_transparent_when_others_are_not() {
        assert_eq!(to_rgba(vec![1, 2, 3, 0, 4, 5, 6, 255]), vec![3, 2, 1, 0, 6, 5, 4, 255]);
    }

    #[cfg(windows)]
    #[test]
    fn a_text_file_gets_its_icon() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("notes.txt");
        std::fs::write(&path, "hello").unwrap();

        let thumb = thumbnail(&path, 512).expect("the shell has an icon for .txt");

        assert_eq!(thumb.kind, ThumbnailKind::Icon);
        assert!(thumb.width <= 256 && thumb.height <= 256);
        assert_eq!(&thumb.png[1..4], b"PNG");
    }

    #[cfg(windows)]
    #[test]
    fn an_image_gets_a_real_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("photo.png");
        // A 300 x 200 solid blue PNG.
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 300, 200);
        encoder.set_color(png::ColorType::Rgb);
        encoder.write_header().unwrap().write_image_data(&[0u8, 0, 255].repeat(300 * 200)).unwrap();
        std::fs::write(&path, bytes).unwrap();

        let thumb = thumbnail(&path, 512).expect("Windows thumbnails PNGs");

        assert_eq!(thumb.kind, ThumbnailKind::Thumbnail);
        assert!(thumb.width >= 200, "{} x {}", thumb.width, thumb.height);

        // Still blue, not red: the channels came through the right way round.
        let decoder = png::Decoder::new(std::io::Cursor::new(thumb.png));
        let mut reader = decoder.read_info().unwrap();
        let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut rgba).unwrap();
        let centre = ((frame.height / 2 * frame.width + frame.width / 2) * 4) as usize;
        assert_eq!(&rgba[centre..centre + 4], &[0, 0, 255, 255]);
    }

    /// A one-page US Letter PDF with a black bar across the top, built by
    /// hand so the test needs no fixture file. Offsets in the xref table
    /// are computed, so the file is valid.
    fn letter_pdf_with_a_black_bar() -> Vec<u8> {
        let content = "0 g 72 680 468 60 re f";
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>".to_string(),
            format!("<< /Length {} >>
stream
{content}
endstream", content.len()),
        ];
        let mut pdf = b"%PDF-1.4
".to_vec();
        let mut offsets = Vec::new();
        for (i, object) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend(format!("{} 0 obj
{object}
endobj
", i + 1).bytes());
        }
        let xref = pdf.len();
        pdf.extend(format!("xref
0 {}
0000000000 65535 f 
", objects.len() + 1).bytes());
        for offset in offsets {
            pdf.extend(format!("{offset:010} 00000 n 
").bytes());
        }
        pdf.extend(format!("trailer
<< /Size {} /Root 1 0 R >>
startxref
{xref}
%%EOF
", objects.len() + 1).bytes());
        pdf
    }

    fn pixel(png_bytes: &[u8], x_frac: f32, y_frac: f32) -> [u8; 4] {
        let decoder = png::Decoder::new(std::io::Cursor::new(png_bytes.to_vec()));
        let mut reader = decoder.read_info().unwrap();
        let mut buf = vec![0; reader.output_buffer_size().unwrap()];
        let frame = reader.next_frame(&mut buf).unwrap();
        let channels = frame.color_type.samples();
        let (x, y) = ((frame.width as f32 * x_frac) as usize, (frame.height as f32 * y_frac) as usize);
        let i = (y * frame.width as usize + x) * channels;
        let p = &buf[i..i + channels];
        [p[0], p[1], p[2], if channels == 4 { p[3] } else { 255 }]
    }

    #[cfg(windows)]
    #[test]
    fn a_pdf_gets_its_first_page_even_without_a_reader_installed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Letter.pdf");
        std::fs::write(&path, letter_pdf_with_a_black_bar()).unwrap();

        let thumb = thumbnail(&path, 768).expect("Windows renders PDFs");

        assert_eq!(thumb.kind, ThumbnailKind::Thumbnail);
        // Portrait 8.5 x 11, fitted to 768 tall.
        assert_eq!((thumb.width, thumb.height), (593, 768));
        // The page is white, with the black bar near the top.
        assert_eq!(pixel(&thumb.png, 0.5, 0.6), [255, 255, 255, 255]);
        let bar = pixel(&thumb.png, 0.5, 0.1);
        assert!(bar[0] < 40 && bar[1] < 40 && bar[2] < 40, "{bar:?}");
    }

    #[cfg(windows)]
    #[test]
    fn a_damaged_pdf_falls_back_to_its_icon() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Broken.pdf");
        std::fs::write(&path, "%PDF-1.4 this is not really a pdf").unwrap();

        let thumb = thumbnail(&path, 768).expect("the shell has an icon for .pdf");

        assert_eq!(thumb.kind, ThumbnailKind::Icon);
    }

    #[test]
    fn a_missing_file_has_no_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        assert!(thumbnail(&dir.path().join("gone.pdf"), 512).is_none());
    }
}
