include!("support/snapshot.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn image() -> RgbaImage {
        RgbaImage::from_pixel(4, 4, Rgba([10, 20, 30, 255]))
    }
    #[test]
    fn golden_pngs_stay_below_300kb() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots");
        let images: Vec<_> = std::fs::read_dir(root)
            .unwrap()
            .map(|p| p.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "png"))
            .collect();
        assert_eq!(
            images.len(),
            10,
            "create goldens with UPDATE_SNAPSHOTS=1 cargo test --test rendering"
        );
        for image in images {
            assert!(
                std::fs::metadata(&image).unwrap().len() <= 300_000,
                "{} exceeds 300 KB",
                image.display()
            );
        }
    }
    #[test]
    fn equal_images_match() {
        assert_eq!(compare(&image(), &image()), Ok(()));
    }
    #[test]
    fn each_channel_can_differ_by_two() {
        let actual = RgbaImage::from_pixel(4, 4, Rgba([12, 18, 32, 253]));
        assert_eq!(compare(&image(), &actual), Ok(()));
    }
    #[test]
    fn over_channel_tolerance_fails() {
        let mut actual = image();
        actual.get_pixel_mut(0, 0)[0] += 3;
        assert!(compare(&image(), &actual).unwrap_err().contains("1"));
    }
    #[test]
    fn pixel_budget_is_strictly_more_than_point_zero_five_percent() {
        let expected = RgbaImage::from_pixel(100, 20, Rgba([10, 20, 30, 255]));
        let mut actual = expected.clone();
        actual.get_pixel_mut(0, 0)[0] += 3;
        assert_eq!(compare(&expected, &actual), Ok(())); // exactly 0.05%
        actual.get_pixel_mut(1, 0)[0] += 3;
        assert!(compare(&expected, &actual).unwrap_err().contains("2"));
    }
    #[test]
    fn size_mismatch_fails_even_with_the_same_pixel_count() {
        assert!(
            compare(&image(), &RgbaImage::new(2, 8))
                .unwrap_err()
                .contains("dimensions")
        );
    }
    #[test]
    fn missing_golden_fails_with_update_instructions() {
        let root = tempfile::tempdir().unwrap();
        let error =
            check_snapshot("missing", &image(), root.path(), root.path(), false).unwrap_err();
        assert!(error.contains("UPDATE_SNAPSHOTS=1"));
        assert!(!root.path().join("missing.png").exists());
    }
    #[test]
    fn update_is_explicit_and_mismatch_writes_diagnostics() {
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        check_snapshot("sample", &image(), root.path(), output.path(), true).unwrap();
        let mut actual = image();
        actual.get_pixel_mut(0, 0)[0] += 3;
        let error =
            check_snapshot("sample", &actual, root.path(), output.path(), false).unwrap_err();
        assert!(error.contains("1"));
        let diff = image::open(output.path().join("sample.diff.png"))
            .unwrap()
            .into_rgba8();
        assert_ne!(diff.get_pixel(0, 0), diff.get_pixel(1, 0));
        assert_eq!(
            image::open(output.path().join("sample.actual.png"))
                .unwrap()
                .into_rgba8(),
            actual
        );
        assert_eq!(
            image::open(root.path().join("sample.png"))
                .unwrap()
                .into_rgba8(),
            image()
        );
    }
    #[test]
    fn downscale_uses_two_by_two_box_average() {
        let input = RgbaImage::from_fn(2, 2, |x, y| Rgba([((x + 2 * y) * 4) as u8, 20, 30, 255]));
        let output = box_half(&input);
        assert_eq!(output.dimensions(), (1, 1));
        assert_eq!(output.get_pixel(0, 0), &Rgba([6, 20, 30, 255]));
    }
}
