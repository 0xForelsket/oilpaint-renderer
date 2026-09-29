"""side-by-side native crops: sbs.py out.png x0 y0 w h zoom img1 img2 ... (x0,y0 in px)"""
import sys, cv2, numpy as np
out = sys.argv[1]; x0, y0, w, h = (int(v) for v in sys.argv[2:6]); z = float(sys.argv[6])
ims = [cv2.imread(p) for p in sys.argv[7:]]
tiles = []
for im in ims:
    t = im[y0:y0 + h, x0:x0 + w]
    t = cv2.copyMakeBorder(t, 0, h - t.shape[0], 0, w - t.shape[1], cv2.BORDER_CONSTANT, value=(255, 255, 255))
    tiles.append(t); tiles.append(np.full((h, 6, 3), 255, np.uint8))
o = np.concatenate(tiles[:-1], 1)
if z != 1: o = cv2.resize(o, None, fx=z, fy=z, interpolation=cv2.INTER_NEAREST)
cv2.imwrite(out, o)
