/*  oilpaint brush kernel (v2: bristle-lane surface model)
 *
 *  A stroke is a polyline of samples (x, y, width, pressure) in pixels.  Each segment is swept as an
 *  oriented rectangle; every pixel inside it is painted once per stroke.  In stroke coordinates
 *  (u across in half-widths, s along in widths) the stroke is a bundle of bristle LANES generated once per
 *  stroke from its seed.  Lanes decide everything at bristle scale: the ragged outline, the ridges and
 *  furrows of the relief, the parallel colour streaks (each lane carries its own colour: loaded colour A or
 *  B, tube inhomogeneity, and what that lane picked up from wet paint underneath), and the broken coverage
 *  of a dry tail.  Between the lanes the paint body fills in while the brush is loaded.  Everything is
 *  evaluated at continuous (u, s), so the same stroke list replays identically at any resolution; lanes
 *  narrower than a pixel are widened with their integral preserved (anti-aliasing).
 *
 *  Canvas colour lives in Mixbox latent space (7 floats/pixel).  Mixbox polynomial:
 *  (c) 2022 Secret Weapons, CC BY-NC 4.0 (https://scrtwpns.com/mixbox).
 */
#include <math.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define LAT 7
#define MAX_NB 72
#define ALONG_PER_WIDTH 6.0f
#define MAX_CONTRIB 10

typedef struct {
    int W, H;
    float *lat;      /* W*H*7 */
    float *rgb;      /* W*H*3 */
    float *h;        /* W*H   paint height */
    float *wet;      /* W*H   0..1 */
    float *cover;    /* W*H   accumulated deposit */
    float *hblur;    /* W*H   blurred height for scumble mode, may be NULL */
    uint8_t *region; /* W*H   region ids, may be NULL */
} Canvas;

typedef struct {
    int mode;            /* 0 paint, 1 scumble, 2 smudge, 3 glaze */
    float opacity;
    float pickup;        /* 0..1 share of the canvas colour a wet lane blends into its deposit */
    float load;          /* initial paint load V (1 = full) */
    float deplete;       /* V lost per width-unit of travel */
    float vdry;          /* below this V the alpha scales down and the body opens into lanes (dry brush) */
    float hgain;         /* mean paint thickness of the stroke */
    float flatten;       /* levelling of the paint under the stroke, 0..1 (replace share) */
    float streak;        /* light/dark colour variation between lanes (tube inhomogeneity) */
    float hardness;      /* 0 soft edge .. 1 hard edge of the paint body outline */
    float grain;         /* per-pixel hash noise amplitude on alpha */
    float dry_thresh;    /* scumble: h - hblur threshold */
    float dry_width;     /* scumble: smoothstep width */
    int nb;              /* bristle lanes across */
    uint32_t allow_mask; /* unused (kept for ABI) */
    float override_p;    /* unused (kept for ABI) */
    uint32_t seed;
    float dropout;       /* base lane dropout probability per along-sample at full load */
    float ragged;        /* start/end raggedness of lanes, in widths */
    float body;          /* paint body filling between lanes at full load (0 = lanes only) */
    float release;       /* per-segment decay of picked-up paint on a lane (0..1) */
    float streak_mix;    /* lane-to-lane variation of how much picked-up paint shows */
    float ridge;         /* lane ridge/furrow relief amplitude (relative to hgain) */
    float levee;         /* edge levee amplitude (relative to hgain) */
    float furrow;        /* centre furrow depth (relative to hgain) */
    float blob;          /* extra paint at the stroke start (relative to hgain) */
    float stiff;         /* multi-scale roughness amplitude of stiff paint (relative to hgain) */
    float marble;        /* share of colour B in the lanes assigned to it (two-colour load) */
    float splay;         /* number-ish of stray hairs outside the outline (0..3) */
} BrushParams;

