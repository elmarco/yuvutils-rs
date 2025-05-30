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
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::avx2::avx2_rgba_to_nv;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
use crate::neon::neon_rgbx_to_nv_row;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use crate::sse::sse_rgba_to_nv_row;
use crate::yuv_support::*;

fn rgbx_to_nv<const ORIGIN_CHANNELS: u8, const UV_ORDER: u8, const SAMPLING: u8>(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgba: &[u8],
    rgba_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    let order: YuvNVOrder = UV_ORDER.into();
    let chroma_subsampling: YuvChromaSample = SAMPLING.into();
    let source_channels: YuvSourceChannels = ORIGIN_CHANNELS.into();
    let channels = source_channels.get_channels_count();
    let range = get_yuv_range(8, range);
    let kr_kb = matrix.get_kr_kb();
    let max_range_p8 = (1u32 << 8u32) - 1;
    let transform_precise = get_forward_transform(
        max_range_p8,
        range.range_y,
        range.range_uv,
        kr_kb.kr,
        kr_kb.kb,
    );
    const PRECISION: i32 = 8;
    let transform = transform_precise.to_integers(PRECISION as u32);
    const ROUNDING_CONST_BIAS: i32 = 1 << (PRECISION - 1);
    let bias_y = range.bias_y as i32 * (1 << PRECISION) + ROUNDING_CONST_BIAS;
    let bias_uv = range.bias_uv as i32 * (1 << PRECISION) + ROUNDING_CONST_BIAS;

    let iterator_step = match chroma_subsampling {
        YuvChromaSample::YUV420 => 2usize,
        YuvChromaSample::YUV422 => 2usize,
        YuvChromaSample::YUV444 => 1usize,
    };

    let mut y_offset = 0usize;
    let mut uv_offset = 0usize;
    let mut rgba_offset = 0usize;

    let i_bias_y = range.bias_y as i32;
    let i_cap_y = range.range_y as i32 + i_bias_y;
    let i_cap_uv = i_bias_y + range.range_uv as i32;

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    let _use_sse = std::arch::is_x86_feature_detected!("sse4.1");
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    let _use_avx2 = std::arch::is_x86_feature_detected!("avx2");

    for y in 0..height as usize {
        #[allow(unused_variables)]
        #[allow(unused_mut)]
        let mut cx = 0usize;
        let mut ux = 0usize;

        let compute_uv_row = chroma_subsampling == YuvChromaSample::YUV444
            || chroma_subsampling == YuvChromaSample::YUV422
            || y & 1 == 0;

        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        unsafe {
            if _use_avx2 {
                let offset = avx2_rgba_to_nv::<ORIGIN_CHANNELS, UV_ORDER, SAMPLING>(
                    y_plane,
                    y_offset,
                    uv_plane,
                    uv_offset,
                    rgba,
                    rgba_offset,
                    width,
                    &range,
                    &transform,
                    cx,
                    ux,
                    compute_uv_row,
                );
                cx = offset.cx;
                ux = offset.ux;
            }
            if _use_sse {
                let offset = sse_rgba_to_nv_row::<ORIGIN_CHANNELS, UV_ORDER, SAMPLING>(
                    y_plane,
                    y_offset,
                    uv_plane,
                    uv_offset,
                    rgba,
                    rgba_offset,
                    width,
                    &range,
                    &transform,
                    cx,
                    ux,
                    compute_uv_row,
                );
                cx = offset.cx;
                ux = offset.ux;
            }
        }

        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        unsafe {
            let offset = neon_rgbx_to_nv_row::<ORIGIN_CHANNELS, UV_ORDER, SAMPLING>(
                y_plane,
                y_offset,
                uv_plane,
                uv_offset,
                rgba,
                rgba_offset,
                width,
                &range,
                &transform,
                cx,
                ux,
                compute_uv_row,
            );
            cx = offset.cx;
            ux = offset.ux;
        }

        for x in (cx..width as usize).step_by(iterator_step) {
            let px = x * channels;
            let rgba_shift = rgba_offset + px;
            let source_slice = unsafe { rgba.get_unchecked(rgba_shift..) };
            let r0 = unsafe { *source_slice.get_unchecked(source_channels.get_r_channel_offset()) }
                as i32;
            let g0 = unsafe { *source_slice.get_unchecked(source_channels.get_g_channel_offset()) }
                as i32;
            let b0 = unsafe { *source_slice.get_unchecked(source_channels.get_b_channel_offset()) }
                as i32;

            let mut r1 = r0;
            let mut g1 = g0;
            let mut b1 = b0;

            match chroma_subsampling {
                YuvChromaSample::YUV420 | YuvChromaSample::YUV422 => {
                    let next_x = x + 1;
                    if next_x < width as usize {
                        let next_px = next_x * channels;
                        let rgba_shift = rgba_offset + next_px;
                        let source_slice = unsafe { rgba.get_unchecked(rgba_shift..) };
                        r1 = unsafe {
                            *source_slice.get_unchecked(source_channels.get_r_channel_offset())
                        } as i32;
                        g1 = unsafe {
                            *source_slice.get_unchecked(source_channels.get_g_channel_offset())
                        } as i32;
                        b1 = unsafe {
                            *source_slice.get_unchecked(source_channels.get_b_channel_offset())
                        } as i32;
                        let y_1 =
                            (r1 * transform.yr + g1 * transform.yg + b1 * transform.yb + bias_y)
                                >> PRECISION;
                        unsafe {
                            *y_plane.get_unchecked_mut(y_offset + next_x) =
                                y_1.clamp(i_bias_y, i_cap_y) as u8;
                        }
                    }
                }
                _ => {}
            }

            if compute_uv_row {
                let r = if chroma_subsampling == YuvChromaSample::YUV444 {
                    r0
                } else {
                    (r0 + r1 + 1) >> 1
                };
                let g = if chroma_subsampling == YuvChromaSample::YUV444 {
                    g0
                } else {
                    (g0 + g1 + 1) >> 1
                };
                let b = if chroma_subsampling == YuvChromaSample::YUV444 {
                    b0
                } else {
                    (b0 + b1 + 1) >> 1
                };
                let y_0 = (r0 * transform.yr + g0 * transform.yg + b0 * transform.yb + bias_y)
                    >> PRECISION;
                let cb = (r * transform.cb_r + g * transform.cb_g + b * transform.cb_b + bias_uv)
                    >> PRECISION;
                let cr = (r * transform.cr_r + g * transform.cr_g + b * transform.cr_b + bias_uv)
                    >> PRECISION;
                unsafe {
                    *y_plane.get_unchecked_mut(y_offset + x) = y_0.clamp(i_bias_y, i_cap_y) as u8;
                }
                let uv_pos = uv_offset + ux;
                unsafe {
                    *uv_plane.get_unchecked_mut(uv_pos + order.get_u_position()) =
                        cb.clamp(i_bias_y, i_cap_uv) as u8;
                    *uv_plane.get_unchecked_mut(uv_pos + order.get_v_position()) =
                        cr.clamp(i_bias_y, i_cap_uv) as u8;
                }
            }

            ux += 2;
        }

        y_offset += y_stride as usize;
        rgba_offset += rgba_stride as usize;
        match chroma_subsampling {
            YuvChromaSample::YUV420 => {
                if y & 1 == 1 {
                    uv_offset += uv_stride as usize;
                }
            }
            YuvChromaSample::YUV444 | YuvChromaSample::YUV422 => {
                uv_offset += uv_stride as usize;
            }
        }
    }
}

