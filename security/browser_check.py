"""Open seedling.html from disk in headless Chromium and verify:
   1. it still grows, blooms and withers (no errors, no CSP violations),
   2. leaf links work, are safe, and are reachable by keyboard, and
   3. the security policy actually blocks what it claims to.

Besides the committed page, it tests variants that differ only in the
settings block (no links; hostile links; broken JSON). The settings block is
JSON, never executed, so the variants keep the same Content-Security-Policy.

Usage: python3 security/browser_check.py [screenshot_dir]"""
import asyncio, math, pathlib, re, sys, tempfile
from playwright.async_api import async_playwright

PAGE_FILE = pathlib.Path(__file__).resolve().parent.parent / "seedling.html"
SHOTS = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else None
SETTINGS_RE = re.compile(r'(<script type="application/json" id="seedling-config">)(.*?)(</script>)', re.S)

COUNT_GREEN = """() => { const c=document.getElementById('c');
            const d=c.getContext('2d').getImageData(0,0,c.width,c.height).data; let n=0;
            for (let i=0;i<d.length;i+=4) if (d[i+1]>d[i]+15 && d[i+1]>d[i+2]) n++; return n; }"""

VISIBLE_LINKS = """() => [...document.querySelectorAll('.leaf-link')].filter(a => !a.hidden)
                         .map(a => ({ href: a.href, label: a.textContent }))"""
ALL_HREFS = """() => [...document.querySelectorAll('a')].map(a => a.getAttribute('href'))"""

HOSTILE = """{ "links": [
  { "href": "javascript:alert(1)", "label": "script" },
  { "href": " JaVaScRiPt:alert(2)", "label": "script, mixed case" },
  { "href": "java\\tscript:alert(3)", "label": "script, hidden tab" },
  { "href": "data:text/html,<b>hi<\\/b>", "label": "data url" },
  { "href": "vbscript:msgbox(5)", "label": "vbscript" },
  { "href": "file:///etc/passwd", "label": "local file" },
  { "href": 42, "label": "not a string" },
  { "label": "no address" },
  { "href": "https://example.com/ok", "label": "<img src=x onerror=alert(6)>" },
  { "href": "about.html", "label": "relative" },
  { "href": "mailto:hello@example.com", "label": "email" }
], "persist": true }"""


def variant(settings: str, tmp: pathlib.Path, name: str) -> str:
    """The committed page with a different settings block, as a file:// URL."""
    html = PAGE_FILE.read_text(encoding="utf-8")
    if not SETTINGS_RE.search(html):
        raise SystemExit("seedling.html has no settings block to vary")
    out = tmp / name
    out.write_text(SETTINGS_RE.sub(lambda m: m.group(1) + settings + m.group(3), html, count=1), encoding="utf-8")
    return out.as_uri()


async def wait_for(pg, js, done, timeout_ms, step_ms=250):
    """Poll a page expression until done(value) or the timeout: wait for an
    outcome, never a fixed time (how far the plant gets in a fixed time
    depends on the machine's frame rate). Returns (last value, ms waited)."""
    waited = 0
    while True:
        v = await pg.evaluate(js)
        if done(v) or waited >= timeout_ms:
            return v, waited
        await pg.wait_for_timeout(step_ms)
        waited += step_ms


async def open_page(browser, url):
    pg = await browser.new_page(viewport={"width": 1000, "height": 700})
    errors, requests = [], []
    pg.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
    pg.on("pageerror", lambda e: errors.append(str(e)))
    pg.on("request", lambda r: requests.append(r.url) if not r.url.startswith("file:") else None)
    await pg.goto(url)
    await pg.wait_for_timeout(600)
    return pg, errors, requests


async def sweep(pg):
    """Move the pointer in a slow upward S-curve, growing the plant."""
    for k in range(150):
        t = k / 150
        await pg.mouse.move(500 + 220 * math.sin(t * 5), 520 - 430 * t)
        await pg.wait_for_timeout(25)


async def leave(pg):
    # Move the cursor genuinely outside the viewport (a parked in-window
    # cursor gets synthetic moves from Chrome and would re-wake the seed).
    await pg.mouse.move(500, -40, steps=4)
    await pg.evaluate("document.documentElement.dispatchEvent(new PointerEvent('pointerleave'))")


