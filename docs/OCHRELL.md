# Ochrell integration

The existing Python scene, planner, Canvas and CLI now support
`--mixer ochrell`, using `native/ochrell-brush` and the independent sibling Rust
library. The original `mixbox` default and C kernel remain available.

The detailed [integration guide](../../ochrell/docs/integration.md) describes
setup, storage formats, quantity separation, approximations and reproduction.
The [measured report](../../ochrell/docs/integration-results.md) includes native
mixing, Python adapter batches, brush replay, memory, reference errors, optional
Mixbox comparisons and visual evidence. The [implementation plan](../../ochrell/docs/integration-plan.md)
records the inspected baseline.

```powershell
uv venv .venv
uv pip install --python .venv/Scripts/python.exe -r requirements-ochrell.txt
.venv/Scripts/python.exe -m oilpaint render scenes/storm_v3.py --size 600x750 --plan-width 320 --mixer ochrell --no-layers --out out/ochrell-storm
```

For direct brush use:

```python
from oilpaint import mix, strokes
from oilpaint.canvas import Canvas

mix.set_backend("ochrell")
canvas = Canvas(600, 300)
yellow = strokes.make_stroke(strokes.straight(.1, .2, .7, .2, .08), "#ffdc00", 1)
blue = strokes.make_stroke(strokes.straight(.3, .22, .9, .22, .08), "#1446ff", 2, pickup=.8)
canvas.render([yellow, blue])
# canvas.lat is preserved material; amount, cover, h, wet and rgb are separate.
```

Native builds need Rust and a linker; neither C compilation nor Mixbox is needed
for Ochrell. The separately selected `OILPAINT_KERNEL=rust` enables the original
Rust kernel for the old RGB/Mixbox backends on hosts without a C compiler.

Tests: `python -m unittest discover -s tests -p test_ochrell.py -v` and
`python tools/check_ochrell.py`. The optional Mixbox comparison is
`uv pip install --python .venv/Scripts/python.exe pymixbox==2.0.0` followed by
`python tools/compare_mixbox.py`. It uses existing comparator code only; see
`NOTICE.md` before distributing either backend.

This is an offline painting path, not a new live GUI. Ochrell does not model
glazing, optical thickness, real tubes or conserved paint transport. Full f32
states are retained; naive f16 was measured and rejected for tiny updates.
