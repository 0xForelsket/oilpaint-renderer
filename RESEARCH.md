# Research notes for the "Storm Light" oil-painting renderer

Phase 1 (2026-09-29). Everything marked **[verified]** was run or read by me in this container;
everything marked **[read]** comes from the cited page and was not independently reproduced.

## 0. Environment facts [verified]

| Item | Result |
|---|---|
| Python 3.11.15, numpy 2.4.4, scipy 1.17.1, OpenCV 4.13.0, Pillow 12.2, scikit-image 0.26, imageio 2.37 + imageio-ffmpeg 0.6 | present |
| cffi 2.0, matplotlib 3.10, scikit-learn 1.8 | present |
| gcc 13.3, ffmpeg 6.1.1 | present |
| numba, torch, mixbox, noise, opensimplex | absent |
| CPUs = 2, RAM = 7 GB free | as stated |
| OpenCV contrib modules (`cv2.xphoto.oilPainting`, `cv2.ximgproc`) | **not usable**: three opencv wheels are installed, the loaded one is built without contrib. Not needed. |
| `skimage.feature.structure_tensor` | present (useful for automatic flow fields) |
| `pip download pymixbox` | works; wheel is pure Python, 123 kB |
| `pip download opensimplex` (MIT, pure Python, has vectorised `noise2array`) | works |
| `pip download pykuwahara` | works but **GPL-3** and only isotropic; do not use |
| `pip download torch` (CPU wheel, 196 MB) from download.pytorch.org/whl/cpu | works (wheel deleted again) |
| Hugging Face: API and LFS range download (`timm/vgg16.tv_in1k/model.safetensors`) | **reachable** (HTTP 200 / 206) |
| GitHub: `raw.githubusercontent.com` | reachable; the search API and unlisted-repo API are blocked (403) |
| Wikimedia Commons image download | 429 rate-limited every time, even with a User-Agent: **not reliable** |
| Art Institute of Chicago IIIF images | 403 (blocked); their JSON API works |
| Met Museum: image host reachable, but the API flags every Monet as `isPublicDomain: false` with no image URL | **no usable Monet references** |

Conclusion: no Monet reference images can be fetched programmatically. If the user wants calibration against
real Monets they must attach them to the conversation.

### Timing experiments [verified] (scripts in `oilpaint/exp/`)

`t1_numpy_dabs.py` - pure-numpy alpha-compositing of soft round dabs plus a height add, Python loop per dab:

| canvas | r=4 px | r=12 px | r=40 px |
|---|---|---|---|
| 600x750 | 36.6k dabs/s | 33.0k dabs/s | 7.9k dabs/s |
| 2400x3000 | 37.3k dabs/s | 29.1k dabs/s | 4.9k dabs/s |

Python call overhead (~30 us/dab) dominates for small dabs.

`t2_dab.c` + `t2_ctypes.py` - gcc -O3 shared library, called via ctypes, each dab does a weighted pick-up read of the
canvas, an RGB deposit, and a height add (build time 0.17 s):

| canvas | r=4 | r=12 | r=40 | throughput |
|---|---|---|---|---|
| 600x750 | 862k dabs/s | 148k dabs/s | 11.7k dabs/s | 43-67 Mpix/s |
| 2400x3000 | 216k dabs/s | 71k dabs/s | 9.0k dabs/s | 11-45 Mpix/s (cache misses on random positions; real strokes are spatially coherent) |

`t3_mixbox_np.py` - vectorised Mixbox 2.0 in numpy (LUT gather + 20-term cubic polynomial):
matches the reference scalar code to 2e-7; rgb->latent 1.4 Mpix/s, latent->rgb 3.3 Mpix/s (single core).
Qualitative check: cobalt blue `#002185` + cadmium yellow `#fcd200` at 50% gives **#298139 (green)** with Mixbox
vs **#7e7a42 (olive-grey)** with RGB lerp; sea blue `#2f5490` + white gives `#7ea4ca` vs chalky `#94a4bd`.

`t4_lighting.py` at 2400x3000: normals + Blinn-Phong + weave = 0.43 s; GaussianBlur sigma 3 / 15 / 60 = 0.07 / 1.4 / 7.0 s
(large blurs must be done at reduced resolution); PNG write 0.29 s; a 7-channel float32 latent canvas is 202 MB.

## 1. Painterly rendering (stroke-based rendering)

