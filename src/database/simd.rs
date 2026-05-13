#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

#[cfg(target_arch = "x86_64")]
const MIN_DIM_SIZE_AVX: usize = 32;

#[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64"))]
const MIN_DIM_SIZE_SIMD: usize = 8;

#[inline]
pub fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return f32::INFINITY;
    }

    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2")
            && is_x86_feature_detected!("fma")
            && a.len() >= MIN_DIM_SIZE_AVX
        {
            // SAFETY: guarded by CPU feature checks above.
            return unsafe { l2_distance_avx2(a, b) };
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected!("sse") && a.len() >= MIN_DIM_SIZE_SIMD {
            // SAFETY: guarded by CPU feature checks above.
            return unsafe { l2_distance_sse(a, b) };
        }
    }

    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") && a.len() >= MIN_DIM_SIZE_SIMD {
            // SAFETY: guarded by CPU feature checks above.
            return unsafe { l2_distance_neon(a, b) };
        }
    }

    l2_distance_scalar(a, b)
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2", enable = "fma")]
#[inline]
unsafe fn l2_distance_avx2(a: &[f32], b: &[f32]) -> f32 {
    let dim = a.len();
    let mut i = 0;

    let mut sum1 = _mm256_setzero_ps();
    let mut sum2 = _mm256_setzero_ps();

    while i + 15 < dim {
        // SAFETY: i is bounded by dim in while condition.
        let va1 = unsafe { _mm256_loadu_ps(a.as_ptr().add(i)) };
        // SAFETY: i is bounded by dim in while condition.
        let vb1 = unsafe { _mm256_loadu_ps(b.as_ptr().add(i)) };
        // SAFETY: i + 8 is bounded by dim in while condition.
        let va2 = unsafe { _mm256_loadu_ps(a.as_ptr().add(i + 8)) };
        // SAFETY: i + 8 is bounded by dim in while condition.
        let vb2 = unsafe { _mm256_loadu_ps(b.as_ptr().add(i + 8)) };

        let diff1 = _mm256_sub_ps(va1, vb1);
        let diff2 = _mm256_sub_ps(va2, vb2);

        sum1 = _mm256_fmadd_ps(diff1, diff1, sum1);
        sum2 = _mm256_fmadd_ps(diff2, diff2, sum2);

        i += 16;
    }

    let combined = _mm256_add_ps(sum1, sum2);
    let sum_high = _mm256_extractf128_ps(combined, 1);
    let sum_low = _mm256_castps256_ps128(combined);
    let mut sum_128 = _mm_add_ps(sum_high, sum_low);

    sum_128 = _mm_hadd_ps(sum_128, sum_128);
    sum_128 = _mm_hadd_ps(sum_128, sum_128);

    let mut sum_sq = _mm_cvtss_f32(sum_128);

    while i < dim {
        let diff = a[i] - b[i];
        sum_sq += diff * diff;
        i += 1;
    }

    sum_sq.sqrt()
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "sse")]
#[inline]
unsafe fn l2_distance_sse(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let dim = a.len();
    let mut i = 0;
    let mut sum = _mm_setzero_ps();

    while i + 3 < dim {
        // SAFETY: i is bounded by dim in while condition.
        let va = unsafe { _mm_loadu_ps(a.as_ptr().add(i)) };
        // SAFETY: i is bounded by dim in while condition.
        let vb = unsafe { _mm_loadu_ps(b.as_ptr().add(i)) };
        let diff = _mm_sub_ps(va, vb);
        sum = _mm_add_ps(sum, _mm_mul_ps(diff, diff));
        i += 4;
    }

    let shuf = _mm_shuffle_ps(sum, sum, 0b10_11_00_01);
    sum = _mm_add_ps(sum, shuf);
    let shuf = _mm_movehl_ps(sum, sum);
    sum = _mm_add_ss(sum, shuf);

    let mut sum_sq = _mm_cvtss_f32(sum);

    while i < dim {
        let diff = a[i] - b[i];
        sum_sq += diff * diff;
        i += 1;
    }

    sum_sq.sqrt()
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[inline]
unsafe fn l2_distance_neon(a: &[f32], b: &[f32]) -> f32 {
    let dim = a.len();
    let mut i = 0;

    let mut sum1 = vdupq_n_f32(0.0);
    let mut sum2 = vdupq_n_f32(0.0);

    while i + 7 < dim {
        // SAFETY: i is bounded by dim in while condition.
        let va1 = unsafe { vld1q_f32(a.as_ptr().add(i)) };
        // SAFETY: i is bounded by dim in while condition.
        let vb1 = unsafe { vld1q_f32(b.as_ptr().add(i)) };
        // SAFETY: i + 4 is bounded by dim in while condition.
        let va2 = unsafe { vld1q_f32(a.as_ptr().add(i + 4)) };
        // SAFETY: i + 4 is bounded by dim in while condition.
        let vb2 = unsafe { vld1q_f32(b.as_ptr().add(i + 4)) };

        let diff1 = vsubq_f32(va1, vb1);
        let diff2 = vsubq_f32(va2, vb2);

        sum1 = vfmaq_f32(sum1, diff1, diff1);
        sum2 = vfmaq_f32(sum2, diff2, diff2);

        i += 8;
    }

    while i + 3 < dim {
        // SAFETY: i is bounded by dim in while condition.
        let va = unsafe { vld1q_f32(a.as_ptr().add(i)) };
        // SAFETY: i is bounded by dim in while condition.
        let vb = unsafe { vld1q_f32(b.as_ptr().add(i)) };
        let diff = vsubq_f32(va, vb);
        sum1 = vfmaq_f32(sum1, diff, diff);
        i += 4;
    }

    let combined = vaddq_f32(sum1, sum2);
    let mut sum_sq = vaddvq_f32(combined);

    while i < dim {
        let diff = a[i] - b[i];
        sum_sq += diff * diff;
        i += 1;
    }

    sum_sq.sqrt()
}

#[inline]
fn l2_distance_scalar(a: &[f32], b: &[f32]) -> f32 {
    let mut sum0 = 0.0_f32;
    let mut sum1 = 0.0_f32;

    let chunks = a.chunks_exact(4);
    let remainder = chunks.remainder();
    let b_chunks = b.chunks_exact(4);

    for (a_chunk, b_chunk) in chunks.zip(b_chunks) {
        let d0 = a_chunk[0] - b_chunk[0];
        let d1 = a_chunk[1] - b_chunk[1];
        let d2 = a_chunk[2] - b_chunk[2];
        let d3 = a_chunk[3] - b_chunk[3];

        sum0 += d0 * d0 + d1 * d1;
        sum1 += d2 * d2 + d3 * d3;
    }

    for i in (a.len() - remainder.len())..a.len() {
        let diff = a[i] - b[i];
        sum0 += diff * diff;
    }

    (sum0 + sum1).sqrt()
}
