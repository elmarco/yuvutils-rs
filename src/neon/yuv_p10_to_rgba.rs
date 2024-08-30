/*
 * // Copyright (c) the Radzivon Bartoshyk. All rights reserved.
 * //
 * // Use of this source code is governed by a BSD-style
 * // license that can be found in the LICENSE file.
 */

use std::arch::aarch64::*;

use crate::internals::ProcessedOffset;
use crate::yuv_support::{
    CbCrInverseTransform, YuvBytesPacking, YuvChromaRange, YuvChromaSample, YuvEndian,
    YuvSourceChannels,
};

#[inline(always)]
pub unsafe fn neon_yuv_p10_to_rgba_row<
    const DESTINATION_CHANNELS: u8,
    const SAMPLING: u8,
    const ENDIANNESS: u8,
    const BYTES_POSITION: u8,
>(
    y_ld_ptr: *const u16,
    u_ld_ptr: *const u16,
    v_ld_ptr: *const u16,
    rgba: &mut [u8],
    dst_offset: usize,
    width: u32,
    range: &YuvChromaRange,
    transform: &CbCrInverseTransform<i32>,
    start_cx: usize,
    start_ux: usize,
) -> ProcessedOffset {
    let destination_channels: YuvSourceChannels = DESTINATION_CHANNELS.into();
    let channels = destination_channels.get_channels_count();
    let chroma_subsampling: YuvChromaSample = SAMPLING.into();
    let endianness: YuvEndian = ENDIANNESS.into();
    let bytes_position: YuvBytesPacking = BYTES_POSITION.into();
    let dst_ptr = rgba.as_mut_ptr();

    let y_corr = vdupq_n_s16(range.bias_y as i16);
    let uv_corr = vdup_n_s16(range.bias_uv as i16);
    let v_luma_coeff = vdupq_n_s16(transform.y_coef as i16);
    let v_cr_coeff = vdup_n_s16(transform.cr_coef as i16);
    let v_cb_coeff = vdup_n_s16(transform.cb_coef as i16);
    let v_min_values = vdupq_n_s16(0i16);
    let v_g_coeff_1 = vdup_n_s16(-1i16 * (transform.g_coeff_1 as i16));
    let v_g_coeff_2 = vdup_n_s16(-1i16 * (transform.g_coeff_2 as i16));
    let v_alpha = vdup_n_u8(255u8);

    let mut cx = start_cx;
    let mut ux = start_ux;

    while cx + 8 < width as usize {
        let y_values: int16x8_t;

        let u_values_c: int16x4_t;
        let v_values_c: int16x4_t;

        let u_values_l = vld1_u16(u_ld_ptr.add(ux));
        let v_values_l = vld1_u16(v_ld_ptr.add(ux));

        match endianness {
            YuvEndian::BigEndian => {
                let mut y_u_values = vreinterpretq_u16_u8(vrev16q_u8(vreinterpretq_u8_u16(
                    vld1q_u16(y_ld_ptr.add(cx)),
                )));
                if bytes_position == YuvBytesPacking::MostSignificantBytes {
                    y_u_values = vshrq_n_u16::<6>(y_u_values);
                }
                y_values = vsubq_s16(vreinterpretq_s16_u16(y_u_values), y_corr);

                let mut u_v = vreinterpret_u16_u8(vrev16_u8(vreinterpret_u8_u16(u_values_l)));
                let mut v_v = vreinterpret_u16_u8(vrev16_u8(vreinterpret_u8_u16(v_values_l)));
                if bytes_position == YuvBytesPacking::MostSignificantBytes {
                    u_v = vshr_n_u16::<6>(u_v);
                    v_v = vshr_n_u16::<6>(v_v);
                }
                u_values_c = vsub_s16(vreinterpret_s16_u16(u_v), uv_corr);
                v_values_c = vsub_s16(vreinterpret_s16_u16(v_v), uv_corr);
            }
            YuvEndian::LittleEndian => {
                let mut y_vl = vld1q_u16(y_ld_ptr.add(cx));
                if bytes_position == YuvBytesPacking::MostSignificantBytes {
                    y_vl = vshrq_n_u16::<6>(y_vl);
                }
                y_values = vsubq_s16(vreinterpretq_s16_u16(y_vl), y_corr);

                let mut u_vl = u_values_l;
                let mut v_vl = v_values_l;
                if bytes_position == YuvBytesPacking::MostSignificantBytes {
                    u_vl = vshr_n_u16::<6>(u_vl);
                    v_vl = vshr_n_u16::<6>(v_vl);
                }
                u_values_c = vsub_s16(vreinterpret_s16_u16(u_vl), uv_corr);
                v_values_c = vsub_s16(vreinterpret_s16_u16(v_vl), uv_corr);
            }
        }

        let u_high = vzip2_s16(u_values_c, u_values_c);
        let v_high = vzip2_s16(v_values_c, v_values_c);

        let y_high = vmull_high_s16(y_values, v_luma_coeff);

        let r_high = vshrn_n_s32::<6>(vmlal_s16(y_high, v_high, v_cr_coeff));
        let b_high = vshrn_n_s32::<6>(vmlal_s16(y_high, u_high, v_cb_coeff));
        let g_high = vshrn_n_s32::<6>(vmlal_s16(
            vmlal_s16(y_high, v_high, v_g_coeff_1),
            u_high,
            v_g_coeff_2,
        ));

        let y_low = vmull_s16(vget_low_s16(y_values), vget_low_s16(v_luma_coeff));
        let u_low = vzip1_s16(u_values_c, u_values_c);
        let v_low = vzip1_s16(v_values_c, v_values_c);

        let r_low = vshrn_n_s32::<6>(vmlal_s16(y_low, v_low, v_cr_coeff));
        let b_low = vshrn_n_s32::<6>(vmlal_s16(y_low, u_low, v_cb_coeff));
        let g_low = vshrn_n_s32::<6>(vmlal_s16(
            vmlal_s16(y_low, v_low, v_g_coeff_1),
            u_low,
            v_g_coeff_2,
        ));

        let r_values = vqshrun_n_s16::<2>(vmaxq_s16(vcombine_s16(r_low, r_high), v_min_values));
        let g_values = vqshrun_n_s16::<2>(vmaxq_s16(vcombine_s16(g_low, g_high), v_min_values));
        let b_values = vqshrun_n_s16::<2>(vmaxq_s16(vcombine_s16(b_low, b_high), v_min_values));

        match destination_channels {
            YuvSourceChannels::Rgb => {
                let dst_pack: uint8x8x3_t = uint8x8x3_t(r_values, g_values, b_values);
                vst3_u8(dst_ptr.add(dst_offset + cx * channels), dst_pack);
            }
            YuvSourceChannels::Bgr => {
                let dst_pack: uint8x8x3_t = uint8x8x3_t(b_values, g_values, r_values);
                vst3_u8(dst_ptr.add(dst_offset + cx * channels), dst_pack);
            }
            YuvSourceChannels::Rgba => {
                let dst_pack: uint8x8x4_t = uint8x8x4_t(r_values, g_values, b_values, v_alpha);
                vst4_u8(dst_ptr.add(dst_offset + cx * channels), dst_pack);
            }
            YuvSourceChannels::Bgra => {
                let dst_pack: uint8x8x4_t = uint8x8x4_t(b_values, g_values, r_values, v_alpha);
                vst4_u8(dst_ptr.add(dst_offset + cx * channels), dst_pack);
            }
        }

        cx += 8;

        match chroma_subsampling {
            YuvChromaSample::YUV420 | YuvChromaSample::YUV422 => {
                ux += 4;
            }
            YuvChromaSample::YUV444 => {
                ux += 8;
            }
        }
    }

    ProcessedOffset { cx, ux }
}