/* ---------------- random / hashing ---------------- */
static inline uint32_t xs32(uint32_t *s) {
    uint32_t x = *s; x ^= x << 13; x ^= x >> 17; x ^= x << 5; *s = x; return x;
}
static inline float frand(uint32_t *s) { return (xs32(s) >> 8) * (1.0f / 16777216.0f); }
static inline float hash2(int x, int y, uint32_t seed) {
    uint32_t h = (uint32_t)x * 374761393u + (uint32_t)y * 668265263u + seed * 2246822519u;
    h = (h ^ (h >> 13)) * 1274126177u; h ^= h >> 16;
    return (h >> 8) * (1.0f / 16777216.0f);
}
static inline float clamp01(float x) { return x < 0.f ? 0.f : (x > 1.f ? 1.f : x); }
static inline float smoothstep(float e0, float e1, float x) {
    float t = clamp01((x - e0) / (e1 - e0)); return t * t * (3.f - 2.f * t);
}
/* value noise on a lattice, bilinear, in continuous coordinates (resolution independent) */
static inline float vnoise(float x, float y, uint32_t seed) {
    float fx = floorf(x), fy = floorf(y);
    int ix = (int)fx, iy = (int)fy;
    float tx = x - fx, ty = y - fy;
    tx = tx * tx * (3.f - 2.f * tx); ty = ty * ty * (3.f - 2.f * ty);
    float a = hash2(ix, iy, seed), b = hash2(ix + 1, iy, seed), c = hash2(ix, iy + 1, seed), d = hash2(ix + 1, iy + 1, seed);
    return (a * (1 - tx) + b * tx) * (1 - ty) + (c * (1 - tx) + d * tx) * ty;
}

/* ---------------- Mixbox polynomial (latent -> rgb) ---------------- */
static inline void mixbox_latent_to_rgb(const float *l, float *rgb) {
    float c0 = l[0], c1 = l[1], c2 = l[2], c3 = l[3];
    float c00 = c0*c0, c11 = c1*c1, c22 = c2*c2, c33 = c3*c3, c01 = c0*c1, c02 = c0*c2, c12 = c1*c2;
    float r = 0.f, g = 0.f, b = 0.f, w;
    w = c0*c00; r += 0.07717053f*w; g += 0.02826978f*w; b += 0.24832992f*w;
    w = c1*c11; r += 0.95912302f*w; g += 0.80256528f*w; b += 0.03561839f*w;
    w = c2*c22; r += 0.74683774f*w; g += 0.04868586f*w;
    w = c3*c33; r += 0.99518138f*w; g += 0.99978149f*w; b += 0.99704802f*w;
    w = c00*c1; r += 0.04819146f*w; g += 0.83363781f*w; b += 0.32515377f*w;
    w = c01*c1; r += -0.68146950f*w; g += 1.46107803f*w; b += 1.06980936f*w;
    w = c00*c2; r += 0.27058419f*w; g += -0.15324870f*w; b += 1.98735057f*w;
    w = c02*c2; r += 0.80478189f*w; g += 0.67093710f*w; b += 0.18424500f*w;
    w = c00*c3; r += -0.35031003f*w; g += 1.37855826f*w; b += 3.68865000f*w;
    w = c0*c33; r += 1.05128046f*w; g += 1.97815239f*w; b += 2.82989073f*w;
    w = c11*c2; r += 3.21607125f*w; g += 0.81270228f*w; b += 1.03384539f*w;
    w = c1*c22; r += 2.78893374f*w; g += 0.41565549f*w; b += -0.04487295f*w;
    w = c11*c3; r += 3.02162577f*w; g += 2.55374103f*w; b += 0.32766114f*w;
    w = c1*c33; r += 2.95124691f*w; g += 2.81201112f*w; b += 1.17578442f*w;
    w = c22*c3; r += 2.82677043f*w; g += 0.79933038f*w; b += 1.81715262f*w;
    w = c2*c33; r += 2.99691099f*w; g += 1.22593053f*w; b += 1.80653661f*w;
    w = c01*c2; r += 1.87394106f*w; g += 2.05027182f*w; b += -0.29835996f*w;
    w = c01*c3; r += 2.56609566f*w; g += 7.03428198f*w; b += 0.62575374f*w;
    w = c02*c3; r += 4.08329484f*w; g += -1.40408358f*w; b += 2.14995522f*w;
    w = c12*c3; r += 6.00078678f*w; g += 2.55552042f*w; b += 1.90739502f*w;
    rgb[0] = clamp01(r + l[4]); rgb[1] = clamp01(g + l[5]); rgb[2] = clamp01(b + l[6]);
}

