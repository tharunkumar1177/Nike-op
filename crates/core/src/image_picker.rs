/// Windows native file dialog for image selection
use anyhow::{anyhow, Result};
use image::GenericImageView;
use std::path::PathBuf;

/// Open Windows file dialog to select a PNG file
#[cfg(windows)]
pub fn open_image_picker() -> Result<PathBuf> {
    use rfd::FileDialog;

    let file = FileDialog::new()
        .add_filter("PNG Image", &["png"])
        .add_filter("All Files", &["*"])
        .pick_file();

    file.ok_or_else(|| anyhow!("No file selected"))
}

#[cfg(not(windows))]
pub fn open_image_picker() -> Result<PathBuf> {
    Err(anyhow!("File picker only supported on Windows"))
}

/// Validate that the selected image is 100x100 pixels
pub fn validate_crosshair_image(path: &PathBuf) -> Result<()> {
    let reader =
        image::io::Reader::open(path).map_err(|e| anyhow!("Failed to open image: {}", e))?;

    let image = reader
        .decode()
        .map_err(|e| anyhow!("Failed to decode image: {}", e))?;

    let (width, height) = image.dimensions();

    if width != 100 || height != 100 {
        return Err(anyhow!(
            "Invalid image dimensions: {}x{} (expected 100x100)",
            width,
            height
        ));
    }

    Ok(())
}

/// Load and convert image to RGBA8 for preview/rendering
#[allow(dead_code)]
pub fn load_crosshair_image(path: &PathBuf) -> Result<(Vec<u32>, u32, u32)> {
    validate_crosshair_image(path)?;

    let reader =
        image::io::Reader::open(path).map_err(|e| anyhow!("Failed to open image: {}", e))?;

    let image = reader
        .decode()
        .map_err(|e| anyhow!("Failed to decode image: {}", e))?;

    let rgba_image = image.to_rgba8();
    let (width, height) = rgba_image.dimensions();

    // Convert RGBA8 to ARGB32 (u32) format for softbuffer
    let pixels: Vec<u32> = rgba_image
        .chunks_exact(4)
        .map(|chunk| {
            let r = chunk[0] as u32;
            let g = chunk[1] as u32;
            let b = chunk[2] as u32;
            let a = chunk[3] as u32;
            (a << 24) | (r << 16) | (g << 8) | b
        })
        .collect();

    Ok((pixels, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_png(label: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "edge-optimizer-{label}-{}-{suffix}.png",
            std::process::id()
        ))
    }

    #[test]
    fn accepts_a_valid_crosshair_fixture() {
        // Verifies a disposable one-hundred-pixel PNG passes crosshair validation.
        let path = temporary_png("valid");
        let image = ImageBuffer::from_pixel(100, 100, Rgba([0u8, 255, 255, 255]));
        image.save(&path).unwrap();
        assert!(validate_crosshair_image(&path).is_ok());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_wrong_dimensions_and_corrupt_data() {
        // Verifies invalid dimensions and undecodable data fail without opening a real overlay.
        let wrong_size = temporary_png("wrong-size");
        let corrupt = temporary_png("corrupt");
        ImageBuffer::from_pixel(64, 64, Rgba([0u8, 0, 0, 0]))
            .save(&wrong_size)
            .unwrap();
        fs::write(&corrupt, b"not a png").unwrap();
        assert!(validate_crosshair_image(&wrong_size).is_err());
        assert!(validate_crosshair_image(&corrupt).is_err());
        let _ = fs::remove_file(wrong_size);
        let _ = fs::remove_file(corrupt);
    }
}
