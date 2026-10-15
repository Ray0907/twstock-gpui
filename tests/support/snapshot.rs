use image::{
    ImageEncoder, RgbaImage,
    codecs::png::{CompressionType, FilterType, PngEncoder},
};
use std::path::Path;

/// Static GPU scenes exceed 300 KB at native 2x resolution.
/// Store and compare every scene at half resolution, with an exact 2x2 box filter.
pub fn assert_snapshot(name: &str, actual: &RgbaImage) {
    let actual = box_half(actual);
    let goldens = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
    let output = Path::new("/tmp/twstock-gpui-snapshot");
    let update = std::env::var("UPDATE_SNAPSHOTS").is_ok_and(|v| v == "1");
    check_snapshot(name, &actual, &goldens, output, update)
        .unwrap_or_else(|error| panic!("snapshot {name}: {error}"));
}
fn differs(a: &image::Rgba<u8>, b: &image::Rgba<u8>) -> bool {
    a.0.iter().zip(b.0).any(|(a, b)| a.abs_diff(b) > 2)
}
fn compare(expected: &RgbaImage, actual: &RgbaImage) -> Result<(), String> {
    if expected.dimensions() != actual.dimensions() {
        return Err(format!(
            "dimensions differ: golden {:?}, actual {:?}",
            expected.dimensions(),
            actual.dimensions()
        ));
    }
    let count = expected
        .pixels()
        .zip(actual.pixels())
        .filter(|(a, b)| differs(a, b))
        .count();
    let total = u128::from(actual.width()) * u128::from(actual.height());
    if count as u128 * 2000 > total {
        Err(format!(
            "{count} of {total} pixels differ (limit 0.05%, channel tolerance 2)"
        ))
    } else {
        Ok(())
    }
}
fn check_snapshot(
    name: &str,
    actual: &RgbaImage,
    goldens: &Path,
    output: &Path,
    update: bool,
) -> Result<(), String> {
    let golden = goldens.join(format!("{name}.png"));
    if update {
        std::fs::create_dir_all(goldens).map_err(|e| e.to_string())?;
        let file = std::fs::File::create(&golden).map_err(|e| e.to_string())?;
        return PngEncoder::new_with_quality(file, CompressionType::Best, FilterType::Adaptive)
            .write_image(
                actual.as_raw(),
                actual.width(),
                actual.height(),
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| format!("{}: {e}", golden.display()));
    }
    if !golden.exists() {
        return Err(format!(
            "missing golden {}; create it with UPDATE_SNAPSHOTS=1 cargo test --test rendering",
            golden.display()
        ));
    }
    let expected = image::open(&golden)
        .map_err(|e| format!("{}: {e}", golden.display()))?
        .into_rgba8();
    if let Err(reason) = compare(&expected, actual) {
        std::fs::create_dir_all(output).map_err(|e| e.to_string())?;
        let actual_path = output.join(format!("{name}.actual.png"));
        let diff_path = output.join(format!("{name}.diff.png"));
        actual.save(&actual_path).map_err(|e| e.to_string())?;
        let mut count = 0;
        let diff = RgbaImage::from_fn(
            expected.width().max(actual.width()),
            expected.height().max(actual.height()),
            |x, y| {
                let a = expected.get_pixel_checked(x, y);
                let b = actual.get_pixel_checked(x, y);
                if match (a, b) {
                    (Some(a), Some(b)) => differs(a, b),
                    _ => a != b,
                } {
                    count += 1;
                    image::Rgba([255, 0, 255, 255])
                } else {
                    image::Rgba([0, 0, 0, 255])
                }
            },
        );
        diff.save(&diff_path).map_err(|e| e.to_string())?;
        return Err(format!(
            "{reason}; {count} differing pixels\nactual: {}\ndiff: {}",
            actual_path.display(),
            diff_path.display()
        ));
    }
    Ok(())
}
fn box_half(image: &RgbaImage) -> RgbaImage {
    assert!(
        image.width().is_multiple_of(2) && image.height().is_multiple_of(2),
        "snapshot source dimensions must be even for 2x downscaling"
    );
    RgbaImage::from_fn(image.width() / 2, image.height() / 2, |x, y| {
        let a = image.get_pixel(x * 2, y * 2);
        let b = image.get_pixel(x * 2 + 1, y * 2);
        let c = image.get_pixel(x * 2, y * 2 + 1);
        let d = image.get_pixel(x * 2 + 1, y * 2 + 1);
        image::Rgba(std::array::from_fn(|channel| {
            let sum = u16::from(a[channel])
                + u16::from(b[channel])
                + u16::from(c[channel])
                + u16::from(d[channel]);
            ((sum + 2) / 4) as u8
        }))
    })
}
