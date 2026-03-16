use std::process::Command;

/// Generate a thumbnail from the first page of a PDF file using ImageMagick
///
/// # Arguments
/// * `pdf_path` - Path to the PDF file
/// * `output_path` - Path where the thumbnail PNG will be saved
///
/// # Returns
/// * `Result<(), String>` - Ok if successful, Err with error message if failed
pub async fn generate_thumbnail(pdf_path: &str, output_path: &str) -> Result<(), String> {
    // Use ImageMagick's magick command (IMv7) to create a thumbnail from the first page
    // Format: "path/to/file.pdf[0]" means first page only
    let pdf_with_page = format!("{}[0]", pdf_path);

    // Try magick command first (ImageMagick v7)
    let mut cmd = Command::new("magick");
    cmd.arg("convert")
        .arg(&pdf_with_page)
        .arg("-density")
        .arg("200") // DPI for rendering
        .arg("-background")
        .arg("white") // White background
        .arg("-alpha")
        .arg("remove") // Remove transparency
        .arg("-quality")
        .arg("85") // PNG quality
        .arg("-resize")
        .arg("200x280") // Resize to fit within 200x280 pixels
        .arg(&output_path); // Output file

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute ImageMagick magick: {}", e))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        // Try fallback to convert command (older ImageMagick)
        return try_convert_command(&pdf_with_page, output_path, &error_msg);
    }

    Ok(())
}

fn try_convert_command(
    pdf_with_page: &str,
    output_path: &str,
    previous_error: &str,
) -> Result<(), String> {
    let output = Command::new("convert")
        .arg(pdf_with_page)
        .arg("-density")
        .arg("150")
        .arg("-background")
        .arg("white") // White background
        .arg("-alpha")
        .arg("remove") // Remove transparency
        .arg("-quality")
        .arg("85")
        .arg("-resize")
        .arg("200x280")
        .arg(output_path)
        .output()
        .map_err(|e| format!("Failed to execute ImageMagick convert: {}", e))?;

    if !output.status.success() {
        let error_msg = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ImageMagick failed. First attempt: {}. Fallback attempt: {}. Please ensure Ghostscript is installed (brew install ghostscript)",
            previous_error.trim(),
            error_msg.trim()
        ));
    }

    Ok(())
}
