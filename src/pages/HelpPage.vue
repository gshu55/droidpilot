<template>
  <section class="card panel help-page">
    <h4>帮助</h4>
    <!-- USB 连接 -->
    <article class="help-section">
      <h3><span class="step-icon"></span>方式一：USB 线连接</h3>
      <ol class="step-list">
        <li>
          <strong>打开开发者模式</strong><br />
          进入<em>设置 → 关于手机</em>，连续点击「版本号」<strong>7 次</strong>，提示「您已处于开发者模式」即可。
        </li>
        <li>
          <strong>启用 USB 调试</strong><br />
          返回<em>设置 → 系统 → 开发者选项</em>（部分品牌在<em>设置 → 开发者选项</em>），打开「USB 调试」开关。
        </li>
        <li>
          <strong>插入 USB 并授权</strong><br />
          将数据线连接电脑，手机弹出「允许 USB 调试？」对话框，勾选「始终允许」，点击确定。
        </li>
        <li>
          <strong>验证连接</strong><br />
          点击工具栏「刷新」按钮，设备列表中会出现设备序列号，状态显示 <code>device</code> 即为成功。
        </li>
      </ol>
    </article>

    <!-- Wi-Fi 连接（旧版 / adb tcpip） -->
    <article class="help-section">
      <h3><span class="step-icon"></span>方式二：Wi-Fi 连接（Android 10 及以下推荐）</h3>
      <p class="help-note">需先用 USB 线完成初始配对，之后可断开数据线。</p>
      <ol class="step-list">
        <li>按「方式一」先通过 USB 成功连接设备。</li>
        <li>
          查看设备 IP 地址<br />
          在<em>设置 → WLAN → 当前网络详情</em>中找到 IP 地址（如 <code>192.168.1.100</code>）。
        </li>
        <li>
          在投屏页「快捷终端」中执行：
          <div class="code-block"><code>tcpip 5555</code></div>
          若提示成功，记下返回的端口号（不同设备可能不是 5555）。
        </li>
        <li>
          断开 USB 数据线，继续执行：
          <div class="code-block"><code>connect 192.168.1.100:&lt;端口&gt;</code></div>
          出现 <code>connected to 192.168.1.100:&lt;端口&gt;</code> 即连接成功。
        </li>
        <li>点击「刷新」，设备列表中应出现 <code>192.168.1.100:&lt;端口&gt;</code>。</li>
      </ol>
    </article>

    <!-- Wi-Fi 配对（Android 11+） -->
    <article class="help-section">
      <h3><span class="step-icon"></span>方式三：无线配对（Android 11+）</h3>
      <p class="help-note">通常无需 USB 数据线，通过扫码完成首次配对；但部分机型仍需先有线 ADB 授权一次。</p>
      <ol class="step-list">
        <li>
          在手机<em>开发者选项</em>中打开「无线调试」，点击进入后再点击「使用配对码配对设备」。<br />
          屏幕会显示一个 <strong>IP 地址:端口</strong> 和 <strong>6 位配对码</strong>（注意！配对端口与连接端口不同）。
        </li>
        <li>
          在投屏页「快捷终端」中先使用配对端口配对：
          <div class="code-block"><code>pair 192.168.1.100:&lt;配对端口&gt;</code></div>
          执行后根据提示输入 6 位配对码（如 <code>123456</code>），出现 <code>Successfully paired</code> 说明配对成功。
        </li>
        <li>
          配对完成后，使用无线调试页面主界面显示的「连接端口」（非配对端口）进行连接：
          <div class="code-block"><code>connect 192.168.1.100:&lt;连接端口&gt;</code></div>
        </li>
        <li>点击「刷新」确认设备出现在列表中。</li>
      </ol>
    </article>

    <!-- OTG / USB 模式切换 -->
    <article class="help-section">
      <h3><span class="step-icon"></span>注意：部分手机需要切换 USB 角色（OTG）</h3>
      <p class="help-note">某些手机插上数据线后默认处于「USB Host」模式（用于连接 U 盘、鼠标等外设），需要手动切换为「Device / 从机」模式才能被电脑识别。</p>
      <div class="faq-list">
        <div class="faq-item">
          <div class="faq-q">如何切换 USB 角色？</div>
          <div class="faq-a">
            插线后下拉通知栏，找到「USB 连接方式」或「USB 模式」通知，点击进入，
            选择<strong>「文件传输（MTP）」</strong>或<strong>「USB 调试」</strong>等从机模式即可。<br/>
            若通知栏无提示，可前往以下路径手动设置：
            <ul class="brand-list">
              <li><strong>小米 / Redmi：</strong>设置 → 更多连接方式 → USB 设置 → 选择「USB 受控」</li>
              <li><strong>华为 / 荣耀：</strong>设置 → 超级终端 / 更多连接 → USB → 连接 PC 时选「传输文件」</li>
              <li><strong>三星：</strong>设置 → 连接 → USB → USB 受控模式</li>
              <li><strong>OPPO / 一加：</strong>设置 → 其他连接 → OTG 连接 → 关闭 OTG；或通知栏点「USB 已连接」</li>
              <li><strong>vivo：</strong>设置 → 其他网络与连接 → OTG 连接 → 关闭后重新插线</li>
              <li><strong>其它品牌：</strong>在设置中查找「USB 连接方式」或「USB 模式」或「OTG」进行切换，部分设备的名称叫法可能不同</li>
            </ul>
          </div>
        </div>
        <div class="faq-item">
          <div class="faq-q">什么情况下会遇到这个问题？</div>
          <div class="faq-a">当手机插线后电脑无法识别设备，或刷新后始终没有出现设备序列号，可先检查 USB 模式是否被切换到了 Host（主机）模式。开启了「OTG 开关」的手机尤为常见。</div>
        </div>
      </div>
    </article>

    <!-- 常见问题 -->
    <article class="help-section">
      <h3><span class="step-icon"></span>常见问题</h3>
      <div class="faq-list">
        <div class="faq-item">
          <div class="faq-q">刷新后设备不显示？</div>
          <div class="faq-a">确认数据线支持数据传输（非充电专用线）；尝试更换 USB 接口或重启 ADB 服务：执行 <code>kill-server</code> 后再点刷新。</div>
        </div>
        <div class="faq-item">
          <div class="faq-q">两头 Type-C 的线能用于 USB 调试吗？</div>
          <div class="faq-a">
            <strong>可以，但前提是这根线必须是「数据线」，而不是「充电专用线」。</strong><br />
            很多廉价的 USB-C 充电线内部只有供电线芯，没有数据线芯，插上电脑后手机只显示「充电中」而不会弹出 USB 调试授权——这与是不是双头 Type-C 无关，Type-A 转 Type-C 的线也有相同问题。<br />
            判断方法：插线后下拉手机通知栏，若出现「USB 连接方式」或「传输文件」等选项，说明是数据线；若只显示「正在充电」，则是充电专用线，需更换。<br />
            另外，电脑侧的 USB-C 接口也需支持数据传输，部分笔记本的 USB-C 仅提供充电，同样无法识别设备。
          </div>
        </div>
        <div class="faq-item">
          <div class="faq-q">设备状态显示 <code>unauthorized</code>？</div>
          <div class="faq-a">手机屏幕上应有弹窗等待授权，请在手机上确认「允许 USB 调试」，之后点击刷新。</div>
        </div>
        <div class="faq-item">
          <div class="faq-q">Wi-Fi 连接后断开了怎么恢复？</div>
          <div class="faq-a">重新执行 <code>connect &lt;IP&gt;:&lt;端口&gt;</code> 即可恢复（端口以设备当前返回值为准），通常无需重新执行 <code>tcpip</code>。若 IP 已变化，需重新查看设备 IP。</div>
        </div>
        <div class="faq-item">
          <div class="faq-q">为什么有些设备 Wi-Fi ADB 不能用 5555？</div>
          <div class="faq-a">部分 ROM 会禁用或重定向 5555 端口，或仅允许无线调试使用动态端口。请以手机无线调试页面显示的端口，或命令返回的实际端口为准，不要固定写死 5555。</div>
        </div>
        <div class="faq-item">
          <div class="faq-q">为什么已经打开无线调试，仍然连不上？</div>
          <div class="faq-a">部分设备要求先通过 USB 完成一次 ADB 授权与握手。可先按「方式一」有线连通并授权，确认设备状态为 <code>device</code> 后，再执行 <code>pair</code> / <code>connect</code> 进行 Wi-Fi ADB 连接。</div>
        </div>
        <div class="faq-item">
          <div class="faq-q">找不到「开发者选项」菜单？</div>
          <div class="faq-a">不同品牌路径略有差异：小米在<em>设置 → 我的设备 → 全部参数</em>点版本号；华为在<em>设置 → 关于手机</em>点版本号；三星在<em>设置 → 软件信息</em>点版本号。</div>
        </div>
        <div class="faq-item">
          <div class="faq-q">插线后电脑没反应，也没有 USB 授权弹窗？</div>
          <div class="faq-a">很可能手机处于「USB Host 主机」模式，请参考上方「注意：部分手机需要切换 USB 角色」章节，将 USB 模式改为从机（Device）后重新插线。</div>
        </div>
      </div>
    </article>
  </section>
