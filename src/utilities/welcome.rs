use std::io::Cursor;
use std::ops::DerefMut;
use std::sync::LazyLock;

use ab_glyph::{FontRef, PxScale, VariableFont};
use image::RgbaImage;
use imageproc::geometric_transformations::{Border, Interpolation, rotate_about_center};

static TILE: LazyLock<RgbaImage> = LazyLock::new(|| {
    let logo_data: &[u8] = include_bytes!("../../Assets/logo.png");
    let logo = image::load_from_memory(logo_data)
        .expect("Assets/logo.png no se pudo cargar")
        .to_rgba8();

    let logo_small = image::imageops::resize(&logo, 66, 66, image::imageops::FilterType::Lanczos3);
    let logo_rotated = rotate_about_center(
        &logo_small,
        std::f32::consts::PI / 6.0,
        Interpolation::Bilinear,
        Border::Constant(image::Rgba([0u8; 4])),
    );

    let tile_size = 100u32;
    let mut tile = RgbaImage::new(tile_size, tile_size);
    let ox = (tile_size.saturating_sub(logo_rotated.width())) / 2;
    let oy = (tile_size.saturating_sub(logo_rotated.height())) / 2;
    image::imageops::overlay(&mut tile, &logo_rotated, i64::from(ox), i64::from(oy));

    for pixel in tile.deref_mut().chunks_exact_mut(4) {
        pixel[3] = (pixel[3] as f32 * 0.2) as u8;
    }

    tile
});

pub async fn generate_banner(
    username: &str,
    avatar_url: &str,
    member_count: u64,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let avatar_bytes = reqwest::get(avatar_url).await?.bytes().await?.to_vec();
    let username = username.to_string();

    let result = tokio::task::spawn_blocking(
        move || -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
            let mut canvas = RgbaImage::new(700, 250);

            imageproc::drawing::draw_filled_rect_mut(
                &mut canvas,
                imageproc::rect::Rect::at(0, 0).of_size(700, 250),
                image::Rgba([0x1a, 0x1a, 0x1a, 255]),
            );

            image::imageops::tile(&mut canvas, &*TILE);

            imageproc::drawing::draw_hollow_rect_mut(
                &mut canvas,
                imageproc::rect::Rect::at(0, 0).of_size(700, 250),
                image::Rgba([255u8; 4]),
            );

            let avatar_img = image::load_from_memory(&avatar_bytes)?.to_rgba8();
            let avatar_resized = image::imageops::resize(
                &avatar_img,
                160,
                160,
                image::imageops::FilterType::Lanczos3,
            );
            let avatar_circular = make_circular(&avatar_resized);
            image::imageops::overlay(&mut canvas, &avatar_circular, 45, 45);

            let font_data: &[u8] =
                include_bytes!("../../Fonts/UbuntuSans-VariableFont_wdth,wght.ttf");
            let mut font = FontRef::try_from_slice(font_data)?;
            font.set_variation(b"wdth", 100.0);

            font.set_variation(b"wght", 700.0);
            imageproc::drawing::draw_text_mut(
                &mut canvas,
                image::Rgba([255u8; 4]),
                250,
                60,
                PxScale::from(42.0),
                &font,
                "¡BIENVENIDO/A!",
            );

            font.set_variation(b"wght", 400.0);
            imageproc::drawing::draw_text_mut(
                &mut canvas,
                image::Rgba([255u8; 4]),
                250,
                115,
                PxScale::from(32.0),
                &font,
                &username,
            );

            font.set_variation(b"wght", 300.0);
            let count_text = format!("Miembro #{member_count}");
            imageproc::drawing::draw_text_mut(
                &mut canvas,
                image::Rgba([204u8, 204, 204, 255]),
                250,
                160,
                PxScale::from(20.0),
                &font,
                &count_text,
            );

            let mut buf = Vec::new();
            canvas.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)?;
            Ok(buf)
        },
    )
    .await;

    match result {
        Ok(Ok(png)) => Ok(png),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(format!("Error en generación de imagen: {e}").into()),
    }
}

fn make_circular(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let radius = w.min(h) as f32 / 2.0;
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let mut out = RgbaImage::new(w, h);
    for (x, y, pixel) in img.enumerate_pixels() {
        let dx = x as f32 - cx;
        let dy = y as f32 - cy;
        if dx * dx + dy * dy <= radius * radius {
            out.put_pixel(x, y, *pixel);
        }
    }
    out
}