/// Convert RGB image data to YUV NV16 bi-planar format.
///
/// This function performs RGB to YUV conversion and stores the result in YUV NV16 bi-planar format,
/// with plane for Y (luminance), and bi-plane UV (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the UV plane.
/// * `rgb` - The input RGB image data slice.
/// * `rgb_stride` - The stride (bytes per row) for the RGB image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input RGB data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn rgb_to_yuv_nv16(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgb: &[u8],
    rgb_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Rgb as u8 },
        { YuvNVOrder::UV as u8 },
        { YuvChromaSample::YUV422 as u8 },
    >(
        y_plane, y_stride, uv_plane, uv_stride, rgb, rgb_stride, width, height, range, matrix,
    );
}

/// Convert RGB image data to YUV NV61 bi-planar format.
///
/// This function performs RGB to YUV conversion and stores the result in YUV NV61 bi-planar format,
/// with plane for Y (luminance), and bi-plane VU (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the VU (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the VU plane.
/// * `rgb` - The input RGB image data slice.
/// * `rgb_stride` - The stride (bytes per row) for the RGB image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input RGB data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn rgb_to_yuv_nv61(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgb: &[u8],
    rgb_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Rgb as u8 },
        { YuvNVOrder::VU as u8 },
        { YuvChromaSample::YUV422 as u8 },
    >(
        y_plane, y_stride, uv_plane, uv_stride, rgb, rgb_stride, width, height, range, matrix,
    );
}

