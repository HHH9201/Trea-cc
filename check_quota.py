import requests
import json
import base64

# 解析 JWT token 获取基本信息
token = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJkYXRhIjp7ImlkIjoiNzU0ODcxNDQ1MjM5MzIxNDk4NCIsInNvdXJjZSI6InJlZnJlc2hfdG9rZW4iLCJzb3VyY2VfaWQiOiJZV3VUTXJiY0RWNE9yQUxPaHV3Y2laWjBmMzFNZ0JQMW5ZaVNRR1VRNUlZPS4xODljNTg1ZDBlNjNhZWIwIiwidGVuYW50X2lkIjoiN28yZDg5NHA3ZHIwbzQiLCJ0eXBlIjoidXNlciJ9LCJleHAiOjE3NzQ1OTkxMDksImlhdCI6MTc3MzM4OTUwOX0.oXd9QYXLgC5IjOyA5cIKg2LzzpWyg_VLV_OoPRiTVSmIMC7zMN7uxejL-CSHvBddAhC4tc2Vl_tQDJwMjM0CzBPfbMn7rEe41PcdCJ74BuAdEYF1lsSoyX59UioXDJ5pIXdNeLxiuRwuROLm3eYXIr73Qz2A4-gxwCZlARc2Davp_wtwbJ8OqAweUr3Qz3r1kkcZvafnUi9J0I4XX8RTY9u0AkHeOpBvx6Hjk48UfFE05gScPPpOfl3vZuB4tRqqPOzi8yOAha2A7g0mL1VCI6TaWTPkaRe0kcUqWNlhQ9y9LBq8uRWixSQI7pHpcjTzqzNZD9KiV-o53qxLNuWE1JZXcVfaaGduDR7MIJktEceaz3UtpEOyEbvvQMg52Zruzqi0gdcIW-D2iNzzwLLh3mVNVKSUjRO5iquhynLLa9aiTio2S4-gawy62PuIzmOVwV9SLqsg6YfDLAmvENWMf7A75vw-XBKY_uEkF18tfr8x4_R7lGjDt0dFv6gvK7SBRXIHV7IBcB6kTaA9gku3A7U-MXyvgSI-gLkxIis9SvzqczRqVIPkyGyFauEQ1oqIu783W8e1ngo683ytnFheEOqZemFR4MrFHv3lJMMtVMKqgK6qx96ELWX-FHtb8g6KnafiNI_mfZAEH9K53iHaEvAYx5aHI4o1ufEJKQ9-HaQ"

# 解析 payload
parts = token.split('.')
payload_b64 = parts[1]
padding = (4 - len(payload_b64) % 4) % 4
padded = payload_b64 + '=' * padding
standard_b64 = padded.replace('-', '+').replace('_', '/')
payload_bytes = base64.b64decode(standard_b64)
payload = json.loads(payload_bytes.decode('utf-8'))

print("Token 信息:")
print(f"  User ID: {payload['data']['id']}")
print(f"  Tenant ID: {payload['data']['tenant_id']}")
print()

# 查询额度
headers = {
    'Content-Type': 'application/json',
    'Accept': 'application/json, text/plain, */*',
    'Origin': 'https://www.trae.ai',
    'Referer': 'https://www.trae.ai/',
    'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
    'Authorization': f'Cloud-IDE-JWT {token}'
}

endpoints = [
    'https://api-sg-central.trae.ai',
    'https://api-us-east.trae.ai'
]

for base in endpoints:
    url = f"{base}/trae/api/v1/pay/user_current_entitlement_list"
    try:
        resp = requests.post(url, headers=headers, json={"require_usage": True}, timeout=10)
        if resp.status_code == 200:
            data = resp.json()
            print(f"✅ API 端点: {base}")
            print(f"   美元计费模式: {data.get('is_dollar_usage_billing', False)}")
            print(f"   新用户: {data.get('is_pay_freshman', False)}")
            
            for pack in data.get('user_entitlement_pack_list', []):
                base_info = pack.get('entitlement_base_info', {})
                usage = pack.get('usage', {})
                quota = base_info.get('quota', {})
                
                print(f"\n   套餐类型: {'Free' if base_info.get('product_id') == 0 else 'Pro'}")
                print(f"   产品类型: {base_info.get('product_type')}")
                print(f"\n   📊 额度信息:")
                print(f"      Fast Request 限制: {quota.get('premium_model_fast_request_limit')} 次")
                print(f"      Fast Request 已使用: ${usage.get('premium_model_fast_amount', 0):.2f}")
                print(f"      Slow Request 限制: {quota.get('premium_model_slow_request_limit')} 次")
                print(f"      Slow Request 已使用: ${usage.get('premium_model_slow_amount', 0):.2f}")
                print(f"      Advanced Model 限制: {quota.get('advanced_model_request_limit')} 次")
                print(f"      Advanced Model 已使用: ${usage.get('advanced_model_amount', 0):.2f}")
                print(f"      Auto Completion 限制: {quota.get('auto_completion_limit')} 次")
                print(f"      Auto Completion 已使用: ${usage.get('auto_completion_amount', 0):.2f}")
            break
    except Exception as e:
        print(f"❌ {base} 失败: {e}")
