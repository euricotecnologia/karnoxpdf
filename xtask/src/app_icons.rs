//! `cargo xtask app-icons`: render every application icon from `img/icone1.png`.
//!
//! Writes, under `assets/app-icon/`:
//! - `karnoxpdf.ico`: Windows, 16–256 px (PNG entries), embedded in `karnoxpdf.exe` by `apps/karnoxpdf/build.rs`;
//! - `karnoxpdf.icns`: macOS, 16–1024 px (PNG entries) on Apple's grid;
//! - `karnoxpdf-1024.png`: macOS Dock icon at run time (an 824 px body on a transparent 1024 px canvas);
//! - `hicolor/<n>x<n>/apps/com.karnox.karnoxpdf.png`: Linux theme icons, 16–512 px, full-bleed.
//!
//! The source tile may not be square (it is 1036 × 1032): it is centred on a transparent square first.
//! After running it, update the `sha256` values in `ATTRIBUTION.toml`, then `cargo xtask assets --write`.

use std::path::Path;

use anyhow::{Context, Result, bail};

const SOURCE: &str = "img/icone1.png";
const APP_ID: &str = "com.karnox.karnoxpdf";
const ICO_SIZES: [usize; 7] = [16, 24, 32, 48, 64, 128, 256];
const HICOLOR_SIZES: [usize; 8] = [16, 24, 32, 48, 64, 128, 256, 512];
/// `.icns` entries that hold a PNG: (type, size).
const ICNS_ENTRIES: [(&[u8; 4], usize); 7] =
    [(b"icp4", 16), (b"icp5", 32), (b"icp6", 64), (b"ic07", 128), (b"ic08", 256), (b"ic09", 512), (b"ic10", 1024)];

/// A square, straight-alpha RGBA image.
struct Square {
    side: usize,
    px: Vec<u8>,
}

pub fn run(_args: &[String]) -> Result<()> {
    let root = crate::gates::root();
    let source = read_square(&root.join(SOURCE))?;
    let out = root.join("assets/app-icon");

    // Windows and Linux: the full-bleed tile.
    let ico: Vec<(usize, Vec<u8>)> = ICO_SIZES.iter().map(|&n| Ok((n, encode_png(&resize(&source, n))?))).collect::<Result<_>>()?;
    std::fs::write(out.join("karnoxpdf.ico"), ico_file(&ico)).context("writing karnoxpdf.ico")?;
    for n in HICOLOR_SIZES {
        let dir = out.join(format!("hicolor/{n}x{n}/apps"));
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(format!("{APP_ID}.png")), encode_png(&resize(&source, n))?)?;
    }

    // macOS: Apple's grid, an 824 px body centred on a transparent 1024 px canvas.
    let mac = on_canvas(&resize(&source, 824), 1024);
    std::fs::write(out.join("karnoxpdf-1024.png"), encode_png(&mac)?).context("writing karnoxpdf-1024.png")?;
    let icns: Vec<(&[u8; 4], Vec<u8>)> = ICNS_ENTRIES.iter().map(|&(kind, n)| Ok((kind, encode_png(&resize(&mac, n))?))).collect::<Result<_>>()?;
    std::fs::write(out.join("karnoxpdf.icns"), icns_file(&icns)).context("writing karnoxpdf.icns")?;

    println!("app-icons: wrote the icons in assets/app-icon from {SOURCE}; update their sha256 in ATTRIBUTION.toml");
    Ok(())
}

/// Read an RGBA or RGB 8-bit PNG and centre it on a transparent square.
fn read_square(path: &Path) -> Result<Square> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut reader = png::Decoder::new(std::io::BufReader::new(file)).read_info().context("reading the PNG header")?;
    let size = reader.output_buffer_size().context("the PNG is too large")?;
    let mut buf = vec![0u8; size];
    let info = reader.next_frame(&mut buf).context("decoding the PNG")?;
    if info.bit_depth != png::BitDepth::Eight {
        bail!("{} must be an 8-bit PNG", path.display());
    }
    let (w, h) = (info.width as usize, info.height as usize);
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        other => bail!("{} must be RGB or RGBA, not {other:?}", path.display()),
    };
    let side = w.max(h);
    let mut px = vec![0u8; side * side * 4];
    let (ox, oy) = ((side - w) / 2, (side - h) / 2);
    for y in 0..h {
        for x in 0..w {
            let from = (y * w + x) * channels;
            let to = ((y + oy) * side + x + ox) * 4;
            px[to..to + 3].copy_from_slice(&buf[from..from + 3]);
            px[to + 3] = if channels == 4 { buf[from + 3] } else { 255 };
        }
    }
    Ok(Square { side, px })
}

