from pathlib import Path
from playwright.sync_api import sync_playwright

output = Path(__file__).resolve().parents[1] / "docs" / "assets" / "screenshot.png"
output.parent.mkdir(parents=True, exist_ok=True)

with sync_playwright() as playwright:
    browser = playwright.chromium.launch(headless=True)
    page = browser.new_page(viewport={"width": 1440, "height": 900}, device_scale_factor=1)
    page.goto("http://127.0.0.1:1420", wait_until="networkidle")
    page.wait_for_selector(".welcome")
    page.screenshot(path=str(output), full_page=True)
    browser.close()

print(output)
