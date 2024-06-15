#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[inline]
pub unsafe fn sse_rgb_to_ycgco_r_epi16(
    r: __m128i,
    g: __m128i,
    b: __m128i,
    y_bias: __m128i,
    uv_bias: __m128i,
    y_range: __m128i,
    uv_range: __m128i,
) -> (__m128i, __m128i, __m128i) {
    let zeros = _mm_setzero_si128();
    let r_l = _mm_unpacklo_epi16(r, zeros);
    let g_l = _mm_unpacklo_epi16(g, zeros);
    let b_l = _mm_unpacklo_epi16(b, zeros);

    let co_l = _mm_sub_epi32(r_l, b_l);
    let t_l = _mm_add_epi32(b_l, _mm_srai_epi32::<1>(co_l));
    let cg_l = _mm_sub_epi32(g_l, t_l);
    let y_l = _mm_add_epi32(
        _mm_mullo_epi32(_mm_add_epi32(_mm_srai_epi32::<1>(cg_l), t_l), y_range),
        y_bias,
    );
    let co_l = _mm_add_epi32(_mm_mullo_epi32(co_l, uv_range), uv_bias);
    let cg_l = _mm_add_epi32(_mm_mullo_epi32(cg_l, uv_range), uv_bias);

    let r_h = _mm_unpackhi_epi16(r, zeros);
    let g_h = _mm_unpackhi_epi16(g, zeros);
    let b_h = _mm_unpackhi_epi16(b, zeros);

    let co_h = _mm_sub_epi32(r_h, b_h);
    let t_h = _mm_add_epi32(b_h, _mm_srai_epi32::<1>(co_h));
    let cg_h = _mm_sub_epi32(g_h, t_h);
    let y_h = _mm_add_epi32(
        _mm_mullo_epi32(_mm_add_epi32(_mm_srai_epi32::<1>(cg_h), t_h), y_range),
        y_bias,
    );
    let co_h = _mm_add_epi32(_mm_mullo_epi32(co_h, uv_range), uv_bias);
    let cg_h = _mm_add_epi32(_mm_mullo_epi32(cg_h, uv_range), uv_bias);

    (
        _mm_packus_epi32(_mm_srai_epi32::<8>(y_l), _mm_srai_epi32::<8>(y_h)),
        _mm_packus_epi32(_mm_srai_epi32::<8>(cg_l), _mm_srai_epi32::<8>(cg_h)),
        _mm_packus_epi32(_mm_srai_epi32::<8>(co_l), _mm_srai_epi32::<8>(co_h)),
    )
}

#[inline(always)]
pub unsafe fn sse_ycgco_r_to_rgb_epi16(
    y: __m128i,
    cg: __m128i,
    co: __m128i,
    y_bias: __m128i,
    uv_bias: __m128i,
    y_range: __m128i,
    uv_range: __m128i,
) -> (__m128i, __m128i, __m128i) {
    let y = _mm_mullo_epi16(_mm_subs_epi16(y, y_bias), y_range);
    let cg = _mm_mullo_epi16(_mm_subs_epi16(cg, uv_bias), uv_range);
    let co = _mm_mullo_epi16(_mm_subs_epi16(co, uv_bias), uv_range);

    let t_l = _mm_subs_epi16(y, _mm_srai_epi16::<1>(cg));
    let g = _mm_adds_epi16(t_l, cg);
    let b = _mm_subs_epi16(t_l, _mm_srai_epi16::<1>(co));
    let r = _mm_adds_epi16(b, co);
    let zeros = _mm_setzero_si128();

    (
        _mm_srai_epi16::<6>(_mm_max_epi16(r, zeros)),
        _mm_srai_epi16::<6>(_mm_max_epi16(g, zeros)),
        _mm_srai_epi16::<6>(_mm_max_epi16(b, zeros)),
    )
}