</template>

<style scoped>
.help-page {
  margin: 0 auto;
}

.help-page h4 {
  margin: 0 0 8px;
  color: var(--text-main);
  font-size: 16px;
  font-weight: 600;
}

.help-intro {
  color: #475569;
  font-size: 12px;
  line-height: 1.55;
  margin-top: 4px;
}

.help-section {
  border: 1px solid #dbe3ef;
  border-radius: 8px;
  padding: 10px;
  background: #f8fafc;
  margin-top: 8px;
}

.help-section:first-of-type {
  margin-top: 0;
}

.help-section h3 {
  margin: 0 0 8px;
  font-size: 13px;
  line-height: 1.35;
  display: flex;
  align-items: center;
  gap: 6px;
}

.step-icon {
  font-size: 14px;
  line-height: 1;
}

.help-note {
  margin: 0 0 8px;
  font-size: 12px;
  color: #f59e0b;
  background: #fefce8;
  border: 1px solid #fde68a;
  border-radius: 0;
  padding: 6px 10px;
}

.step-list {
  margin: 0;
  padding-left: 20px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  font-size: 12px;
  color: #334155;
  line-height: 1.6;
}

.step-list li strong {
  color: #0f172a;
}

.step-list li em {
  font-style: normal;
  color: #2563eb;
}

.code-block {
  margin-top: 6px;
  background: linear-gradient(180deg, #f8fbff 0%, #eef6ff 100%);
  border: 1px solid #cfe3ff;
  border-radius: 0;
  padding: 7px 8px;
  display: inline-block;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.85);
}

.code-block code {
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 12px;
  color: #1d4ed8;
  background: transparent;
  padding: 0;
}

code {
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 12px;
  background: #e2e8f0;
  color: #1e40af;
  padding: 1px 5px;
  border-radius: 0;
}

.faq-list {
  display: flex;
  flex-direction: column;
  gap: 0;
}

.faq-item {
  border-radius: 0;
  background: #fff;
  border: 0;
  border-bottom: 1px solid #e2e8f0;
  padding: 8px 10px;
}

.faq-q {
  font-weight: 600;
  font-size: 12px;
  color: #0f172a;
  margin-bottom: 4px;
}

.faq-a {
  font-size: 12px;
  color: #475569;
  line-height: 1.6;
}

.brand-list {
  margin: 8px 0 0;
  padding-left: 18px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.brand-list li {
  font-size: 12px;
  color: #475569;
  line-height: 1.6;
}
</style>