**Hertzmann 1998, "Painterly Rendering with Curved Brush Strokes of Multiple Sizes"** [read, PDF at mrl.cs.nyu.edu]
- Coarse-to-fine: for each brush radius R_i (largest first) build a reference = source blurred with sigma = f_s * R_i,
  then `paintLayer`: difference image D = |canvas - reference|; grid of spacing f_g * R_i; for each cell, if the area error
  exceeds threshold T, start a stroke at the max-error pixel in the cell; strokes rendered in random order.
- `makeSplineStroke`: colour = reference colour at start; walk in steps of R along the **gradient normal** (Sobel of
  luminance), choosing the sign that minimises curvature; IIR filter with coefficient f_c on the direction; stop when
  max length reached, when |reference - strokeColour| > |reference - canvas| at the point, or when the gradient vanishes.
- Style parameters: T, radii, f_c, f_s, alpha, f_g, min/max length, colour jitter (h,s,v or r,g,b).
  Impressionist preset: T=100, R=(8,4,2), f_c=1, f_s=0.5, alpha=1, f_g=1, len 4..16.
  Expressionist: T=50, f_c=0.25, alpha=0.7, len 10..16, jitter v=0.5. Pointillist: R=(4,2), f_g=0.5, len 0, jitter v=1,h=0.3.
