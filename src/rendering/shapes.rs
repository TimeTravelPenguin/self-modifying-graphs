use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

const TEXTURE_SIDE: u32 = 256;

/// Generates a reusable white mask. Tint belongs to the sprite, not the image.
pub(super) fn rounded_square_image(corner_radius_ratio: f32) -> Image {
    assert!(
        corner_radius_ratio.is_finite() && (0.0..=0.5).contains(&corner_radius_ratio),
        "corner radius must be between zero and half the tile side",
    );

    let half_side = TEXTURE_SIDE as f32 / 2.0;
    let radius = TEXTURE_SIDE as f32 * corner_radius_ratio;
    let straight_half_side = Vec2::splat(half_side - radius);
    let mut pixels = Vec::with_capacity((TEXTURE_SIDE * TEXTURE_SIDE * 4) as usize);

    for row in 0..TEXTURE_SIDE {
        for column in 0..TEXTURE_SIDE {
            let position =
                Vec2::new(column as f32 + 0.5, row as f32 + 0.5) - Vec2::splat(half_side);
            let corner_offset = position.abs() - straight_half_side;
            let distance = corner_offset.max(Vec2::ZERO).length()
                + corner_offset.max_element().min(0.0)
                - radius;
            let coverage = (0.5 - distance).clamp(0.0, 1.0);
            let alpha = (coverage * 255.0).round() as u8;
            pixels.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }

    let mut image = Image::new(
        Extent3d {
            width: TEXTURE_SIDE,
            height: TEXTURE_SIDE,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::linear();

    image
}

pub(super) fn circle_image() -> Image {
    rounded_square_image(0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_has_an_opaque_center_and_transparent_corners() {
        let image = rounded_square_image(8.0 / 44.0);
        let pixels = image.data.as_ref().expect("generated image has pixel data");
        let center_offset = ((TEXTURE_SIDE / 2 * TEXTURE_SIDE + TEXTURE_SIDE / 2) * 4) as usize;

        assert_eq!(pixels[center_offset + 3], 255);
        assert_eq!(pixels[3], 0);
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|pixel| pixel[..3] == [255; 3])
        );
    }

    #[test]
    fn circle_mask_is_symmetric_and_antialiased() {
        let image = circle_image();
        let pixels = image.data.as_ref().expect("generated image has pixel data");
        let row_size = (TEXTURE_SIDE * 4) as usize;

        for row in 0..TEXTURE_SIDE as usize {
            let opposite_row = TEXTURE_SIDE as usize - row - 1;

            assert_eq!(
                &pixels[row * row_size..(row + 1) * row_size],
                &pixels[opposite_row * row_size..(opposite_row + 1) * row_size],
            );
        }

        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .any(|pixel| (1..255).contains(&pixel[3]))
        );
    }
}
