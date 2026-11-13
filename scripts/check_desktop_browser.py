"""Built offline editor/preview acceptance under the real desktop CSP."""
import functools
import http.server
import json
import re
from pathlib import Path
import threading
from urllib.parse import urlparse
from playwright.sync_api import sync_playwright, expect

ROOT = Path(__file__).resolve().parents[1]
CSP = json.loads((ROOT / 'apps/desktop/src-tauri/tauri.conf.json').read_text(encoding='utf-8'))['app']['security']['csp']
FIXTURE = r"""
window.__qa={calls:[],unknown:[],csp:[]};
document.addEventListener('securitypolicyviolation',event=>window.__qa.csp.push({directive:event.violatedDirective,uri:event.blockedURI}));
const diagnostic={id:'fixture',engine:'synthetic',language:'typescript',rule_id:'synthetic-rule',severity:'warning',category:'style',confidence:'fixture',file:'src/fixture.ts',range:{start_byte:0,end_byte:3,start_line:1,start_column:1,end_line:1,end_column:4},message:'Synthetic fixture diagnostic',explanation:'Self-authored browser fixture; not a scan of any user repository.',source:'var value = 1;\n',fixes:[{id:'fixture-fix',title:'Synthetic replacement',edits:[],safe:true}],tags:[]};
const workspace={summary:{root:'C:/fixture',name:'Synthetic workspace',languages:{typescript:{files:1,bytes:15}},project_markers:[],git_repository:false},engines:[]};
window.__TAURI_INTERNALS__={invoke:async(command,args)=>{
 window.__qa.calls.push({command,args});
 switch(command){
  case 'plugin:dialog|open':return 'C:/fixture';
  case 'open_workspace':return workspace;
  case 'get_workspace':return workspace;
  case 'run_review':return {diagnostics:[diagnostic],files_analyzed:1,languages:['typescript'],engines_used:['synthetic'],duration_ms:1};
  case 'preview_fix':return {preview_id:'preview',diagnostic,patch:{id:'patch',transformation_ids:[],files:[],additions:1,deletions:1},files:[{path:'src/fixture.ts',before:'var value = 1;\n',after:'let value = 1;\n'}]};
  default:window.__qa.unknown.push(command);throw Error('Unexpected synthetic IPC '+command);
 }
}};
"""


def contrast_on_white(color):
    channels = [float(value) / 255 for value in re.findall(r'[\d.]+', color)[:3]]
    linear = [value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4 for value in channels]
    luminance = sum(channel * weight for channel, weight in zip(linear, [0.2126, 0.7152, 0.0722]))
    return 1.05 / (luminance + 0.05)


class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def end_headers(self):
        self.send_header('Content-Security-Policy', CSP)
        super().end_headers()


def main():
    output = ROOT / 'work/dependency-browser'
    output.mkdir(parents=True, exist_ok=True)
    server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), functools.partial(Handler, directory=str(ROOT / 'apps/desktop/dist')))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        with sync_playwright() as p:
            browser = p.chromium.launch(headless=True)
            context = browser.new_context(viewport={'width': 1600, 'height': 1000})
            context.add_init_script(FIXTURE)
            blocked = []
            def route_request(route):
                if urlparse(route.request.url).hostname in ('127.0.0.1', 'localhost', None):
                    route.continue_()
                else:
                    blocked.append(route.request.url)
                    route.abort()
            context.route('**/*', route_request)
            page = context.new_page()
            errors = []
            page.on('pageerror', lambda error: errors.append(str(error)))
            page.goto(f'http://127.0.0.1:{server.server_port}', wait_until='networkidle')
            page.get_by_role('button', name='Open Repository', exact=True).first.click()
            expect(page.get_by_role('heading', name='Synthetic workspace', exact=True)).to_be_visible()
            page.get_by_role('button', name='Run review', exact=True).click()
            expect(page.locator('.diagnostic-detail')).to_contain_text('Synthetic fixture diagnostic')
            expect(page.locator('.monaco-editor').first).to_be_visible(timeout=30000)
            page.get_by_role('button', name='Preview fix', exact=True).click()
            expect(page.locator('.monaco-diff-editor')).to_be_visible(timeout=30000)
            expect(page.get_by_role('button', name='Apply fix', exact=True)).to_be_visible()
            page.screenshot(path=str(output / 'comparison.png'), full_page=True)
            page.locator('.nav-item').filter(has_text='Settings').click()
            page.get_by_role('button', name='light', exact=True).click()
            expect(page.locator('html')).to_have_attribute('data-theme', 'light')
            for locator in [page.get_by_role('heading', name='Local-first defaults', exact=True), page.locator('.setting-row strong').first]:
                assert contrast_on_white(locator.evaluate('(element)=>getComputedStyle(element).color')) >= 4.5
            page.screenshot(path=str(output / 'settings.png'), full_page=True)
            page.locator('.nav-item').filter(has_text='Review').click()
            expect(page.locator('.monaco-diff-editor')).to_be_visible(timeout=30000)
            qa = page.evaluate('window.__qa')
            assert not qa['unknown'], qa['unknown']
            assert not qa['csp'], qa['csp']
            assert not any(c['command'] in ['apply_fix', 'undo_fix', 'run_verification'] for c in qa['calls'])
            assert not blocked, blocked
            assert not errors, errors
            evidence = dict(ok=True, checks=['actual built React rendering', 'synthetic open/review/preview IPC', 'local TypeScript source editor and side-by-side diff', 'unchanged desktop CSP', 'no attempted CDN or remote request', 'theme switch', 'zero page errors'], errors=errors, blocked=blocked, csp_violations=qa['csp'], limits=['Synthetic IPC; no actual native source analysis, patch application, verification process or user file access.'])
            (output / 'report.json').write_text(json.dumps(evidence, indent=2) + '\n', encoding='utf-8')
            browser.close()
    finally:
        server.shutdown()
        server.server_close()


if __name__ == '__main__':
    main()