- Take-away for us: error-driven placement + gradient-normal paths is the right skeleton, but gradient normals are
  meaningless in flat sky/sea regions, so we need an **authored flow field** (Litwinowicz's problem too).

**Hertzmann 2002, "Fast Paint Texture"** [read, dgp.toronto.edu PDF]
- Each stroke gets an opacity texture and a height texture (random per stroke, per size).
- Height map is **composited, not summed**: "We experimented with adding stroke heights instead of compositing, but found
  it difficult to prevent hidden strokes from appearing in the resulting height field." Strokes are drawn with a rising
  base level so later strokes sit above earlier ones; boundaries appear as height discontinuities.
- Final: normals from the height field, Phong shading. 10k-40k strokes per painting.

**Litwinowicz 1997, "Processing Images and Video for an Impressionist Effect"** [read via davis.wpi.edu course notes]
- Strokes: centre, length, radius, angle; angle = gradient direction + 90; positions jittered; colour = bilinear sample.
- Areas of near-zero gradient: orientations interpolated with a thin-plate spline from confident pixels.
- Strokes are grown from the centre and **clipped where the Sobel edge magnitude decreases along the stroke** (edge
  preservation). Random perturbation of length, radius, colour, angle in user ranges.

**Shiraishi & Yamaguchi 2000 (NPAR)** [ACM DOI 10.1145/340916.340923; page fetch blocked, summarised from memory]
- Rectangular strokes whose position, size and orientation come from the 0th/1st/2nd-order image moments of a local
  colour-difference image; stroke density from dithering a "stroke area" image; strokes sorted large to small.
- Take-away: moment-based orientation is a good automatic fallback where no flow is authored (rock facets).

**Kyprianidis, Kang, Doellner 2009, "Image and Video Abstraction by Anisotropic Kuwahara Filtering"** [read project page]
- Structure tensor (Sobel gradients, Gaussian-smoothed) -> orientation and anisotropy A = (l1-l2)/(l1+l2); ellipse with
  a = (alpha+A)/alpha * r, b = alpha/(alpha+A) * r; N=8 weighted sectors; output = sum(m_i * s_i^-q) / sum(s_i^-q),
  typical r = 6..8, q = 8, alpha = 1. Code was on code.google.com/p/gpuakf (GPU). We will implement it in numpy/cv2 if
  needed (8 `filter2D` passes of mean and mean-of-squares, cheap), but only as an **optional target pre-flattening step**,
  never on the painting itself: applied to the painting it reads as a filter.

## 2. Physically based paint models

**Baxter, Scheib, Lin, Manocha 2001, DAB** [read gamma.cs.unc.edu PDF]
- Two layers per surface (wet "surface" layer + "deep" reservoir); brush footprint by projecting the 3D brush head onto the
  canvas; **bidirectional transfer** each step: volume leaving V_l = V_i - T*R (rate*time), colour C_new = V_r*C_i + V_l'*C_i'
  (additive/volume-weighted). Wet/dry composite alpha = min(V_w * O_t, 1). Deep canvas layer is a height field with unlimited
  accumulation. They note KM would be more accurate but too costly then.

**Baxter, Wendt, Lin 2004, IMPaSTo** [read gamma.cs.unc.edu PDF]
- One active wet height-field layer, unlimited dry layers, a brush layer; per cell: paint volume + up to 8 pigment
  concentrations.
- Transfer rule (Algorithm 1): direction decided by whether the canvas cell or the brush cell holds more paint; amount =
  0.1 of the source, cut off smoothly when amounts are nearly equal (cutoff 1/30), cut off below a brush speed of
  0.2-0.3 cells/step (friction), max 0.001 units/step; concentrations updated by volume-weighted average.
- Motion: v = v_brush/2 + (-c * grad(pressure)); conservative flux advection; CFL limited to 1 cell/step; **a minimum
  amount of paint always stays in a cell** (canvas absorption).
- Rendering: Kubelka-Munk with 8 wavelengths chosen by Gaussian quadrature; per-layer R = 1/(1 + K/S + b*tanh(b*S*d)),
  T = b*R*sinh(b*S*d), b = sqrt(K/S*(K/S+2)); layers composited with R_tot = R + T^2*R_prev/(1 - R*R_prev);
  then Blinn-Phong specular + bump map from the height field. Drying = the bottom X% of the wet layer becomes a dry layer.
- Take-away: the *ideas* we keep are (a) the wet/dry distinction, (b) transfer proportional to the difference in
  loads, (c) height field + bump lighting, (d) "some paint always stays". Full advection is unnecessary for an offline
  renderer that never drags a brush interactively.

**Stuyck, Da, Chen, Bonanni 2017, "Real-Time Oil Painting on Mobile Hardware"** [read tuurstuyck.github.io PDF]
- Brush = **two textures**: a grayscale stamp shape (scales deposit) and a reservoir texture holding pigments.
  Reservoir is refilled 5% per stroke with the chosen colour; brush pigments = lerp(brush, canvas under stamp, artist-set).
- Canvas: height h, velocity (u,v), background height b (paper + dried paint), fixed number of pigment layers
  (3 pigments per RGBA texture per layer); new pigment goes into the lowest free layer; layers compress when full.
- Paint motion: shallow-water equations with viscosity/gravity modulated by local pigment density (viscoelastic
  behaviour); pigments advected semi-Lagrangian; a minimum residue always sticks to the paper.
- Colour: Kubelka-Munk per pigment layer, converted to RGB. Lighting: SH irradiance (9 coeffs) + Torrance-Sparrow fit to
  measured oil paint + Schlick reflections. 1024x768 sim at 46 fps on iPad Air 2; stamping 27%, height solve 30%.
- Take-away: a *stamp + reservoir* brush is what a shipping oil app uses; bristles are not simulated. Good precedent for
  our brush model.

**Chen, Kim, Wang, Yu, Sigal, Bryant 2015, Wetbrush** [read wanghmin.github.io abstract]
- Bristle-level 3D simulation, hybrid particles (near brush) + density field (canvas), all in CUDA. Out of reach on a
  2-core CPU; cited only as the quality ceiling.

**Chu & Tai 2005, MoXi** - lattice-Boltzmann ink diffusion on GPU; watercolour/ink, not oil; not pursued.

**libmypaint (ISC licence) smudge model** [verified by reading `mypaint-brush.c` and `helpers.c` from GitHub raw]
- Per-dab: sample the canvas colour under a radius `radius*exp(smudge_radius_log)` (at most every second dab), then
  `smudge_state = fac_old*smudge_state + (1-fac_old)*a*sampled` with `fac_old = smudge_length` (0.01..1);
  dab colour = `smudge_value*smudge_state + (1-smudge_value)*brush_colour`.
- MyPaint 2.0 "pigment" mode: RGB -> 10-band spectral reflectance by three fixed upsampling curves
  (`spectral_r_small/g/b`), mix by **weighted geometric mean** `prod(spec_i^w_i)`, back to RGB by a 3x10 matrix
  `T_MATRIX_SMALL`. This is a permissively licensed alternative to Mixbox; I copied the tables into
  `oilpaint/exp/helpers.c` for reference.

**Corel Painter impasto** [read product help]: per-stroke depth channel with Amount (bump strength), Picture (how much
colour vs relief), Shine (specular), light Brightness/Concentration/Exposure. Rebelle "oils" similarly keep a thickness
map plus canvas texture. Both are proprietary; only the parameter vocabulary is borrowed.

## 3. Pigment mixing

**Mixbox (Sochorova & Jamriska, SIGGRAPH Asia 2021), pymixbox 2.0.0** [verified from the wheel]
- Licence in the wheel: **CC BY-NC 4.0**, "If you want to obtain commercial license, please contact mixbox@scrtwpns.com".
  Fine for a hobbyist's personal picture; attribution required ("Mixbox (c) 2022 Secret Weapons"); not for sale/commerce.
- Algorithm: a 64^3 LUT maps sRGB to concentrations of 4 latent pigments (c0..c2 stored, c3 = 1 - sum); a 20-term cubic
  polynomial maps concentrations back to RGB; a per-colour RGB **residual** (latent[4:7]) makes the round trip exact.
  Mixing = linear interpolation of the 7-vector latent, then `latent_to_rgb`. Pure Python scalar in the wheel
  (~0.05 ms per call); trivially vectorised in numpy (done, see timings) and portable to C (LUT is 786 kB).
- Official ports: C++, Python, JS, GLSL, HLSL, C#, Java, Rust.

**spectral.js (Ronald van Wijnen)** [verified licence on GitHub raw: MIT]
- RGB -> reflectance via 7 primaries (W,C,M,Y,R,G,B), single-constant KM: K/S = (1-R)^2/(2R), concentrations
  C = f^2 * T^2 * L (mixing factor, tinting strength, luminance), inverse KM, XYZ via CIE 1931 + D65, OKLab gamut mapping.
  JavaScript only; porting is ~150 lines. MIT makes it a clean fallback.

**Kubelka-Munk proper** (IMPaSTo): needs measured K, S per pigment per wavelength; we have none, and deriving them from
RGB tube colours is an ill-posed inversion. Not worth it here.

Recommendation: **Mixbox latent space as the canvas colour representation** (mix = lerp in latent; convert to RGB only
for display), with the libmypaint 10-band WGM as a switchable permissive backend. RGB lerp kept only for A/B tests.

## 4. Monet: materials, technique, implementable rules

Sources: Jackson's Art blog on the National Gallery Water Lilies analysis [read]; liveabout.com palette article [read];
Art Institute of Chicago online catalogue, *Cliff Walk at Pourville* (1882) technical entry [read]; MFA Boston Rouen
Cathedral article [read]; Wikipedia *Pyramides at Port-Coton, Rough Sea* [read]; Belle-Ile letters via mydailyartdisplay
[read]; Callen, *The Art of Impressionism* via a review [read].

Facts:
- Palette (Monet's 1905 letter): "flake white, cadmium yellow, vermilion, deep madder, cobalt blue, emerald green, and
  that's all." Analyses add French ultramarine, cobalt violet, viridian, chrome/cadmium yellows, cadmium orange, red lake.
  Ivory black abandoned around 1868; darks made from complementaries; violet in shadows.
- Ground: shop-primed then often a second ground by Monet, "often white, sometimes a slightly tinted pink or grey";
  Pourville ground is cream (lead white + chalk), 60-125 um.
- Pourville: sky has a pale blue-grey underlayer; clouds built with **thicker strokes of lead white toned with cobalt
  blue and vermilion**; the intense blue applied *around* the clouds. Sea: **pale greenish-grey underlayer** left visible
  through a network of **roughly horizontal, curving strokes**. Grass: randomly oriented strokes, **low impasto, light
  strokes that skip across the high points of the underlying texture** (dry-brush over dry impasto). Figures: thick
  wet-in-wet buildup "but with little actual blending of colours on the canvas"; final grass/figure strokes dragged
  back and forth wet-in-wet to blur the boundary. Both wet-in-wet and wet-over-dry present (several sessions).
  Brushes identified: **1.0 and 1.5 cm flat** on a 66.5 x 82.3 cm canvas (1.2-1.8 % of canvas width).
  Canvas: plain weave, 19-20 threads/cm.
- Water Lilies (late): "painted darks to lights, pastel colours over midtone blues", "no real darks", thick rough layer
  left to dry then another rough layer skipping over the texture; long thin strokes for oranges/yellows; final
  highlights = single colour + white impasto touches.
- Rouen Cathedral: "layer on layer of paint encrust the canvas"; reworked for months; architecture read through colour
  (pink/violet pastel, deep blue recesses, oranges emanating from the stone) rather than drawing.
- Belle-Ile 1886: "terrifying rocks and a sea of unbelievable colours"; "sinister, diabolical and magnificent"; dark
  craggy rocks against vivid blue/green/violet waves, white on crests; rocks painted "in a fluid or flame-like way".
  Series of the same rocks under different light. Canvases 65 x 81 cm.

Implementable rules (my synthesis, in canvas-width units "cw"):

| Aspect | Rule |
|---|---|
| Ground | cream or pale warm grey-violet, visible in a few breaks only |
| Palette | 7-9 tubes + white; every colour on canvas is a Mixbox mixture of <= 3 tubes + white; no black tube |
| Shadows | violet/ultramarine + a little madder; never below L* ~ 12 |
| Light | lead white tinted with cadmium yellow/orange/vermilion; thickest paint in the lights |
| Brush widths | main work 1.2-2 % cw (flat), block-in 4-8 % cw, accents 0.5-1 % cw |
| Stroke length | sky 8-30 % cw dragged; water 2-6 % cw choppy; rock 3-8 % cw curved; touches 1-2 % cw |
| Direction | sky: sweeping arcs with the weather; water: near-horizontal with wave undulation, spray upward flicks; rock: follow contour/facet; architecture: vertical but broken by atmosphere |
| Layering | thin dilute block-in -> dry -> thick strokes wet-in-wet with little blending -> dry -> scumbles that skip ridges -> final impasto touches |
| Edges | no drawn contours; boundaries formed by adjacent strokes of different colour; later strokes cross the boundary |
| Complementary vibration | orange/gold flecks in violet sky, violet flecks in gold light, green in the red bands |

## 5. Open-source code survey

| Project | Licence | Verdict |
|---|---|---|
| pymixbox 2.0.0 (PyPI) | CC BY-NC 4.0 | use for personal project; vectorise ourselves (done) |
| libmypaint (GitHub) | ISC | port the smudge and 10-band WGM ideas (tables copied); do not build the library (needs glib/json-c and a surface backend) |
| spectral.js | MIT | port as fallback mixer if Mixbox licence is unacceptable |
| PyPainterly, painterPython, etc. (Hertzmann ports) | GPL-3 / unlicensed | do not reuse; algorithm is ~150 lines, write our own |
| pykuwahara | GPL-3, isotropic only | no |
| opensimplex | MIT | optional noise source (pure Python, vectorised) |
| Stylized Neural Painting, Paint Transformer, Learning to Paint | MIT/Apache but weights on Google Drive/Baidu | not obtainable; and CPU inference at 2400x3000 would be hours |
| Gatys style transfer with VGG from Hugging Face + torch CPU | reachable and installable (~700 MB) | technically possible only if the user attaches Monet style images; would still need to be followed by a paint pass to get paint body; 2 cores -> tens of minutes at preview size, hours at full size. Not recommended as core. |

## 6. Sources actually used
- Hertzmann 1998 PDF: https://mrl.cs.nyu.edu/publications/painterly98/hertzmann-siggraph98.pdf
- Hertzmann 2002 Fast Paint Texture: https://www.dgp.toronto.edu/papers/ahertzmann_NPAR2002.pdf
- Litwinowicz 1997 (course summary): https://davis.wpi.edu/~matt/courses/impressionist/
- Shiraishi & Yamaguchi 2000: https://dl.acm.org/doi/10.1145/340916.340923
- Kyprianidis et al. 2009: https://www.kyprianidis.com/p/pg2009/
- DAB 2001: http://gamma.cs.unc.edu/DAB/files/DAB.pdf
- IMPaSTo 2004: http://gamma.cs.unc.edu/IMPASTO/publications/Baxter-IMPaSTo_Print-NPAR04.pdf
- Stuyck et al. 2017: https://tuurstuyck.github.io/assets/oilpaint_low_res.pdf
- Wetbrush 2015: https://wanghmin.github.io/publication/chen-2015-wgb/
- libmypaint sources: https://raw.githubusercontent.com/mypaint/libmypaint/master/mypaint-brush.c and .../helpers.c
- Mixbox: https://scrtwpns.com/mixbox/ and the pymixbox 2.0.0 wheel (LICENSE, mixbox.py)
- spectral.js: https://raw.githubusercontent.com/rvanwijnen/spectral.js/master/README.md and .../LICENSE
- Corel Painter impasto help: http://product.corel.com/help/Painter/540213829/Main/EN/Win-Documentation/Corel-Painter-Impasto-lighting-and-depth.html
- Monet palette/technique: https://www.jacksonsart.com/blog/2024/04/02/recreating-the-colour-palette-of-claude-monet/ ,
  https://www.liveabout.com/impressionist-masters-palettes-techniques-claude-monet-2578614
- AIC Cliff Walk at Pourville technical entry: https://publications.artic.edu/api/epub/monet/135468/print_view
- MFA Rouen Cathedral: https://www.mfa.org/article/2020/rouen-cathedral-series
- Belle-Ile: https://en.wikipedia.org/wiki/The_Pyramides_at_Port-Coton,_Rough_Sea ,
  https://mydailyartdisplay.uk/2013/10/09/john-peter-russell-part-2-belle-ile-monet-and-matisse/
- Callen review: https://makingamark.blogspot.com/2008/02/art-of-impressionism-and-associated.html
- Hertzmann ports (licence check): https://raw.githubusercontent.com/pschaldenbrand/PyPainterly/master/LICENSE
