
/* Benchmark/validation host only. The FFT algorithm is compiled from Vibe. */
#include <math.h>
#include <stdlib.h>
#include <time.h>

enum { FFT_N = 1024, FFT_BATCH = 1024 };
static volatile double fft_sink;

static double seconds(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) abort();
    return (double)t.tv_sec + (double)t.tv_nsec * 1e-9;
}

static void invoke(float _Complex *input, float _Complex *twiddle, float _Complex *output) {
    VIBE_FFT_SYMBOL((vibe_array_complex64){input, FFT_N},
                    (vibe_array_complex64){twiddle, FFT_N / 2},
                    (vibe_array_complex64){output, FFT_N});
}

int main(void) {
    _Static_assert(sizeof(float _Complex) == 8, "complex64 must occupy eight bytes");
    float _Complex twiddle[FFT_N / 2], input[FFT_N], output[FFT_N];
    const double pi = acos(-1.0);
    for (int k = 0; k < FFT_N / 2; ++k) {
        double angle = -2.0 * pi * k / FFT_N;
        twiddle[k] = __builtin_complex((float)cos(angle), (float)sin(angle));
    }
    uint32_t rng = 0x12345678;
    for (int test = 0; test < 5; ++test) {
        for (int k = 0; k < FFT_N; ++k) {
            rng = rng * 1664525u + 1013904223u;
            float re = (float)(rng >> 8) / 8388608.0f - 1.0f;
            rng = rng * 1664525u + 1013904223u;
            float im = (float)(rng >> 8) / 8388608.0f - 1.0f;
            switch (test) {
                case 0: input[k] = k == 0 ? 1.0f : 0.0f; break;
                case 1: input[k] = 1.0f; break;
                case 2: input[k] = __builtin_complex((float)cos(2*pi*17*k/FFT_N), (float)sin(2*pi*17*k/FFT_N)); break;
                case 3: input[k] = __builtin_complex(re, im); break;
                default: input[k] = k % 2 ? -1.0f : 1.0f; break;
            }
        }
        /* Fix identical input values across the precision experiment. */
        for (int k = 0; k < FFT_N; ++k)
            input[k] = __builtin_complex((float)creal(input[k]), (float)cimag(input[k]));
        invoke(input, twiddle, output);
        for (int k = 0; k < FFT_N; ++k)
            printf("V %.9g %.9g %.9g %.9g\n", crealf(input[k]), cimagf(input[k]), crealf(output[k]), cimagf(output[k]));
    }
    size_t count = (size_t)FFT_BATCH * FFT_N;
    float _Complex *batch = malloc(count * sizeof(*batch));
    float _Complex *result = malloc(count * sizeof(*result));
    if (!batch || !result) return 2;
    for (size_t k = 0; k < count; ++k) {
        rng = rng * 1664525u + 1013904223u;
        batch[k] = __builtin_complex((float)(rng >> 8) / 8388608.0f - 1.0f, (float)0.25);
        result[k] = 0.0f;
    }
    for (int trial = -2; trial < 9; ++trial) {
        double start = seconds();
        for (int b = 0; b < FFT_BATCH; ++b)
            invoke(batch + b * FFT_N, twiddle, result + b * FFT_N);
        double elapsed = seconds() - start;
        double checksum = 0.0;
        for (size_t k = 0; k < count; ++k)
            checksum += (double)crealf(result[k]) + (double)cimagf(result[k]);
        fft_sink = checksum;
        if (trial >= 0) printf("T %.12g\n", elapsed);
    }
    free(batch);
    free(result);
    return 0;
}
