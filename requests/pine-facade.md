# TradingView Pine facade

```http
# @name json
GET /pine-facade/list?filter=fundamental HTTP/2.0
Host: pine-facade.tradingview.com
Accept: application/json
```

```http
# @name json
GET /pine-facade/list?filter=standard HTTP/2.0
Host: pine-facade.tradingview.com
Accept: application/json
```

```http
GET /pine-facade/list?filter=candlestick HTTP/1.1
Host: pine-facade.tradingview.com{{host}}
Accept: application/json
```

```http
GET /pine-facade/translate/STD%3BPrice_Oscillator/28.0 HTTP/2.0
Host: pine-facade.tradingview.com
Accept: application/json
```

```http
GET /pine-facade/list?filter=saved HTTP/1.1
Host: pine-facade.tradingview.com
Cookie: sessionid={{$dotenv TV_SESSION}}; sessionid_sign={{$dotenv TV_SIGNATURE}}; device_t={{$dotenv TV_DEVICE_ID}};
Accept: application/json
```

```http
GET /pine-facade/translate/USER%3B7a78efc35a354d5b8b2e7dc1833524fc/1.0 HTTP/1.1
Host: pine-facade.tradingview.com
Cookie: sessionid={{$dotenv TV_SESSION}}; sessionid_sign={{$dotenv TV_SIGNATURE}}; device_t={{$dotenv TV_DEVICE_ID}};
```
