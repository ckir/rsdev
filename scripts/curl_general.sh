#!/bin/bash
curl --fail --silent --show-error $1 \
  -H 'authority: api.nasdaq.com' \
  -H 'accept: application/json, text/plain, */*' \
  -H 'accept-language: en-US,en;q=0.9,el-GR;q=0.8,el;q=0.7,it;q=0.6' \
  -H 'cache-control: no-cache' \
  -H 'dnt: 1' \
  -H 'origin: https://www.nasdaq.com' \
  -H 'pragma: no-cache' \
  -H 'referer: https://www.nasdaq.com/' \
  -H 'sec-ch-ua: "Google Chrome";v="119", "Chromium";v="119", "Not?A_Brand";v="24"' \
  -H 'sec-ch-ua-mobile: ?0' \
  -H 'sec-ch-ua-platform: "Windows"' \
  -H 'sec-fetch-dest: empty' \
  -H 'sec-fetch-mode: cors' \
  -H 'sec-fetch-site: same-site' \
  -H 'user-agent: Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36' \
  --compressed
exit $?
