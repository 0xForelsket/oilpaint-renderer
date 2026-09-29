"""Contact sheets: tile labelled images into one PNG."""
import numpy as np
import cv2


def label(img8, text, scale=0.55):
    img = img8.copy()
    cv2.rectangle(img, (0, 0), (min(img.shape[1], 12 + int(len(text) * 11 * scale)), 20), (20, 20, 20), -1)
    cv2.putText(img, text, (5, 14), cv2.FONT_HERSHEY_SIMPLEX, scale, (240, 240, 240), 1, cv2.LINE_AA)
    return img


def sheet(items, cols=3, cell=(300, 300), pad=6, bg=40):
    """items: list of (title, rgb8 image). Images are resized to fit the cell keeping aspect."""
    cw, ch = cell
    rows = (len(items) + cols - 1) // cols
    out = np.full((rows * (ch + pad) + pad, cols * (cw + pad) + pad, 3), bg, np.uint8)
    for i, (title, img) in enumerate(items):
        if img.ndim == 2:
            img = np.repeat(img[..., None], 3, -1)
        h, w = img.shape[:2]
        s = min(cw / w, ch / h)
        nw, nh = max(1, int(w * s)), max(1, int(h * s))
        im = cv2.resize(img, (nw, nh), interpolation=cv2.INTER_AREA if s < 1 else cv2.INTER_NEAREST)
        im = label(im, title)
        r, c = divmod(i, cols)
        y0 = pad + r * (ch + pad); x0 = pad + c * (cw + pad)
        out[y0:y0 + nh, x0:x0 + nw] = im
    return out


def save_sheet(path, items, **kw):
    cv2.imwrite(path, cv2.cvtColor(sheet(items, **kw), cv2.COLOR_RGB2BGR))