/// Convert BGR image data to YUV NV16 bi-planar format.
///
/// This function performs BGR to YUV conversion and stores the result in YUV NV16 bi-planar format,
/// with plane for Y (luminance), and bi-plane UV (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the UV plane.
/// * `rgb` - The input BGR image data slice.
/// * `rgb_stride` - The stride (bytes per row) for the BGR image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input BGR data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn bgr_to_yuv_nv16(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    bgr: &[u8],
    bgr_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Bgr as u8 },
        { YuvNVOrder::UV as u8 },
        { YuvChromaSample::YUV422 as u8 },
    >(
        y_plane, y_stride, uv_plane, uv_stride, bgr, bgr_stride, width, height, range, matrix,
    );
}

/// Convert BGR image data to YUV NV61 bi-planar format.
///
/// This function performs BGR to YUV conversion and stores the result in YUV NV61 bi-planar format,
/// with plane for Y (luminance), and bi-plane VU (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the VU (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the VU plane.
/// * `rgb` - The input BGR image data slice.
/// * `rgb_stride` - The stride (bytes per row) for the BGR image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input BGR data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn bgr_to_yuv_nv61(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    bgr: &[u8],
    bgr_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Bgr as u8 },
        { YuvNVOrder::VU as u8 },
        { YuvChromaSample::YUV422 as u8 },
    >(
        y_plane, y_stride, uv_plane, uv_stride, bgr, bgr_stride, width, height, range, matrix,
    );
}

/// Convert RGBA image data to YUV NV16 bi-planar format.
///
/// This function performs RGBA to YUV conversion and stores the result in YUV NV16 bi-planar format,
/// with plane for Y (luminance), and bi-plane UV (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the UV plane.
/// * `rgba` - The input RGBA image data slice.
/// * `rgba_stride` - The stride (bytes per row) for the RGBA image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input RGBA data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn rgba_to_yuv_nv16(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgba: &[u8],
    rgba_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Rgba as u8 },
        { YuvNVOrder::UV as u8 },
        { YuvChromaSample::YUV422 as u8 },
    >(
        y_plane,
        y_stride,
        uv_plane,
        uv_stride,
        rgba,
        rgba_stride,
        width,
        height,
        range,
        matrix,
    );
}

/// Convert RGBA image data to YUV NV61 bi-planar format.
///
/// This function performs RGBA to YUV conversion and stores the result in YUV NV61 bi-planar format,
/// with plane for Y (luminance), and bi-plane VU (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the VU (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the VU plane.
/// * `rgba` - The input RGBA image data slice.
/// * `rgba_stride` - The stride (bytes per row) for the RGBA image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input RGBA data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn rgba_to_yuv_nv61(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgba: &[u8],
    rgba_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Rgba as u8 },
        { YuvNVOrder::VU as u8 },
        { YuvChromaSample::YUV422 as u8 },
    >(
        y_plane,
        y_stride,
        uv_plane,
        uv_stride,
        rgba,
        rgba_stride,
        width,
        height,
        range,
        matrix,
    );
}

/// Convert BGRA image data to YUV NV16 bi-planar format.
///
/// This function performs BGRA to YUV conversion and stores the result in YUV NV16 bi-planar format,
/// with plane for Y (luminance), and bi-plane UV (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the UV plane.
/// * `bgra` - The input BGRA image data slice.
/// * `bgra_stride` - The stride (bytes per row) for the BGRA image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input BGRA data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn bgra_to_yuv_nv16(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    bgra: &[u8],
    bgra_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Bgra as u8 },
        { YuvNVOrder::UV as u8 },
        { YuvChromaSample::YUV422 as u8 },
    >(
        y_plane,
        y_stride,
        uv_plane,
        uv_stride,
        bgra,
        bgra_stride,
        width,
        height,
        range,
        matrix,
    );
}

/// Convert BGRA image data to YUV NV61 bi-planar format.
///
/// This function performs BGRA to YUV conversion and stores the result in YUV NV61 bi-planar format,
/// with plane for Y (luminance), and bi-plane VU (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the VU (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the VU plane.
/// * `bgra` - The input BGRA image data slice.
/// * `bgra_stride` - The stride (bytes per row) for the BGRA image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input BGRA data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn bgra_to_yuv_nv61(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    bgra: &[u8],
    bgra_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Bgra as u8 },
        { YuvNVOrder::VU as u8 },
        { YuvChromaSample::YUV422 as u8 },
    >(
        y_plane,
        y_stride,
        uv_plane,
        uv_stride,
        bgra,
        bgra_stride,
        width,
        height,
        range,
        matrix,
    );
}