void latent_to_rgb_array(const float *lat, float *rgb, int n) {
    for (int i = 0; i < n; i++) mixbox_latent_to_rgb(lat + i * LAT, rgb + i * 3);
}

/* ---------------- bristle lanes ---------------- */
typedef struct {
    int nb, ns, ntot;         /* nb lanes + splay hairs = ntot */
    float u0[MAX_NB], sigR[MAX_NB], sigF[MAX_NB], wgt[MAX_NB], tstreak[MAX_NB], colB[MAX_NB];
    float wob_amp[MAX_NB], wob_k[MAX_NB], wob_ph[MAX_NB];
    float *along;             /* ntot * ns */
    float *alongF;            /* ntot * ns: max-filtered along profile (the loaded body fills lane dropouts) */
    float nph1, nph2;         /* noise phases */
} Bristles;

static void make_bristles(Bristles *B, const BrushParams *bp, float total_s, uint32_t *rs) {
    int nb = bp->nb < 2 ? 2 : (bp->nb > MAX_NB - 4 ? MAX_NB - 4 : bp->nb);
    int nsplay = (int)(bp->splay + frand(rs));
    if (nsplay > 3) nsplay = 3;
    B->nb = nb; B->ntot = nb + nsplay;
    B->ns = (int)ceilf(total_s * ALONG_PER_WIDTH) + 3;
    B->along = (float *)malloc(sizeof(float) * B->ntot * B->ns);
    B->alongF = (float *)malloc(sizeof(float) * B->ntot * B->ns);
    float hard = clamp01(bp->hardness);
    float pitch = 2.f / nb;
    int colstate = frand(rs) < 0.5f;
    B->nph1 = frand(rs) * 100.f; B->nph2 = frand(rs) * 100.f;
    for (int b = 0; b < B->ntot; b++) {
        int is_splay = b >= nb;
        float u, sig, w;
        if (!is_splay) {
            u = -1.f + (b + 0.5f) * pitch + (frand(rs) - 0.5f) * 0.5f * pitch;
            sig = (0.28f + 0.30f * frand(rs)) * pitch;          /* lane half-width: lanes nearly touch */
            w = 0.55f + 0.45f * frand(rs);
            /* two-colour load: runs of adjacent lanes carry colour B */
            if (frand(rs) < 0.35f) colstate = !colstate;
            B->colB[b] = colstate ? 1.f : 0.f;
        } else {
            float side = frand(rs) < 0.5f ? -1.f : 1.f;
            u = side * (0.98f + 0.18f * frand(rs));              /* stray hair at / just outside the outline */
            sig = (0.18f + 0.15f * frand(rs)) * pitch;
            w = 0.25f + 0.25f * frand(rs);
            B->colB[b] = frand(rs) < 0.5f ? 1.f : 0.f;
        }
        B->u0[b] = u; B->sigR[b] = sig;
        B->sigF[b] = is_splay ? sig * 1.3f : sig * (2.8f - 1.0f * hard);   /* fill bump: wider, overlaps */
        B->wgt[b] = w;
        B->tstreak[b] = 2.f * frand(rs) - 1.f;
        B->wob_amp[b] = (0.10f + 0.30f * frand(rs)) * pitch * (is_splay ? 2.f : 1.f);
        B->wob_k[b] = 6.2831853f / (1.5f + 4.f * frand(rs));
        B->wob_ph[b] = 6.2831853f * frand(rs);
        /* along profile: ragged start and end, slow load variation, dropouts growing with dryness */
        float *A = B->along + b * B->ns;
        float start = frand(rs) * bp->ragged * ALONG_PER_WIDTH;
        float endcut = frand(rs) * bp->ragged * ALONG_PER_WIDTH * (is_splay ? 3.f : 1.f);
        int drop_left = 0;
        float base = 0.85f + 0.15f * frand(rs);
        float lk = 6.2831853f / ((2.f + 4.f * frand(rs)) * ALONG_PER_WIDTH), lph = 6.2831853f * frand(rs);
        for (int i = 0; i < B->ns; i++) {
            float s_w = i / ALONG_PER_WIDTH;
            float V = bp->load - bp->deplete * s_w;
            float dryness = V < bp->vdry ? clamp01(1.f - V / bp->vdry) : 0.f;
            float pdrop = bp->dropout * (1.f + 6.f * dryness) * (is_splay ? 4.f : 1.f);
            if (i > B->ns - 2 - endcut) { A[i] = 0.f; continue; }
            if (drop_left > 0) { drop_left--; A[i] = 0.f; continue; }
            if (frand(rs) < pdrop) {
                drop_left = 2 + (int)(frand(rs) * (3.f + 8.f * dryness));
                A[i] = 0.f; continue;
            }
            float v = base * (0.92f + 0.08f * sinf(lk * i + lph));
            if (i < start) v *= clamp01((i - start + 2.f) / 2.f);
            A[i] = v;
        }
        for (int pass = 0; pass < 2; pass++)
            for (int i = 1; i < B->ns - 1; i++) {
                float m = 0.25f * A[i - 1] + 0.5f * A[i] + 0.25f * A[i + 1];
                A[i] = 0.5f * A[i] + 0.5f * m;
            }
        float *AF = B->alongF + b * B->ns;
        for (int i = 0; i < B->ns; i++) {
            float amax = 0.f;
            for (int j = -4; j <= 4; j++) { int ii = i + j; if (ii >= 0 && ii < B->ns && A[ii] > amax) amax = A[ii]; }
            AF[i] = is_splay ? A[i] : amax;
        }
    }
}

