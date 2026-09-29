"""Optional Mixbox 2 comparison, using installed pymixbox and existing renderer.

Outputs observations/timings only. No LUT, source or coefficients are exported.
The original renderer and matched-transport control are explicitly distinguished.
"""
import argparse
import csv
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT))
os.environ["OILPAINT_KERNEL"]="rust"
import numpy as np
import cv2
from oilpaint import mix,light
from oilpaint._build import build_and_load
from oilpaint.ochrell import library
from evaluate_ochrell import actions,prepare,replay,stats,error,SEED


def csv_write(path,rows):
    with path.open("w",newline="") as f:
        w=csv.DictWriter(f,fieldnames=list(rows[0]));w.writeheader();w.writerows(rows)


def native(out,colors):
    mix.set_backend("mixbox"); mb=mix.rgb_to_latent(colors)
    # This temporary file contains encoded sample states, not the encoder LUT.
    with tempfile.TemporaryDirectory(prefix="oilpaint-mixer-comparison-") as d:
        p=Path(d)/"inputs.bin"; np.concatenate([colors,mb],axis=1).astype("<f4").tofile(p)
        cmd=["cargo","run","--release","--offline","--manifest-path",str(ROOT/"native/mixer-comparison/Cargo.toml"),"--bin","bench","--",str(p)]
        with (out/"native-mixing.csv").open("w") as f: subprocess.run(cmd,stdout=f,check=True)


