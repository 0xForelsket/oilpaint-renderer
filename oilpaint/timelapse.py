"""Time-lapse capture: frames taken during the replay, downsampled, lit, captioned, encoded with ffmpeg."""
import numpy as np
import cv2
import imageio.v2 as imageio

from . import light as L


class TimelapseWriter:
    def __init__(self, path, size=(1200, 1500), fps=24, captions=True, light_kw=None, crf=20):
        self.path = path
        self.size = (int(size[0]), int(size[1]))
        self.fps = fps
        self.captions = captions
        self.light_kw = dict(light_kw or {})
        self.n = 0
        self.writer = imageio.get_writer(path, fps=fps, codec="libx264", pixelformat="yuv420p",
                                         output_params=["-crf", str(crf)], macro_block_size=1)

    def frame(self, rgb_full, h_full, caption=None, lit=True):
        W, H = self.size
        rgb = cv2.resize(rgb_full, (W, H), interpolation=cv2.INTER_AREA)
        if lit:
            h = cv2.resize(h_full, (W, H), interpolation=cv2.INTER_AREA)
            img = L.relight(rgb, h, **self.light_kw)
        else:
            img = rgb
        img8 = L.to8(img)
        if self.captions and caption:
            img8 = self._caption(img8, caption)
        self.writer.append_data(img8)
        self.n += 1
        return img8

    def hold(self, img8, seconds):
        for _ in range(int(round(seconds * self.fps))):
            self.writer.append_data(img8); self.n += 1

    def _caption(self, img8, text):
        img = img8.copy()
        H, W = img.shape[:2]
        scale = W / 1200.0
        fs = 0.9 * scale
        (tw, th), _ = cv2.getTextSize(text, cv2.FONT_HERSHEY_SIMPLEX, fs, 2)
        pad = int(14 * scale)
        x0, y0 = pad, H - pad - th - 2 * pad
        overlay = img.copy()
        cv2.rectangle(overlay, (x0 - pad, y0 - pad), (x0 + tw + pad, y0 + th + pad), (20, 18, 28), -1)
        img = cv2.addWeighted(overlay, 0.65, img, 0.35, 0)
        cv2.putText(img, text, (x0, y0 + th), cv2.FONT_HERSHEY_SIMPLEX, fs, (245, 240, 230), 2, cv2.LINE_AA)
        return img

    def close(self):
        self.writer.close()
        return self.n
