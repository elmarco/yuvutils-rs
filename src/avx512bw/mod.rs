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

mod avx512_rgb_to_yuv;
mod avx512_setr;
mod avx512_utils;
mod rgb_to_y;
mod rgb_to_ycgco;
mod rgba_to_yuv;
mod y_to_rgb;
mod ycgco_to_rgb;
mod ycgco_to_rgba_alpha;
mod yuv_nv_to_rgba;
mod yuv_to_rgba;
mod yuv_to_rgba_alpha;

pub use rgb_to_y::avx512_row_rgb_to_y;
pub use rgb_to_ycgco::avx512_rgb_to_ycgco_row;
pub use rgba_to_yuv::avx512_rgba_to_yuv;
pub use y_to_rgb::avx512_y_to_rgb_row;
pub use ycgco_to_rgb::avx512_ycgco_to_rgb_row;
pub use ycgco_to_rgba_alpha::avx512_ycgco_to_rgba_alpha;
pub use yuv_nv_to_rgba::avx512_yuv_nv_to_rgba;
pub use yuv_to_rgba::avx512_yuv_to_rgba;
pub use yuv_to_rgba_alpha::avx512_yuv_to_rgba_alpha;
