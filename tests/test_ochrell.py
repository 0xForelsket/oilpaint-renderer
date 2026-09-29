"""Run: python -m unittest discover -s tests -p test_ochrell.py -v"""
import tempfile
import importlib.util
import unittest
from pathlib import Path
import numpy as np
from oilpaint import mix, strokes, light
from oilpaint.canvas import Canvas, Stroke
from oilpaint.render import save_state, load_state, save_strokes, load_strokes
from oilpaint.ochrell import validate_state


class Integration(unittest.TestCase):
    def setUp(self):
        mix.set_backend("ochrell")

    def stroke(self, color="#1455ff", mode="paint", **kwargs):
        return strokes.make_stroke(strokes.straight(.08, .22, .92, .22, .13), color, 1907,
                                   mode=mode, deplete=0., **kwargs)

    def test_canonical_and_grouped_weight(self):
        a = mix.rgb_to_latent(np.array([255, 220, 0]) / 255)
        b = mix.rgb_to_latent(np.array([20, 70, 255]) / 255)
        w = mix.rgb_to_latent(np.ones(3))
        np.testing.assert_array_equal(np.round(mix.latent_to_rgb((a+b)/2)*255), [101,152,94])
        direct = (2*a+3*b+5*w)/10
        grouped = ((2*a+3*b)/5*5+5*w)/10
        np.testing.assert_allclose(direct, grouped, rtol=2e-7, atol=2e-7)

    def test_batch_single_determinism_and_exact_resume(self):
        ss = [self.stroke("#ffdc00"), self.stroke(), self.stroke("#ee2244", pickup=.8),
              self.stroke(mode="smudge", pickup=.9), self.stroke("#ffffff", mode="scumble")]
        batch, single, resumed = [Canvas(100, 48) for _ in range(3)]
        batch.render(ss)
        for s in ss:
            single.render_stroke(s)
        with tempfile.TemporaryDirectory() as d:
            split = Canvas(100, 48); split.render(ss[:3])
            path = Path(d)/"state.npz"; save_state(path, split); load_state(path, resumed)
            resumed.render(ss[3:])
            path = Path(d)/"strokes.npz"; save_strokes(path, ss, [(0,len(ss))]); loaded, ranges = load_strokes(path)
            replay = Canvas(100, 48); replay.render(loaded)
        for field in ("lat", "rgb", "amount", "h", "wet", "cover"):
            for cv in (single, resumed, replay):
                np.testing.assert_array_equal(getattr(batch, field), getattr(cv, field), err_msg=field)
        validate_state(batch.lat, "ochrell")
        self.assertTrue(np.any(batch.amount > 0))
        self.assertTrue(np.isfinite(light.relight(batch.rgb, batch.h)).all())

    def test_glaze_and_empty_smudge_do_not_change_material(self):
        cv = Canvas(100,48); before = cv.lat.copy()
        cv.render_stroke(self.stroke(mode="smudge", pickup=1.))
        np.testing.assert_array_equal(cv.amount, 0)
        np.testing.assert_array_equal(cv.lat, before)
        before_rgb = cv.rgb.copy()
        cv.render_stroke(self.stroke(mode="glaze"))
        np.testing.assert_array_equal(cv.lat, before)
        np.testing.assert_array_equal(cv.amount, 0)
        self.assertGreater(float(np.max(np.abs(before_rgb-cv.rgb))), .1)

    def test_wet_pickup_and_repeated_streaks_keep_valid_states(self):
        wet, dry = Canvas(100,48), Canvas(100,48)
        yellow, blue = self.stroke("#ffdc00"), self.stroke(pickup=1., release=0., streak=1., marble=1., color2="#000000")
        wet.render_stroke(yellow); dry.render_stroke(yellow); dry.dry(0.)
        for _ in range(20):
            wet.render_stroke(blue); dry.render_stroke(blue)
        validate_state(wet.lat, "ochrell"); validate_state(dry.lat, "ochrell")
        self.assertGreater(float(np.max(np.abs(wet.rgb-dry.rgb))), 1e-3)

    def test_wrong_backend_and_invalid_buffers_are_rejected(self):
        cv = Canvas(80,40); st = self.stroke()
        st.params["author_note"] = "metadata ignored by make_params"
        cv.render_stroke(st)
        st.zcol = st.zcol[:7]
        with self.assertRaises(ValueError): cv.render_stroke(st)
        st = self.stroke(); st.params["marble"] = 2
        with self.assertRaises(ValueError): cv.render_stroke(st)
        st = self.stroke(); st.zcol[41] = 0
        with self.assertRaises(ValueError): cv.render_stroke(st)
        with tempfile.TemporaryDirectory() as d:
            p = Path(d)/"state.npz"; save_state(p, cv)
            mix.set_backend("ochrell-roundtrip")
            with self.assertRaises(ValueError): load_state(p, Canvas(80,40))
            with self.assertRaises(ValueError): Canvas(80,40).render_stroke(self._old_stroke())

    def _old_stroke(self):
        mix.set_backend("ochrell")
        return self.stroke()

    def test_matched_controls_have_identical_surface_geometry(self):
        planes = []
        for backend in ("ochrell", "ochrell-roundtrip", "ochrell-srgb"):
            mix.set_backend(backend); cv = Canvas(100,48)
            cv.render([self.stroke("#ffdc00"), self.stroke(pickup=.8), self.stroke("#ff2233", mode="scumble")])
            planes.append((cv.h,cv.cover,cv.amount))
        for p in planes[1:]:
            for a,b in zip(p,planes[0]): np.testing.assert_array_equal(a,b)

    @unittest.skipUnless(importlib.util.find_spec("mixbox"), "optional Mixbox comparator not installed")
    def test_mixbox_control_uses_original_decoder_and_same_transport(self):
        cols = np.random.default_rng(7).random((128,3),dtype=np.float32)
        mix.set_backend("mixbox"); expected = mix.latent_to_rgb(mix.rgb_to_latent(cols))
        mix.set_backend("mixbox-material"); actual = mix.latent_to_rgb(mix.rgb_to_latent(cols))
        np.testing.assert_allclose(actual,expected,atol=5e-7)
        controls=[]
        for backend in ("mixbox-material","ochrell"):
            mix.set_backend(backend); cv=Canvas(100,48)
            cv.render([self.stroke("#ffdc00"),self.stroke(pickup=.8),self.stroke(mode="smudge",pickup=.8)])
            controls.append(cv)
        for k in ("h","amount","cover","wet"):
            np.testing.assert_array_equal(getattr(controls[0],k),getattr(controls[1],k))


if __name__ == "__main__": unittest.main()