/// Scale to `n` × `n` with a box filter over premultiplied colour (exact area coverage).
fn resize(src: &Square, n: usize) -> Square {
    if src.side == n {
        return Square { side: n, px: src.px.clone() };
    }
    let scale = src.side as f64 / n as f64;
    let mut px = vec![0u8; n * n * 4];
    for y in 0..n {
        let (y0, y1) = (y as f64 * scale, (y + 1) as f64 * scale);
        for x in 0..n {
            let (x0, x1) = (x as f64 * scale, (x + 1) as f64 * scale);
            let mut acc = [0.0f64; 4];
            let mut weight = 0.0;
            for sy in y0.floor() as usize..(y1.ceil() as usize).min(src.side) {
                let wy = (y1.min(sy as f64 + 1.0) - y0.max(sy as f64)).max(0.0);
                for sx in x0.floor() as usize..(x1.ceil() as usize).min(src.side) {
                    let w = wy * (x1.min(sx as f64 + 1.0) - x0.max(sx as f64)).max(0.0);
                    let p = &src.px[(sy * src.side + sx) * 4..][..4];
                    let a = f64::from(p[3]) / 255.0;
                    for c in 0..3 {
                        acc[c] += w * a * f64::from(p[c]);
                    }
                    acc[3] += w * a;
                    weight += w;
                }
            }
            let out = &mut px[(y * n + x) * 4..][..4];
            if weight > 0.0 && acc[3] > 0.0 {
                for c in 0..3 {
                    out[c] = (acc[c] / acc[3]).round().clamp(0.0, 255.0) as u8;
                }
                out[3] = (acc[3] / weight * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Square { side: n, px }
}

/// Centre `inner` on a transparent `side` × `side` canvas.
fn on_canvas(inner: &Square, side: usize) -> Square {
    let mut px = vec![0u8; side * side * 4];
    let off = (side - inner.side) / 2;
    for y in 0..inner.side {
        let from = y * inner.side * 4;
        let to = ((y + off) * side + off) * 4;
        px[to..to + inner.side * 4].copy_from_slice(&inner.px[from..from + inner.side * 4]);
    }
    Square { side, px }
}

fn encode_png(img: &Square) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.side as u32, img.side as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::High);
        let mut writer = enc.write_header()?;
        writer.write_image_data(&img.px)?;
    }
    Ok(out)
}

/// A Windows `.ico` whose entries are PNG images (Vista and later).
fn ico_file(entries: &[(usize, Vec<u8>)]) -> Vec<u8> {
    let mut out = vec![0, 0, 1, 0];
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * entries.len();
    for (n, data) in entries {
        // A width or height of 256 is stored as 0.
        let dim = if *n >= 256 { 0 } else { *n as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += data.len();
    }
    for (_, data) in entries {
        out.extend_from_slice(data);
    }
    out
}

/// A macOS `.icns` whose entries are PNG images.
fn icns_file(entries: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    let total = 8 + entries.iter().map(|(_, d)| 8 + d.len()).sum::<usize>();
    let mut out = b"icns".to_vec();
    out.extend_from_slice(&(total as u32).to_be_bytes());
    for (kind, data) in entries {
        out.extend_from_slice(*kind);
        out.extend_from_slice(&((8 + data.len()) as u32).to_be_bytes());
        out.extend_from_slice(data);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(side: usize, rgba: [u8; 4]) -> Square {
        Square { side, px: rgba.iter().copied().cycle().take(side * side * 4).collect() }
    }

    #[test]
    fn resizing_keeps_a_flat_colour_and_its_transparency() {
        let small = resize(&square(64, [10, 120, 250, 255]), 16);
        assert!(small.px.chunks(4).all(|p| p == [10, 120, 250, 255]));
        let clear = resize(&square(64, [0, 0, 0, 0]), 16);
        assert!(clear.px.iter().all(|&b| b == 0));
        // A half-transparent edge keeps the colour (straight alpha) and half the coverage.
        let mut half = square(2, [200, 0, 0, 255]);
        half.px[3] = 0;
        half.px[7] = 0;
        let one = resize(&half, 1);
        assert_eq!(&one.px[..3], &[200, 0, 0]);
        assert!((i32::from(one.px[3]) - 128).abs() <= 1, "{:?}", one.px);
    }

    #[test]
    fn the_icon_containers_have_valid_headers() {
        let ico = ico_file(&[(16, vec![1, 2, 3]), (256, vec![4, 5])]);
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 2, 0]);
        assert_eq!((ico[6], ico[22]), (16, 0), "256 is stored as 0");
        assert_eq!(u32::from_le_bytes(ico[18..22].try_into().unwrap()) as usize, 6 + 32, "the first image follows the directory");
        let icns = icns_file(&[(b"ic07", vec![9; 4])]);
        assert_eq!(&icns[..4], b"icns");
        assert_eq!(u32::from_be_bytes(icns[4..8].try_into().unwrap()) as usize, icns.len());
        assert_eq!(&icns[8..12], b"ic07");
    }

    #[test]
    fn a_non_square_source_is_centred_on_a_transparent_square() {
        let on = on_canvas(&square(2, [1, 2, 3, 255]), 4);
        assert_eq!(on.side, 4);
        assert_eq!(&on.px[..4], &[0, 0, 0, 0]);
        assert_eq!(&on.px[(4 + 1) * 4..][..4], &[1, 2, 3, 255]);
    }
}
