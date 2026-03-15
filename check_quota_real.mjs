import https from 'https';

const token = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.eyJkYXRhIjp7ImlkIjoiNzYxNjY4NDEyMzQzMjYzMzM1OCIsInNvdXJjZSI6InNlc3Npb24iLCJzb3VyY2VfaWQiOiJ6QTJSdmhld3FMamZYdWpwT01PbUc2cjRKM0JpWWtnMGMtZmhWeFI3ZUNzPS4xODljNWZhMTljYTk2YjNkIiwidGVuYW50X2lkIjoiN28yZDg5NHA3ZHIwbzQiLCJ0eXBlIjoidXNlciJ9LCJleHAiOjE3NzM0MjYzMDMsImlhdCI6MTc3MzM5NzUwM30.ioWEk9DfK6DBeyTjN9QclCJP4kZjVkwXyfWeh9Vn4zw5VsfStfwbI4P0HNTDJGTqlNmWpNtw8S5MST1hCNgTDupI0XOXWCB4tWMdo34IQXiLfNpYbnLIBplzqiurfAWMO6jwMeythZbMURvU3hYWgGTGYdrIktISYp9VJSmILZjqb9jWNSRzblIuDsxGSF2U_n_ptXwN3ptp0q6eUfCsY-ICO4PD6psPooE5Nx-tQnfhnIhTIrb2XYVRz1JlOH-4s1P-ojlMb1WxCcFF1lAkByNK2PGP_XggleovxurvPJR02r-vpb0Q-H4qf1mEtkNIp7MMjkVRmmsmwOBq8rUc5aqR5OSwa8zR871OPv8qu9pU7Ocxf7ePHfK8wZpOSYCXr017Jbl6S8zslzprCsonlxtfxALe8fF3M9kW7aa3D0qVO8d-MCejCqmUqS2VHPr_KXAAqxM4gNkEkJ_EAiuO5cUHpG3X6iNnzmehEAwl5JGDU8hyH3NHW_P1jHZwsYPVU86Oxgd0PI380gh6Aon9b_4FyiJaj_xJEyMcJwFJtikv8zpUF38TF6zM1zq8FOx5PkCeRpLlDqSNgh0kXHuDwlAD2_9QCSCqKfHG28mdY9PAfgA6X2ZPcfGlguXsFYX6HOfv0iMyRIjRN6VOJDgRVQzd_CNEQPv4fRJIykSEKec";

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
console.log(`  Source: ${payload.data.source}`);
console.log(`  Expire: ${new Date(payload.exp * 1000).toLocaleString()}`);
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
      console.log();
      console.log(`💰 计费模式: ${data.is_dollar_usage_billing ? '美元计费' : '请求次数计费'}`);
      console.log(`🆕 新用户: ${data.is_pay_freshman ? '是' : '否'}`);
      console.log();
      
      for (const pack of data.user_entitlement_pack_list || []) {
        const baseInfo = pack.entitlement_base_info || {};
        const usage = pack.usage || {};
        const quota = baseInfo.quota || {};
        
        console.log(`📦 套餐信息:`);
        console.log(`   类型: ${baseInfo.product_id === 0 ? 'Free' : 'Pro'}`);
        console.log(`   产品类型: ${baseInfo.product_type === 0 ? '普通' : '礼包'}`);
        console.log(`   开始时间: ${new Date(baseInfo.start_time * 1000).toLocaleString()}`);
        console.log(`   结束时间: ${new Date(baseInfo.end_time * 1000).toLocaleString()}`);
        console.log();
        
        console.log(`📊 原始额度数据 (来自 API):`);
        console.log(`   basic_usage_amount: ${usage.basic_usage_amount ?? 0}`);
        console.log(`   bonus_usage_amount: ${usage.bonus_usage_amount ?? 0}`);
        console.log(`   premium_model_fast_amount: ${usage.premium_model_fast_amount ?? 0}`);
        console.log(`   premium_model_slow_amount: ${usage.premium_model_slow_amount ?? 0}`);
        console.log();
        
        console.log(`📊 配额限制 (quota):`);
        console.log(`   basic_usage_limit: ${quota.basic_usage_limit ?? 3}`);
        console.log(`   bonus_usage_limit: ${quota.bonus_usage_limit ?? 3}`);
        console.log(`   premium_model_fast_request_limit: ${quota.premium_model_fast_request_limit ?? 0}`);
        console.log(`   premium_model_slow_request_limit: ${quota.premium_model_slow_request_limit ?? 0}`);
        console.log();
        
        // 计算真实额度
        const basicLimit = quota.basic_usage_limit ?? 3;
        const basicUsed = usage.basic_usage_amount ?? 0;
        const basicLeft = basicLimit - basicUsed;
        
        const bonusLimit = quota.bonus_usage_limit ?? 3;
        const bonusUsed = usage.bonus_usage_amount ?? 0;
        const bonusLeft = bonusLimit - bonusUsed;
        
        const totalLimit = basicLimit + bonusLimit;
        const totalUsed = basicUsed + bonusUsed;
        const totalLeft = totalLimit - totalUsed;
        
        console.log(`💵 计算后的真实额度:`);
        console.log(`   ┌─────────────────────────────────────────┐`);
        console.log(`   │  Basic 额度: $${basicUsed.toFixed(2)} / $${basicLimit.toFixed(2)} (剩余 $${basicLeft.toFixed(2)}) │`);
        console.log(`   │  Bonus 额度: $${bonusUsed.toFixed(2)} / $${bonusLimit.toFixed(2)} (剩余 $${bonusLeft.toFixed(2)}) │`);
        console.log(`   │  ─────────────────────────────────────  │`);
        console.log(`   │  总计额度:   $${totalUsed.toFixed(2)} / $${totalLimit.toFixed(2)} (剩余 $${totalLeft.toFixed(2)}) │`);
        console.log(`   └─────────────────────────────────────────┘`);
        console.log();
        
        if (totalLeft <= 0) {
          console.log(`⚠️ 警告: 额度已用完！`);
        } else if (totalLeft < 1) {
          console.log(`⚠️ 警告: 额度即将用完！`);
        } else {
          console.log(`✅ 额度充足`);
        }
      }
      break;
    } catch (e) {
      console.log(`❌ ${host} 失败: ${e.message}`);
    }
  }
})();