typedef struct {
    float cov;                 /* fill coverage (sum of fill bumps), ~1-2 inside, 0 outside */
    float ridge;               /* lane ridge pattern 0..1 (1 on lane centres) */
    int n;                     /* contributing lanes */
    int idx[MAX_CONTRIB];
    float wR[MAX_CONTRIB];     /* lane-sharp weights (colour) */
    float wF[MAX_CONTRIB];     /* fill weights (fallback colour between lanes) */
} Sample;

/* per-segment lane geometry: wobble position and anti-aliased sigmas (min_sig from the segment's half width) */
typedef struct { float uc[MAX_NB], isf[MAX_NB], gf[MAX_NB], isr[MAX_NB], gr[MAX_NB]; int reach; } SegLanes;

static inline void prep_lanes(const Bristles *B, float s_mid, float min_sig, SegLanes *G) {
    for (int b = 0; b < B->ntot; b++) {
        G->uc[b] = B->u0[b] + B->wob_amp[b] * sinf(B->wob_k[b] * s_mid + B->wob_ph[b]);
        float sf = B->sigF[b], gf = 1.f; if (sf < min_sig) { gf = sf / min_sig; sf = min_sig; }
        float sr = B->sigR[b], gr = 1.f; if (sr < min_sig) { gr = sr / min_sig; sr = min_sig; }
        G->isf[b] = 1.f / sf; G->gf[b] = gf; G->isr[b] = 1.f / sr; G->gr[b] = gr;
    }
    G->reach = 3 + (int)(min_sig * B->nb) + 1;
}

