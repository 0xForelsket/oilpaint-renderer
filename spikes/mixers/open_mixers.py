"""Open (MIT/ISC-compatible) pigment mixers shaped like Mixbox: colour -> linear-mixable latent (+ RGB residual so an
unmixed colour round-trips exactly), mix = lerp in latent, latent -> sRGB.
  wgm : libmypaint 10-band weighted geometric mean (tables from libmypaint helpers.c, ISC).  latent = log(spectrum), 10 + 3.
  km  : Kubelka-Munk single-constant mixing on spectral.js 3.0 data (MIT, (c) Ronald van Wijnen): 7-primary spectral
        upsampling, KS = (1-R)^2/2R, luminance-weighted KS mixing (as spectral.js).  latent = (Y*KS_1..N, Y) + 3.
        `bands` reduces the 38 samples to N groups (base spectra averaged, CMFs summed) to cut per-pixel cost.
All functions are float32 numpy, vectorised over (..., 3) / (..., LAT)."""
import json, os
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))

def srgb_to_lin(c):
    c = np.asarray(c, np.float32)
    return np.where(c <= 0.04045, c / 12.92, ((c + 0.055) / 1.055) ** 2.4).astype(np.float32)

def lin_to_srgb(c):
    c = np.clip(np.asarray(c, np.float32), 0, 1)
    return np.where(c <= 0.0031308, c * 12.92, 1.055 * c ** (1 / 2.4) - 0.055).astype(np.float32)

# ---------------- libmypaint WGM (ISC)
T_SMALL = np.array([[0.026595621243689, 0.049779426257903, 0.022449850859496, -0.218453689278271, -0.256894883201278, 0.445881722194840, 0.772365886289756, 0.194498761382537, 0.014038157587820, 0.007687264480513],
                    [-0.032601672674412, -0.061021043498478, -0.052490001018404, 0.206659098273522, 0.572496335158169, 0.317837248815438, -0.021216624031211, -0.019387668756117, -0.001521339050858, -0.000835181622534],
                    [0.339475473216284, 0.635401374177222, 0.771520797089589, 0.113222640692379, -0.055251113343776, -0.048222578468680, -0.012966666339586, -0.001523814504223, -0.000094718948810, -0.000051604594741]], np.float32)
SR = np.array([0.009281362787953, 0.009732627042016, 0.011254252737167, 0.015105578649573, 0.024797924177217, 0.083622585502406, 0.977865045723212, 1.0, 0.999961046144372, 0.999999992756822], np.float32)
SG = np.array([0.002854127435775, 0.003917589679914, 0.012132151699187, 0.748259205918013, 1.0, 0.865695937531795, 0.037477469241101, 0.022816789725717, 0.021747419446456, 0.021384940572308], np.float32)
SB = np.array([0.537052150373386, 0.546646402401469, 0.575501819073983, 0.258778829633924, 0.041709923751716, 0.012662638828324, 0.007485593127390, 0.006766900622462, 0.006699764779016, 0.006676219883241], np.float32)
EPS = np.float32(0.001)

def _wgm_spec(rgb):
    lin = srgb_to_lin(np.clip(rgb, 0, 1)) * (1 - EPS) + EPS
    return lin[..., 0:1] * SR + lin[..., 1:2] * SG + lin[..., 2:3] * SB

def _wgm_spec_to_srgb(spec):
    lin = np.clip((spec @ T_SMALL.T - EPS) / (1 - EPS), 0, 1)
    return lin_to_srgb(lin)

def wgm_rgb_to_latent(rgb):
    rgb = np.clip(np.asarray(rgb, np.float32), 0, 1)
    spec = _wgm_spec(rgb)
    res = rgb - _wgm_spec_to_srgb(spec)
    return np.concatenate([np.log(spec), res], -1).astype(np.float32)

def wgm_latent_to_rgb(lat):
    lat = np.asarray(lat, np.float32)
    return np.clip(_wgm_spec_to_srgb(np.exp(lat[..., :10])) + lat[..., 10:13], 0, 1).astype(np.float32)

# ---------------- spectral.js-data Kubelka-Munk (MIT data)
_D = json.load(open(os.path.join(HERE, "spectral_data.json")))
_BASE38 = {k: np.array(v, np.float64) for k, v in _D["BASE"].items()}
_CMF38 = np.array(_D["CIE"]["CMF"], np.float64)            # 3 x 38 (D65-weighted)
_XYZ_RGB = np.array(_D["CONV"]["XYZ_RGB"], np.float64)

class KM:
    def __init__(self, bands=38):
        self.N = bands
        groups = np.array_split(np.arange(38), bands)
        self.base = {k: np.array([v[g].mean() for g in groups], np.float32) for k, v in _BASE38.items()}
        self.cmf = np.stack([np.array([row[g].sum() for g in groups]) for row in _CMF38]).astype(np.float32)   # 3 x N
        self.xyz_rgb = _XYZ_RGB.astype(np.float32)
        self.LAT = bands + 1 + 3

    def _R(self, rgb):
        l = srgb_to_lin(np.clip(rgb, 0, 1))
        w = l.min(-1, keepdims=True); l = l - w
        r_, g_, b_ = l[..., 0:1], l[..., 1:2], l[..., 2:3]
        c = np.minimum(g_, b_); m = np.minimum(r_, b_); y = np.minimum(r_, g_)
        r = np.maximum(0, np.minimum(r_ - b_, r_ - g_)); g = np.maximum(0, np.minimum(g_ - b_, g_ - r_)); b = np.maximum(0, np.minimum(b_ - g_, b_ - r_))
        B = self.base
        R = w * B["W"] + c * B["C"] + m * B["M"] + y * B["Y"] + r * B["R"] + g * B["G"] + b * B["B"]
        return np.maximum(R, 1e-7).astype(np.float32)

    def _R_to_srgb(self, R):
        xyz = R @ self.cmf.T
        return lin_to_srgb(np.clip(xyz @ self.xyz_rgb.T, 0, 1))

    def rgb_to_latent(self, rgb):
        rgb = np.clip(np.asarray(rgb, np.float32), 0, 1)
        R = self._R(rgb)
        Y = np.maximum((R @ self.cmf.T)[..., 1:2], 1e-6)
        ks = (1 - R) ** 2 / (2 * R)
        res = rgb - self._R_to_srgb(R)
        return np.concatenate([Y * ks, Y, res], -1).astype(np.float32)

    def latent_to_rgb(self, lat):
        lat = np.asarray(lat, np.float32)
        N = self.N
        Y = np.maximum(lat[..., N:N + 1], 1e-6)
        ks = np.maximum(lat[..., :N] / Y, 0)
        R = 1 + ks - np.sqrt(ks * ks + 2 * ks)
        return np.clip(self._R_to_srgb(R) + lat[..., N + 1:N + 4], 0, 1).astype(np.float32)
