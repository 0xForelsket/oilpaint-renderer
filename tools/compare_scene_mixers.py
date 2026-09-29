"""Replay one existing scene's stroke geometry with identical authored RGB inputs.

The saved Ochrell stroke colors are decoded once to a shared RGB corpus. All
methods re-encode that corpus and regenerate the same authoring streak rule.
Planning is not repeated, so planner decisions cannot confound the comparison.
"""
import argparse
import csv
import hashlib
import json
import os
from pathlib import Path
import sys
import time
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT))
os.environ["OILPAINT_KERNEL"]="rust"
import numpy as np
import cv2
from oilpaint import mix,light
from oilpaint.canvas import Canvas,Stroke
from oilpaint.strokes import streak_vector
from oilpaint.render import load_strokes
from oilpaint.scene import load_scene

p=argparse.ArgumentParser();p.add_argument("run",type=Path);p.add_argument("--out",type=Path,default=Path("../ochrell/results/integration/mixbox/scene"));p.add_argument("--width",type=int,default=600);p.add_argument("--repeats",type=int,default=3);a=p.parse_args()
a.out.mkdir(parents=True,exist_ok=True);cv2.setNumThreads(1)
run=json.loads((a.run/"run.json").read_text());mix.set_backend(run["mixer"])
original,ranges=load_strokes(a.run/"strokes.npz")
colors=mix.latent_to_rgb(np.stack([s.zcol for s in original])); colors2=mix.latent_to_rgb(np.stack([s.zcol2 for s in original]))
common_hash=hashlib.sha256(colors.tobytes()+colors2.tobytes()+b"".join(s.pts.tobytes() for s in original)).hexdigest()
# Scene loading can author colors, so use the source mixer once to get schedule.
scene=load_scene(run["scene"]);layers=scene.layer_list
rows=[];summary={};pictures=[];height_ref=None
for backend in ("mixbox","mixbox-material","ochrell"):
    mix.set_backend(backend);start=time.perf_counter()
    z=mix.rgb_to_latent(colors);z2=mix.rgb_to_latent(colors2)
    ss=[Stroke(st.pts,z[i],streak_vector(z[i]),st.params,st.seed,st.layer,st.region,zcol2=z2[i]) for i,st in enumerate(original)]
    encoding=time.perf_counter()-start
    for rep in range(a.repeats):
        cv=Canvas(a.width,scene.size(a.width)[1],scene.ground);paint=0.;prep=0.;visits=0
        for li,(start,end) in enumerate(ranges):
            t=time.perf_counter();cv.set_hblur(light.blur(cv.h,max(1.,layers[li]["hblur_sigma"]*a.width)));prep+=time.perf_counter()-t
            t=time.perf_counter();visits+=cv.render(ss[start:end]);paint+=time.perf_counter()-t
            if layers[li].get("dry_after") is not None:cv.dry(layers[li]["dry_after"])
        t=time.perf_counter();lit=light.relight(cv.rgb,cv.h);lighting=time.perf_counter()-t
        sha=hashlib.sha256(cv.rgb.tobytes()).hexdigest()
        rows.append(dict(backend=backend,repeat=rep,width=a.width,height=cv.H,strokes=len(ss),pixel_visits=visits,
            stroke_encode_seconds=encoding,paint_seconds=paint,layer_blur_seconds=prep,lighting_seconds=lighting))
        print(f"{backend} repeat {rep+1}: paint {paint:.3f}s, light {lighting:.3f}s",flush=True)
        if rep==0:
            summary[backend]=dict(rgb_sha256=sha)
            light.save_png(str(a.out/f"{backend}.png"),lit);pictures.append(light.to8(lit))
            if backend=="mixbox-material":height_ref=cv.h.copy()
            if backend=="ochrell":
                summary[backend]["same_height_as_matched_mixbox"]=bool(np.array_equal(cv.h,height_ref))
                assert summary[backend]["same_height_as_matched_mixbox"]
        else:assert summary[backend]["rgb_sha256"]==sha
        del cv,lit
    measured=[r for r in rows if r["backend"]==backend]
    summary[backend]["paint_seconds_median"]=float(np.median([r["paint_seconds"] for r in measured]))
    summary[backend]["lighting_seconds_median"]=float(np.median([r["lighting_seconds"] for r in measured]))
with (a.out/"timings.csv").open("w",newline="") as f:
    w=csv.DictWriter(f,fieldnames=list(rows[0]));w.writeheader();w.writerows(rows)
sheet=np.concatenate(pictures,axis=1);sheet=np.pad(sheet,((54,0),(0,0),(0,0)),constant_values=245)
for j,title in enumerate(("Mixbox / original transport","Mixbox / matched transport","Ochrell / matched transport")):
    cv2.putText(sheet,title,(j*a.width+12,34),cv2.FONT_HERSHEY_SIMPLEX,.62,(25,25,25),1,cv2.LINE_AA)
cv2.imwrite(str(a.out/"scene-comparison.png"),cv2.cvtColor(sheet,cv2.COLOR_RGB2BGR))
(a.out/"summary.json").write_text(json.dumps(dict(source_run=str(a.run),source_strokes_sha256=hashlib.sha256((a.run/"strokes.npz").read_bytes()).hexdigest(),
    shared_rgb_geometry_sha256=common_hash,repeats=a.repeats,width=a.width,strokes=len(original),backends=summary,
    scope="same source-derived RGB loads, regenerated authoring streaks, same paths/pressure/params/seeds/layers; no replanning; scalar CPU native kernels",seed=run["seed"]),indent=2))
