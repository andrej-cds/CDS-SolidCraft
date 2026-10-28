//! The CDS logo (round, gold on black) used as the application icon by desktop, packaging and
//! the CLI. Source: `assets/branding/cds/cds-logo.png`, kept here as raw 512 x 512 RGBA.
static LOGO_512: &[u8] = include_bytes!("../../../../branding/cds/cds-logo-512.rgba");
const SRC: usize = 512;

/// The logo as straight-alpha RGBA at `size` x `size` (area-averaged from the 512 px source).
pub fn rgba(size: usize) -> Result<Vec<u8>, String> {
    if !(1..=2048).contains(&size) {
        return Err("icon size must be 1..=2048".into());
    }
    let mut out = Vec::with_capacity(size * size * 4);
    let at = |x: usize, y: usize| &LOGO_512[(y.min(SRC - 1) * SRC + x.min(SRC - 1)) * 4..][..4];
    for y in 0..size {
        for x in 0..size {
            let (x0, x1) = (x * SRC / size, ((x + 1) * SRC).div_ceil(size).max(x * SRC / size + 1));
            let (y0, y1) = (y * SRC / size, ((y + 1) * SRC).div_ceil(size).max(y * SRC / size + 1));
            let (mut sum, mut alpha, mut cnt) = ([0u32; 3], 0u32, 0u32);
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let p = at(xx, yy);
                    let a = u32::from(p[3]);
                    for k in 0..3 {
                        sum[k] += u32::from(p[k]) * a;
                    }
                    alpha += a;
                    cnt += 1;
                }
            }
            if alpha == 0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                out.extend_from_slice(&[(sum[0] / alpha) as u8, (sum[1] / alpha) as u8, (sum[2] / alpha) as u8, (alpha / cnt) as u8]);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn cds_logo_has_transparent_corners_and_gold() {
        for size in [16, 32, 64, 256] {
            let pixels = super::rgba(size).expect("valid size");
            assert_eq!(pixels.len(), size * size * 4);
            assert_eq!(pixels[3], 0, "corner is transparent");
            assert!(pixels.chunks_exact(4).any(|p| p[3] == 255 && p[0] > p[2] + 60), "gold present");
        }
        assert!(super::rgba(0).is_err());
        assert!(super::rgba(2049).is_err());
    }
}
