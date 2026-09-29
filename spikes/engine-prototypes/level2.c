/* yield-slope levelling with a precomputed mobility map; pair mobility = max of the two cells (so an edge band on
   either side of a cliff lets it relax).  Jacobi update: order/thread independent. */
#include <stdlib.h>
#include <string.h>
#include <math.h>
void level_mob(float *h, const float *mob, int W, int H, float s_y, float rate, int iters) {
    float *hn = (float *)malloc(sizeof(float) * (size_t)W * H);
    const int dx[4] = {1, 0, 1, -1}, dy[4] = {0, 1, 1, 1};
    const float dist[4] = {1.f, 1.f, 1.41421356f, 1.41421356f};
    for (int it = 0; it < iters; it++) {
        memcpy(hn, h, sizeof(float) * (size_t)W * H);
        for (int y = 0; y < H; y++) for (int x = 0; x < W; x++) {
            size_t i = (size_t)y * W + x; float mi = mob[i];
            for (int k = 0; k < 4; k++) {
                int x2 = x + dx[k], y2 = y + dy[k];
                if (x2 < 0 || x2 >= W || y2 >= H) continue;
                size_t j = (size_t)y2 * W + x2; float mj = mob[j];
                float m = mi > mj ? mi : mj; if (m <= 0.f) continue;
                float d = h[i] - h[j], ad = fabsf(d), thr = s_y * dist[k];
                if (ad <= thr) continue;
                float q = rate * m * (ad - thr) * (d > 0 ? 1.f : -1.f) / dist[k];
                hn[i] -= q; hn[j] += q;
            }
        }
        memcpy(h, hn, sizeof(float) * (size_t)W * H);
    }
    free(hn);
}