/// Convert RGB image data to YUV NV12 bi-planar format.
///
/// This function performs RGB to YUV conversion and stores the result in YUV NV12 bi-planar format,
/// with plane for Y (luminance), and bi-plane UV (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the UV plane.
/// * `rgb` - The input RGB image data slice.
/// * `rgb_stride` - The stride (bytes per row) for the RGB image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input RGB data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn rgb_to_yuv_nv12(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgb: &[u8],
    rgb_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Rgb as u8 },
        { YuvNVOrder::UV as u8 },
        { YuvChromaSample::YUV420 as u8 },
    >(
        y_plane, y_stride, uv_plane, uv_stride, rgb, rgb_stride, width, height, range, matrix,
    );
}

/// Convert RGB image data to YUV NV21 bi-planar format.
///
/// This function performs RGB to YUV conversion and stores the result in YUV NV21 bi-planar format,
/// with plane for Y (luminance), and bi-plane VU (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the VU (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the VU plane.
/// * `rgb` - The input RGB image data slice.
/// * `rgb_stride` - The stride (bytes per row) for the RGB image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input RGB data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn rgb_to_yuv_nv21(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgb: &[u8],
    rgb_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Rgb as u8 },
        { YuvNVOrder::VU as u8 },
        { YuvChromaSample::YUV420 as u8 },
    >(
        y_plane, y_stride, uv_plane, uv_stride, rgb, rgb_stride, width, height, range, matrix,
    );
}

/// Convert BGR image data to YUV NV12 bi-planar format.
///
/// This function performs BGR to YUV conversion and stores the result in YUV NV12 bi-planar format,
/// with plane for Y (luminance), and bi-plane UV (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the UV plane.
/// * `bgr` - The input BGR image data slice.
/// * `bgr_stride` - The stride (bytes per row) for the BGR image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input BGR data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn bgr_to_yuv_nv12(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    bgr: &[u8],
    bgr_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Bgr as u8 },
        { YuvNVOrder::UV as u8 },
        { YuvChromaSample::YUV420 as u8 },
    >(
        y_plane, y_stride, uv_plane, uv_stride, bgr, bgr_stride, width, height, range, matrix,
    );
}

/// Convert BGR image data to YUV NV21 bi-planar format.
///
/// This function performs BGR to YUV conversion and stores the result in YUV NV21 bi-planar format,
/// with plane for Y (luminance), and bi-plane VU (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the VU (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the VU plane.
/// * `bgr` - The input BGR image data slice.
/// * `bgr_stride` - The stride (bytes per row) for the BGR image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input BGR data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn bgr_to_yuv_nv21(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    bgr: &[u8],
    bgr_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Bgr as u8 },
        { YuvNVOrder::VU as u8 },
        { YuvChromaSample::YUV420 as u8 },
    >(
        y_plane, y_stride, uv_plane, uv_stride, bgr, bgr_stride, width, height, range, matrix,
    );
}

/// Convert RGBA image data to YUV NV12 bi-planar format.
///
/// This function performs RGBA to YUV conversion and stores the result in YUV NV12 bi-planar format,
/// with plane for Y (luminance), and bi-plane UV (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the UV plane.
/// * `rgba` - The input RGBA image data slice.
/// * `rgba_stride` - The stride (bytes per row) for the RGBA image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input RGBA data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn rgba_to_yuv_nv12(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgba: &[u8],
    rgba_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Rgba as u8 },
        { YuvNVOrder::UV as u8 },
        { YuvChromaSample::YUV420 as u8 },
    >(
        y_plane,
        y_stride,
        uv_plane,
        uv_stride,
        rgba,
        rgba_stride,
        width,
        height,
        range,
        matrix,
    );
}

/// Convert RGBA image data to YUV NV21 bi-planar format.
///
/// This function performs RGBA to YUV conversion and stores the result in YUV NV21 bi-planar format,
/// with plane for Y (luminance), and bi-plane VU (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the VU (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the VU plane.
/// * `rgba` - The input RGBA image data slice.
/// * `rgba_stride` - The stride (bytes per row) for the RGBA image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input RGBA data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn rgba_to_yuv_nv21(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgba: &[u8],
    rgba_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Rgba as u8 },
        { YuvNVOrder::VU as u8 },
        { YuvChromaSample::YUV420 as u8 },
    >(
        y_plane,
        y_stride,
        uv_plane,
        uv_stride,
        rgba,
        rgba_stride,
        width,
        height,
        range,
        matrix,
    );
}