static inline void eval_lanes(const Bristles *B, const SegLanes *G, float u, float s, float bodyf, Sample *o) {
    float fs = s * ALONG_PER_WIDTH;
    int is = (int)fs; if (is < 0) is = 0; if (is >= B->ns - 1) is = B->ns - 2;
    float ts = fs - is; if (ts < 0) ts = 0; if (ts > 1) ts = 1;
    o->cov = 0.f; o->ridge = 0.f; o->n = 0;
    int bc = (int)((u + 1.f) * 0.5f * B->nb);
    int reach = G->reach;
    int b0 = bc - reach, b1 = bc + reach;
    if (b0 < 0) b0 = 0; if (b1 > B->nb - 1) b1 = B->nb - 1;
    for (int pass = 0; pass < 2; pass++) {
        int lo = pass ? B->nb : b0, hi = pass ? B->ntot - 1 : b1;   /* second pass: splay hairs */
        for (int b = lo; b <= hi; b++) {
            float d = u - G->uc[b];
            float df = d * G->isf[b];
            if (df > 2.f || df < -2.f) continue;                     /* outside the fill bump: nothing */
            const float *A = B->along + b * B->ns;
            float a = A[is] * (1.f - ts) + A[is + 1] * ts;
            /* the paint body fills lane dropouts while the brush is loaded (except for the ragged ends) */
            float afill = a;
            if (!pass && bodyf > 0.f) {
                const float *AF = B->alongF + b * B->ns;
                float body_a = bodyf * (AF[is] * (1.f - ts) + AF[is + 1] * ts);
                if (body_a > afill) afill = body_a;
            }
            if (a <= 0.003f && afill <= 0.003f) continue;
            /* fill bump (wide) */
            float bf = 1.f - 0.25f * df * df;
            if (bf > 0.f) { bf *= bf * G->gf[b]; o->cov += bf * afill; }
            /* ridge bump (lane-sharp) */
            float dr = d * G->isr[b], br = 1.f - 0.25f * dr * dr;
            float gr = G->gr[b];
            if (br > 0.f) { br *= br * gr; if (br * a > o->ridge) o->ridge = br * a; }
            if ((bf > 0.f || br > 0.f) && o->n < MAX_CONTRIB) {
                o->idx[o->n] = b; o->wR[o->n] = br > 0.f ? br * a : 0.f; o->wF[o->n] = bf > 0.f ? bf * afill : 0.f; o->n++;
            }
        }
    }
}

