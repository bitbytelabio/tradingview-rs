import requests
from google.oauth2 import service_account

# Create a service account and get the OAuth 2.0 credentials.
credentials, _ = service_account.Credentials.from_service_account_file("credentials.json")

# Create a request to the TradingView API.
url = "https://www.tradingview.com/oauth/token"
params = {
    "client_id": "YOUR_CLIENT_ID",
    "client_secret": "YOUR_CLIENT_SECRET",
    "grant_type": "authorization_code",
    "code": "YOUR_CODE",
}

# Make the request and get the access token.
response = requests.post(url, params=params, headers={"Authorization": "Bearer " + credentials.access_token})
access_token = response.json()["access_token"]

# Use the access token to login to TradingView.
url = "https://www.tradingview.com/api/v1/user/login"
headers = {"Authorization": "Bearer " + access_token}

response = requests.post(url, headers=headers)

if response.status_code == 200:
    print("Successfully logged in to TradingView.")
else:
    print("Login failed.")