/// Convert BGRA image data to YUV NV12 bi-planar format.
///
/// This function performs BGRA to YUV conversion and stores the result in YUV NV12 bi-planar format,
/// with plane for Y (luminance), and bi-plane UV (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the UV plane.
/// * `bgra` - The input BGRA image data slice.
/// * `bgra_stride` - The stride (bytes per row) for the BGRA image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input BGRA data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn bgra_to_yuv_nv12(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    bgra: &[u8],
    bgra_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Bgra as u8 },
        { YuvNVOrder::UV as u8 },
        { YuvChromaSample::YUV420 as u8 },
    >(
        y_plane,
        y_stride,
        uv_plane,
        uv_stride,
        bgra,
        bgra_stride,
        width,
        height,
        range,
        matrix,
    );
}

/// Convert BGRA image data to YUV NV21 bi-planar format.
///
/// This function performs BGRA to YUV conversion and stores the result in YUV NV21 bi-planar format,
/// with plane for Y (luminance), and bi-plane VU (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the VU (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the VU plane.
/// * `bgra` - The input BGRA image data slice.
/// * `bgra_stride` - The stride (bytes per row) for the BGRA image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input BGRA data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn bgra_to_yuv_nv21(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    bgra: &[u8],
    bgra_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Bgra as u8 },
        { YuvNVOrder::VU as u8 },
        { YuvChromaSample::YUV420 as u8 },
    >(
        y_plane,
        y_stride,
        uv_plane,
        uv_stride,
        bgra,
        bgra_stride,
        width,
        height,
        range,
        matrix,
    );
}

/// Convert RGB image data to YUV NV24 bi-planar format.
///
/// This function performs RGB to YUV conversion and stores the result in YUV NV24 bi-planar format,
/// with plane for Y (luminance), and bi-plane UV (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the UV plane.
/// * `rgb` - The input RGB image data slice.
/// * `rgb_stride` - The stride (bytes per row) for the RGB image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input RGB data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn rgb_to_yuv_nv24(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgb: &[u8],
    rgb_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Rgb as u8 },
        { YuvNVOrder::UV as u8 },
        { YuvChromaSample::YUV444 as u8 },
    >(
        y_plane, y_stride, uv_plane, uv_stride, rgb, rgb_stride, width, height, range, matrix,
    );
}

/// Convert RGB image data to YUV NV42 bi-planar format.
///
/// This function performs RGB to YUV conversion and stores the result in YUV NV42 bi-planar format,
/// with plane for Y (luminance), and bi-plane VU (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the VU (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the VU plane.
/// * `rgb` - The input RGB image data slice.
/// * `rgb_stride` - The stride (bytes per row) for the RGB image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input RGB data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn rgb_to_yuv_nv42(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgb: &[u8],
    rgb_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Rgb as u8 },
        { YuvNVOrder::VU as u8 },
        { YuvChromaSample::YUV444 as u8 },
    >(
        y_plane, y_stride, uv_plane, uv_stride, rgb, rgb_stride, width, height, range, matrix,
    );
}

/// Convert BGR image data to YUV NV24 bi-planar format.
///
/// This function performs BGR to YUV conversion and stores the result in YUV NV24 bi-planar format,
/// with plane for Y (luminance), and bi-plane UV (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the UV plane.
/// * `bgr` - The input BGR image data slice.
/// * `bgr_stride` - The stride (bytes per row) for the BGR image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input BGR data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn bgr_to_yuv_nv24(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    bgr: &[u8],
    bgr_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Bgr as u8 },
        { YuvNVOrder::UV as u8 },
        { YuvChromaSample::YUV444 as u8 },
    >(
        y_plane, y_stride, uv_plane, uv_stride, bgr, bgr_stride, width, height, range, matrix,
    );
}

/// Convert BGR image data to YUV NV42 bi-planar format.
///
/// This function performs BGR to YUV conversion and stores the result in YUV NV42 bi-planar format,
/// with plane for Y (luminance), and bi-plane VU (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the VU (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the VU plane.
/// * `bgr` - The input BGR image data slice.
/// * `bgr_stride` - The stride (bytes per row) for the BGR image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input BGR data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn bgr_to_yuv_nv42(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    bgr: &[u8],
    bgr_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Bgr as u8 },
        { YuvNVOrder::VU as u8 },
        { YuvChromaSample::YUV444 as u8 },
    >(
        y_plane, y_stride, uv_plane, uv_stride, bgr, bgr_stride, width, height, range, matrix,
    );
}

