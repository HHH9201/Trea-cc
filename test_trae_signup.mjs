import { chromium } from 'playwright';

(async () => {
  console.log('启动浏览器...');
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  
  console.log('访问注册页面: https://www.trae.ai/sign-up');
  await page.goto('https://www.trae.ai/sign-up', { waitUntil: 'networkidle' });
  
  console.log('等待页面渲染 (3秒)...');
  await page.waitForTimeout(3000);
  
  console.log('\n--- 查找所有输入框 ---');
  const inputs = await page.$$eval('input', els => els.map(el => ({
    type: el.type,
    name: el.name,
    placeholder: el.placeholder,
    className: el.className
  })));
  console.log(JSON.stringify(inputs, null, 2));
  
  console.log('\n--- 查找 "发送验证码" 按钮 ---');
  const selectors = [
    ".right-part.send-code",
    ".send-code",
    ".verification-code .send-code",
    ".verification-code .right-part",
    ".input-con .right-part.send-code",
    "div[class*='send-code']",
    ".right-part"
  ];
  
  for (const sel of selectors) {
    const els = await page.$$(sel);
    if (els.length > 0) {
      console.log(`✅ 选择器 '${sel}' 找到 ${els.length} 个元素:`);
      for (let i = 0; i < els.length; i++) {
        const text = await els[i].innerText();
        const html = await els[i].evaluate(e => e.outerHTML);
        console.log(`   [${i}] 文本: "${text}"`);
        console.log(`   [${i}] HTML: ${html}`);
      }
    } else {
      console.log(`❌ 选择器 '${sel}' 未找到任何元素`);
    }
  }
  
  console.log('\n--- 模拟交互流程 ---');
  try {
    console.log('1. 填入邮箱...');
    await page.fill('input[type="email"]', 'test_script_123@hhxyyq.online');
    
    // 给 React 一点时间响应状态变化
    await page.waitForTimeout(1000);
    
    console.log('2. 查找并点击 Send Code...');
    // 使用最通用的包含文本的选择器，或者用 xpath
    const sendCodeBtn = await page.locator('text="Send Code"').first();
    const isVisible = await sendCodeBtn.isVisible();
    
    if (isVisible) {
      console.log('   找到按钮，执行点击...');
      // 强制触发原生点击，绕过可能存在的遮挡
      await sendCodeBtn.dispatchEvent('click');
      
      console.log('   等待3秒钟查看请求或状态变化...');
      await page.waitForTimeout(3000);
      
      const textAfter = await sendCodeBtn.innerText();
      console.log(`   点击后按钮文本变为: "${textAfter}"`);
      
      // 检查页面上是否有错误提示
      const errors = await page.$$eval('.error-text', els => els.map(e => e.innerText).filter(t => t));
      if (errors.length > 0) {
        console.log('   ⚠️ 页面出现错误提示:', errors);
      }
    } else {
      console.log('   ❌ 未找到可以点击的 Send Code 按钮');
    }
  } catch (err) {
    console.error('交互过程发生错误:', err);
  }

  await browser.close();
  console.log('测试结束。');
})();
