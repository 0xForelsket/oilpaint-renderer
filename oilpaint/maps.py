"""Guide maps: what the planner paints from.  target (soft RGB), region ids and soft masks, flow field,
light map.  Built from a scene at any resolution, or from arrays/images for tests."""
import numpy as np
import cv2
from skimage.feature import structure_tensor

from . import light as L


class GuideMaps:
    def __init__(self, W, H, target, region_id, masks, names, flow, light_map, size_scale=None, region_flows=None):
        self.W, self.H = W, H
        self.region_flows = {k: np.ascontiguousarray(v, np.float32) for k, v in (region_flows or {}).items()}
        self.target = np.ascontiguousarray(target, np.float32)          # (H,W,3) 0..1
        self.region_id = np.ascontiguousarray(region_id, np.uint8)      # (H,W)
        self.masks = {k: np.ascontiguousarray(v, np.float32) for k, v in masks.items()}
        self.names = list(names)                                        # index -> name
        self.flow = np.ascontiguousarray(flow, np.float32)              # (H,W,2) unit vectors
        self.light = np.ascontiguousarray(light_map, np.float32)        # (H,W) 0..1
        self.size_scale = np.ones((H, W), np.float32) if size_scale is None else size_scale
        self._ref_cache = {}

    def index(self, name):
        return self.names.index(name)

    def reference(self, sigma_px):
        key = round(float(sigma_px), 2)
        if key not in self._ref_cache:
            self._ref_cache[key] = L.blur(self.target, sigma_px) if sigma_px > 0.3 else self.target.copy()
        return self._ref_cache[key]

    @staticmethod
    def from_arrays(target, region_id=None, masks=None, names=None, flow=None, light_map=None, flow_sigma=0.02):
        H, W = target.shape[:2]
        if region_id is None:
            region_id = np.zeros((H, W), np.uint8)
            names = ["all"]; masks = {"all": np.ones((H, W), np.float32)}
        if flow is None:
            flow = structure_flow(target, sigma_px=flow_sigma * W)
        if light_map is None:
            light_map = np.zeros((H, W), np.float32)
        return GuideMaps(W, H, target, region_id, masks, names, flow, light_map)

    def resized(self, W2):
        """Same maps at another width (nearest for ids, area/linear for the rest)."""
        H2 = int(round(self.H * W2 / self.W))
        inter = cv2.INTER_AREA if W2 < self.W else cv2.INTER_LINEAR
        target = cv2.resize(self.target, (W2, H2), interpolation=inter)
        rid = cv2.resize(self.region_id, (W2, H2), interpolation=cv2.INTER_NEAREST)
        masks = {k: cv2.resize(v, (W2, H2), interpolation=inter) for k, v in self.masks.items()}
        flow = cv2.resize(self.flow, (W2, H2), interpolation=inter)
        n = np.linalg.norm(flow, axis=-1, keepdims=True) + 1e-6
        flow = flow / n
        lm = cv2.resize(self.light, (W2, H2), interpolation=inter)
        rf = {}
        for k, v in self.region_flows.items():
            f2 = cv2.resize(v, (W2, H2), interpolation=inter)
            rf[k] = f2 / (np.linalg.norm(f2, axis=-1, keepdims=True) + 1e-6)
        return GuideMaps(W2, H2, target, rid, masks, self.names, flow, lm, region_flows=rf)


def structure_flow(target, sigma_px=12.0):
    """Flow along local edges (minor eigenvector of the smoothed structure tensor), unit vectors (H,W,2).
    Where the tensor is isotropic the direction is meaningless; callers blend with authored fields."""
    gray = (0.299 * target[..., 0] + 0.587 * target[..., 1] + 0.114 * target[..., 2]).astype(np.float32)
    Axx, Axy, Ayy = structure_tensor(gray, sigma=max(1.0, sigma_px), order="rc")  # rc: A[0]=rr (y), A[2]=cc (x)
    # orientation of the gradient (major eigenvector): theta = 0.5*atan2(2*Axy, Axx - Ayy) in (row, col) terms
    theta = 0.5 * np.arctan2(2 * Axy, Ayy - Axx)   # angle of the gradient measured from the x axis
    # flow is perpendicular to the gradient
    fx = -np.sin(theta); fy = np.cos(theta)
    flow = np.stack([fx, fy], -1).astype(np.float32)
    # anisotropy for optional blending
    l1 = 0.5 * (Axx + Ayy + np.sqrt((Axx - Ayy) ** 2 + 4 * Axy ** 2))
    l2 = 0.5 * (Axx + Ayy - np.sqrt((Axx - Ayy) ** 2 + 4 * Axy ** 2))
    aniso = (l1 - l2) / (l1 + l2 + 1e-9)
    return flow, aniso.astype(np.float32)


def flow_preview(flow, W, H, step=None, bg=None):
    """Draw the flow field as short line segments on a light background (for the guides output)."""
    step = step or max(6, W // 60)
    img = np.full((H, W, 3), 235, np.uint8) if bg is None else L.to8(bg).copy()
    for y in range(step // 2, H, step):
        for x in range(step // 2, W, step):
            dx, dy = flow[y, x]
            l = step * 0.45
            cv2.line(img, (int(x - dx * l), int(y - dy * l)), (int(x + dx * l), int(y + dy * l)), (40, 40, 160), 1, cv2.LINE_AA)
    return img


def regions_preview(region_id, names):
    rng = np.random.default_rng(3)
    lut = (rng.random((256, 3)) * 180 + 60).astype(np.uint8)
    img = lut[region_id]
    for i, n in enumerate(names):
        ys, xs = np.where(region_id == i)
        if len(xs):
            cv2.putText(img, n, (int(xs.mean()) - 20, int(ys.mean())), cv2.FONT_HERSHEY_SIMPLEX, 0.5, (0, 0, 0), 1, cv2.LINE_AA)
    return img
