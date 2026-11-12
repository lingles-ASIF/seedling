"""Open seedling.html from disk in headless Chromium and verify:
   1. it still grows, blooms and withers (no errors, no CSP violations), and
   2. the security policy actually blocks what it claims to.
Usage: python3 security/browser_check.py [screenshot_dir]"""
import asyncio, math, pathlib, sys
from playwright.async_api import async_playwright

PAGE = (pathlib.Path(__file__).resolve().parent.parent / "seedling.html").as_uri()
SHOTS = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else None

async def main():
    results = []
    async with async_playwright() as p:
        b = await p.chromium.launch()
        pg = await b.new_page(viewport={"width": 1000, "height": 700})
        errors, requests = [], []
        pg.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
        pg.on("pageerror", lambda e: errors.append(str(e)))
        pg.on("request", lambda r: requests.append(r.url) if not r.url.startswith("file:") else None)
        await pg.goto(PAGE)
        await pg.wait_for_timeout(600)

        for k in range(150):
            t = k / 150
            await pg.mouse.move(500 + 220 * math.sin(t * 5), 520 - 430 * t)
            await pg.wait_for_timeout(25)
        await pg.wait_for_timeout(2000)
        if SHOTS: await pg.screenshot(path=str(SHOTS / "sec_bloom.png"))
        greenish = await pg.evaluate("""() => { const c=document.getElementById('c');
            const d=c.getContext('2d').getImageData(0,0,c.width,c.height).data; let n=0;
            for (let i=0;i<d.length;i+=4) if (d[i+1]>d[i]+15 && d[i+1]>d[i+2]) n++; return n; }""")
        results.append(("plant grows and renders", greenish > 500, f"{greenish} green pixels"))

        # Move the cursor genuinely outside the viewport (a parked in-window
        # cursor gets synthetic moves from Chrome and would re-wake the seed).
        await pg.mouse.move(500, -40, steps=4)
        await pg.evaluate("document.documentElement.dispatchEvent(new PointerEvent('pointerleave'))")
        await pg.wait_for_timeout(2500)  # headless frame rates vary; target is ~1.2 s
        if SHOTS: await pg.screenshot(path=str(SHOTS / "sec_gone.png"))
        greenish2 = await pg.evaluate("""() => { const c=document.getElementById('c');
            const d=c.getContext('2d').getImageData(0,0,c.width,c.height).data; let n=0;
            for (let i=0;i<d.length;i+=4) if (d[i+1]>d[i]+15 && d[i+1]>d[i+2]) n++; return n; }""")
        results.append(("withers back to seed on leave", greenish2 < 50, f"{greenish2} green pixels"))
        results.append(("no runtime errors or CSP violations", not errors, "; ".join(errors)[:200] or "none"))

        # --- policy probes: each of these must be BLOCKED ---
        injected = await pg.evaluate("""async () => {
            window.__ran = false;
            const s = document.createElement('script'); s.textContent = 'window.__ran = true';
            document.body.appendChild(s); await new Promise(r => setTimeout(r, 50)); return window.__ran; }""")
        results.append(("injected inline script is blocked", injected is False, f"ran={injected}"))
        # (The test tool's own evaluate() is exempt from page policy, so probe
        #  code-from-strings the way page code would hit it: a string timer.)
        strtimer = await pg.evaluate("""async () => { window.__t = false;
            setTimeout("window.__t = true", 0); await new Promise(r => setTimeout(r, 60)); return window.__t; }""")
        results.append(("code-from-strings (eval-class) is blocked", strtimer is False, f"ran={strtimer}"))
        fetched = await pg.evaluate("""async () => { try { await fetch('https://example.com/'); return true }
            catch (e) { return false } }""")
        results.append(("network requests are blocked", fetched is False and not requests, f"fetch ok={fetched}, requests={requests}"))
        await b.close()

    ok = all(r[1] for r in results)
    for name, passed, detail in results:
        print(f"{'PASS' if passed else 'FAIL'}  {name}  ({detail})")
    sys.exit(0 if ok else 1)

asyncio.run(main())
