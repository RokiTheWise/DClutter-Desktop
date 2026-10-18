//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! Previews from the Windows shell: the same thumbnails File Explorer
//! shows, standing in for QuickLook on macOS.
//!
//! Which types get a real thumbnail depends on what's installed. Images
//! and videos always do; PDFs and Office files do when an app such as
//! Acrobat or Office provides a thumbnail handler. Everything else gets
//! its file-type icon, which is still better than a blank well.

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

    shell::ensure_com();
    let item = shell::image_factory(path)?;
    // A content thumbnail first. If the type has no handler, ask for the
    // icon explicitly; letting the shell pick would hand back an icon
    // labelled as nothing in particular.
    if let Some(bgra) = shell::image(&item, size, SIIGBF_THUMBNAILONLY | SIIGBF_BIGGERSIZEOK) {
        return encode(ThumbnailKind::Thumbnail, bgra);
    }
    let bgra = shell::image(&item, size.min(256), SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK)?;
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

    #[test]
    fn a_missing_file_has_no_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        assert!(thumbnail(&dir.path().join("gone.pdf"), 512).is_none());
    }
}
