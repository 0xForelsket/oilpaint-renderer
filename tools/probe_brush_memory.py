"""Fresh-process resident memory probe; decimal MB, not just array accounting."""
import argparse
import json
import os
from pathlib import Path
import sys
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT))
os.environ["OILPAINT_KERNEL"]="rust"
import cv2
from oilpaint import light
from oilpaint.eval import _rss_mb
from evaluate_ochrell import actions,prepare,replay

p=argparse.ArgumentParser();p.add_argument("--mixer",choices=["mixbox","mixbox-material","ochrell"],required=True);p.add_argument("--width",type=int,default=600);a=p.parse_args()
cv2.setNumThreads(1)
prepared=prepare(actions()[0],a.mixer)
baseline=_rss_mb(False)
canvas,count,*_=replay(prepared,a.width)
paint_peak=_rss_mb()
lit=light.relight(canvas.rgb,canvas.h)
print(json.dumps(dict(backend=a.mixer,width=a.width,height=canvas.H,baseline_mb=baseline,
    paint_peak_mb=paint_peak,paint_light_peak_mb=_rss_mb(),current_mb=_rss_mb(False),pixel_visits=count)))
