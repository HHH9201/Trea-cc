import https from 'https';

const token = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJkYXRhIjp7ImlkIjoiNzU0ODcxNDQ1MjM5MzIxNDk4NCIsInNvdXJjZSI6InJlZnJlc2hfdG9rZW4iLCJzb3VyY2VfaWQiOiJZV3VUTXJiY0RWNE9yQUxPaHV3Y2laWjBmMzFNZ0JQMW5ZaVNRR1VRNUlZPS4xODljNTg1ZDBlNjNhZWIwIiwidGVuYW50X2lkIjoiN28yZDg5NHA3ZHIwbzQiLCJ0eXBlIjoidXNlciJ9LCJleHAiOjE3NzQ1OTkxMDksImlhdCI6MTc3MzM4OTUwOX0.oXd9QYXLgC5IjOyA5cIKg2LzzpWyg_VLV_OoPRiTVSmIMC7zMN7uxejL-CSHvBddAhC4tc2Vl_tQDJwMjM0CzBPfbMn7rEe41PcdCJ74BuAdEYF1lsSoyX59UioXDJ5pIXdNeLxiuRwuROLm3eYXIr73Qz2A4-gxwCZlARc2Davp_wtwbJ8OqAweUr3Qz3r1kkcZvafnUi9J0I4XX8RTY9u0AkHeOpBvx6Hjk48UfFE05gScPPpOfl3vZuB4tRqqPOzi8yOAha2A7g0mL1VCI6TaWTPkaRe0kcUqWNlhQ9y9LBq8uRWixSQI7pHpcjTzqzNZD9KiV-o53qxLNuWE1JZXcVfaaGduDR7MIJktEceaz3UtpEOyEbvvQMg52Zruzqi0gdcIW-D2iNzzwLLh3mVNVKSUjRO5iquhynLLa9aiTio2S4-gawy62PuIzmOVwV9SLqsg6YfDLAmvENWMf7A75vw-XBKY_uEkF18tfr8x4_R7lGjDt0dFv6gvK7SBRXIHV7IBcB6kTaA9gku3A7U-MXyvgSI-gLkxIis9SvzqczRqVIPkyGyFauEQ1oqIu783W8e1ngo683ytnFheEOqZemFR4MrFHv3lJMMtVMKqgK6qx96ELWX-FHtb8g6KnafiNI_mfZAEH9K53iHaEvAYx5aHI4o1ufEJKQ9-HaQ";

// 解析 JWT payload
const parts = token.split('.');
const payloadB64 = parts[1];
const padding = (4 - payloadB64.length % 4) % 4;
const padded = payloadB64 + '='.repeat(padding);
const standardB64 = padded.replace(/-/g, '+').replace(/_/g, '/');
const payloadStr = Buffer.from(standardB64, 'base64').toString('utf-8');
const payload = JSON.parse(payloadStr);

console.log('Token 信息:');
console.log(`  User ID: ${payload.data.id}`);
console.log(`  Tenant ID: ${payload.data.tenant_id}`);
console.log();

const endpoints = [
  'api-sg-central.trae.ai',
  'api-us-east.trae.ai'
];

async function checkQuota(host) {
  return new Promise((resolve, reject) => {
    const postData = JSON.stringify({ require_usage: true });
    
    const options = {
      hostname: host,
      path: '/trae/api/v1/pay/user_current_entitlement_list',
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Accept': 'application/json, text/plain, */*',
        'Origin': 'https://www.trae.ai',
        'Referer': 'https://www.trae.ai/',
        'User-Agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36',
        'Authorization': `Cloud-IDE-JWT ${token}`,
        'Content-Length': Buffer.byteLength(postData)
      }
    };

    const req = https.request(options, (res) => {
      let data = '';
      res.on('data', (chunk) => data += chunk);
      res.on('end', () => {
        if (res.statusCode === 200) {
          resolve(JSON.parse(data));
        } else {
          reject(new Error(`HTTP ${res.statusCode}`));
        }
      });
    });

    req.on('error', reject);
    req.write(postData);
    req.end();
  });
}

(async () => {
  for (const host of endpoints) {
    try {
      const data = await checkQuota(host);
      console.log(`✅ API 端点: https://${host}`);
      console.log(`   美元计费模式: ${data.is_dollar_usage_billing || false}`);
      console.log(`   新用户: ${data.is_pay_freshman || false}`);
      
      for (const pack of data.user_entitlement_pack_list || []) {
        const baseInfo = pack.entitlement_base_info || {};
        const usage = pack.usage || {};
        const quota = baseInfo.quota || {};
        
        console.log(`\n   套餐类型: ${baseInfo.product_id === 0 ? 'Free' : 'Pro'}`);
        console.log(`   产品类型: ${baseInfo.product_type}`);
        console.log(`\n   📊 额度信息:`);
        console.log(`      Fast Request 限制: ${quota.premium_model_fast_request_limit} 次`);
        console.log(`      Fast Request 已使用: $${(usage.premium_model_fast_amount || 0).toFixed(2)}`);
        console.log(`      Slow Request 限制: ${quota.premium_model_slow_request_limit} 次`);
        console.log(`      Slow Request 已使用: $${(usage.premium_model_slow_amount || 0).toFixed(2)}`);
        console.log(`      Advanced Model 限制: ${quota.advanced_model_request_limit} 次`);
        console.log(`      Advanced Model 已使用: $${(usage.advanced_model_amount || 0).toFixed(2)}`);
        console.log(`      Auto Completion 限制: ${quota.auto_completion_limit} 次`);
        console.log(`      Auto Completion 已使用: $${(usage.auto_completion_amount || 0).toFixed(2)}`);
        
        // 计算剩余额度
        const fastLimit = quota.premium_model_fast_request_limit || 0;
        const fastUsed = usage.premium_model_fast_amount || 0;
        const fastLeft = 3.0 - fastUsed; // 假设3美元额度
        console.log(`\n   💰 美元额度估算:`);
        console.log(`      总额度: $3.00`);
        console.log(`      已使用: $${fastUsed.toFixed(2)}`);
        console.log(`      剩余: $${fastLeft.toFixed(2)}`);
      }
      break;
    } catch (e) {
      console.log(`❌ ${host} 失败: ${e.message}`);
    }
  }
})();