def adapter(out,colors,repeats):
    rows=[]
    for backend in ("mixbox","ochrell"):
        mix.set_backend(backend)
        for size in (1,64,1024,4096):
            rgb=colors[:size]; z=mix.rgb_to_latent(rgb)
            for name,fn in (("encode",lambda:mix.rgb_to_latent(rgb)),("decode",lambda:mix.latent_to_rgb(z)),
                            ("full_rgb_mix",lambda:mix.mix_rgb(rgb,np.roll(rgb,1,axis=0),.37))):
                fn(); iterations=max(5,min(500,30000//size))
                for rep in range(repeats):
                    start=time.perf_counter()
                    for _ in range(iterations): result=fn()
                    elapsed=time.perf_counter()-start
                    assert np.isfinite(result).all()
                    rows.append(dict(backend=backend,operation=name,batch=size,repeat=rep,iterations=iterations,ns_per_color=elapsed*1e9/iterations/size))
    csv_write(out/"python-adapter.csv",rows)


def brushes(out,widths,repeats):
    events,labels=actions(); rows=[];summary={};images={}
    backends=("mixbox","mixbox-material","ochrell")
    for width in widths:
        baseline=None; baseline_height=None
        summary[str(width)]={}
        for backend in backends:
            t=time.perf_counter(); prepared=prepare(events,backend); encode_s=time.perf_counter()-t
            cv,*_=replay(prepared,64);del cv
            for rep in range(repeats):
                cv,count,elapsed,prep=replay(prepared,width)
                t=time.perf_counter(); lit=light.relight(cv.rgb,cv.h); lighting=time.perf_counter()-t
                memory=sum(getattr(cv,k).nbytes for k in ("lat","rgb","h","wet","cover","hblur","region"))
                if cv.amount is not None:memory+=cv.amount.nbytes
                rows.append(dict(backend=backend,width=width,height=cv.H,repeat=rep,pixel_visits=count,paint_seconds=elapsed,
                    lighting_seconds=lighting,prep_seconds=prep,stroke_encode_seconds=encode_s,canvas_bytes=memory))
                if rep==0:
                    summary[str(width)][backend]=dict(rgb_sha256=hashlib.sha256(cv.rgb.tobytes()).hexdigest())
                    if backend=="mixbox-material":baseline=cv.rgb.copy();baseline_height=cv.h.copy()
                    if backend=="ochrell":
                        summary[str(width)][backend]["difference_from_matched_mixbox"]=stats(error(cv.rgb[cv.cover>.01],baseline[cv.cover>.01]))
                        summary[str(width)][backend]["same_height_as_matched_mixbox"]=bool(np.array_equal(cv.h,baseline_height))
                    if width==max(widths):
                        images[backend]=(cv.rgb.copy(),lit.copy()); light.save_png(str(out/f"{backend}-lit.png"),lit)
                else: assert hashlib.sha256(cv.rgb.tobytes()).hexdigest()==summary[str(width)][backend]["rgb_sha256"]
                del cv,lit
            r=[x for x in rows if x["backend"]==backend and x["width"]==width]
            summary[str(width)][backend].update(paint_seconds_median=float(np.median([x["paint_seconds"] for x in r])),
                lighting_seconds_median=float(np.median([x["lighting_seconds"] for x in r])),canvas_bytes=r[0]["canvas_bytes"])
    csv_write(out/"brush-performance.csv",rows)
    w=max(widths);h=images["ochrell"][0].shape[0]
    for idx,name in enumerate(("unlit","lit")):
        sheet=np.full((h+72,240+3*w,3),246,np.uint8)
        for j,backend in enumerate(backends):
            sheet[72:,240+j*w:240+(j+1)*w]=light.to8(images[backend][idx])
            title={"mixbox":"Mixbox / original transport","mixbox-material":"Mixbox / matched transport","ochrell":"Ochrell / matched transport"}[backend]
            cv2.putText(sheet,title,(250+j*w,34),cv2.FONT_HERSHEY_SIMPLEX,.62,(25,25,25),1,cv2.LINE_AA)
        for i,label in enumerate(labels): cv2.putText(sheet,label,(10,72+int((.065+i*.115)*w)),cv2.FONT_HERSHEY_SIMPLEX,.45,(25,25,25),1,cv2.LINE_AA)
        cv2.imwrite(str(out/f"mixbox-comparison-{name}.png"),cv2.cvtColor(sheet,cv2.COLOR_RGB2BGR))
    return summary


def main():
    p=argparse.ArgumentParser();p.add_argument("--out",type=Path,default=Path("../ochrell/results/integration/mixbox"));p.add_argument("--widths",default="320,600");p.add_argument("--repeats",type=int,default=7);a=p.parse_args()
    a.out.mkdir(parents=True,exist_ok=True);cv2.setNumThreads(1)
    version=importlib.metadata.version("pymixbox")
    if version!="2.0.0": raise RuntimeError("record/review a new comparator version before changing this frozen protocol")
    library();library(True);build_and_load("mixbox")
    colors=np.random.default_rng(SEED).random((4096,3),dtype=np.float32)
    native(a.out,colors); print("Native mixing done",flush=True)
    adapter(a.out,colors,a.repeats);print("Adapter mixing done",flush=True)
    b=brushes(a.out,[int(x) for x in a.widths.split(',')],a.repeats)
    result=dict(seed=SEED,mixbox_version=version,mixbox_license="CC BY-NC 4.0, Secret Weapons; existing optional renderer backend",
        native_profile="both kernels in one release binary: lto=false, codegen-units=1, same CPU thread, cached states; no native Mixbox encode claimed",
        adapter_scope="actual Python adapters including validation, allocation, NumPy/ctypes; not a native encoder contest",
        rendering_scope="original Mixbox has alpha-as-mixture transport; matched Mixbox pads 7 floats to 85 and uses the same amount, coverage and geometry as Ochrell",
        brush=b,rustc=subprocess.check_output(["rustc","-Vv"],text=True),python=sys.version,
        source_sha256={str(x.relative_to(ROOT)):hashlib.sha256(x.read_bytes()).hexdigest() for x in [ROOT/"oilpaint/mix.py",ROOT/"spikes/oilcore/src/kernel.rs",ROOT/"native/ochrell-brush/src/material.rs",Path(__file__)]})
    result["fresh_process_memory"] = [json.loads(subprocess.check_output([sys.executable,str(ROOT/"tools/probe_brush_memory.py"),
        "--mixer",backend,"--width",str(max(int(x) for x in a.widths.split(',')))],text=True)) for backend in ("mixbox","mixbox-material","ochrell")]
    (a.out/"summary.json").write_text(json.dumps(result,indent=2));print(f"Wrote {a.out/'summary.json'}")


if __name__=="__main__":main()
