/*
 * Copyright (c) Radzivon Bartoshyk, 10/2024. All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without modification,
 * are permitted provided that the following conditions are met:
 *
 * 1.  Redistributions of source code must retain the above copyright notice, this
 * list of conditions and the following disclaimer.
 *
 * 2.  Redistributions in binary form must reproduce the above copyright notice,
 * this list of conditions and the following disclaimer in the documentation
 * and/or other materials provided with the distribution.
 *
 * 3.  Neither the name of the copyright holder nor the names of its
 * contributors may be used to endorse or promote products derived from
 * this software without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 * CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 * OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
use crate::{get_inverse_transform, get_kr_kb, get_yuv_range, YuvRange, YuvStandardMatrix};
use num_traits::AsPrimitive;

/// Converts Yuv 422 planar format to Rgba
///
/// This support not tightly packed data and crop image using stride in place.
///
/// # Arguments
///
/// * `y_plane`: Luma plane
/// * `y_stride`: Luma stride
/// * `u_plane`: U chroma plane
/// * `u_stride`: U chroma stride, even odd images is supported this always must match `u_stride * height`
/// * `v_plane`: V chroma plane
/// * `v_stride`: V chroma stride, even odd images is supported this always must match `v_stride * height`
/// * `rgb`: RGB image layout
/// * `width`: Image width
/// * `height`: Image height
/// * `range`: see [YuvRange]
/// * `matrix`: see [YuvStandardMatrix]
///
///
pub fn yuv422_to_rgba<V: Copy + AsPrimitive<i32> + 'static>(
    y_plane: &[V],
    y_stride: usize,
    u_plane: &[V],
    u_stride: usize,
    v_plane: &[V],
    v_stride: usize,
    rgb: &mut [V],
    bit_depth: u32,
    width: usize,
    height: usize,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) -> Result<(), String>
where
    i32: AsPrimitive<V>,
{
    if y_plane.len() != y_stride * height {
        return Err(format!(
            "Luma plane expected {} bytes, got {}",
            y_stride * height,
            y_plane.len()
        ));
    }

    if u_plane.len() != u_stride * height {
        return Err(format!(
            "U plane expected {} bytes, got {}",
            u_stride * height,
            u_plane.len()
        ));
    }

    if v_plane.len() != v_stride * height {
        return Err(format!(
            "V plane expected {} bytes, got {}",
            v_stride * height,
            v_plane.len()
        ));
    }

    let max_value = (1 << bit_depth) - 1;

    let range = get_yuv_range(bit_depth, range);
    let kr_kb = get_kr_kb(matrix);
    const PRECISION: i32 = 11;
    const ROUNDING_CONST: i32 = 1 << (PRECISION - 1);
    let inverse_transform = get_inverse_transform(
        (1 << bit_depth) - 1,
        range.range_y,
        range.range_uv,
        kr_kb.kr,
        kr_kb.kb,
        PRECISION as u32,
    )?;
    let cr_coef = inverse_transform.cr_coef;
    let cb_coef = inverse_transform.cb_coef;
    let y_coef = inverse_transform.y_coef;
    let g_coef_1 = inverse_transform.g_coeff_1;
    let g_coef_2 = inverse_transform.g_coeff_2;

    let bias_y = range.bias_y as i32;
    let bias_uv = range.bias_uv as i32;

    const CHANNELS: usize = 4;

    if rgb.len() != width * height * CHANNELS {
        return Err(format!(
            "RGB image layout expected {} bytes, got {}",
            width * height * CHANNELS,
            rgb.len()
        ));
    }

    let rgb_stride = width * CHANNELS;

    let y_iter = y_plane.chunks_exact(y_stride);
    let rgb_iter = rgb.chunks_exact_mut(rgb_stride);
    let u_iter = u_plane.chunks_exact(u_stride);
    let v_iter = v_plane.chunks_exact(v_stride);

    for (((y_src, u_src), v_src), rgb) in y_iter.zip(u_iter).zip(v_iter).zip(rgb_iter) {
        let y_iter = y_src.chunks_exact(2);
        let rgb_chunks = rgb.chunks_exact_mut(CHANNELS * 2);

        for (((y_src, u_src), v_src), rgb_dst) in y_iter.zip(u_src).zip(v_src).zip(rgb_chunks) {
            let y_value = (y_src[0].as_() - bias_y) * y_coef;
            let cb_value = u_src.as_() - bias_uv;
            let cr_value = v_src.as_() - bias_uv;

            let r =
                ((y_value + cr_coef * cr_value + ROUNDING_CONST) >> PRECISION).clamp(0, max_value);
            let b =
                ((y_value + cb_coef * cb_value + ROUNDING_CONST) >> PRECISION).clamp(0, max_value);
            let g = ((y_value - g_coef_1 * cr_value - g_coef_2 * cb_value + ROUNDING_CONST)
                >> PRECISION)
                .clamp(0, max_value);

            rgb_dst[0] = r.as_();
            rgb_dst[1] = g.as_();
            rgb_dst[2] = b.as_();
            rgb_dst[3] = max_value.as_();

            let y_value = (y_src[1].as_() - bias_y) * y_coef;

            let r =
                ((y_value + cr_coef * cr_value + ROUNDING_CONST) >> PRECISION).clamp(0, max_value);
            let b =
                ((y_value + cb_coef * cb_value + ROUNDING_CONST) >> PRECISION).clamp(0, max_value);
            let g = ((y_value - g_coef_1 * cr_value - g_coef_2 * cb_value + ROUNDING_CONST)
                >> PRECISION)
                .clamp(0, max_value);

            rgb_dst[4] = r.as_();
            rgb_dst[5] = g.as_();
            rgb_dst[6] = b.as_();
            rgb_dst[7] = max_value.as_();
        }

        // Process left pixels for odd images, this should work since luma must be always exact

        let y_left = y_src.chunks_exact(2).remainder();
        let rgb_chunks = rgb
            .chunks_exact_mut(CHANNELS * 2)
            .into_remainder()
            .chunks_exact_mut(CHANNELS);
        let u_iter = u_src.iter().rev();
        let v_iter = v_src.iter().rev();

        for (((y_src, u_src), v_src), rgb_dst) in
            y_left.iter().zip(u_iter).zip(v_iter).zip(rgb_chunks)
        {
            let y_value = (y_src.as_() - bias_y) * y_coef;
            let cb_value = u_src.as_() - bias_uv;
            let cr_value = v_src.as_() - bias_uv;

            let r =
                ((y_value + cr_coef * cr_value + ROUNDING_CONST) >> PRECISION).clamp(0, max_value);
            let b =
                ((y_value + cb_coef * cb_value + ROUNDING_CONST) >> PRECISION).clamp(0, max_value);
            let g = ((y_value - g_coef_1 * cr_value - g_coef_2 * cb_value + ROUNDING_CONST)
                >> PRECISION)
                .clamp(0, max_value);

            rgb_dst[0] = r.as_();
            rgb_dst[1] = g.as_();
            rgb_dst[2] = b.as_();
            rgb_dst[3] = max_value.as_();
        }
    }

    Ok(())
}
