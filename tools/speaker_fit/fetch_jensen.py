#!/usr/bin/env python3
"""Download Jensen's published SPL/impedance chart series to jensen_fr.json.

The series are embedded as JSON in each product page on jensentone.com. They
are manufacturer measurements and are not committed to this repository.
"""
import json, urllib.request

PAGES = {
    'p12r': 'https://www.jensentone.com/vintage-alnico/p12r',
    'p12q': 'https://www.jensentone.com/vintage-alnico/p12q',
    'p10r': 'https://www.jensentone.com/vintage-alnico/p10r',
    'p12n': 'https://www.jensentone.com/vintage-alnico/p12n',
    'c12n': 'https://www.jensentone.com/vintage-ceramic/c12n',
}
KEY = '"frequency_response_charts":'
out = {}
for model, url in PAGES.items():
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
    html = urllib.request.urlopen(req).read().decode('utf-8')
    charts, _ = json.JSONDecoder().raw_decode(html[html.index(KEY) + len(KEY):])
    for chart in charts:
        for s in chart['series']:
            out[f"{model}|{s['name']}"] = (s['x'], s['y'])
json.dump(out, open('jensen_fr.json', 'w'))
print(f'{len(out)} series')
