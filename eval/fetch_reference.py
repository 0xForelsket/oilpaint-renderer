"""Download the public-domain reference paintings listed in eval/reference_provenance.json (Wikimedia Commons) into a directory.

    python eval/fetch_reference.py DIR          # then: python -m oilpaint eval reference DIR

Polite by design: identifies itself, honours Retry-After on HTTP 429 (Commons rate-limits shared addresses hard), gives up on any other
error.  It does not try proxies or mirrors; if the network refuses, put your own images (scans or museum open-access downloads) in DIR
together with a PROVENANCE.json (a list of {file, title, source_page, licence, download_url}) and run `eval reference` on it.
"""
import json
import os
import sys
import time
import urllib.error
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
UA = {"User-Agent": "oilpaint-eval-harness/1.0 (reference calibration for a personal oil-painting renderer)"}


def fetch(url, tries=3):
    for _ in range(tries):
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=60) as r:
                return r.read()
        except urllib.error.HTTPError as e:
            if e.code == 429:
                wait = int(e.headers.get("Retry-After", "30")) + 5
                print(f"  429 rate limited, waiting {wait}s", flush=True)
                time.sleep(wait)
                continue
            print(f"  HTTP {e.code}: not retrying", flush=True)
            return None
        except (urllib.error.URLError, OSError) as e:
            print(f"  {e}: not retrying", flush=True)
            return None
    return None


def main(dest):
    prov = json.load(open(os.path.join(HERE, "reference_provenance.json")))
    os.makedirs(dest, exist_ok=True)
    got = []
    for p in prov:
        path = os.path.join(dest, p["file"])
        if os.path.exists(path):
            got.append(p)
            continue
        print("fetching", p["title"], flush=True)
        b = fetch(p["download_url"])
        if b:
            with open(path, "wb") as f:
                f.write(b)
            got.append(p)
            time.sleep(8)
    with open(os.path.join(dest, "PROVENANCE.json"), "w") as f:
        json.dump(got, f, indent=1)
    print(f"{len(got)} of {len(prov)} images in {dest}")
    return 0 if got else 1


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1]))