/* ---------------- stroke rendering ---------------- */
int render_stroke(Canvas *cv, const float *pts, int n, const float *zcol, const float *zcol2, const float *dz,
                  const BrushParams *bp, float *out_stats) {
    if (n < 2) return 0;
    const int W = cv->W, H = cv->H;
    uint32_t rs = bp->seed * 747796405u + 2891336453u; if (!rs) rs = 1;
    float wref = 1e-3f;
    for (int i = 0; i < n; i++) if (pts[i * 4 + 2] > wref) wref = pts[i * 4 + 2];
    float total = 0.f;
    for (int i = 0; i + 1 < n; i++) {
        float dx = pts[(i + 1) * 4] - pts[i * 4], dy = pts[(i + 1) * 4 + 1] - pts[i * 4 + 1];
        total += sqrtf(dx * dx + dy * dy);
    }
    float total_s = total / wref;
    Bristles B; make_bristles(&B, bp, total_s, &rs);
    const int mode = bp->mode;

    /* per-lane colour state: loaded colour (A/B + inhomogeneity), tip (picked up), dirt */
    float Zload[MAX_NB][LAT], Ztip[MAX_NB][LAT];
    float dirt[MAX_NB], segZ[MAX_NB][LAT], segA[MAX_NB], segWet[MAX_NB];
    for (int b = 0; b < B.ntot; b++) {
        float mB = bp->marble * B.colB[b], st = bp->streak * B.tstreak[b];
        for (int k = 0; k < LAT; k++) {
            Zload[b][k] = zcol[k] + mB * (zcol2[k] - zcol[k]) + st * dz[k];
            Ztip[b][k] = Zload[b][k];
        }
        dirt[b] = (mode == 2) ? 1.f : 0.f;
    }
    float V = bp->load;
    float hbase = 0.f; int have_mean = 0;
    double stat_alpha = 0.0; long stat_pix = 0; double sumWetAll = 0.0, sumAAll = 0.0;
    float s_acc = 0.f;
    float ptx = 0.f, pty = 0.f, pnx = 0.f, pny = 0.f, plen = 0.f, px0 = 0.f, py0 = 0.f, pw0 = 0.f, pw1 = 0.f;
    int have_prev = 0;
    const float lw = 0.13f;   /* levee position inset from the outline, in half-widths */

    for (int i = 0; i + 1 < n; i++) {
        float x0 = pts[i * 4], y0 = pts[i * 4 + 1], w0 = pts[i * 4 + 2], p0 = pts[i * 4 + 3];
        float x1 = pts[(i + 1) * 4], y1 = pts[(i + 1) * 4 + 1], w1 = pts[(i + 1) * 4 + 2], p1 = pts[(i + 1) * 4 + 3];
        float dx = x1 - x0, dy = y1 - y0;
        float len = sqrtf(dx * dx + dy * dy);
        if (len < 1e-4f) continue;
        float tx = dx / len, ty = dy / len, nx = -ty, ny = tx;
        float hwmax = 0.5f * (w0 > w1 ? w0 : w1) * 1.45f + 1.f;
        int bx0 = (int)floorf(fminf(x0, x1) - hwmax), bx1 = (int)ceilf(fmaxf(x0, x1) + hwmax);
        int by0 = (int)floorf(fminf(y0, y1) - hwmax), by1 = (int)ceilf(fmaxf(y0, y1) + hwmax);
        if (bx0 < 0) bx0 = 0; if (by0 < 0) by0 = 0; if (bx1 > W - 1) bx1 = W - 1; if (by1 > H - 1) by1 = H - 1;
        int last = (i + 2 == n);
        float wedge_t = have_prev ? -(0.6f * w0 / len) : 0.f;

        /* paint level under this segment (cheap pre-pass) for the levelling base */
        float hcur = 0.f; int hn = 0;
        for (int j = 0; j < 8; j++) {
            float tj = (j + 0.5f) / 8.f;
            float cxj = x0 + dx * tj, cyj = y0 + dy * tj, hwj = 0.5f * (w0 + (w1 - w0) * tj);
            for (int k = -1; k <= 1; k++) {
                int sx = (int)(cxj + nx * hwj * 0.5f * k), sy = (int)(cyj + ny * hwj * 0.5f * k);
                if (sx >= 0 && sy >= 0 && sx < W && sy < H) { hcur += cv->h[(size_t)sy * W + sx]; hn++; }
            }
        }
        hcur = hn ? hcur / hn : 0.f;
        float hprev = have_mean ? hbase : hcur;
        SegLanes G; prep_lanes(&B, (s_acc + 0.5f * len) / wref, 0.7f / (0.25f * (w0 + w1) + 0.25f), &G);
        float loadf = (mode == 3) ? 1.f : clamp01(V / bp->vdry);
        float thick = clamp01(V);
        float bodyf = (mode == 1) ? 0.f : bp->body * loadf;
        int can_deposit = (mode != 2 || have_mean);
        for (int b = 0; b < B.ntot; b++) { segA[b] = 0.f; segWet[b] = 0.f; for (int k = 0; k < LAT; k++) segZ[b][k] = 0.f; }
        /* body outline crispness */
        float e0 = 0.35f, e1 = e0 + 0.12f + 0.7f * (1.f - bp->hardness);

        if (bx1 >= bx0 && by1 >= by0) {
            for (int py = by0; py <= by1; py++) {
                float *latrow = cv->lat + (size_t)py * W * LAT;
                for (int px = bx0; px <= bx1; px++) {
                    float vx = px + 0.5f - x0, vy = py + 0.5f - y0;
                    float t = (vx * tx + vy * ty) / len;
                    if (t < wedge_t || (last ? t > 1.f : t >= 1.f)) continue;
                    if (have_prev) {
                        float qx = px + 0.5f - px0, qy = py + 0.5f - py0;
                        float tp = (qx * ptx + qy * pty) / plen;
                        float hwp = 0.5f * (pw0 + (pw1 - pw0) * tp);
                        float up = hwp > 0.f ? (qx * pnx + qy * pny) / hwp : 9.f;
                        int prev_owns = (tp >= 0.f && tp < 1.f && up >= -1.45f && up <= 1.45f);
                        if (prev_owns) continue;
                        if (t < 0.f && !(tp >= 1.f && up >= -1.45f && up <= 1.45f)) continue;
                        if (t < 0.f) t = 0.f;
                    }
                    float hw = 0.5f * (w0 + (w1 - w0) * t);
                    if (hw < 0.25f) continue;
                    float u = (vx * nx + vy * ny) / hw;
                    if (u < -1.45f || u > 1.45f) continue;
                    float s = (s_acc + t * len) / wref;
                    Sample S; eval_lanes(&B, &G, u, s, bodyf, &S);
                    if (S.cov <= 0.01f) continue;
                    float acov = smoothstep(e0, e1, S.cov);
                    float pres = p0 + (p1 - p0) * t;
                    float lanes = bodyf + (1.f - bodyf) * S.ridge;
                    float alpha = acov * lanes * pres * bp->opacity * loadf;
                    if (bp->grain > 0.f) alpha *= 1.f + bp->grain * (hash2(px, py, bp->seed) - 0.5f);
                    size_t idx = (size_t)py * W + px;
                    if (mode == 1) {
                        float hb = cv->hblur ? cv->hblur[idx] : 0.f;
                        alpha *= smoothstep(bp->dry_thresh - bp->dry_width, bp->dry_thresh + bp->dry_width, cv->h[idx] - hb);
                    }
                    if (alpha <= 0.002f) continue;
                    if (alpha > 1.f) alpha = 1.f;
                    float *L = latrow + (size_t)px * LAT;
                    /* colour: lane-weighted blend of the lanes' own colours (sharp between lanes) */
                    float wsum = 0.f; int useR = 1;
                    for (int c = 0; c < S.n; c++) wsum += S.wR[c];
                    if (wsum < 0.05f) { useR = 0; wsum = 0.f; for (int c = 0; c < S.n; c++) wsum += S.wF[c]; }
                    if (wsum < 1e-6f) continue;
                    /* pick-up accumulation on the dominant lane (canvas before deposit) */
                    int bd = S.idx[0]; float wd = -1.f;
                    for (int c = 0; c < S.n; c++) { float w = useR ? S.wR[c] : S.wF[c]; if (w > wd) { wd = w; bd = S.idx[c]; } }
                    for (int k = 0; k < LAT; k++) segZ[bd][k] += alpha * L[k];
                    segA[bd] += alpha; segWet[bd] += alpha * cv->wet[idx];
                    if (!can_deposit) continue;
                    float Zpix[LAT] = {0, 0, 0, 0, 0, 0, 0};
                    if (mode == 2) {
                        for (int c = 0; c < S.n; c++) {
                            float w = (useR ? S.wR[c] : S.wF[c]) / wsum; int b = S.idx[c];
                            for (int k = 0; k < LAT; k++) Zpix[k] += w * Ztip[b][k];
                        }
                    } else {
                        for (int c = 0; c < S.n; c++) {
                            float w = (useR ? S.wR[c] : S.wF[c]) / wsum; int b = S.idx[c];
                            float dpix = dirt[b] * (1.f + bp->streak_mix * B.tstreak[b]);
                            if (dpix > 1.f) dpix = 1.f; if (dpix < 0.f) dpix = 0.f;
                            for (int k = 0; k < LAT; k++) Zpix[k] += w * (Zload[b][k] + dpix * (Ztip[b][k] - Zload[b][k]));
                        }
                    }
                    float a = alpha;
                    for (int k = 0; k < LAT; k++) L[k] += a * (Zpix[k] - L[k]);
                    mixbox_latent_to_rgb(L, cv->rgb + idx * 3);
                    /* relief */
                    float base = hprev + (hcur - hprev) * t;
                    if (mode == 0) {
                        float au = fabsf(u);
                        float lv = expf(-((au - (1.f - lw)) * (au - (1.f - lw))) / (0.10f * 0.10f));
                        float fur = expf(-(u * u) / (0.35f * 0.35f));
                        float sb = expf(-(s * s) / (0.45f * 0.45f));
                        float nz = vnoise(u * B.nb * 0.5f + B.nph1, s * 4.f + B.nph2, bp->seed)
                                 + 0.5f * vnoise(u * B.nb * 1.5f + B.nph2, s * 12.f + B.nph1, bp->seed + 7u) - 0.75f;
                        float shape = acov * (1.f + bp->levee * lv * thick - bp->furrow * fur * thick + bp->blob * sb * thick)
                                    + bp->ridge * (S.ridge - 0.45f) * acov * (0.5f + 0.5f * loadf)
                                    + bp->stiff * nz * acov;
                        float hadd = bp->hgain * thick * shape;
                        float fl = bp->flatten;
                        cv->h[idx] += a * fl * (base + hadd - cv->h[idx]) + a * (1.f - fl) * 0.5f * hadd;
                        if (cv->wet[idx] < a) cv->wet[idx] = a;
                    } else if (mode == 1) {
                        cv->h[idx] += a * bp->hgain * 0.5f * (0.4f + 0.6f * S.ridge);
                        if (cv->wet[idx] < 0.5f * a) cv->wet[idx] = 0.5f * a;
                    } else if (mode == 2) {
                        cv->h[idx] -= a * bp->flatten * (cv->h[idx] - base);
                    }
                    cv->cover[idx] += a;
                    stat_alpha += a; stat_pix++;
                }
            }
        }
        /* end of segment: per-lane pick-up (limited: fast release, lane-wise), depletion */
        for (int b = 0; b < B.ntot; b++) {
            if (segA[b] <= 1e-6f) { dirt[b] -= bp->release * dirt[b] * (mode == 2 ? 0.f : 1.f); continue; }
            float wetavg = segWet[b] / segA[b];
            sumWetAll += segWet[b]; sumAAll += segA[b];
            if (mode == 2) {
                for (int k = 0; k < LAT; k++) Ztip[b][k] += bp->pickup * (segZ[b][k] / segA[b] - Ztip[b][k]);
                dirt[b] = 1.f;
            } else if (mode != 3) {
                float rate = 0.6f * wetavg;
                for (int k = 0; k < LAT; k++) Ztip[b][k] += rate * (segZ[b][k] / segA[b] - Ztip[b][k]);
                float target = bp->pickup * wetavg;
                if (target > dirt[b]) dirt[b] += 0.6f * (target - dirt[b]);
                else dirt[b] -= bp->release * (dirt[b] - target);
            }
        }
        hbase = hcur; have_mean = 1;
        V -= bp->deplete * (len / wref);
        if (V < 0.f) V = 0.f;
        s_acc += len;
        ptx = tx; pty = ty; pnx = nx; pny = ny; plen = len; px0 = x0; py0 = y0; pw0 = w0; pw1 = w1; have_prev = 1;
    }
    free(B.along); free(B.alongF);
    if (out_stats) { out_stats[0] = (float)stat_alpha; out_stats[1] = (float)stat_pix; out_stats[2] = sumAAll > 0 ? (float)(sumWetAll / sumAAll) : 0.f; }
    return (int)stat_pix;
}

int render_strokes(Canvas *cv, const float *pts_all, const int *offsets, int n_strokes,
                   const float *zcols, const float *zcols2, const float *dzs, const BrushParams *params) {
    long total = 0;
    for (int i = 0; i < n_strokes; i++) {
        int a = offsets[i], b = offsets[i + 1];
        total += render_stroke(cv, pts_all + (size_t)a * 4, b - a, zcols + i * LAT, zcols2 + i * LAT, dzs + i * LAT,
                               params + i, NULL);
    }
    return (int)(total > 2147483647L ? 2147483647L : total);
}
