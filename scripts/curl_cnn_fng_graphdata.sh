#!/bin/bash
curl 'https://production.dataviz.cnn.io/index/fearandgreed/graphdata' \
  -H 'authority: production.dataviz.cnn.io' \
  -H 'accept: */*' \
  -H 'accept-language: en-US,en;q=0.9,el-GR;q=0.8,el;q=0.7,it;q=0.6' \
  -H 'cache-control: no-cache' \
  -H 'dnt: 1' \
  -H 'origin: https://edition.cnn.com' \
  -H 'pragma: no-cache' \
  -H 'referer: https://edition.cnn.com/' \
  -H 'sec-ch-ua: "Not_A Brand";v="8", "Chromium";v="120", "Google Chrome";v="120"' \
  -H 'sec-ch-ua-mobile: ?0' \
  -H 'sec-ch-ua-platform: "Windows"' \
  -H 'sec-fetch-dest: empty' \
  -H 'sec-fetch-mode: cors' \
  -H 'sec-fetch-site: cross-site' \
  -H 'user-agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36' \
  --compressed
 exit $?