async def main():
    results = []
    add = lambda name, ok, detail: results.append((name, bool(ok), detail))
    async with async_playwright() as p:
        b = await p.chromium.launch()
        tmp = pathlib.Path(tempfile.mkdtemp(prefix="seedling-browser-"))

        # ---- 1. The committed page: links on, persist on ----
        pg, errors, requests = await open_page(b, PAGE_FILE.as_uri())
        await sweep(pg)
        green, waited = await wait_for(pg, COUNT_GREEN, lambda n: n > 500, 10000)
        if SHOTS: await pg.screenshot(path=str(SHOTS / "sec_bloom.png"))
        add("plant grows and renders", green > 500, f"{green} green pixels after {waited} ms")

        links, waited = await wait_for(pg, VISIBLE_LINKS, lambda v: len(v) > 0, 6000)
        safe = all(l["href"].startswith("https://github.com/lingles-ASIF/seedling") for l in links)
        add("linked leaves become clickable links", links and safe,
            f"{len(links)} visible after {waited} ms: " + ", ".join(l["label"] for l in links[:5]))

        if links:
            box = await pg.locator(".leaf-link:not([hidden])").first.bounding_box()
            await pg.mouse.move(box["x"] + box["width"] / 2, box["y"] + box["height"] / 2, steps=3)
            await pg.wait_for_timeout(400)
            label = await pg.evaluate("""() => { const a = [...document.querySelectorAll('.leaf-link')]
                .find(a => a.matches(':hover')); return a ? getComputedStyle(a.querySelector('span')).opacity : null }""")
            add("hovering a link shows its label", label is not None and float(label) > 0.5, f"label opacity {label}")

        await leave(pg)
        await pg.wait_for_timeout(2500)  # well past the ~1.2 s a full wither takes
        kept = await pg.evaluate(COUNT_GREEN)
        add("persist keeps the plant when the pointer leaves", kept > 300, f"{kept} green pixels 2.5 s after leaving")

        await pg.keyboard.press("Tab")
        focus = await pg.evaluate("""() => { const a = document.activeElement;
            const r = a && a.closest('#link-list') ? a.getBoundingClientRect() : null;
            return r ? { label: a.textContent, w: r.width, h: r.height } : null }""")
        add("keyboard reaches the links (visible list on focus)", focus and focus["w"] > 10 and focus["h"] > 5,
            f"focused: {focus}")
        add("no runtime errors or CSP violations", not errors, "; ".join(errors)[:200] or "none")

        # --- policy probes: each of these must be BLOCKED ---
        injected = await pg.evaluate("""async () => {
            window.__ran = false;
            const s = document.createElement('script'); s.textContent = 'window.__ran = true';
            document.body.appendChild(s); await new Promise(r => setTimeout(r, 50)); return window.__ran; }""")
        add("injected inline script is blocked", injected is False, f"ran={injected}")
        # (The test tool's own evaluate() is exempt from page policy, so probe
        #  code-from-strings the way page code would hit it: a string timer.)
        strtimer = await pg.evaluate("""async () => { window.__t = false;
            setTimeout("window.__t = true", 0); await new Promise(r => setTimeout(r, 60)); return window.__t; }""")
        add("code-from-strings (eval-class) is blocked", strtimer is False, f"ran={strtimer}")
        fetched = await pg.evaluate("""async () => { try { await fetch('https://example.com/'); return true }
            catch (e) { return false } }""")
        add("network requests are blocked", fetched is False and not requests, f"fetch ok={fetched}, requests={requests}")
        await pg.close()

        # ---- 2. No links: the classic behaviour (wither on leave) ----
        pg, errors, _ = await open_page(b, variant('{ "links": [] }', tmp, "no-links.html"))
        await sweep(pg)
        await wait_for(pg, COUNT_GREEN, lambda n: n > 500, 10000)
        await leave(pg)
        gone, waited = await wait_for(pg, COUNT_GREEN, lambda n: n < 50, 8000)  # target ~1.2 s
        if SHOTS: await pg.screenshot(path=str(SHOTS / "sec_gone.png"))
        add("withers back to seed on leave", gone < 50, f"{gone} green pixels after {waited} ms")
        none = await pg.evaluate("() => document.querySelectorAll('a').length")
        add("no links configured: no links on the page", none == 0 and not errors, f"{none} links; errors: {errors[:1]}")
        await pg.close()

        # ---- 3. Hostile links: only safe addresses survive ----
        pg, errors, _ = await open_page(b, variant(HOSTILE, tmp, "hostile.html"))
        hrefs = await pg.evaluate(ALL_HREFS)
        # Relative links resolve next to the page (file:// here), which is fine;
        # an absolute file: address, or any script/data scheme, is not.
        bad = [h for h in hrefs if not h or re.match(r"\s*(javascript|data|vbscript):", h, re.I) or "etc/passwd" in h]
        labels = await pg.evaluate("() => [...document.querySelectorAll('#link-list a')].map(a => a.textContent)")
        injected_img = await pg.evaluate("() => document.querySelectorAll('img').length")
        expect = 3  # https://example.com/ok, about.html, mailto:
        add("unsafe link addresses are rejected", not bad and len(labels) == expect,
            f"{len(labels)} kept of 11 ({', '.join(labels)}); unsafe in DOM: {bad}")
        add("link labels are text, never HTML", injected_img == 0 and any("<img" in l for l in labels),
            f"{injected_img} <img> elements created")
        await pg.close()

        # ---- 4. Broken settings JSON: the plant still works ----
        pg, errors, _ = await open_page(b, variant('{ "links": [ oops', tmp, "broken.html"))
        await sweep(pg)
        green, _ = await wait_for(pg, COUNT_GREEN, lambda n: n > 500, 10000)
        add("broken settings fall back to defaults", green > 500 and not errors, f"{green} green px; errors: {errors[:1]}")
        await pg.close()
        await b.close()

    ok = all(r[1] for r in results)
    for name, passed, detail in results:
        print(f"{'PASS' if passed else 'FAIL'}  {name}  ({detail})")
    sys.exit(0 if ok else 1)

asyncio.run(main())
