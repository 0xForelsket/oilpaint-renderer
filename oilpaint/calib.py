"""Calibration targets for the diagnostics in metrics.py.

These are GUESSES from reading about Monet, not measurements.  They are printed as diagnostics, never used as
pass/fail gates.  If real Monet reference images become available, measure them with
`python -m oilpaint measure ref1.jpg ref2.jpg ...` and replace the numbers here.
"""
TARGETS = dict(
    # luminance (Lab L*)
    L_min_expected=12.0,          # Monet has no real darks; darkest passages ~ L* 12-20
    L_p01=18.0, L_p50=(55.0, 75.0), L_p99=95.0,
    # chroma (Lab sqrt(a^2 + b^2))
    chroma_mean=(14.0, 30.0), chroma_p90=(35.0, 60.0),
    # hue lobes for this picture (degrees, Lab hue): violet-blue vs gold-peach
    hue_lobe_cool=(230.0, 300.0), hue_lobe_warm=(30.0, 75.0),
    lobe_mass_min=0.25,           # each lobe should hold at least this fraction of chromatic pixels
    # edge softness: silhouette gradient / typical stroke-edge gradient
    edge_ratio_max=1.3,
    # paint body: fraction of pixels whose lit/unlit luminance differs by > 3%
    relief_coverage=(0.3, 0.9),
    # mud: chroma loss in overlapped areas relative to the palette colours actually used
    mud_score_max=0.25,
)

GATES = dict(
    black_L_floor=8.0,            # no pixel darker than this L*
    black_frac_L18=0.005,         # at most this fraction of pixels with L* < 18
)
