// Headless, isolated browser only. No desktop input or existing browser profile.
const { chromium } = require('playwright');
const fs = require('node:fs');
const path = require('node:path');
const http = require('node:http');
const assert = require('node:assert/strict');
const root = path.resolve(process.argv[2]);
const output = path.resolve(process.argv[3] || 'target/marketing-review');
fs.mkdirSync(output,{recursive:true});
const types={'.html':'text/html','.js':'text/javascript','.css':'text/css','.json':'application/json','.png':'image/png','.ttf':'font/ttf'};
const server = http.createServer((req,res)=>{
  const pathname=decodeURIComponent(new URL(req.url,'http://localhost').pathname);
  const file=path.resolve(root,'.'+(pathname==='/'?'/index.html':pathname));
  if(!file.startsWith(root+path.sep)){res.writeHead(403);res.end();return;}
  try {res.writeHead(200,{'Content-Type':types[path.extname(file)]||'text/plain'});res.end(fs.readFileSync(file));}
  catch {res.writeHead(404);res.end('Not found');}
});
(async()=>{
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  const browser=await chromium.launch({headless:true});
  const origin=`http://127.0.0.1:${server.address().port}`;
  const errors=[]; const failures=[];
  try{
    const page=await browser.newPage();
    page.on('pageerror',error=>errors.push(error.message));
    page.on('response',response=>{if(response.status()>=400)failures.push(`${response.status()} ${response.url()}`);});
    for(const width of [390,768,1280]){
      await page.setViewportSize({width,height:900});
      await page.goto(origin);
      await page.evaluate(async()=>{
        document.querySelectorAll('img').forEach(image=>image.loading='eager');
        await document.fonts.ready;
        await Promise.all([...document.images].filter(image=>image.hasAttribute('src')).map(image=>image.decode()));
      });
      for(const mode of ['dark','light']){
        if (await page.locator('html').getAttribute('data-mode') !== mode) await page.locator('#mode').click();
        assert(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1),`Overflow ${width}/${mode}`);
        await page.screenshot({path:path.join(output,`page-${width}-${mode}.png`),fullPage:true});
        await page.screenshot({path:path.join(output,`hero-${width}-${mode}.png`)});
      }
    }
    assert.equal(await page.locator('.theme-card').count(),6);
    assert.equal(await page.locator('link[rel=canonical]').getAttribute('href'),'https://tront.xyz/powershellmanager/');
    assert.equal(await page.locator('meta[property="og:image"]').getAttribute('content'),'https://tront.xyz/powershellmanager/media/og.png');
    assert(!await page.locator('body').innerText().then(t=>/[\u2013\u2014]/.test(t)),'Unexpected dash punctuation');
    for(const slug of ['spectrum','carbon','daylight','electric']){
      await page.locator(`[data-preview=${slug}]`).click();
      await page.waitForFunction(slug=>document.querySelector('#hero-image').src.endsWith(`${slug}-workspace.png`),slug);
      assert.equal(await page.locator(`[data-preview=${slug}]`).getAttribute('aria-pressed'),'true');
    }
    await page.locator('.hero-shot').click();
    assert(await page.locator('#lightbox').evaluate(d=>d.open));
    await page.keyboard.press('Escape');
    assert(!await page.locator('#lightbox').evaluate(d=>d.open));
    await page.locator('.theme-card [data-zoom]').first().click();
    await page.locator('#close-lightbox').click();
    assert(!await page.locator('#lightbox').evaluate(d=>d.open));
    for(const href of await page.locator('a[download]').evaluateAll(links=>links.map(a=>a.href))){
      const response=await page.request.get(href);assert.equal(response.status(),200);
      const theme=await response.json();assert.equal(theme.format,'trontop-theme');assert.equal(theme.version,4);
    }
    await page.locator('.faq summary').first().click();
    assert(await page.locator('.faq details').first().evaluate(d=>d.open));
    await page.locator('#mode').click();
    const mode=await page.locator('html').getAttribute('data-mode');await page.reload();
    assert.equal(await page.locator('html').getAttribute('data-mode'),mode);
    await page.emulateMedia({reducedMotion:'reduce'});
    assert.equal(await page.evaluate(()=>getComputedStyle(document.documentElement).scrollBehavior),'auto');
    const withoutJs=await browser.newPage({javaScriptEnabled:false,viewport:{width:390,height:844}});
    await withoutJs.goto(origin);assert.equal(await withoutJs.locator('.theme-card').count(),6);
    assert(await withoutJs.locator('.hero-shot').getAttribute('href'));
    assert.deepEqual(errors,[]);assert.deepEqual(failures,[]);
    const receipt={result:'PASS',screenshots:6,viewports:[390,768,1280],themes:6,checks:['no overflow','all images load','metadata','theme switching','lightbox and Escape','theme downloads','FAQ','mode persistence','reduced motion','no-JS fallback','no page errors or HTTP errors']};
    fs.writeFileSync(path.join(output,'checks.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt));
  }finally{await browser.close();server.close();}
})().catch(error=>{console.error(error);server.close();process.exitCode=1;});
