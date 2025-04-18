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

use crate::internals::ProcessedOffset;
use crate::sse::sse_support::{sse_div_by255, sse_store_rgb_u8, sse_store_rgba};
use crate::yuv_support::{
    CbCrInverseTransform, YuvChromaRange, YuvChromaSample, YuvSourceChannels,
};
#[cfg(target_arch = "x86")]
use core::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use core::arch::x86_64::*;

#[target_feature(enable = "sse4.1")]
pub unsafe fn sse_yuv_to_rgba_alpha_row<const DESTINATION_CHANNELS: u8, const SAMPLING: u8>(
    range: &YuvChromaRange,
    transform: &CbCrInverseTransform<i32>,
    y_plane: &[u8],
    u_plane: &[u8],
    v_plane: &[u8],
    a_plane: &[u8],
    rgba: &mut [u8],
    start_cx: usize,
    start_ux: usize,
    y_offset: usize,
    u_offset: usize,
    v_offset: usize,
    a_offset: usize,
    rgba_offset: usize,
    width: usize,
    use_premultiply: bool,
) -> ProcessedOffset {
    let chroma_subsampling: YuvChromaSample = SAMPLING.into();
    let destination_channels: YuvSourceChannels = DESTINATION_CHANNELS.into();
    let channels = destination_channels.get_channels_count();

    let mut cx = start_cx;
    let mut uv_x = start_ux;

    let y_ptr = y_plane.as_ptr();
    let u_ptr = u_plane.as_ptr();
    let v_ptr = v_plane.as_ptr();
    let a_ptr = a_plane.as_ptr();
    let rgba_ptr = rgba.as_mut_ptr();

    let y_corr = _mm_set1_epi8(range.bias_y as i8);
    let uv_corr = _mm_set1_epi16(range.bias_uv as i16);
    let v_luma_coeff = _mm_set1_epi16(transform.y_coef as i16);
    let v_cr_coeff = _mm_set1_epi16(transform.cr_coef as i16);
    let v_cb_coeff = _mm_set1_epi16(transform.cb_coef as i16);
    let v_min_values = _mm_setzero_si128();
    let v_g_coeff_1 = _mm_set1_epi16(-(transform.g_coeff_1 as i16));
    let v_g_coeff_2 = _mm_set1_epi16(-(transform.g_coeff_2 as i16));
    let rounding_const = _mm_set1_epi16(1 << 5);

    let zeros = _mm_setzero_si128();

    while cx + 16 < width {
        let y_values = _mm_subs_epu8(
            _mm_loadu_si128(y_ptr.add(y_offset + cx) as *const __m128i),
            y_corr,
        );

        let a_values = _mm_loadu_si128(a_ptr.add(a_offset + cx) as *const __m128i);

        let (u_high_u16, v_high_u16, u_low_u16, v_low_u16);

        match chroma_subsampling {
            YuvChromaSample::YUV420 | YuvChromaSample::YUV422 => {
                let reshuffle = _mm_setr_epi8(0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7);
                let (u_values, v_values);
                u_values = _mm_shuffle_epi8(_mm_loadu_si64(u_ptr.add(u_offset + uv_x)), reshuffle);
                v_values = _mm_shuffle_epi8(_mm_loadu_si64(v_ptr.add(v_offset + uv_x)), reshuffle);

                u_high_u16 = _mm_unpackhi_epi8(u_values, zeros);
                v_high_u16 = _mm_unpackhi_epi8(v_values, zeros);
                u_low_u16 = _mm_unpacklo_epi8(u_values, zeros);
                v_low_u16 = _mm_unpacklo_epi8(v_values, zeros);
            }
            YuvChromaSample::YUV444 => {
                let u_values = _mm_loadu_si128(u_ptr.add(u_offset + uv_x) as *const __m128i);
                let v_values = _mm_loadu_si128(v_ptr.add(v_offset + uv_x) as *const __m128i);

                u_high_u16 = _mm_unpackhi_epi8(u_values, zeros);
                v_high_u16 = _mm_unpackhi_epi8(v_values, zeros);
                u_low_u16 = _mm_unpacklo_epi8(u_values, zeros);
                v_low_u16 = _mm_unpacklo_epi8(v_values, zeros);
            }
        }

        let u_high = _mm_subs_epi16(u_high_u16, uv_corr);
        let v_high = _mm_subs_epi16(v_high_u16, uv_corr);
        let y_high = _mm_mullo_epi16(_mm_unpackhi_epi8(y_values, zeros), v_luma_coeff);

        let r_high = _mm_srai_epi16::<6>(_mm_adds_epi16(
            _mm_max_epi16(
                _mm_adds_epi16(y_high, _mm_mullo_epi16(v_high, v_cr_coeff)),
                v_min_values,
            ),
            rounding_const,
        ));
        let b_high = _mm_srai_epi16::<6>(_mm_adds_epi16(
            _mm_max_epi16(
                _mm_adds_epi16(y_high, _mm_mullo_epi16(u_high, v_cb_coeff)),
                v_min_values,
            ),
            rounding_const,
        ));
        let g_high = _mm_srai_epi16::<6>(_mm_adds_epi16(
            _mm_max_epi16(
                _mm_adds_epi16(
                    y_high,
                    _mm_adds_epi16(
                        _mm_mullo_epi16(v_high, v_g_coeff_1),
                        _mm_mullo_epi16(u_high, v_g_coeff_2),
                    ),
                ),
                v_min_values,
            ),
            rounding_const,
        ));

        let u_low = _mm_sub_epi16(u_low_u16, uv_corr);
        let v_low = _mm_sub_epi16(v_low_u16, uv_corr);
        let y_low = _mm_mullo_epi16(_mm_cvtepu8_epi16(y_values), v_luma_coeff);

        let r_low = _mm_srai_epi16::<6>(_mm_adds_epi16(
            _mm_max_epi16(
                _mm_adds_epi16(y_low, _mm_mullo_epi16(v_low, v_cr_coeff)),
                v_min_values,
            ),
            rounding_const,
        ));
        let b_low = _mm_srai_epi16::<6>(_mm_adds_epi16(
            _mm_max_epi16(
                _mm_adds_epi16(y_low, _mm_mullo_epi16(u_low, v_cb_coeff)),
                v_min_values,
            ),
            rounding_const,
        ));
        let g_low = _mm_srai_epi16::<6>(_mm_adds_epi16(
            _mm_max_epi16(
                _mm_adds_epi16(
                    y_low,
                    _mm_adds_epi16(
                        _mm_mullo_epi16(v_low, v_g_coeff_1),
                        _mm_mullo_epi16(u_low, v_g_coeff_2),
                    ),
                ),
                v_min_values,
            ),
            rounding_const,
        ));

        let (r_values, g_values, b_values);

        if use_premultiply {
            let a_h = _mm_cvtepu8_epi16(_mm_srli_si128::<8>(a_values));
            let a_l = _mm_cvtepu8_epi16(a_values);
            let r_h_16 = sse_div_by255(_mm_mullo_epi16(r_high, a_h));
            let r_l_16 = sse_div_by255(_mm_mullo_epi16(r_low, a_l));
            let g_h_16 = sse_div_by255(_mm_mullo_epi16(g_high, a_h));
            let g_l_16 = sse_div_by255(_mm_mullo_epi16(g_low, a_l));
            let b_h_16 = sse_div_by255(_mm_mullo_epi16(b_high, a_h));
            let b_l_16 = sse_div_by255(_mm_mullo_epi16(b_low, a_l));

            r_values = _mm_packus_epi16(r_l_16, r_h_16);
            g_values = _mm_packus_epi16(g_l_16, g_h_16);
            b_values = _mm_packus_epi16(b_l_16, b_h_16);
        } else {
            r_values = _mm_packus_epi16(r_low, r_high);
            g_values = _mm_packus_epi16(g_low, g_high);
            b_values = _mm_packus_epi16(b_low, b_high);
        }

        let dst_shift = rgba_offset + cx * channels;

        match destination_channels {
            YuvSourceChannels::Rgb => {
                sse_store_rgb_u8(rgba_ptr.add(dst_shift), r_values, g_values, b_values);
            }
            YuvSourceChannels::Bgr => {
                sse_store_rgb_u8(rgba_ptr.add(dst_shift), b_values, g_values, r_values);
            }
            YuvSourceChannels::Rgba => {
                sse_store_rgba(
                    rgba_ptr.add(dst_shift),
                    r_values,
                    g_values,
                    b_values,
                    a_values,
                );
            }
            YuvSourceChannels::Bgra => {
                sse_store_rgba(
                    rgba_ptr.add(dst_shift),
                    b_values,
                    g_values,
                    r_values,
                    a_values,
                );
            }
        }

        cx += 16;

        match chroma_subsampling {
            YuvChromaSample::YUV420 | YuvChromaSample::YUV422 => {
                uv_x += 8;
            }
            YuvChromaSample::YUV444 => {
                uv_x += 16;
            }
        }
    }

    ProcessedOffset { cx, ux: uv_x }
}
