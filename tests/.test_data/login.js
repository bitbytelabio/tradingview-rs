fetch("https://www.tradingview.com/accounts/signin/", {
  headers: {
    accept: "*/*",
    "accept-language": "en-US,en;q=0.9,vi;q=0.8",
    "content-type":
      "multipart/form-data; boundary=----WebKitFormBoundaryOiVaBPiAM96TSeX9",
    "sec-ch-ua":
      '"Google Chrome";v="112", "Chromium";v="112", "Not=A?Brand";v="24"',
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": '"Linux"',
    "sec-fetch-dest": "empty",
    "sec-fetch-mode": "same-origin",
    "sec-fetch-site": "same-origin",
    "sec-gpc": "1",
    "x-language": "en",
    "x-requested-with": "XMLHttpRequest",
    cookie:
      'cookiePrivacyPreferenceBannerProduction=notApplicable; cookiesSettings={"analytics":true,"advertising":true}; theme=dark; device_t=dmxSaEF3OjA._iyOV1r3ASfeaHfNf3zitLkpU2iQNW56nelPQj5zk-k; png=bc3f63aa-61b1-4707-9c63-adf0ec26c245; etg=bc3f63aa-61b1-4707-9c63-adf0ec26c245; cachec=bc3f63aa-61b1-4707-9c63-adf0ec26c245; tv_ecuid=bc3f63aa-61b1-4707-9c63-adf0ec26c245; _sp_ses.cf1a=*; _sp_id.cf1a=dba5821b-3644-41c3-b7eb-67959d10eacc.1686197092.3.1686558123.1686219038.653c90f1-0c83-4575-85a0-6b77c3f3e3d6',
    Referer: "https://www.tradingview.com/u/lite_bitbytelab/",
    "Referrer-Policy": "origin-when-cross-origin",
  },
  body: '------WebKitFormBoundaryOiVaBPiAM96TSeX9\r\nContent-Disposition: form-data; name="username"\r\n\r\nlite@bitbytelab.io\r\n------WebKitFormBoundaryOiVaBPiAM96TSeX9\r\nContent-Disposition: form-data; name="password"\r\n\r\ndAIuLpdzmEy8HWnIYRGwigRA4XwJT4Ny/WIsD/rXy5qurJwu\r\n------WebKitFormBoundaryOiVaBPiAM96TSeX9\r\nContent-Disposition: form-data; name="remember"\r\n\r\ntrue\r\n------WebKitFormBoundaryOiVaBPiAM96TSeX9--\r\n',
  method: "POST",
});
fetch("https://www.tradingview.com/accounts/two-factor/signin/totp/", {
  headers: {
    accept: "*/*",
    "accept-language": "en-US,en;q=0.9,vi;q=0.8",
    "content-type":
      "multipart/form-data; boundary=----WebKitFormBoundaryBWBnVBIQ6jlEfeFp",
    "sec-ch-ua":
      '"Google Chrome";v="112", "Chromium";v="112", "Not=A?Brand";v="24"',
    "sec-ch-ua-mobile": "?0",
    "sec-ch-ua-platform": '"Linux"',
    "sec-fetch-dest": "empty",
    "sec-fetch-mode": "same-origin",
    "sec-fetch-site": "same-origin",
    "sec-gpc": "1",
    "x-language": "en",
    "x-requested-with": "XMLHttpRequest",
    cookie:
      'cookiePrivacyPreferenceBannerProduction=notApplicable; cookiesSettings={"analytics":true,"advertising":true}; theme=dark; device_t=dmxSaEF3OjA._iyOV1r3ASfeaHfNf3zitLkpU2iQNW56nelPQj5zk-k; png=bc3f63aa-61b1-4707-9c63-adf0ec26c245; etg=bc3f63aa-61b1-4707-9c63-adf0ec26c245; cachec=bc3f63aa-61b1-4707-9c63-adf0ec26c245; tv_ecuid=bc3f63aa-61b1-4707-9c63-adf0ec26c245; _sp_ses.cf1a=*; _sp_id.cf1a=dba5821b-3644-41c3-b7eb-67959d10eacc.1686197092.3.1686558123.1686219038.653c90f1-0c83-4575-85a0-6b77c3f3e3d6; sessionid=cxulm0vwscabktj2n9pxk134m1cv7htw; sessionid_sign=v1:JiiJuk1JiwfPqe77WUSgMP1qQO7I8MtWg7EgNfUr+aM=',
    Referer: "https://www.tradingview.com/u/lite_bitbytelab/",
    "Referrer-Policy": "origin-when-cross-origin",
  },
  body: '------WebKitFormBoundaryBWBnVBIQ6jlEfeFp\r\nContent-Disposition: form-data; name="code"\r\n\r\n266342\r\n------WebKitFormBoundaryBWBnVBIQ6jlEfeFp--\r\n',
  method: "POST",
});
