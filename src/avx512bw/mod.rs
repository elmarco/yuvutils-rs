#![cfg_attr(feature = "nightly_avx512", feature(cfg_version))]
#![cfg_attr(feature = "nightly_avx512", feature(avx512_target_feature))]
#![cfg_attr(feature = "nightly_avx512", feature(stdarch_x86_avx512))]


mod avx512_rgb_to_yuv;
mod avx512_setr;
mod avx512_utils;
mod rgb_to_y;
mod rgb_to_ycgco;
mod rgba_to_yuv;
mod y_to_rgb;
mod ycgco_to_rgb;
mod yuv_nv_to_rgba;
mod yuv_to_rgba;
mod yuv_to_rgba_alpha;
mod ycgco_to_rgba_alpha;

pub use rgb_to_y::avx512_row_rgb_to_y;
pub use rgb_to_ycgco::avx512_rgb_to_ycgco_row;
pub use rgba_to_yuv::avx512_rgba_to_yuv;
pub use y_to_rgb::avx512_y_to_rgb_row;
pub use ycgco_to_rgb::avx512_ycgco_to_rgb_row;
pub use yuv_nv_to_rgba::avx512_yuv_nv_to_rgba;
pub use yuv_to_rgba::avx512_yuv_to_rgba;
pub use yuv_to_rgba_alpha::avx512_yuv_to_rgba_alpha;
pub use ycgco_to_rgba_alpha::avx512_ycgco_to_rgba_alpha;