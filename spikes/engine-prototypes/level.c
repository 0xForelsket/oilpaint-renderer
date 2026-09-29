/* Yield-slope levelling of a paint height field (prototype).
 * Viscoplastic (Bingham-like) heuristic: wet paint flows down a slope only where the slope exceeds a
 * yield slope s_y; the flux is proportional to the excess and to the mobility m = wet^p (dry paint does not
 * move).  Mass conserving; Jacobi update (reads old h, writes new h) so the result does not depend on
 * traversal order or thread count.  4-neighbour + diagonal exchange (diagonals with sqrt2 distance). */
#include <stdlib.h>
#include <string.h>
#include <math.h>
void level_height(float *h, const float *wet, int W, int H, float s_y, float rate, float wet_pow, int iters) {
    float *hn = (float *)malloc(sizeof(float) * (size_t)W * H);
    const int dx[4] = {1, 0, 1, -1}, dy[4] = {0, 1, 1, 1};
    const float dist[4] = {1.f, 1.f, 1.41421356f, 1.41421356f};
    for (int it = 0; it < iters; it++) {
        memcpy(hn, h, sizeof(float) * (size_t)W * H);
        for (int y = 0; y < H; y++) {
            for (int x = 0; x < W; x++) {
                size_t i = (size_t)y * W + x;
                float mi = wet ? powf(wet[i], wet_pow) : 1.f;
                for (int k = 0; k < 4; k++) {
                    int x2 = x + dx[k], y2 = y + dy[k];
                    if (x2 < 0 || x2 >= W || y2 >= H) continue;
                    size_t j = (size_t)y2 * W + x2;
                    float d = h[i] - h[j];
                    float thr = s_y * dist[k];
                    float ad = fabsf(d);
                    if (ad <= thr) continue;
                    float mj = wet ? powf(wet[j], wet_pow) : 1.f;
                    float m = d > 0 ? mi : mj;          /* paint moves from the higher cell: its mobility */
                    float q = rate * m * (ad - thr) * (d > 0 ? 1.f : -1.f) / dist[k];
                    hn[i] -= q; hn[j] += q;
                }
            }
        }
        memcpy(h, hn, sizeof(float) * (size_t)W * H);
    }
    free(hn);
}
