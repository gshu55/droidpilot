<script setup lang="ts">
import { getTauriVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { onMounted, ref } from "vue";

const appVersion = ref(__APP_VERSION__);
const tauriVersion = ref("读取中");

onMounted(async () => {
  try {
    const version = await invoke<string>("get_cargo_package_version");

    if (version.trim()) {
      appVersion.value = version;
    }
  } catch {
    appVersion.value = __APP_VERSION__;
  }

  try {
    const version = await getTauriVersion();

    if (version.trim()) {
      tauriVersion.value = version;
    }
  } catch {
    tauriVersion.value = "不可用";
  }
});
</script>

<template>
  <section class="card panel about-page">
    <h4>关于</h4>

    <div class="about-grid">
      <article class="about-item">
        <h3>应用信息</h3>
        <ul>
          <li>应用名称：DroidPilot 智控助手</li>
          <li>适用场景：开发调试、测试验证、设备维护</li>
          <li>运行方式：本地桌面应用</li>
        </ul>
      </article>

      <article class="about-item">
        <h3>版本信息</h3>
        <ul>
          <li>应用版本：{{ appVersion }}</li>
          <li>Tauri 版本：{{ tauriVersion }}</li>
          <li>发布日期：2026-05-11</li>
          <li>数据处理：本地优先，不提供云同步服务</li>
        </ul>
      </article>

      <article class="about-item">
        <h3>使用边界</h3>
        <ul>
          <li>仅连接和操作你有权管理的设备</li>
          <li>高风险命令执行前请自行确认影响范围</li>
          <li>涉及业务或个人数据时请遵守授权和合规要求</li>
        </ul>
      </article>
    </div>

    <article class="about-item agreement">
      <h3>开源组件说明</h3>
      <p class="agreement-intro">
        本应用包含或调用下列主要第三方开源组件。各组件版权归其原作者及贡献者所有，并分别按照其开源许可证授权。
        本说明采用 SPDX 许可证标识，许可证全文请以组件发布包、官方仓库或随包许可证文件为准。
      </p>
      <ul class="oss-components">
        <li>
          <strong>scrcpy / scrcpy server</strong>
          <span>Apache-2.0</span>
          <small>用于 Android 设备投屏协议与服务端能力，项目地址：https://github.com/Genymobile/scrcpy。</small>
        </li>
        <li>
          <strong>Tauri、Tauri API 与官方插件</strong>
          <span>Apache-2.0 OR MIT</span>
          <small>用于桌面应用外壳、系统能力调用、文件选择、打开外部资源和单实例控制。</small>
        </li>
        <li>
          <strong>Vue、Vue Router、Pinia</strong>
          <span>MIT</span>
          <small>用于前端界面、路由和状态管理。</small>
        </li>
        <li>
          <strong>xterm.js 与 addon-fit</strong>
          <span>MIT</span>
          <small>用于快捷终端显示、输入和终端尺寸适配。</small>
        </li>
        <li>
          <strong>serde、serde_json、base64、shlex、windows-rs</strong>
          <span>MIT OR Apache-2.0</span>
          <small>用于数据序列化、编码处理、命令参数解析和 Windows 平台能力调用。</small>
        </li>
        <li>
          <strong>portable-pty</strong>
          <span>MIT</span>
          <small>用于本地伪终端创建和交互。</small>
        </li>
        <li>
          <strong>Vite、Vue TSC、TypeScript、rimraf 等构建工具</strong>
          <span>MIT / Apache-2.0 / BlueOak-1.0.0</span>
          <small>用于项目构建、类型检查和开发流程，通常不作为应用运行时功能直接提供。</small>
        </li>
      </ul>
      <ul class="oss-notes">
        <li>本应用保留第三方组件的版权声明、许可证声明和必要的归属说明。</li>
        <li>除非许可证另有约定，第三方组件均按“按原样”方式提供，不附带任何明示或默示担保。</li>
        <li>列出组件名称不表示相关开源项目、作者或组织认可、赞助或背书本应用。</li>
        <li>scrcpy 版权归 Genymobile、Romain Vimont 及其贡献者所有；本应用不声称与 scrcpy 项目存在官方隶属关系。</li>
      </ul>
    </article>

    <article class="about-item agreement">
      <h3>使用协议</h3>
      <p class="agreement-intro">
        使用本工具即表示你已阅读并同意以下条款。请仅在合法、授权、可审计的场景下使用。
      </p>
      <ul>
        <li>仅用于你有权管理或测试的设备，禁止对未授权设备执行任何操作。</li>
        <li>请遵守所在组织的安全规范、数据合规制度与相关法律法规。</li>
        <li>执行命令、传输文件、截图录屏等操作前，请自行确认风险与影响范围。</li>
        <li>本工具不对因误操作、设备异常、数据丢失或业务中断造成的损失承担责任。</li>
        <li>不得使用本工具绕过设备安全策略、侵犯他人隐私或破坏第三方系统。</li>
      </ul>
    </article>

    <article class="about-item agreement">
      <h3>隐私说明</h3>
      <ul>
        <li>本工具不提供账号体系，不主动上传你的设备列表、命令记录、截图、录屏或文件内容。</li>
        <li>设备信息、命令历史、传输记录和偏好设置仅用于本地功能展示与操作便利。</li>
        <li>截图、录屏和文件传输内容由用户主动触发并保存到用户选择的位置。</li>
        <li>如在团队或客户环境中使用，请先取得必要授权，并按组织要求处理敏感数据。</li>
      </ul>
    </article>

    <article class="about-item agreement">
      <h3>免责声明</h3>
      <p class="agreement-note">
        ADB 操作可能改变设备状态、覆盖文件或影响正在运行的业务。请在执行前确认命令、目标设备和数据路径。
        因误操作、未授权使用、设备异常或第三方环境限制造成的后果，由使用者自行承担。
      </p>
    </article>
  </section>
</template>

<style scoped>
.about-page {
  margin: 0 auto;
}

.about-page h4 {
  margin: 0 0 8px;
  color: var(--text-main);
  font-size: 16px;
  font-weight: 600;
}

.about-grid {
  display: grid;
  grid-template-columns: 1fr;
  gap: 8px;
  margin-top: 0;
}

.about-item {
  border: 1px solid #dbe3ef;
  border-radius: 8px;
  padding: 10px;
  background: #f8fafc;
}

.about-item:first-child {
  border-top: 1px solid #dbe3ef;
}

.about-item h3 {
  margin: 0 0 8px;
  font-size: 14px;
  line-height: 1.35;
}

.about-item ul {
  margin: 0;
  padding-left: 18px;
  color: #334155;
  font-size: 13px;
  line-height: 1.55;
}

.oss-components {
  display: grid;
  gap: 8px;
  padding-left: 0;
  list-style: none;
}

.oss-components li {
  display: grid;
  grid-template-columns: minmax(160px, 1.1fr) minmax(120px, .8fr) minmax(0, 2fr);
  gap: 8px;
  align-items: start;
  padding: 8px;
  border: 1px solid #dbe3ef;
  border-radius: 6px;
  background: #ffffff;
}

.oss-components strong {
  color: #0f172a;
  font-size: 14px;
}

.oss-components span {
  color: #1d4ed8;
  font-family: ui-monospace, SFMono-Regular, Consolas, "Liberation Mono", monospace;
  font-size: 13px;
  word-break: break-word;
}

.oss-components small {
  color: #475569;
  font-size: 13px;
  line-height: 1.5;
}

.oss-notes {
  margin-top: 10px;
}

.agreement {
  margin-top: 12px;
}

.agreement-intro,
.agreement-note {
  margin: 0 0 8px;
  color: #475569;
  font-size: 13px;
  line-height: 1.6;
}

@media (max-width: 820px) {
  .oss-components li {
    grid-template-columns: 1fr;
  }
}

</style>
