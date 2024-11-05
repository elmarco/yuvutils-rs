/*
 * Copyright (c) Radzivon Bartoshyk, 11/2024. All rights reserved.
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
use crate::yuv_support::{CbCrForwardTransform, YuvSourceChannels};
use std::arch::aarch64::*;

#[inline(always)]
pub unsafe fn rdp_neon_rgba_to_yuv<const ORIGIN_CHANNELS: u8, const PRECISION: i32>(
    transform: &CbCrForwardTransform<i32>,
    y_plane: *mut u16,
    u_plane: *mut u16,
    v_plane: *mut u16,
    rgba: &[u8],
    start_cx: usize,
    start_ux: usize,
    width: usize,
) -> ProcessedOffset {
    let source_channels: YuvSourceChannels = ORIGIN_CHANNELS.into();
    let channels = source_channels.get_channels_count();

    const V_SCALE: i32 = 7;

    let y_ptr = y_plane;
    let u_ptr = u_plane;
    let v_ptr = v_plane;
    let rgba_ptr = rgba.as_ptr();

    let i_bias = vdupq_n_s16(-4096);
    let i_cap = vdupq_n_s16(4095);

    let y_bias = vdupq_n_s16(-4096);
    let uv_bias = vdupq_n_s16(0);
    let v_yr = vdupq_n_s16(transform.yr as i16);
    let v_yg = vdupq_n_s16(transform.yg as i16);
    let v_yb = vdupq_n_s16(transform.yb as i16);
    let v_cb_r = vdupq_n_s16(transform.cb_r as i16);
    let v_cb_g = vdupq_n_s16(transform.cb_g as i16);
    let v_cb_b = vdupq_n_s16(transform.cb_b as i16);
    let v_cr_r = vdupq_n_s16(transform.cr_r as i16);
    let v_cr_g = vdupq_n_s16(transform.cr_g as i16);
    let v_cr_b = vdupq_n_s16(transform.cr_b as i16);

    let mut cx = start_cx;
    let mut ux = start_ux;

    while cx + 16 < width {
        let r_values_u8: uint8x16_t;
        let g_values_u8: uint8x16_t;
        let b_values_u8: uint8x16_t;

        match source_channels {
            YuvSourceChannels::Rgb | YuvSourceChannels::Bgr => {
                let rgb_values = vld3q_u8(rgba_ptr.add(cx * channels));
                if source_channels == YuvSourceChannels::Rgb {
                    r_values_u8 = rgb_values.0;
                    g_values_u8 = rgb_values.1;
                    b_values_u8 = rgb_values.2;
                } else {
                    r_values_u8 = rgb_values.2;
                    g_values_u8 = rgb_values.1;
                    b_values_u8 = rgb_values.0;
                }
            }
            YuvSourceChannels::Rgba => {
                let rgb_values = vld4q_u8(rgba_ptr.add(cx * channels));
                r_values_u8 = rgb_values.0;
                g_values_u8 = rgb_values.1;
                b_values_u8 = rgb_values.2;
            }
            YuvSourceChannels::Bgra => {
                let rgb_values = vld4q_u8(rgba_ptr.add(cx * channels));
                r_values_u8 = rgb_values.2;
                g_values_u8 = rgb_values.1;
                b_values_u8 = rgb_values.0;
            }
        }

        let r_high = vreinterpretq_s16_u16(vshll_high_n_u8::<V_SCALE>(r_values_u8));
        let g_high = vreinterpretq_s16_u16(vshll_high_n_u8::<V_SCALE>(g_values_u8));
        let b_high = vreinterpretq_s16_u16(vshll_high_n_u8::<V_SCALE>(b_values_u8));

        let mut y_high = vqrdmlahq_s16(y_bias, r_high, v_yr);
        y_high = vqrdmlahq_s16(y_high, g_high, v_yg);
        y_high = vqrdmlahq_s16(y_high, b_high, v_yb);

        let y_high = vreinterpretq_u16_s16(vminq_s16(vmaxq_s16(y_high, i_bias), i_cap));

        let r_low = vreinterpretq_s16_u16(vshll_n_u8::<V_SCALE>(vget_low_u8(r_values_u8)));
        let g_low = vreinterpretq_s16_u16(vshll_n_u8::<V_SCALE>(vget_low_u8(g_values_u8)));
        let b_low = vreinterpretq_s16_u16(vshll_n_u8::<V_SCALE>(vget_low_u8(b_values_u8)));

        let mut y_low = vqrdmlahq_s16(y_bias, r_low, v_yr);
        y_low = vqrdmlahq_s16(y_low, g_low, v_yg);
        y_low = vqrdmlahq_s16(y_low, b_low, v_yb);

        let y_low = vreinterpretq_u16_s16(vminq_s16(vmaxq_s16(y_low, i_bias), i_cap));

        vst1q_u16_x2(y_ptr.add(cx), uint16x8x2_t(y_low, y_high));

        let mut cb_high = vqrdmlahq_s16(uv_bias, r_high, v_cb_r);
        cb_high = vqrdmlahq_s16(cb_high, g_high, v_cb_g);
        cb_high = vqrdmlahq_s16(cb_high, b_high, v_cb_b);

        let cb_high = vreinterpretq_u16_s16(vminq_s16(vmaxq_s16(cb_high, i_bias), i_cap));

        let mut cr_high = vqrdmlahq_s16(uv_bias, r_high, v_cr_r);
        cr_high = vqrdmlahq_s16(cr_high, g_high, v_cr_g);
        cr_high = vqrdmlahq_s16(cr_high, b_high, v_cr_b);

        let cr_high = vreinterpretq_u16_s16(vminq_s16(vmaxq_s16(cr_high, i_bias), i_cap));

        let mut cb_low = vqrdmlahq_s16(uv_bias, r_low, v_cb_r);
        cb_low = vqrdmlahq_s16(cb_low, g_low, v_cb_g);
        cb_low = vqrdmlahq_s16(cb_low, b_low, v_cb_b);

        let cb_low = vreinterpretq_u16_s16(vminq_s16(vmaxq_s16(cb_low, i_bias), i_cap));

        let mut cr_low = vqrdmlahq_s16(uv_bias, r_low, v_cr_r);
        cr_low = vqrdmlahq_s16(cr_low, g_low, v_cr_g);
        cr_low = vqrdmlahq_s16(cr_low, b_low, v_cr_b);

        let cr_low = vreinterpretq_u16_s16(vminq_s16(vmaxq_s16(cr_low, i_bias), i_cap));

        vst1q_u16_x2(u_ptr.add(ux), uint16x8x2_t(cb_low, cb_high));
        vst1q_u16_x2(v_ptr.add(ux), uint16x8x2_t(cr_low, cr_high));

        ux += 16;
        cx += 16;
    }

    ProcessedOffset { cx, ux }
}
