"""Read-only browser smoke against the separate local dual-member preview."""
import hashlib,json
from pathlib import Path
from playwright.sync_api import sync_playwright
ROOT=Path(__file__).resolve().parents[1]
def main():
    out=ROOT/'artifacts/sdk-5-26';errors=[];methods=[]
    with sync_playwright() as p:
        browser=p.chromium.launch();page=browser.new_page(viewport={'width':1440,'height':1000})
        page.on('pageerror',lambda e:errors.append(str(e)))
        page.on('console',lambda m:errors.append(m.text) if m.type=='error' else None)
        page.on('request',lambda r:methods.append(r.method))
        page.goto('http://127.0.0.1:4213/',wait_until='networkidle')
        info=page.request.get('http://127.0.0.1:4213/api/config').json()
        assert info['sdk']['crateVersion']=='0.28.0' and len(info['scenarios'])==2 and page.locator('#create').is_visible()
        page.screenshot(path=str(out/'preview.png'),full_page=True);browser.close()
    assert not errors and set(methods)=={'GET'}
    report={'passed':True,'port':4213,'url':'http://127.0.0.1:4213/','createdMatch':False,'requestMethods':methods,'independentDatabase':'.local/sdk-5-26-demo.sqlite','binarySha256':hashlib.sha256((out/'candidate/bin/kernel-host').read_bytes()).hexdigest(),'scenarios':[s['id'] for s in info['scenarios']],'browserErrors':errors}
    (out/'local-preview.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report))
if __name__=='__main__':main()