/// Convert RGBA image data to YUV NV24 bi-planar format.
///
/// This function performs RGBA to YUV conversion and stores the result in YUV NV24 bi-planar format,
/// with plane for Y (luminance), and bi-plane UV (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the UV plane.
/// * `rgba` - The input RGBA image data slice.
/// * `rgba_stride` - The stride (bytes per row) for the RGBA image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input RGBA data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn rgba_to_yuv_nv24(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgba: &[u8],
    rgba_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Rgba as u8 },
        { YuvNVOrder::UV as u8 },
        { YuvChromaSample::YUV444 as u8 },
    >(
        y_plane,
        y_stride,
        uv_plane,
        uv_stride,
        rgba,
        rgba_stride,
        width,
        height,
        range,
        matrix,
    );
}

/// Convert RGBA image data to YUV NV42 bi-planar format.
///
/// This function performs RGBA to YUV conversion and stores the result in YUV NV42 bi-planar format,
/// with plane for Y (luminance), and bi-plane VU (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the VU plane.
/// * `rgba` - The input RGBA image data slice.
/// * `rgba_stride` - The stride (bytes per row) for the RGBA image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input RGBA data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn rgba_to_yuv_nv42(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    rgba: &[u8],
    rgba_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Rgba as u8 },
        { YuvNVOrder::VU as u8 },
        { YuvChromaSample::YUV444 as u8 },
    >(
        y_plane,
        y_stride,
        uv_plane,
        uv_stride,
        rgba,
        rgba_stride,
        width,
        height,
        range,
        matrix,
    );
}

/// Convert BGRA image data to YUV NV24 bi-planar format.
///
/// This function performs BGRA to YUV conversion and stores the result in YUV NV24 bi-planar format,
/// with plane for Y (luminance), and bi-plane UV (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the UV (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the UV plane.
/// * `bgra` - The input BGRA image data slice.
/// * `bgra_stride` - The stride (bytes per row) for the BGRA image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input BGRA data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn bgra_to_yuv_nv24(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    bgra: &[u8],
    bgra_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Bgra as u8 },
        { YuvNVOrder::UV as u8 },
        { YuvChromaSample::YUV444 as u8 },
    >(
        y_plane,
        y_stride,
        uv_plane,
        uv_stride,
        bgra,
        bgra_stride,
        width,
        height,
        range,
        matrix,
    );
}

/// Convert BGRA image data to YUV NV42 bi-planar format.
///
/// This function performs BGRA to YUV conversion and stores the result in YUV NV42 bi-planar format,
/// with plane for Y (luminance), and bi-plane VU (chrominance) components.
///
/// # Arguments
///
/// * `y_plane` - A mutable slice to store the Y (luminance) plane data.
/// * `y_stride` - The stride (bytes per row) for the Y plane.
/// * `uv_plane` - A mutable slice to store the VU (chrominance) plane data.
/// * `uv_stride` - The stride (bytes per row) for the VU plane.
/// * `bgra` - The input BGRA image data slice.
/// * `bgra_stride` - The stride (bytes per row) for the BGRA image data.
/// * `width` - The width of the image in pixels.
/// * `height` - The height of the image in pixels.
/// * `range` - The YUV range (limited or full).
/// * `matrix` - The YUV standard matrix (BT.601 or BT.709 or BT.2020 or other).
///
/// # Panics
///
/// This function panics if the lengths of the planes or the input BGRA data are not valid based
/// on the specified width, height, and strides, or if invalid YUV range or matrix is provided.
///
pub fn bgra_to_yuv_nv42(
    y_plane: &mut [u8],
    y_stride: u32,
    uv_plane: &mut [u8],
    uv_stride: u32,
    bgra: &[u8],
    bgra_stride: u32,
    width: u32,
    height: u32,
    range: YuvRange,
    matrix: YuvStandardMatrix,
) {
    rgbx_to_nv::<
        { YuvSourceChannels::Bgra as u8 },
        { YuvNVOrder::VU as u8 },
        { YuvChromaSample::YUV444 as u8 },
    >(
        y_plane,
        y_stride,
        uv_plane,
        uv_stride,
        bgra,
        bgra_stride,
        width,
        height,
        range,
        matrix,
    );
}
