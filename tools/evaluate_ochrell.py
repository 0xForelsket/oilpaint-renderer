"""Reproducible numerical/storage study and matched brush replay, no Mixbox.

Run from renderer root: python tools/evaluate_ochrell.py --out ../ochrell/results/integration
Uses the existing bristle kernel, Canvas, Stroke and relighting, not a second renderer.
"""
import argparse
import csv
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import numpy as np
import cv2
from skimage.color import rgb2lab, deltaE_ciede2000
from oilpaint import mix, light, strokes
from oilpaint.canvas import Canvas
from oilpaint.ochrell import library, encode, decode, FP

SEED = 20260930
MODES = ("ochrell", "ochrell-roundtrip", "ochrell-srgb")


def reference(x, decoding=False):
    lib = library()
    dp = ctypes.POINTER(ctypes.c_double)
    if decoding:
        x = np.ascontiguousarray(x, np.float64)
        out = np.empty(x.shape[:-1] + (3,), np.float32)
        fn = lib.ochrell_reference_decode; fn.argtypes = [dp, FP, ctypes.c_size_t]
        status = fn(x.ctypes.data_as(dp), out.ctypes.data_as(FP), x.size // 165)
    else:
        x = np.ascontiguousarray(x, np.float32)
        out = np.empty(x.shape[:-1] + (165,), np.float64)
        fn = lib.ochrell_reference_encode; fn.argtypes = [FP, dp, ctypes.c_size_t]
        status = fn(x.ctypes.data_as(FP), out.ctypes.data_as(dp), x.size // 3)
    if status: raise ValueError(f"reference adapter failed: {status}")
    return out


def error(a,b):
    return deltaE_ciede2000(rgb2lab(np.asarray(a,np.float64)), rgb2lab(np.asarray(b,np.float64)))


def stats(x):
    return dict(mean=float(np.mean(x)), p95=float(np.percentile(x,95)), max=float(np.max(x)))


def quantize(z, mode):
    if mode == "f16_all": return z.astype(np.float16).astype(np.float32)
    if mode == "f16_optics":
        z = z.copy(); z[...,:82] = z[...,:82].astype(np.float16).astype(np.float32); return z
    if mode == "rgb_roundtrip": return encode(decode(z,"ochrell"),"ochrell")
    return z.copy()


def quality(out, n):
    rng = np.random.default_rng(SEED)
    colors = rng.random((4,n,3), dtype=np.float32)
    colors[:, :8] = np.array([[0,0,0],[1,1,1],[1,0,0],[0,1,0],[0,0,1],[1,1,0],[0,1,1],[1,0,1]],np.float32)
    z = encode(colors,"ochrell"); rz = reference(colors)
    white = encode(np.ones((n,3),np.float32),"ochrell"); rw = reference(np.ones((n,3),np.float32))
    black = encode(np.zeros((n,3),np.float32),"ochrell"); rb = reference(np.zeros((n,3),np.float32))
    t = rng.uniform(.05,.95,(n,1)).astype(np.float32)
    np.savez_compressed(out/"quality-inputs.npz", colors=colors,t=t,seed=SEED)
    summary, rows = {}, []
    for method in ("f32", "f16_all", "f16_optics", "rgb_roundtrip"):
        zs = quantize(z,method)
        cases = {}
        cases["reconstruction"] = (decode(zs[0],"ochrell"), reference(rz[0],True))
        pair = quantize((1-t)*zs[0]+t*zs[1],method)
        rpair = (1-t)*rz[0]+t*rz[1]
        cases["mixture"] = (decode(pair,"ochrell"), reference(rpair,True))
        cases["white_tint"] = (decode(quantize(.3*pair+.7*white,method),"ochrell"),reference(.3*rpair+.7*rw,True))
        cases["black_addition"] = (decode(quantize(.9*pair+.1*black,method),"ochrell"),reference(.9*rpair+.1*rb,True))
        group = quantize((2*zs[0]+3*zs[1])/5,method)
        grouped = quantize((5*group+5*zs[2]+7*zs[3])/17,method)
        direct = quantize((2*zs[0]+3*zs[1]+5*zs[2]+7*zs[3])/17,method)
        cases["grouping_self_error"] = (decode(grouped,"ochrell"),decode(direct,"ochrell"))
        cases["four_colors"] = (decode(grouped,"ochrell"),reference((2*rz[0]+3*rz[1]+5*rz[2]+7*rz[3])/17,True))
        accum, raccum = zs[0].copy(), rz[0].copy()
        for step in range(256):
            a = .01 + .09 * float(rng_for_step(step))
            accum = quantize((1-a)*accum+a*zs[(step+1)%4],method)
            raccum = (1-a)*raccum+a*rz[(step+1)%4]
            if step in (15,63,255):
                cases[f"repeated_{step+1}"] = (decode(accum,"ochrell"),reference(raccum,True))
        cases["repeated_then_white"] = (decode(quantize(.5*accum+.5*white,method),"ochrell"),reference(.5*raccum+.5*rw,True))
        # Half precision can absorb small updates even when one-step color error
        # is small. Keep this unfavorable stress case separate from normal dabs.
        tiny, rtiny = zs[0,:128].copy(), rz[0,:128].copy()
        for _ in range(4096):
            tiny = quantize(.9999*tiny+.0001*zs[1,:128],method)
            rtiny = .9999*rtiny+.0001*rz[1,:128]
        cases["tiny_pickup_4096"] = (decode(tiny,"ochrell"),reference(rtiny,True))
        summary[method] = {}
        for name,(a,b) in cases.items():
            de = error(a,b); summary[method][name] = stats(de)
            for i,d in enumerate(de): rows.append([method,name,i,float(d),*map(float,a[i]),*map(float,b[i])])
    with (out/"quality.csv").open("w",newline="") as f:
        w=csv.writer(f); w.writerow(["method","case","sample","deltaE2000","r","g","b","ref_r","ref_g","ref_b"]); w.writerows(rows)
    # Multi-color gradual amount accumulation must agree with one weighted mix.
    weights = np.array([2,3,5,7],np.float32)
    acc=z[0].copy(); mass=float(weights[0])
    for i in range(1,4):
        total=mass+float(weights[i]); acc=(mass/total)*acc+(weights[i]/total)*z[i]; mass=total
    direct=np.sum(z*weights[:,None,None],axis=0)/weights.sum()
    summary["amount_grouping"] = stats(error(decode(acc,"ochrell"),decode(direct,"ochrell")))
    return summary


def rng_for_step(i):
    return ((SEED + i*1664525 + 1013904223)&0xffffffff)/2**32


def actions():
    events=[]
    labels=["Yellow + blue (wet)","Blue + white", "Red + black", "Four colors / 12 passes",
            "Pickup across three colors", "Smudge into bare ground", "Scumble over ridges",
            "Two-color load / dry tail", "Dry underpaint + blue", "Glaze (RGB composite)"]
    def stroke(row,color,x0=.1,x1=.9,dy=0,width=.055,**kw):
        events.append(dict(kind="stroke", row=row, color=color, x0=x0,x1=x1,y=.055+row*.115+dy,
                           width=width, seed=SEED+len(events), params=dict(deplete=0.,streak=0.,opacity=.95,**kw)))
    stroke(0,"#ffdc00",x1=.68); stroke(0,"#1446ff",x0=.34,dy=.012,pickup=.8,release=.02)
    stroke(1,"#1446ff",x1=.7); stroke(1,"#ffffff",x0=.32,dy=.012,pickup=.6,release=.02)
    stroke(2,"#e61e28",x1=.7); stroke(2,"#000000",x0=.32,dy=.012,load=.15,pickup=.6,release=.02)
    for i in range(12): stroke(3,["#e61e28","#1446ff","#ffdc00","#ffffff"][i%4],dy=(i%3-1)*.004,pickup=.7,release=.02,load=.3)
    for r in (4,5):
        for c,x in zip(["#ffdc00","#1446ff","#e61e28"],[.1,.3,.5]): stroke(r,c,x0=x,x1=x+.2)
    stroke(4,"#ffffff",dy=.01,pickup=1.,release=0.,load=.25)
    stroke(5,"#ffffff",x0=.15,x1=.94,dy=.01,mode="smudge",pickup=1.,release=0.)
    stroke(6,"#1446ff",ridge=1.2); events.append(dict(kind="blur")); stroke(6,"#ffffff",dy=.005,mode="scumble",dry_width=.15)
    stroke(7,"#e61e28",color2="#ffdc00",marble=1.)
    events[-1]["params"].update(deplete=.07,streak=.5)
    stroke(8,"#ffdc00",x1=.68); events.append(dict(kind="dry",factor=0.)); stroke(8,"#1446ff",x0=.34,dy=.012,pickup=.8,release=.02)
    stroke(9,"#e61e28"); stroke(9,"#1446ff",dy=.007,mode="glaze")
    events[-1]["params"]["opacity"] = .28
    return events,labels


def prepare(events, backend):
    mix.set_backend(backend); out=[]
    for e in events:
        if e["kind"] != "stroke": out.append(e); continue
        params = e["params"].copy()
        color2 = params.pop("color2",None)
        st=strokes.make_stroke(strokes.straight(e["x0"],e["y"],e["x1"],e["y"]+.006,e["width"]),e["color"],e["seed"],color2=color2,**params)
        out.append(st)
    return out


def replay(prepared,width):
    cv=Canvas(width,round(1.15*width),(.92,.90,.86)); count=0; seconds=0.; prep=0.
    # Separate blur preparation, canvas allocation, and relighting from paint calls.
    for e in prepared:
        if isinstance(e,dict):
            t=time.perf_counter()
            if e["kind"] == "dry": cv.dry(e["factor"])
            else: cv.set_hblur(light.blur(cv.h,max(1.,width*.01)))
            prep += time.perf_counter()-t
        else:
            t=time.perf_counter(); count+=cv.render_stroke(e); seconds+=time.perf_counter()-t
    return cv,count,seconds,prep


def comparisons(out,widths,repeats):
    events,labels=actions()
    (out/"brush-actions.json").write_text(json.dumps(dict(seed=SEED,labels=labels,events=events),indent=2))
    rows=[]; summary={}; images={}; reference_planes={}
    for width in widths:
        summary[str(width)]={}
        for backend in MODES:
            t=time.perf_counter(); prepared=prepare(events,backend); prepare_s=time.perf_counter()-t
            # Warmup is a small canvas, not included in recorded repetitions.
            warm,*_=replay(prepared,64); del warm
            for rep in range(repeats):
                cv,count,elapsed,prep=replay(prepared,width)
                t=time.perf_counter(); lit=light.relight(cv.rgb,cv.h); lighting=time.perf_counter()-t
                bytes_=sum(getattr(cv,k).nbytes for k in ("lat","rgb","h","wet","cover","hblur","region","amount"))
                rows.append(dict(width=width,height=cv.H,backend=backend,repeat=rep,strokes=sum(not isinstance(x,dict) for x in prepared),pixel_visits=count,
                                 paint_seconds=elapsed,layer_prep_seconds=prep,lighting_seconds=lighting,canvas_bytes=bytes_,encode_strokes_seconds=prepare_s))
                if rep == 0:
                    sha=hashlib.sha256(cv.rgb.tobytes()).hexdigest()
                    summary[str(width)][backend]=dict(rgb_sha256=sha)
                    if backend == MODES[0]: reference_planes[width]=(cv.rgb.copy(),cv.h.copy(),cv.cover.copy(),cv.amount.copy())
                    else:
                        base=reference_planes[width]
                        assert all(np.array_equal(a,b) for a,b in zip(base[1:],(cv.h,cv.cover,cv.amount)))
                        mask=cv.cover>.01
                        summary[str(width)][backend]["vs_persistent_deltaE2000"]=stats(error(cv.rgb[mask],base[0][mask]))
                    if width == max(widths):
                        images[backend]=(cv.rgb.copy(),lit.copy())
                        light.save_png(str(out/f"{backend}-unlit.png"),cv.rgb)
                        light.save_png(str(out/f"{backend}-lit.png"),lit)
                else:
                    assert hashlib.sha256(cv.rgb.tobytes()).hexdigest()==summary[str(width)][backend]["rgb_sha256"]
                del cv,lit
        for backend in MODES:
            r=[x for x in rows if x["width"]==width and x["backend"]==backend]
            summary[str(width)][backend].update(paint_seconds_median=float(np.median([x["paint_seconds"] for x in r])),
                lighting_seconds_median=float(np.median([x["lighting_seconds"] for x in r])),canvas_bytes=r[0]["canvas_bytes"],pixel_visits=r[0]["pixel_visits"])
    with (out/"brush-benchmark.csv").open("w",newline="") as f:
        w=csv.DictWriter(f,fieldnames=list(rows[0])); w.writeheader(); w.writerows(rows)
    w=max(widths); h=images[MODES[0]][0].shape[0]; left=240; top=72
    for index,name in enumerate(("unlit","lit")):
        sheet=np.full((h+top,left+3*w,3),246,np.uint8)
        for j,backend in enumerate(MODES):
            sheet[top:,left+j*w:left+(j+1)*w]=light.to8(images[backend][index])
            cv2.putText(sheet,backend,(left+j*w+10,34),cv2.FONT_HERSHEY_SIMPLEX,.7,(25,25,25),1,cv2.LINE_AA)
        for i,label in enumerate(labels):
            cv2.putText(sheet,label,(10,top+int((.065+i*.115)*w)),cv2.FONT_HERSHEY_SIMPLEX,.45,(25,25,25),1,cv2.LINE_AA)
        cv2.imwrite(str(out/f"brush-comparison-{name}.png"),cv2.cvtColor(sheet,cv2.COLOR_RGB2BGR))
    return summary


def main():
    p=argparse.ArgumentParser(); p.add_argument("--out",type=Path,default=Path("../ochrell/results/integration")); p.add_argument("--samples",type=int,default=2048)
    p.add_argument("--widths",default="320,600"); p.add_argument("--repeats",type=int,default=5); a=p.parse_args()
    a.out.mkdir(parents=True,exist_ok=True)
    env=dict(seed=SEED,samples=a.samples,repeats=a.repeats,widths=a.widths,python=sys.version,platform=platform.platform(),cpu=platform.processor(),
             rustc=subprocess.check_output(["rustc","-Vv"],text=True),numpy=np.__version__,opencv=cv2.__version__,
             profile="release, native scalar source, no explicit SIMD, single paint thread, lto=false, codegen-units=1; OpenCV lighting threads=1",
             renderer_base=subprocess.check_output(["git","-C",str(ROOT),"rev-parse","HEAD"],text=True).strip())
    env["source_sha256"]={str(p.relative_to(ROOT.parent)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [
        ROOT/"spikes/oilcore/src/kernel.rs",ROOT/"native/ochrell-brush/src/material.rs",ROOT/"native/ochrell-brush/src/lib.rs",
        ROOT.parent/"ochrell/src/optical.rs",ROOT.parent/"ochrell/src/optical_generated.rs",Path(__file__)]}
    cv2.setNumThreads(1)
    print("Numerical/storage study",flush=True); q=quality(a.out,a.samples)
    print("Matched brush replay",flush=True); b=comparisons(a.out,[int(w) for w in a.widths.split(",")],a.repeats)
    result=dict(environment=env,quality=q,brush=b,memory=dict(latent_bytes=340,canvas_bytes_per_pixel=373,
        canvases={f"{w}x{h}":dict(latent_bytes=w*h*340,canvas_bytes=w*h*373) for w,h in [(600,750),(1920,1080),(2400,3000),(3840,2160)]}))
    (a.out/"integration-summary.json").write_text(json.dumps(result,indent=2))
    print(f"Wrote {a.out / 'integration-summary.json'}")


if __name__ == "__main__": main()
